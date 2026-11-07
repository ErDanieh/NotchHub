import SwiftUI

struct DockerContainer: Identifiable, Equatable {
    let id: String
    var name: String, image: String, state: String, status: String, ports: String
    var project: String?
    var isRunning: Bool { ["running", "restarting", "paused"].contains(state) }
    /// First published host port of a running container, for the "open in browser" button.
    var hostPort: String? {
        guard isRunning,
              let m = try? NSRegularExpression(pattern: #":(\d+)(?:-\d+)?->"#).firstMatch(in: ports, range: NSRange(ports.startIndex..., in: ports)),
              let r = Range(m.range(at: 1), in: ports) else { return nil }
        return String(ports[r])
    }
}

/// Controls containers through the `docker` command line, so it works with Docker Desktop, OrbStack and Colima.
/// The tab is hidden when no docker CLI is installed.
@MainActor
final class DockerStore: ObservableObject {
    enum Daemon { case unknown, up, down }
    @Published private(set) var installed = false
    @Published private(set) var daemon: Daemon = .unknown
    @Published private(set) var containers: [DockerContainer] = []
    @Published private(set) var busy = Set<String>()
    @Published private(set) var message: String?
    @Published var pendingRemove: String?

    private var cli: String?
    private var pollTimer: Timer?, installTimer: Timer?

    private static let cliCandidates = [
        "/usr/local/bin/docker", "/opt/homebrew/bin/docker", "/Applications/Docker.app/Contents/Resources/bin/docker",
        NSHomeDirectory() + "/.docker/bin/docker", NSHomeDirectory() + "/.orbstack/bin/docker",
    ]
    private static let apps = ["/Applications/Docker.app", "/Applications/OrbStack.app"]

    func start() {
        checkInstalled()
        installTimer = Timer.scheduledTimer(withTimeInterval: 60, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.checkInstalled() }
        }
    }
    private func checkInstalled() {
        cli = Self.cliCandidates.first { FileManager.default.isExecutableFile(atPath: $0) }
        installed = cli != nil
    }

    // MARK: polling while the tab is visible
    func beginPolling() {
        refresh()
        pollTimer?.invalidate()
        pollTimer = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.refresh() }
        }
    }
    func endPolling() { pollTimer?.invalidate(); pollTimer = nil; pendingRemove = nil }

    func refresh() {
        guard let cli else { return }
        Task {
            let r = await Self.run(cli, ["ps", "-a", "--format", "{{json .}}"], timeout: 8)
            guard r.status == 0 else { daemon = .down; containers = []; return }
            daemon = .up
            let parsed: [DockerContainer] = r.out.split(separator: "\n").compactMap { line in
                guard let d = try? JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any],
                      let id = d["ID"] as? String else { return nil }
                let labels = d["Labels"] as? String ?? ""
                var project: String?
                if let range = labels.range(of: "com.docker.compose.project=") {
                    project = String(labels[range.upperBound...].prefix { $0 != "," })
                }
                return DockerContainer(id: id, name: d["Names"] as? String ?? id, image: d["Image"] as? String ?? "",
                                       state: d["State"] as? String ?? "", status: d["Status"] as? String ?? "",
                                       ports: d["Ports"] as? String ?? "", project: project)
            }
            containers = parsed.sorted { ($0.isRunning ? 0 : 1, $0.name) < ($1.isRunning ? 0 : 1, $1.name) }
        }
    }

    // MARK: actions
    func act(_ verb: String, on ids: [String]) {
        guard let cli, !ids.isEmpty else { return }
        busy.formUnion(ids); message = nil
        Task {
            let r = await Self.run(cli, [verb] + ids, timeout: 40)
            busy.subtract(ids)
            if r.status != 0 { message = r.err.split(separator: "\n").last.map(String.init) ?? "\(verb) failed" }
            refresh()
        }
    }
    func remove(_ c: DockerContainer) {
        if pendingRemove == c.id { pendingRemove = nil; act("rm", on: [c.id]) }
        else {
            pendingRemove = c.id
            DispatchQueue.main.asyncAfter(deadline: .now() + 3) { MainActor.assumeIsolated { if self.pendingRemove == c.id { self.pendingRemove = nil } } }
        }
    }
    func openApp() {
        guard let path = Self.apps.first(where: { FileManager.default.fileExists(atPath: $0) }) else { return }
        let cfg = NSWorkspace.OpenConfiguration(); cfg.activates = false
        NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: path), configuration: cfg)
    }
    var hasApp: Bool { Self.apps.contains { FileManager.default.fileExists(atPath: $0) } }

    // MARK: process helper
    nonisolated private static func run(_ exe: String, _ args: [String], timeout: TimeInterval) async -> (status: Int32, out: String, err: String) {
        await withCheckedContinuation { cont in
            DispatchQueue.global(qos: .userInitiated).async {
                let p = Process()
                p.executableURL = URL(fileURLWithPath: exe); p.arguments = args
                var env = ProcessInfo.processInfo.environment
                env["PATH"] = "/usr/local/bin:/opt/homebrew/bin:/Applications/Docker.app/Contents/Resources/bin:" + (env["PATH"] ?? "/usr/bin:/bin")
                p.environment = env
                let o = Pipe(), e = Pipe(); p.standardOutput = o; p.standardError = e
                guard (try? p.run()) != nil else { cont.resume(returning: (-1, "", "Could not run docker")); return }
                DispatchQueue.global().asyncAfter(deadline: .now() + timeout) { if p.isRunning { p.terminate() } }
                let out = o.fileHandleForReading.readDataToEndOfFile(), err = e.fileHandleForReading.readDataToEndOfFile()
                p.waitUntilExit()
                cont.resume(returning: (p.terminationStatus, String(decoding: out, as: UTF8.self), String(decoding: err, as: UTF8.self)))
            }
        }
    }
}

