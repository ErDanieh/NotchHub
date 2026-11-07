import Foundation
import IOKit.ps
import SwiftUI
import Charts

/// CPU, memory, network, battery, disk and the busiest processes, plus "high usage" alerts.
/// Samples every second (cheap kernel counters); processes and battery every few seconds.
@MainActor
final class SystemMonitor: ObservableObject {
    @Published private(set) var cpu = 0.0              // 0...1
    @Published private(set) var cpuHistory: [Double] = Array(repeating: 0, count: 40)
    @Published private(set) var memUsed = 0.0          // 0...1
    @Published private(set) var memText = ""
    @Published private(set) var netDown = 0.0   // bytes per second
    @Published private(set) var netUp = 0.0
    @Published private(set) var battery: (percent: Int, charging: Bool)?
    @Published private(set) var diskFreeGB = 0.0
    @Published private(set) var topProcesses: [(name: String, cpu: Double)] = []

    var visible = false { didSet { if visible { slowTick = 99 } } }
    private var timer: Timer?
    private var prevTicks: (UInt32, UInt32, UInt32, UInt32)?
    private var prevNet: (UInt32, UInt32, Date)?
    private var slowTick = 99
    private var cpuHigh = 0, memHigh = 0
    private var lastAlert: [String: Date] = [:]

    func start() {
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.sample() }
        }
    }

    private func sample() {
        sampleCPU(); sampleMemory(); sampleNetwork()
        slowTick += 1
        if slowTick >= (visible ? 3 : 10) { slowTick = 0; sampleBattery(); sampleDisk(); if visible { sampleProcesses() } }
        checkAlerts()
    }

    private func sampleCPU() {
        var info = host_cpu_load_info()
        var count = mach_msg_type_number_t(MemoryLayout<host_cpu_load_info>.size / MemoryLayout<integer_t>.size)
        let r = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { host_statistics(mach_host_self(), HOST_CPU_LOAD_INFO, $0, &count) }
        }
        guard r == KERN_SUCCESS else { return }
        let t = (info.cpu_ticks.0, info.cpu_ticks.1, info.cpu_ticks.2, info.cpu_ticks.3)   // user, system, idle, nice
        if let p = prevTicks {
            let user = Double(t.0 &- p.0), sys = Double(t.1 &- p.1), idle = Double(t.2 &- p.2), nice = Double(t.3 &- p.3)
            let total = user + sys + idle + nice
            if total > 0 { cpu = (user + sys + nice) / total }
            cpuHistory.removeFirst(); cpuHistory.append(cpu)
        }
        prevTicks = t
    }

    private func sampleMemory() {
        var stats = vm_statistics64()
        var count = mach_msg_type_number_t(MemoryLayout<vm_statistics64>.size / MemoryLayout<integer_t>.size)
        let r = withUnsafeMutablePointer(to: &stats) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { host_statistics64(mach_host_self(), HOST_VM_INFO64, $0, &count) }
        }
        guard r == KERN_SUCCESS else { return }
        let page = Double(sysconf(_SC_PAGESIZE))
        let used = (Double(stats.active_count) + Double(stats.wire_count) + Double(stats.compressor_page_count)) * page
        let total = Double(ProcessInfo.processInfo.physicalMemory)
        memUsed = min(1, used / total)
        memText = String(format: "%.1f of %.0f GB", used / 1_073_741_824, total / 1_073_741_824)
    }

    private func sampleNetwork() {
        var inB: UInt32 = 0, outB: UInt32 = 0
        var addrs: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&addrs) == 0 else { return }
        var ptr = addrs
        while let p = ptr {
            let a = p.pointee
            if let sa = a.ifa_addr, sa.pointee.sa_family == UInt8(AF_LINK), let data = a.ifa_data,
               String(cString: a.ifa_name).hasPrefix("en") {
                let d = data.assumingMemoryBound(to: if_data.self).pointee
                inB &+= d.ifi_ibytes; outB &+= d.ifi_obytes
            }
            ptr = a.ifa_next
        }
        freeifaddrs(addrs)
        let now = Date()
        if let p = prevNet {
            let dt = now.timeIntervalSince(p.2)
            if dt > 0 { netDown = Double(inB &- p.0) / dt; netUp = Double(outB &- p.1) / dt }
        }
        prevNet = (inB, outB, now)
    }

    private func sampleBattery() {
        guard let snap = IOPSCopyPowerSourcesInfo()?.takeRetainedValue(),
              let list = IOPSCopyPowerSourcesList(snap)?.takeRetainedValue() as? [CFTypeRef] else { return }
        for ps in list {
            guard let d = IOPSGetPowerSourceDescription(snap, ps)?.takeUnretainedValue() as? [String: Any],
                  let pct = d[kIOPSCurrentCapacityKey] as? Int else { continue }
            battery = (pct, (d[kIOPSIsChargingKey] as? Bool) ?? false || (d[kIOPSPowerSourceStateKey] as? String) == kIOPSACPowerValue)
            return
        }
        battery = nil   // desktop Mac
    }

    private func sampleDisk() {
        if let v = try? URL(fileURLWithPath: "/").resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey]),
           let b = v.volumeAvailableCapacityForImportantUsage { diskFreeGB = Double(b) / 1_000_000_000 }
    }

    private func sampleProcesses() {
        DispatchQueue.global(qos: .utility).async {
            let p = Process(); p.executableURL = URL(fileURLWithPath: "/bin/ps")
            p.arguments = ["-Aceo", "pcpu,comm", "-r"]
            let pipe = Pipe(); p.standardOutput = pipe; p.standardError = FileHandle.nullDevice
            guard (try? p.run()) != nil else { return }
            let out = String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
            p.waitUntilExit()
            let rows: [(String, Double)] = out.split(separator: "\n").dropFirst().prefix(3).compactMap { line in
                let parts = line.trimmingCharacters(in: .whitespaces).split(separator: " ", maxSplits: 1)
                guard parts.count == 2, let v = Double(parts[0].replacingOccurrences(of: ",", with: ".")) else { return nil }
                return (String(parts[1]), v)
            }
            DispatchQueue.main.async { MainActor.assumeIsolated { self.topProcesses = rows.map { (name: $0.0, cpu: $0.1) } } }
        }
    }

    // MARK: alerts
    private func checkAlerts() {
        let s = Settings.shared
        guard s.alertsPerformance else { cpuHigh = 0; memHigh = 0; return }
        cpuHigh = cpu * 100 >= s.cpuThreshold ? cpuHigh + 1 : 0
        memHigh = memUsed * 100 >= s.memThreshold ? memHigh + 1 : 0
        if cpuHigh >= 20, cooldown("cpu", 300) {
            let culprit = topProcesses.first.map { " · \($0.name)" } ?? ""
            AlertCenter.shared.post(symbol: "cpu", title: "CPU high", detail: String(format: "%.0f%%", cpu * 100) + culprit)
            if topProcesses.isEmpty { sampleProcesses() }
        }
        if memHigh >= 20, cooldown("mem", 300) {
            AlertCenter.shared.post(symbol: "memorychip", title: "Memory high", detail: String(format: "%.0f%% used", memUsed * 100))
        }
        if let b = battery, !b.charging, Double(b.percent) <= s.batteryThreshold, cooldown("battery", 1800) {
            AlertCenter.shared.post(symbol: "battery.25percent", title: "Battery low", detail: "\(b.percent)%")
        }
    }
    private func cooldown(_ key: String, _ seconds: TimeInterval) -> Bool {
        if let t = lastAlert[key], Date().timeIntervalSince(t) < seconds { return false }
        lastAlert[key] = Date(); return true
    }
}

