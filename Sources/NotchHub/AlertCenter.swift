import SwiftUI

struct NotchAlert: Identifiable, Equatable {
    let id = UUID()
    var symbol: String
    var title: String
    var detail: String
    var artwork: URL? = nil
}

/// Short messages shown by widening the closed notch for a few seconds, one after another.
@MainActor
final class AlertCenter: ObservableObject {
    static let shared = AlertCenter()
    @Published private(set) var current: NotchAlert?
    private var queue: [NotchAlert] = []
    private var showing = false

    func post(symbol: String, title: String, detail: String = "", artwork: URL? = nil) {
        queue.append(NotchAlert(symbol: symbol, title: title, detail: detail, artwork: artwork))
        if queue.count > 5 { queue.removeFirst(queue.count - 5) }
        if !showing { next() }
    }

    private func next() {
        guard !queue.isEmpty else { showing = false; current = nil; return }
        showing = true
        current = queue.removeFirst()
        let shown = current
        DispatchQueue.main.asyncAfter(deadline: .now() + 3.2) { MainActor.assumeIsolated {
            if self.current == shown { self.current = nil }
            // small gap so the notch visibly closes between two alerts
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { MainActor.assumeIsolated { self.next() } }
        } }
    }
}