struct DockerTab: View {
    @ObservedObject var docker: DockerStore

    private var groups: [(name: String, items: [DockerContainer])] {
        let byProject = Dictionary(grouping: docker.containers) { $0.project ?? "" }
        return byProject.keys.sorted { a, b in
            if a.isEmpty != b.isEmpty { return !a.isEmpty }      // standalone containers last
            let ar = byProject[a]!.contains { $0.isRunning }, br = byProject[b]!.contains { $0.isRunning }
            return ar != br ? ar : a < b
        }.map { (name: $0, items: byProject[$0]!) }
    }

    var body: some View {
        VStack(spacing: Tok.Space.xs) {
            switch docker.daemon {
            case .unknown:
                ProgressView().controlSize(.small).frame(maxWidth: .infinity, maxHeight: .infinity)
            case .down:
                VStack(spacing: Tok.Space.sm) {
                    Text("Docker no está en marcha.").font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted)
                    if docker.hasApp {
                        Button("Abrir Docker") { docker.openApp() }.buttonStyle(.borderedProminent).tint(Tok.Color.accent).controlSize(.small)
                    }
                }.frame(maxWidth: .infinity, maxHeight: .infinity)
            case .up:
                header
                if docker.containers.isEmpty {
                    Text("No hay contenedores.").font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted).frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    ScrollView {
                        LazyVStack(spacing: Tok.Space.xxs, pinnedViews: []) {
                            ForEach(groups, id: \.name) { g in
                                if !g.name.isEmpty || groups.count > 1 { groupHeader(g.name, g.items) }
                                ForEach(g.items) { ContainerRow(c: $0, docker: docker) }
                            }
                        }
                    }.scrollIndicators(.hidden)
                }
            }
        }
        .padding(Tok.Space.sm)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Tok.Color.surface, in: RoundedRectangle(cornerRadius: Tok.Radius.lg))
        .onAppear { docker.beginPolling() }
        .onDisappear { docker.endPolling() }
    }

    private var header: some View {
        let running = docker.containers.filter(\.isRunning).count
        return HStack {
            Circle().fill(running > 0 ? Tok.Color.success : Tok.Color.textMuted).frame(width: 7, height: 7)
            Text("\(running) en marcha de \(docker.containers.count)").font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted)
            Spacer()
            if let m = docker.message { Text(m).font(Tok.Font.xs).foregroundStyle(Tok.Color.warning).lineLimit(1) }
        }
    }

    private func groupHeader(_ name: String, _ items: [DockerContainer]) -> some View {
        let anyRunning = items.contains(where: \.isRunning)
        return HStack(spacing: Tok.Space.xs) {
            Text(name.isEmpty ? "Sueltos" : name).font(Tok.Font.xs.weight(.semibold)).foregroundStyle(Tok.Color.textMuted)
            Spacer()
            if !name.isEmpty {
                let ids = items.filter { $0.isRunning == anyRunning }.map(\.id)
                Button { docker.act(anyRunning ? "stop" : "start", on: ids) } label: {
                    Label(anyRunning ? "Parar todo" : "Iniciar todo", systemImage: anyRunning ? "stop.fill" : "play.fill")
                        .font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted)
                }.buttonStyle(.plain)
            }
        }.padding(.horizontal, Tok.Space.xs).padding(.top, Tok.Space.xs)
    }
}

struct ContainerRow: View {
    let c: DockerContainer
    @ObservedObject var docker: DockerStore
    @State private var hover = false

    private var dot: Color {
        switch c.state { case "running": Tok.Color.success; case "paused", "restarting": Tok.Color.warning; default: Tok.Color.textMuted }
    }

    var body: some View {
        let working = docker.busy.contains(c.id)
        HStack(spacing: Tok.Space.sm) {
            Circle().fill(dot).frame(width: 7, height: 7)
            VStack(alignment: .leading, spacing: 1) {
                Text(c.name).font(Tok.Font.sm).foregroundStyle(Tok.Color.text).lineLimit(1).truncationMode(.middle)
                Text([c.image, c.hostPort.map { ":\($0)" }, c.status].compactMap { $0 }.joined(separator: " · "))
                    .font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted).lineLimit(1)
            }
            Spacer(minLength: 0)
            if working { ProgressView().controlSize(.mini) }
            else {
                if let port = c.hostPort {
                    btn("safari", "Abrir localhost:\(port)") { if let u = URL(string: "http://localhost:\(port)") { NSWorkspace.shared.open(u) } }
                }
                if c.isRunning {
                    btn("arrow.clockwise", "Reiniciar") { docker.act("restart", on: [c.id]) }
                    btn("stop.fill", "Parar") { docker.act("stop", on: [c.id]) }
                } else {
                    btn("play.fill", "Iniciar") { docker.act("start", on: [c.id]) }
                    if docker.pendingRemove == c.id {
                        Button("¿Borrar?") { docker.remove(c) }.buttonStyle(.borderedProminent).tint(Tok.Color.danger).controlSize(.mini)
                    } else {
                        btn("trash", "Borrar") { docker.remove(c) }
                    }
                }
            }
        }
        .padding(.horizontal, Tok.Space.xs).padding(.vertical, 3)
        .background(hover ? Tok.Color.surfaceHover : .clear, in: RoundedRectangle(cornerRadius: Tok.Radius.sm))
        .onHover { hover = $0 }
    }

    private func btn(_ symbol: String, _ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 11)).foregroundStyle(Tok.Color.text).frame(width: 24, height: 22)
                .background(Tok.Color.surfaceHover, in: RoundedRectangle(cornerRadius: Tok.Radius.sm))
        }.buttonStyle(.plain).accessibilityLabel(label).help(label)
    }
}
