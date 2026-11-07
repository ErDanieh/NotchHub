import SwiftUI
import ServiceManagement

/// User settings, stored in UserDefaults. Views observe this and update live.
@MainActor
final class Settings: ObservableObject {
    static let shared = Settings()
    private let d = UserDefaults.standard

    @Published var haloEnabled: Bool { didSet { d.set(haloEnabled, forKey: "haloEnabled") } }
    /// Keep the halo on even when nothing is playing.
    @Published var haloAlways: Bool { didSet { d.set(haloAlways, forKey: "haloAlways") } }
    /// Show a small ball on screens that have no notch.
    @Published var ballsEnabled: Bool { didSet { d.set(ballsEnabled, forKey: "ballsEnabled") } }
    /// Pixel pet that walks along the top of the screen and peeks out from the notch.
    @Published var petsEnabled: Bool { didSet { d.set(petsEnabled, forKey: "petsEnabled") } }
    @Published var petKind: String { didSet { d.set(petKind, forKey: "petKind") } }
    @Published var petSize: Int { didSet { d.set(petSize, forKey: "petSize") } }
    @Published var haloBeat: Bool { didSet { d.set(haloBeat, forKey: "haloBeat") } }
    @Published var haloColorHex: String { didSet { d.set(haloColorHex, forKey: "haloColorHex") } }
    @Published var haloIntensity: Double { didSet { d.set(haloIntensity, forKey: "haloIntensity") } }
    @Published var pulseSeconds: Double { didSet { d.set(pulseSeconds, forKey: "pulseSeconds") } }
    @Published var openDelay: Double { didSet { d.set(openDelay, forKey: "openDelay") } }
    @Published var hideInFullscreen: Bool { didSet { d.set(hideInFullscreen, forKey: "hideInFullscreen") } }
    @Published var alertsSong: Bool { didSet { d.set(alertsSong, forKey: "alertsSong") } }
    @Published var alertsPerformance: Bool { didSet { d.set(alertsPerformance, forKey: "alertsPerformance") } }
    /// Order and visibility of the notch tabs (raw values of NotchTab). New tabs that are not listed yet go at the end.
    @Published var tabOrder: [String] { didSet { d.set(tabOrder, forKey: "tabOrder") } }
    @Published var hiddenTabs: [String] { didSet { d.set(hiddenTabs, forKey: "hiddenTabs") } }
    /// Seconds the lyrics are shown early (+) or late (-). Synced lyrics rarely match every version of a song exactly.
    @Published var lyricsLead: Double { didSet { d.set(lyricsLead, forKey: "lyricsLead") } }
    @Published var rememberTab: Bool { didSet { d.set(rememberTab, forKey: "rememberTab") } }
    @Published var lastTab: String { didSet { d.set(lastTab, forKey: "lastTab") } }
    @Published var mirrorLevel: Int { didSet { d.set(mirrorLevel, forKey: "mirrorLevel") } }
    @Published var alertsCalendar: Bool { didSet { d.set(alertsCalendar, forKey: "alertsCalendar") } }
    @Published var alertsConversion: Bool { didSet { d.set(alertsConversion, forKey: "alertsConversion") } }
    @Published var calendarLeadMinutes: Double { didSet { d.set(calendarLeadMinutes, forKey: "calendarLeadMinutes") } }
    @Published var cpuThreshold: Double { didSet { d.set(cpuThreshold, forKey: "cpuThreshold") } }
    @Published var memThreshold: Double { didSet { d.set(memThreshold, forKey: "memThreshold") } }
    @Published var batteryThreshold: Double { didSet { d.set(batteryThreshold, forKey: "batteryThreshold") } }

