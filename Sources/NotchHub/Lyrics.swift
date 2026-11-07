import SwiftUI

struct LyricLine: Identifiable, Hashable { let id: Int; let time: Double; let text: String }

struct Lyrics: Equatable {
    var synced: [LyricLine]      // empty if the song only has plain text
    var plain: String
    var instrumental = false
}

/// Lyrics from LRCLIB (lrclib.net): free, no key. Only the title, artist and length of the song are sent.
@MainActor
final class LyricsStore: ObservableObject {
    enum State: Equatable { case idle, loading, ready(Lyrics), none, offline }
    @Published private(set) var state: State = .idle
    /// The line being sung, for the ribbon under the closed notch. It changes only when a new line starts,
    /// so the view updates every few seconds instead of several times a second.
    @Published private(set) var liveLine = ""
    var positionProvider: (() -> (position: Double, playing: Bool))?
    private var liveTimer: Timer?
    private var key = ""
    private var cache: [String: Lyrics?] = [:]

    func load(for track: Track?) {
        guard let track else { state = .idle; key = ""; return }
        let k = "\(track.title)|\(track.artist)"
        guard k != key else { return }
        key = k
        if let hit = cache[k] { state = hit.map { .ready($0) } ?? .none; refreshLive(); return }
        state = .loading
        Task {
            let result = await Self.fetch(title: track.title, artist: track.artist, duration: track.duration)
            guard key == k else { return }          // the song changed while we were waiting
            switch result {
            case .failure: state = .offline
            case .success(let l): cache[k] = l; state = l.map { .ready($0) } ?? .none
            }
            refreshLive()
        }
    }

