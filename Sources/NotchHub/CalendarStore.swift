import SwiftUI
import EventKit

/// Reads your calendars with EventKit (read only) and reminds you before events start.
@MainActor
final class CalendarStore: ObservableObject {
    enum Access { case unknown, granted, denied }
    @Published private(set) var access: Access = .unknown
    @Published private(set) var monthEvents: [EKEvent] = []
    @Published var month: Date = CalendarStore.startOfMonth(Date()) { didSet { loadMonth() } }
    @Published var selected = Date()

    private let store = EKEventStore()
    private var timer: Timer?
    private var alerted = Set<String>()
    private let cal = Calendar.current

    static func startOfMonth(_ d: Date) -> Date {
        Calendar.current.date(from: Calendar.current.dateComponents([.year, .month], from: d)) ?? d
    }

    func start() {
        refreshAccess()
        NotificationCenter.default.addObserver(forName: .EKEventStoreChanged, object: store, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.loadMonth() }
        }
        timer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.checkUpcoming() }
        }
    }

    private func refreshAccess() {
        switch EKEventStore.authorizationStatus(for: .event) {
        case .fullAccess: access = .granted; loadMonth()
        case .notDetermined: access = .unknown
        default: access = .denied
        }
    }

    /// Triggered by a button, so the macOS permission prompt never appears out of nowhere.
    func requestAccess() {
        store.requestFullAccessToEvents { [weak self] granted, _ in
            DispatchQueue.main.async { MainActor.assumeIsolated { self?.refreshAccess() } }
        }
    }

    func loadMonth() {
        guard access == .granted, let end = cal.date(byAdding: .month, value: 1, to: month) else { return }
        let p = store.predicateForEvents(withStart: month, end: end, calendars: nil)
        monthEvents = store.events(matching: p).sorted { $0.startDate < $1.startDate }
    }

    func events(on day: Date) -> [EKEvent] {
        let start = cal.startOfDay(for: day), end = cal.date(byAdding: .day, value: 1, to: start)!
        return monthEvents.filter { $0.startDate < end && $0.endDate > start }
    }

    func shiftMonth(_ by: Int) { if let m = cal.date(byAdding: .month, value: by, to: month) { month = m } }
    func goToday() { month = Self.startOfMonth(Date()); selected = Date() }

    /// Days of the displayed month, padded with nil so the first one lands on the right weekday.
    func gridDays() -> [Date?] {
        let lead = (cal.component(.weekday, from: month) - cal.firstWeekday + 7) % 7
        let count = cal.range(of: .day, in: .month, for: month)?.count ?? 30
        var days: [Date?] = Array(repeating: nil, count: lead)
        for i in 0..<count { days.append(cal.date(byAdding: .day, value: i, to: month)) }
        while days.count % 7 != 0 { days.append(nil) }
        return days
    }

    private func checkUpcoming() {
        guard access == .granted, Settings.shared.alertsCalendar else { return }
        let now = Date(), lead = Settings.shared.calendarLeadMinutes
        let p = store.predicateForEvents(withStart: now, end: now.addingTimeInterval((lead + 1) * 60), calendars: nil)
        for e in store.events(matching: p) where !e.isAllDay {
            let key = "\(e.eventIdentifier ?? e.title ?? "")-\(e.startDate.timeIntervalSince1970)"
            let secs = e.startDate.timeIntervalSince(now)
            guard secs > 0, secs <= lead * 60, !alerted.contains(key) else { continue }
            alerted.insert(key)
            let mins = Int((secs / 60).rounded(.up))
            AlertCenter.shared.post(symbol: "calendar", title: e.title ?? "Event", detail: mins <= 1 ? "starting now" : "in \(mins) min")
        }
    }
}

struct CalendarTab: View {
    @ObservedObject var cal: CalendarStore
    private let c = Calendar.current