struct SystemTab: View {
    @ObservedObject var m: SystemMonitor

    private func rate(_ b: Double) -> String {
        b >= 1_000_000 ? String(format: "%.1f MB/s", b / 1_000_000) : String(format: "%.0f KB/s", b / 1000)
    }

    var body: some View {
        HStack(spacing: Tok.Space.sm) {
            card("CPU", String(format: "%.0f%%", m.cpu * 100)) {
                Chart(Array(m.cpuHistory.enumerated()), id: \.offset) { i, v in
                    AreaMark(x: .value("t", i), y: .value("cpu", v)).foregroundStyle(Tok.Color.neon.opacity(0.35))
                    LineMark(x: .value("t", i), y: .value("cpu", v)).foregroundStyle(Tok.Color.neon)
                }
                .chartYScale(domain: 0...1).chartXAxis(.hidden).chartYAxis(.hidden)
            }
            card("Memory", String(format: "%.0f%%", m.memUsed * 100)) {
                VStack(alignment: .leading, spacing: 4) {
                    bar(m.memUsed)
                    Text(m.memText).font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted)
                }
            }
            card("Network", "") {
                VStack(alignment: .leading, spacing: 2) {
                    Label(rate(m.netDown), systemImage: "arrow.down").font(Tok.Font.sm)
                    Label(rate(m.netUp), systemImage: "arrow.up").font(Tok.Font.sm)
                }.foregroundStyle(Tok.Color.text).labelStyle(.titleAndIcon)
            }
            card("Busiest", "") {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(Array(m.topProcesses.enumerated()), id: \.offset) { _, p in
                        HStack { Text(p.name).lineLimit(1); Spacer(minLength: 2); Text(String(format: "%.0f%%", p.cpu)).monospacedDigit() }
                            .font(Tok.Font.xs).foregroundStyle(Tok.Color.text)
                    }
                    if let b = m.battery {
                        Label("\(b.percent)%" + (b.charging ? " charging" : ""), systemImage: b.charging ? "battery.100.bolt" : "battery.50")
                            .font(Tok.Font.xs).foregroundStyle(b.percent <= 15 && !b.charging ? Tok.Color.danger : Tok.Color.textMuted)
                    }
                    Text(String(format: "%.0f GB free", m.diskFreeGB)).font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted)
                }
            }
        }
        .onAppear { m.visible = true }
        .onDisappear { m.visible = false }
    }

    private func bar(_ f: Double) -> some View {
        GeometryReader { g in
            ZStack(alignment: .leading) {
                Capsule().fill(Tok.Color.surfaceHover)
                Capsule().fill(f > 0.85 ? Tok.Color.danger : Tok.Color.neon).frame(width: max(4, g.size.width * f))
            }
        }.frame(height: 6)
    }

    private func card<C: View>(_ title: String, _ value: String, @ViewBuilder content: () -> C) -> some View {
        VStack(alignment: .leading, spacing: Tok.Space.xs) {
            HStack { Text(title).font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted); Spacer(); Text(value).font(Tok.Font.base).foregroundStyle(Tok.Color.text).monospacedDigit() }
            content()
            Spacer(minLength: 0)
        }
        .padding(Tok.Space.sm)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Tok.Color.surface, in: RoundedRectangle(cornerRadius: Tok.Radius.md))
    }
}