    /// Recomputes the live line and schedules the next change exactly when the next line starts.
    func refreshLive() {
        liveTimer?.invalidate()
        guard UserDefaults.standard.bool(forKey: "musicShowLyrics"), let (pos, playing) = positionProvider?(),
              case .ready(let l) = state, !l.synced.isEmpty else {
            if !liveLine.isEmpty { liveLine = "" }
            return
        }
        let p = pos + Settings.shared.lyricsLead
        let idx = l.synced.lastIndex { $0.time <= p }
        let text = idx.map { l.synced[$0].text } ?? ""
        if text != liveLine { liveLine = text }
        guard playing, let next = (idx.map { $0 + 1 } ?? 0) as Int?, next < l.synced.count else { return }
        let delay = max(0.05, l.synced[next].time - p)
        liveTimer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated { self?.refreshLive() }
        }
    }

    // MARK: network
    private struct Hit: Decodable {
        var trackName: String?, artistName: String?, duration: Double?
        var plainLyrics: String?, syncedLyrics: String?, instrumental: Bool?
    }

    /// Removes the usual YouTube decorations: "(Official Video)", "[Lyrics]", "- Topic", "ft. ...".
    nonisolated static func clean(_ s: String) -> String {
        var t = s.replacingOccurrences(of: #"\s*[\(\[][^\)\]]*(official|video|audio|lyrics?|letra|visuali[sz]er|remaster|hd|4k)[^\)\]]*[\)\]]"#,
                                       with: "", options: [.regularExpression, .caseInsensitive])
        t = t.replacingOccurrences(of: #"\s*-\s*Topic$"#, with: "", options: [.regularExpression, .caseInsensitive])
        return t.trimmingCharacters(in: .whitespaces)
    }

    nonisolated private static func get(_ path: String, _ q: [String: String]) async -> (data: Data, ok: Bool)? {
        var c = URLComponents(string: "https://lrclib.net/api/\(path)")!
        c.queryItems = q.map { URLQueryItem(name: $0.key, value: $0.value) }
        var req = URLRequest(url: c.url!, timeoutInterval: 10)
        req.setValue("NotchHub/1.0", forHTTPHeaderField: "User-Agent")
        guard let (data, resp) = try? await URLSession.shared.data(for: req) else { return nil }
        return (data, (resp as? HTTPURLResponse)?.statusCode == 200)
    }

    nonisolated static func fetch(title: String, artist: String, duration: Double) async -> Result<Lyrics?, Error> {
        struct Offline: Error {}
        let t = clean(title), a = clean(artist)
        var q = ["track_name": t, "artist_name": a]
        if duration > 0 { q["duration"] = String(Int(duration)) }
        guard let first = await get("get", q) else { return .failure(Offline()) }
        if first.ok, let h = try? JSONDecoder().decode(Hit.self, from: first.data), let l = lyrics(h) { return .success(l) }

        // not found by exact match: search, and prefer synced lyrics with the closest length
        for query in [["track_name": t, "artist_name": a], ["q": "\(a) \(t)"]] {
            guard let r = await get("search", query), r.ok, let hits = try? JSONDecoder().decode([Hit].self, from: r.data) else { continue }
            let usable = hits.filter { $0.syncedLyrics?.isEmpty == false || $0.plainLyrics?.isEmpty == false }
            let near = usable.filter { duration <= 0 || abs(($0.duration ?? 0) - duration) <= 5 }
            let best = (near.isEmpty ? usable : near).min {
                let s0 = $0.syncedLyrics?.isEmpty == false ? 0 : 1, s1 = $1.syncedLyrics?.isEmpty == false ? 0 : 1
                return (s0, abs(($0.duration ?? 0) - duration)) < (s1, abs(($1.duration ?? 0) - duration))
            }
            if let best, let l = lyrics(best) { return .success(l) }
        }
        return .success(nil)
    }

    nonisolated private static func lyrics(_ h: Hit) -> Lyrics? {
        if h.instrumental == true { return Lyrics(synced: [], plain: "", instrumental: true) }
        let synced = h.syncedLyrics.map(parseLRC) ?? []
        let plain = h.plainLyrics ?? ""
        return synced.isEmpty && plain.isEmpty ? nil : Lyrics(synced: synced, plain: plain)
    }

    /// "[01:23.45] text" lines. Several timestamps on one line are allowed.
    nonisolated static func parseLRC(_ s: String) -> [LyricLine] {
        let stamp = try! NSRegularExpression(pattern: #"\[(\d+):(\d+(?:\.\d+)?)\]"#)
        var out: [(Double, String)] = []
        for line in s.split(separator: "\n") {
            let str = String(line), ns = NSRange(str.startIndex..., in: str)
            let ms = stamp.matches(in: str, range: ns)
            guard let last = ms.last, let end = Range(last.range, in: str)?.upperBound else { continue }
            let text = str[end...].trimmingCharacters(in: .whitespaces)
            for m in ms {
                if let r1 = Range(m.range(at: 1), in: str), let r2 = Range(m.range(at: 2), in: str),
                   let mins = Double(str[r1]), let secs = Double(str[r2]) { out.append((mins * 60 + secs, text)) }
            }
        }
        return out.sorted { $0.0 < $1.0 }.enumerated().map { LyricLine(id: $0.offset, time: $0.element.0, text: $0.element.1) }
    }
}

/// Lyrics area inside the Music tab.
struct LyricsBody: View {
    @ObservedObject var nowPlaying: NowPlaying
    @ObservedObject var store: LyricsStore

    var body: some View {
        Group {
            if nowPlaying.track == nil {
                message("No hay nada sonando.")
            } else {
                switch store.state {
                case .idle, .loading: ProgressView().controlSize(.small).frame(maxWidth: .infinity, maxHeight: .infinity)
                case .none: message("No encuentro la letra de esta canción.")
                case .offline: message("Sin conexión. No se ha podido buscar la letra.")
                case .ready(let l):
                    if l.instrumental { message("Esta canción es instrumental.") }
                    else if !l.synced.isEmpty { SyncedLyrics(lines: l.synced, nowPlaying: nowPlaying) }
                    else { ScrollView { Text(l.plain).font(Tok.Font.sm).foregroundStyle(Tok.Color.text).frame(maxWidth: .infinity, alignment: .leading).padding(Tok.Space.sm) }.scrollIndicators(.hidden) }
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onAppear { store.load(for: nowPlaying.track) }
        .onChange(of: nowPlaying.track?.title) { _, _ in store.load(for: nowPlaying.track) }
    }

    private func message(_ s: String) -> some View {
        Text(s).font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted).multilineTextAlignment(.center).padding(Tok.Space.md)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// Time-synced lyrics: the current line is highlighted and kept in the middle. Click a line to jump there.
struct SyncedLyrics: View {
    let lines: [LyricLine]
    @ObservedObject var nowPlaying: NowPlaying

    var body: some View {
        TimelineView(.periodic(from: .now, by: 0.25)) { _ in
            let pos = nowPlaying.estimatedPosition
            let current = lines.lastIndex { $0.time <= pos + Settings.shared.lyricsLead } ?? -1
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(spacing: Tok.Space.sm) {
                        Spacer().frame(height: 40)
                        ForEach(lines) { line in
                            Button { nowPlaying.seek(to: line.time) } label: {
                                Text(line.text.isEmpty ? "♪" : line.text)
                                    .font(.system(size: line.id == current ? 17 : 14, weight: line.id == current ? .semibold : .regular))
                                    .foregroundStyle(line.id == current ? Tok.Color.text : Tok.Color.textMuted.opacity(line.id < current ? 0.5 : 0.8))
                                    .multilineTextAlignment(.center)
                                    .frame(maxWidth: .infinity)
                            }.buttonStyle(.plain).id(line.id)
                        }
                        Spacer().frame(height: 40)
                    }
                    .padding(.horizontal, Tok.Space.md)
                }
                .scrollIndicators(.hidden)
                .onChange(of: current) { _, c in withAnimation(.easeInOut(duration: 0.35)) { proxy.scrollTo(max(c, 0), anchor: .center) } }
                .onAppear { proxy.scrollTo(max(current, 0), anchor: .center) }
            }
        }
    }
}