    private init() {
        d.register(defaults: ["haloEnabled": true, "petsEnabled": true, "petKind": "clawd", "petSize": 3, "haloAlways": false, "ballsEnabled": true, "haloBeat": false, "haloColorHex": "3DA9FF", "haloIntensity": 1.0,
                              "pulseSeconds": 1.6, "openDelay": 0.12, "hideInFullscreen": false,
                              "rememberTab": true, "lyricsLead": 0.3, "lastTab": "tray.and.arrow.down", "mirrorLevel": 0, "alertsSong": true, "alertsCalendar": true, "alertsConversion": true, "calendarLeadMinutes": 10.0, "alertsPerformance": true, "cpuThreshold": 90.0, "memThreshold": 90.0, "batteryThreshold": 15.0])
        haloEnabled = d.bool(forKey: "haloEnabled")
        haloAlways = d.bool(forKey: "haloAlways")
        ballsEnabled = d.bool(forKey: "ballsEnabled")
        petsEnabled = d.bool(forKey: "petsEnabled")
        petKind = d.string(forKey: "petKind") ?? "clawd"
        petSize = max(2, min(5, d.integer(forKey: "petSize")))
        haloBeat = d.bool(forKey: "haloBeat")
        haloColorHex = d.string(forKey: "haloColorHex") ?? "3DA9FF"
        haloIntensity = d.double(forKey: "haloIntensity")
        pulseSeconds = d.double(forKey: "pulseSeconds")
        openDelay = d.double(forKey: "openDelay")
        hideInFullscreen = d.bool(forKey: "hideInFullscreen")
        alertsSong = d.bool(forKey: "alertsSong")
        alertsPerformance = d.bool(forKey: "alertsPerformance")
        tabOrder = d.stringArray(forKey: "tabOrder") ?? []
        hiddenTabs = d.stringArray(forKey: "hiddenTabs") ?? []
        lyricsLead = d.double(forKey: "lyricsLead")
        rememberTab = d.bool(forKey: "rememberTab")
        lastTab = d.string(forKey: "lastTab") ?? "tray.and.arrow.down"
        mirrorLevel = d.integer(forKey: "mirrorLevel")
        alertsCalendar = d.bool(forKey: "alertsCalendar")
        alertsConversion = d.bool(forKey: "alertsConversion")
        calendarLeadMinutes = d.double(forKey: "calendarLeadMinutes")
        cpuThreshold = d.double(forKey: "cpuThreshold")
        memThreshold = d.double(forKey: "memThreshold")
        batteryThreshold = d.double(forKey: "batteryThreshold")
    }

    var haloColor: Color {
        get { Color(hex: UInt32(haloColorHex, radix: 16) ?? 0x3DA9FF) }
        set {
            let c = NSColor(newValue).usingColorSpace(.sRGB) ?? .systemBlue
            haloColorHex = String(format: "%02X%02X%02X", Int(round(c.redComponent * 255)),
                                  Int(round(c.greenComponent * 255)), Int(round(c.blueComponent * 255)))
        }
    }

    /// All tabs in the user's order (including hidden ones, for the reorder window).
    var allTabsOrdered: [NotchTab] {
        let known = tabOrder.compactMap { NotchTab(rawValue: $0) }
        return known + NotchTab.allCases.filter { !known.contains($0) }
    }
    /// What the tab bar shows: ordered, minus hidden ones, minus Docker when it is not installed.
    func visibleTabs(dockerInstalled: Bool) -> [NotchTab] {
        let v = allTabsOrdered.filter { !hiddenTabs.contains($0.rawValue) && ($0 != .docker || dockerInstalled) }
        return v.isEmpty ? [.shelf] : v
    }

    func reset() {
        tabOrder = []; hiddenTabs = []
        haloEnabled = true; petsEnabled = true; petKind = "clawd"; petSize = 3; haloAlways = false; ballsEnabled = true; haloBeat = false; haloColorHex = "3DA9FF"; haloIntensity = 1.0
        pulseSeconds = 1.6; openDelay = 0.12; hideInFullscreen = false
        lyricsLead = 0.3; rememberTab = true; alertsSong = true; alertsCalendar = true; alertsConversion = true; calendarLeadMinutes = 10; alertsPerformance = true; cpuThreshold = 90; memThreshold = 90; batteryThreshold = 15
    }
}

struct SettingsView: View {
    @ObservedObject var settings = Settings.shared
    @State private var launchAtLogin = SMAppService.mainApp.status == .enabled
    @State private var loginError: String?

