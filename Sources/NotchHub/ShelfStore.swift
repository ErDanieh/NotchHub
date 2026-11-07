import Foundation
import AppKit

struct ShelfItem: Identifiable, Codable, Equatable {
    var id = UUID()
    var name: String
    var bookmark: Data
    var addedAt = Date()

    /// Resolve to a file URL, nil if the file is gone.
    var url: URL? {
        var stale = false
        guard let u = try? URL(resolvingBookmarkData: bookmark, options: [], relativeTo: nil, bookmarkDataIsStale: &stale),
              FileManager.default.fileExists(atPath: u.path) else { return nil }
        return u
    }
}

/// Slice 1 data layer: a JSON file. Same function names the SQLite version will keep
/// (add / remove / clear / items), so screens do not change when it is swapped.
@MainActor
final class ShelfStore: ObservableObject {
    @Published private(set) var items: [ShelfItem] = []
    private let fileURL: URL

    init() {
        let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("NotchHub", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        fileURL = dir.appendingPathComponent("shelf.json")
        if let data = try? Data(contentsOf: fileURL),
           let saved = try? JSONDecoder().decode([ShelfItem].self, from: data) { items = saved }
    }

    func add(_ urls: [URL]) {
        for url in urls {
            guard let bm = try? url.bookmarkData() else { continue }
            if items.contains(where: { $0.url?.standardizedFileURL.path == url.standardizedFileURL.path }) { continue }   // same file twice: ignore
            items.insert(ShelfItem(name: url.lastPathComponent, bookmark: bm), at: 0)
        }
        save()
    }
    func remove(_ id: UUID) { items.removeAll { $0.id == id }; save() }
    func clear() { items.removeAll(); save() }

    private func save() {
        if let data = try? JSONEncoder().encode(items) { try? data.write(to: fileURL, options: .atomic) }
    }
}