    var body: some View {
        Group {
            switch cal.access {
            case .granted: content
            case .unknown:
                prompt("NotchHub can show your events and remind you before they start. It only reads your calendar.",
                       button: "Allow calendar access") { cal.requestAccess() }
            case .denied:
                prompt("Calendar access is off. Turn it on in System Settings > Privacy & Security > Calendars.",
                       button: "Open System Settings") {
                    NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars")!)
                }
            }
        }
        .padding(Tok.Space.sm)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Tok.Color.surface, in: RoundedRectangle(cornerRadius: Tok.Radius.lg))
        .clipShape(RoundedRectangle(cornerRadius: Tok.Radius.lg))
        .onAppear { cal.loadMonth() }
    }

    private func prompt(_ text: String, button: String, action: @escaping () -> Void) -> some View {
        VStack(spacing: Tok.Space.sm) {
            Text(text).font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted).multilineTextAlignment(.center)
            Button(button, action: action).buttonStyle(.borderedProminent).tint(Tok.Color.accent).controlSize(.small)
        }.frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var content: some View {
        HStack(spacing: Tok.Space.md) {
            monthGrid.frame(width: 190)
            Divider().overlay(Tok.Color.border)
            dayList
        }
    }

    private var monthGrid: some View {
        let symbols = (0..<7).map { c.veryShortWeekdaySymbols[(c.firstWeekday - 1 + $0) % 7] }
        return VStack(spacing: 2) {
            HStack(spacing: Tok.Space.sm) {
                Text(cal.month.formatted(.dateTime.month(.wide).year())).font(Tok.Font.xs.weight(.semibold)).foregroundStyle(Tok.Color.text)
                Spacer()
                nav("chevron.left", "Previous month") { cal.shiftMonth(-1) }
                nav("circle.fill", "Today") { cal.goToday() }
                nav("chevron.right", "Next month") { cal.shiftMonth(1) }
            }
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 0), count: 7), spacing: 0) {
                ForEach(Array(symbols.enumerated()), id: \.offset) { _, s in
                    Text(s).font(.system(size: 9, weight: .medium)).foregroundStyle(Tok.Color.textMuted).frame(height: 12)
                }
                ForEach(Array(cal.gridDays().enumerated()), id: \.offset) { _, d in
                    if let d { dayCell(d) } else { Color.clear.frame(height: 15) }
                }
            }
        }
    }

    private func dayCell(_ d: Date) -> some View {
        let isSel = c.isDate(d, inSameDayAs: cal.selected), isToday = c.isDateInToday(d)
        let has = !cal.events(on: d).isEmpty
        return Button { cal.selected = d } label: {
            Text("\(c.component(.day, from: d))").font(.system(size: 10)).monospacedDigit()
                .foregroundStyle(isSel ? Tok.Color.onAccent : Tok.Color.text)
                .frame(maxWidth: .infinity).frame(height: 15)
                .background(isSel ? Tok.Color.accent : .clear, in: Circle())
                .overlay(Circle().strokeBorder(isToday && !isSel ? Tok.Color.neon : .clear, lineWidth: 1))
                .overlay(alignment: .bottom) { if has && !isSel { Circle().fill(Tok.Color.neon).frame(width: 2.5, height: 2.5).offset(y: 1) } }
        }.buttonStyle(.plain).accessibilityLabel(d.formatted(date: .complete, time: .omitted) + (has ? ", has events" : ""))
    }

    private func nav(_ symbol: String, _ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) { Image(systemName: symbol).font(.system(size: symbol == "circle.fill" ? 6 : 9, weight: .semibold))
            .foregroundStyle(Tok.Color.textMuted).frame(width: 14, height: 14) }.buttonStyle(.plain).accessibilityLabel(label)
    }

    private var dayList: some View {
        let events = cal.events(on: cal.selected)
        return VStack(alignment: .leading, spacing: Tok.Space.xs) {
            Text(c.isDateInToday(cal.selected) ? "Today" : cal.selected.formatted(.dateTime.weekday(.wide).day().month()))
                .font(Tok.Font.xs.weight(.semibold)).foregroundStyle(Tok.Color.textMuted)
            if events.isEmpty {
                Text("No events").font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted).frame(maxHeight: .infinity, alignment: .center)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: Tok.Space.xs) {
                        ForEach(events, id: \.eventIdentifier) { e in
                            HStack(spacing: Tok.Space.sm) {
                                Capsule().fill(Color(nsColor: e.calendar.color)).frame(width: 3, height: 26)
                                VStack(alignment: .leading, spacing: 0) {
                                    Text(e.title ?? "Untitled").font(Tok.Font.sm).foregroundStyle(Tok.Color.text).lineLimit(1)
                                    Text(e.isAllDay ? "All day" : e.startDate.formatted(date: .omitted, time: .shortened) + " – " + e.endDate.formatted(date: .omitted, time: .shortened))
                                        .font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted)
                                }
                            }
                        }
                    }
                }.scrollIndicators(.hidden)
            }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
}