    var body: some View {
        Form {
            Section("Pet") {
                Toggle("Show a pet that walks along the top of the screen", isOn: $settings.petsEnabled)
                Picker("Pet", selection: $settings.petKind) {
                    ForEach(PetSprites.kinds, id: \.id) { Text($0.name).tag($0.id) }
                }.disabled(!settings.petsEnabled)
                Picker("Size", selection: $settings.petSize) {
                    Text("Small").tag(2); Text("Medium").tag(3); Text("Large").tag(4); Text("Huge").tag(5)
                }.disabled(!settings.petsEnabled)
                Text("It wanders, sleeps, peeks out from under the notch and dances when music plays.").font(.caption).foregroundStyle(.secondary)
            }
            Section("Halo") {
                Toggle("Show halo while something plays", isOn: $settings.haloEnabled)
                Toggle("Keep the halo on even when nothing plays", isOn: $settings.haloAlways).disabled(!settings.haloEnabled)
                Toggle("Show a ball on the other screens", isOn: $settings.ballsEnabled)
                Toggle("Make the halo follow the beat", isOn: $settings.haloBeat)
                if settings.haloBeat {
                    TimelineView(.periodic(from: .now, by: 1)) { _ in
                        Text("Listening to the system audio: " + AudioLevel.shared.status + ". macOS asks for permission the first time (System Audio Recording).")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
                ColorPicker("Colour", selection: Binding(get: { settings.haloColor }, set: { settings.haloColor = $0 }),
                            supportsOpacity: false)
                LabeledContent("Intensity") { Slider(value: $settings.haloIntensity, in: 0.3...1.6) }
                LabeledContent("Pulse speed") {
                    // slider shows faster to the right, stored as seconds per pulse
                    Slider(value: Binding(get: { 3.2 - settings.pulseSeconds }, set: { settings.pulseSeconds = 3.2 - $0 }), in: 0.4...2.6)
                }
            }
            Section("Notch") {
                LabeledContent("Open delay") {
                    Slider(value: $settings.openDelay, in: 0...0.6)
                    Text(String(format: "%.2fs", settings.openDelay)).monospacedDigit().frame(width: 48, alignment: .trailing)
                }
                Toggle("Open on the tab I used last", isOn: $settings.rememberTab)
                LabeledContent("Lyrics timing") { Slider(value: $settings.lyricsLead, in: -2...2, step: 0.1)
                    Text(String(format: "%+.1fs", settings.lyricsLead)).monospacedDigit().frame(width: 48, alignment: .trailing) }
                Toggle("Hide when an app is full screen", isOn: $settings.hideInFullscreen)
            }
            Section("Alerts") {
                Toggle("Show the song when it changes", isOn: $settings.alertsSong)
                Toggle("Remind me before calendar events", isOn: $settings.alertsCalendar)
                LabeledContent("Remind me") { Slider(value: $settings.calendarLeadMinutes, in: 1...30, step: 1)
                    Text("\(Int(settings.calendarLeadMinutes)) min before").monospacedDigit().frame(width: 96, alignment: .trailing) }
                Toggle("Tell me when a conversion finishes", isOn: $settings.alertsConversion)
                Toggle("Warn about high CPU, memory or low battery", isOn: $settings.alertsPerformance)
                LabeledContent("CPU above") { Slider(value: $settings.cpuThreshold, in: 50...100, step: 5)
                    Text("\(Int(settings.cpuThreshold))%").monospacedDigit().frame(width: 44, alignment: .trailing) }
                LabeledContent("Memory above") { Slider(value: $settings.memThreshold, in: 50...100, step: 5)
                    Text("\(Int(settings.memThreshold))%").monospacedDigit().frame(width: 44, alignment: .trailing) }
                LabeledContent("Battery below") { Slider(value: $settings.batteryThreshold, in: 5...40, step: 5)
                    Text("\(Int(settings.batteryThreshold))%").monospacedDigit().frame(width: 44, alignment: .trailing) }
            }
            Section("System") {
                Toggle("Launch at login", isOn: $launchAtLogin).onChange(of: launchAtLogin) { _, on in
                    do { if on { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }; loginError = nil }
                    catch { loginError = "Only works from the installed NotchHub.app. \(error.localizedDescription)"; launchAtLogin = !on }
                }
                if let loginError { Text(loginError).font(.caption).foregroundStyle(.secondary) }
            }
            Button("Reset to defaults") { settings.reset() }
        }
        .formStyle(.grouped)
        .frame(width: 480, height: 840)
    }
}


/// Window to put the tabs in the order you like and hide the ones you do not use.
struct TabOrderView: View {
    @ObservedObject var settings = Settings.shared
    @State private var order: [NotchTab] = Settings.shared.allTabsOrdered

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Drag to reorder. Turn off the ones you do not want in the notch.")
                .font(.callout).foregroundStyle(.secondary).padding(.horizontal, 16).padding(.top, 12)
            List {
                ForEach(order, id: \.self) { tab in
                    HStack(spacing: 10) {
                        Image(systemName: "line.3.horizontal").foregroundStyle(.tertiary)
                        Image(systemName: tab.rawValue).frame(width: 20)
                        Text(tab.label + (tab == .docker ? " (only if Docker is installed)" : ""))
                        Spacer()
                        Toggle("", isOn: Binding(
                            get: { !settings.hiddenTabs.contains(tab.rawValue) },
                            set: { on in
                                if on { settings.hiddenTabs.removeAll { $0 == tab.rawValue } }
                                else if order.filter({ !settings.hiddenTabs.contains($0.rawValue) }).count > 1 { settings.hiddenTabs.append(tab.rawValue) }
                            })).labelsHidden().toggleStyle(.switch)
                    }
                }
                .onMove { from, to in
                    order.move(fromOffsets: from, toOffset: to)
                    settings.tabOrder = order.map(\.rawValue)
                }
            }
            HStack {
                Button("Reset order") { settings.tabOrder = []; settings.hiddenTabs = []; order = NotchTab.allCases }
                Spacer()
            }.padding([.horizontal, .bottom], 16)
        }
        .frame(width: 380, height: 460)
    }
}
