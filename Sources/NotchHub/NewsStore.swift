import SwiftUI
import Combine

// MARK: model

struct NewsItem: Identifiable, Codable, Hashable {
    var id: String { link }
    var title: String
    var link: String
    var source: String
    var date: Date
    var image: String?
}

/// A tab of the News panel. A category either has its own sources, or is a keyword-filtered view of another one (the built-in "IA" tabs).
struct NewsCategory: Codable, Identifiable, Hashable {
    var id: String
    var name: String
    var parent: String? = nil
    var filter: String? = nil       // "ai" or "aiMarket"
    var builtin = false
}
let defaultCategories: [NewsCategory] = [
    NewsCategory(id: "tech", name: "Tecnología", builtin: true),
    NewsCategory(id: "ai", name: "IA", parent: "tech", filter: "ai", builtin: true),
    NewsCategory(id: "market", name: "Bolsa", builtin: true),
    NewsCategory(id: "aiMarket", name: "Bolsa e IA", parent: "market", filter: "aiMarket", builtin: true),
]

/// A news source. The defaults below ship with the app; users can switch them off and add their own.
struct Feed: Codable, Identifiable, Hashable {
    var name: String
    var url: String
    var category: String
    var onlyLinksContaining: String? = nil
    var enabled = true
    var builtin = false
    var id: String { url }

    init(name: String, url: String, category: String, onlyLinksContaining: String? = nil, builtin: Bool = false) {
        self.name = name; self.url = url; self.category = category; self.onlyLinksContaining = onlyLinksContaining; self.builtin = builtin
    }
    private enum CodingKeys: String, CodingKey { case name, url, category, onlyLinksContaining, enabled, builtin, group }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        name = try c.decode(String.self, forKey: .name); url = try c.decode(String.self, forKey: .url)
        // older versions stored a "group" (tech / finance) instead of a category
        let old = try c.decodeIfPresent(String.self, forKey: .group)
        category = try c.decodeIfPresent(String.self, forKey: .category) ?? (old == "finance" ? "market" : "tech")
        onlyLinksContaining = try c.decodeIfPresent(String.self, forKey: .onlyLinksContaining)
        enabled = try c.decodeIfPresent(Bool.self, forKey: .enabled) ?? true
        builtin = try c.decodeIfPresent(Bool.self, forKey: .builtin) ?? false
    }
    func encode(to e: Encoder) throws {
        var c = e.container(keyedBy: CodingKeys.self)
        try c.encode(name, forKey: .name); try c.encode(url, forKey: .url); try c.encode(category, forKey: .category)
        try c.encodeIfPresent(onlyLinksContaining, forKey: .onlyLinksContaining)
        try c.encode(enabled, forKey: .enabled); try c.encode(builtin, forKey: .builtin)
    }
}
/// Public RSS feeds in Spanish.
let defaultFeeds: [Feed] = [
    Feed(name: "Xataka", url: "https://www.xataka.com/feedburner.xml", category: "tech", builtin: true),
    Feed(name: "Genbeta", url: "https://www.genbeta.com/feedburner.xml", category: "tech", builtin: true),
    Feed(name: "Hipertextual", url: "https://hipertextual.com/feed", category: "tech", builtin: true),
    Feed(name: "El País Tecnología", url: "https://feeds.elpais.com/mrss-s/pages/ep/site/elpais.com/section/tecnologia/portada", category: "tech", builtin: true),
    Feed(name: "Expansión", url: "https://e00-expansion.uecdn.es/rss/mercados.xml", category: "market", builtin: true),
    Feed(name: "Cinco Días", url: "https://feeds.elpais.com/mrss-s/pages/ep/site/cincodias.elpais.com/portada", category: "market", onlyLinksContaining: "/mercados-financieros/", builtin: true),
    Feed(name: "Investing.com", url: "https://es.investing.com/rss/news_25.rss", category: "market", builtin: true),
]

/// Keyword filters for the "IA" and "Bolsa e IA" views.
private let aiWords = "inteligencia artificial|ia generativa|openai|chatgpt|anthropic|claude|gemini|deepseek|copilot|mistral|grok|llm|gpt-?\\d|modelos? de lenguaje|machine learning|agentes? de ia"
private let marketWords = "nvidia|amd|tsmc|broadcom|palantir|semiconductores|chips?|centros? de datos|alphabet|microsoft|meta platforms|oracle|burbuja"
private let aiRegex = try! NSRegularExpression(pattern: "(?i)\\b(\(aiWords))\\b|(?-i:\\bIA\\b)")
private let aiMarketRegex = try! NSRegularExpression(pattern: "(?i)\\b(\(aiWords)|\(marketWords))\\b|(?-i:\\bIA\\b)")

private func matches(_ re: NSRegularExpression, _ text: String) -> Bool {
    re.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)) != nil
}

// MARK: store

@MainActor
final class NewsStore: ObservableObject {
    /// Stories by category id.
    @Published private(set) var items: [String: [NewsItem]] = [:]
    @Published private(set) var loading = false
    @Published private(set) var updated: Date?
    @Published private(set) var failedSources: [String] = []

    private var byFeed: [String: [NewsItem]] = [:]     // feed address -> its latest stories
    private var timer: Timer?
    private let cacheURL: URL
    private var bag = Set<AnyCancellable>()
    private var pendingRefresh = false

    init() {
        let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("NotchHub", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        cacheURL = dir.appendingPathComponent("news-v2.json")
        if let data = try? Data(contentsOf: cacheURL),
           let c = try? JSONDecoder.news.decode(Cache.self, from: data) {
            byFeed = c.byFeed; updated = c.updated; rebuild()
        }
        // sources or categories changed in the manager: reload
        Publishers.Merge(Settings.shared.$feeds.map { _ in () }, Settings.shared.$categories.map { _ in () })
            .dropFirst(2).debounce(for: 1, scheduler: DispatchQueue.main)
            .sink { [weak self] in MainActor.assumeIsolated { self?.rebuild(); self?.refresh() } }.store(in: &bag)
    }

    func start() {
        timer = Timer.scheduledTimer(withTimeInterval: 20 * 60, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.refresh() }
        }
    }

    /// Called when the News tab opens: refresh only if the data is older than 10 minutes.
    func refreshIfStale() { if updated == nil || Date().timeIntervalSince(updated!) > 600 { refresh() } }

    func refresh() {
        guard !loading else { pendingRefresh = true; return }
        loading = true
        let feeds = Settings.shared.feeds.filter(\.enabled)
        Task {
            var results: [String: [NewsItem]] = [:], failed: [String] = []
            await withTaskGroup(of: (Feed, [NewsItem]?).self) { group in
                for feed in feeds { group.addTask { (feed, await Self.fetch(feed)) } }
                for await (feed, result) in group {
                    if let result { results[feed.url] = result } else { failed.append(feed.name) }
                }
            }
            // a source that failed this time keeps its previous stories
            if !results.isEmpty { for (u, r) in results { byFeed[u] = r }; updated = Date(); save() }
            failedSources = failed
            rebuild()
            loading = false
            if pendingRefresh { pendingRefresh = false; refresh() }
        }
    }

    private func rebuild() {
        let s = Settings.shared
        func newest(_ a: [NewsItem]) -> [NewsItem] {
            var seen = Set<String>()
            return a.sorted { $0.date > $1.date }.filter { seen.insert($0.link).inserted }
        }
        func all(of category: String) -> [NewsItem] {
            newest(s.feeds.filter { $0.enabled && $0.category == category }.flatMap { byFeed[$0.url] ?? [] })
        }
        var out: [String: [NewsItem]] = [:]
        for c in s.categories {
            if let p = c.parent {
                out[c.id] = all(of: p).filter { matches(c.filter == "aiMarket" ? aiMarketRegex : aiRegex, $0.title) }
            } else { out[c.id] = all(of: c.id) }
            out[c.id] = Array(out[c.id]!.prefix(40))
        }
        items = out
    }

    private struct Cache: Codable { var byFeed: [String: [NewsItem]]; var updated: Date }
    private func save() {
        guard let updated, let data = try? JSONEncoder.news.encode(Cache(byFeed: byFeed, updated: updated)) else { return }
        try? data.write(to: cacheURL, options: .atomic)
    }

    // MARK: network + parsing (off the main actor)
    nonisolated static func fetch(_ feed: Feed) async -> [NewsItem]? {
        guard let url = URL(string: feed.url) else { return nil }
        var req = URLRequest(url: url, timeoutInterval: 12)
        req.setValue("Mozilla/5.0 (Macintosh) NotchHub", forHTTPHeaderField: "User-Agent")
        guard let (data, resp) = try? await URLSession.shared.data(for: req),
              (resp as? HTTPURLResponse)?.statusCode == 200 else { return nil }
        let all = FeedParser(source: feed.name).parse(data)
        guard let key = feed.onlyLinksContaining else { return all }
        return all.filter { $0.link.contains(key) }   // the front page also carries non-market stories
    }
}

extension NewsStore {
    /// Takes the address of a website or of a feed and returns a working feed address with its items.
    /// Tries the address itself, then the feed announced in the page's HTML, then the usual paths.
    nonisolated static func discover(_ address: String, category: String, name: String) async -> (Feed, [NewsItem])? {
        func attempt(_ u: String) async -> (Feed, [NewsItem])? {
            let f = Feed(name: name, url: u, category: category)
            if let items = await fetch(f), !items.isEmpty { return (f, items) }
            return nil
        }
        if let ok = await attempt(address) { return ok }
        guard let base = URL(string: address) else { return nil }
        var candidates: [String] = []
        var req = URLRequest(url: base, timeoutInterval: 12)
        req.setValue("Mozilla/5.0 (Macintosh) NotchHub", forHTTPHeaderField: "User-Agent")
        if let (data, _) = try? await URLSession.shared.data(for: req) {
            let html = String(decoding: data.prefix(400_000), as: UTF8.self)
            let tags = try! NSRegularExpression(pattern: "<link\\b[^>]*>", options: .caseInsensitive)
            let href = try! NSRegularExpression(pattern: "href=[\"']([^\"']+)[\"']", options: .caseInsensitive)
            for m in tags.matches(in: html, range: NSRange(html.startIndex..., in: html)) {
                guard let r = Range(m.range, in: html) else { continue }
                let tag = String(html[r]).lowercased()
                guard tag.contains("alternate"), tag.contains("rss") || tag.contains("atom") else { continue }
                if let h = href.firstMatch(in: String(html[r]), range: NSRange(location: 0, length: String(html[r]).utf16.count)),
                   let hr = Range(h.range(at: 1), in: String(html[r])),
                   let u = URL(string: String(String(html[r])[hr]).replacingOccurrences(of: "&amp;", with: "&"), relativeTo: base)?.absoluteURL {
                    candidates.append(u.absoluteString)
                }
            }
        }
        // pages that list their feeds ("rss.html") link them with ordinary <a href> links
        if let (data, _) = try? await URLSession.shared.data(for: req) {
            let html = String(decoding: data.prefix(400_000), as: UTF8.self)
            let anchors = try! NSRegularExpression(pattern: "<a\\b[^>]*href=[\"']([^\"']+)[\"']", options: .caseInsensitive)
            var seen = Set(candidates)
            for m in anchors.matches(in: html, range: NSRange(html.startIndex..., in: html)) {
                guard let r = Range(m.range(at: 1), in: html) else { continue }
                let h = String(html[r]).replacingOccurrences(of: "&amp;", with: "&")
                let low = h.lowercased()
                guard low.contains("rss") || low.contains("feed") || low.hasSuffix(".xml"), !low.hasSuffix(".html"), !low.hasSuffix(".htm"),
                      let u = URL(string: h, relativeTo: base)?.absoluteURL.absoluteString, seen.insert(u).inserted else { continue }
                candidates.append(u)
                if candidates.count >= 12 { break }
            }
        }
        if let root = URL(string: "/", relativeTo: base)?.absoluteURL.absoluteString.dropLast() {
            candidates += ["/feed", "/rss", "/rss.xml", "/feed.xml", "/rss/portada.xml", "/index.xml"].map { String(root) + $0 }
        }
        for c in candidates { if let ok = await attempt(c) { return ok } }
        return nil
    }
}

extension JSONDecoder { static var news: JSONDecoder { let d = JSONDecoder(); d.dateDecodingStrategy = .iso8601; return d } }
extension JSONEncoder { static var news: JSONEncoder { let e = JSONEncoder(); e.dateEncodingStrategy = .iso8601; return e } }

/// Minimal RSS and Atom reader: title, link, date and a picture if the feed has one.
final class FeedParser: NSObject, XMLParserDelegate {
    private let source: String
    private var items: [NewsItem] = []
    private var inItem = false
    private var element = "", text = ""
    private var title = "", link = "", dateText = "", image: String?

    init(source: String) { self.source = source }

    func parse(_ data: Data) -> [NewsItem] {
        let p = XMLParser(data: data); p.delegate = self; p.parse()
        return items
    }

    func parser(_ parser: XMLParser, didStartElement name: String, namespaceURI: String?, qualifiedName: String?, attributes a: [String: String]) {
        if name == "item" || name == "entry" { inItem = true; title = ""; link = ""; dateText = ""; image = nil }
        guard inItem else { return }
        element = name; text = ""
        if name == "link", let href = a["href"], link.isEmpty { link = href }                       // Atom
        if name == "enclosure", let u = a["url"], (a["type"] ?? "").hasPrefix("image") { image = image ?? u }
        if name == "media:content" || name == "media:thumbnail", let u = a["url"], (a["type"] ?? "image").hasPrefix("image") { image = image ?? u }
    }
    func parser(_ parser: XMLParser, foundCharacters s: String) { if inItem { text += s } }
    func parser(_ parser: XMLParser, foundCDATA b: Data) { if inItem, let s = String(data: b, encoding: .utf8) { text += s } }

    func parser(_ parser: XMLParser, didEndElement name: String, namespaceURI: String?, qualifiedName: String?) {
        guard inItem else { return }
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        switch name {
        case "title": if title.isEmpty { title = t }
        case "link": if link.isEmpty { link = t }
        case "pubDate", "published", "updated", "dc:date": if dateText.isEmpty { dateText = t }
        case "description", "summary", "content:encoded": if image == nil { image = Self.firstImage(in: t) }
        case "item", "entry":
            inItem = false
            if !title.isEmpty, URL(string: link)?.scheme?.hasPrefix("http") == true {
                items.append(NewsItem(title: Self.clean(title), link: link, source: source, date: Self.date(dateText) ?? Date(), image: image))
            }
        default: break
        }
        text = ""
    }

    private static func clean(_ s: String) -> String {
        s.replacingOccurrences(of: "&nbsp;", with: " ").replacingOccurrences(of: "&amp;", with: "&")
         .replacingOccurrences(of: "&quot;", with: "\"").replacingOccurrences(of: "&#8217;", with: "’")
    }
    private static let imgRegex = try! NSRegularExpression(pattern: "<img[^>]+src=[\"']([^\"']+)[\"']", options: .caseInsensitive)
    private static func firstImage(in html: String) -> String? {
        for m in imgRegex.matches(in: html, range: NSRange(html.startIndex..., in: html)) {
            if let r = Range(m.range(at: 1), in: html) {
                let u = String(html[r])
                if u.hasPrefix("http"), !u.contains("imrworldwide"), !u.contains("pixel") { return u.replacingOccurrences(of: "http://", with: "https://") }
            }
        }
        return nil
    }
    private static let formats: [(String, String?)] = [
        ("EEE, dd MMM yyyy HH:mm:ss Z", nil), ("EEE, dd MMM yyyy HH:mm:ss zzz", nil), ("yyyy-MM-dd HH:mm:ss", "UTC"), ("yyyy-MM-dd'T'HH:mm:ssZ", nil),
    ]
    private static func date(_ s: String) -> Date? {
        if let d = ISO8601DateFormatter().date(from: s) { return d }
        for (fmt, tz) in formats {
            let f = DateFormatter(); f.locale = Locale(identifier: "en_US_POSIX"); f.dateFormat = fmt
            if let tz { f.timeZone = TimeZone(identifier: tz) }
            if let d = f.date(from: s) { return d }
        }
        return nil
    }
}

// MARK: view

struct NewsTab: View {
    @ObservedObject var news: NewsStore
    @ObservedObject var settings = Settings.shared
    @AppStorage("newsSection") private var sectionRaw = "tech"

    /// Tabs to show: a filtered view disappears if the category it filters is gone.
    private var cats: [NewsCategory] {
        settings.categories.filter { c in c.parent == nil || settings.categories.contains { $0.id == c.parent } }
    }
    private var section: NewsCategory? { cats.first { $0.id == sectionRaw } ?? cats.first }

    var body: some View {
        VStack(spacing: Tok.Space.xs) {
            HStack(spacing: Tok.Space.xs) {
                ScrollView(.horizontal) {
                    HStack(spacing: Tok.Space.xs) {
                        ForEach(cats) { c in
                            Button { sectionRaw = c.id } label: {
                                Text(c.name).font(Tok.Font.xs)
                                    .foregroundStyle(section?.id == c.id ? Tok.Color.onAccent : Tok.Color.textMuted)
                                    .padding(.horizontal, 9).frame(height: 20)
                                    .background(section?.id == c.id ? Tok.Color.accent : Tok.Color.surface, in: Capsule())
                            }.buttonStyle(.plain)
                        }
                    }
                }.scrollIndicators(.hidden)
                if news.loading { ProgressView().controlSize(.mini) }
                else {
                    if let u = news.updated { Text(u.formatted(.relative(presentation: .numeric, unitsStyle: .abbreviated)))
                        .font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted) }
                    Button { news.refresh() } label: { Image(systemName: "arrow.clockwise").font(.system(size: 11)).foregroundStyle(Tok.Color.textMuted) }
                        .buttonStyle(.plain).accessibilityLabel("Refresh news")
                }
                Button { NewsManagerWindow.show(select: section?.id) } label: { Image(systemName: "gearshape").font(.system(size: 11)).foregroundStyle(Tok.Color.textMuted) }
                    .buttonStyle(.plain).accessibilityLabel("Manage categories and sources").help("Categorías y fuentes")
            }
            list
        }
        .onAppear { news.refreshIfStale() }
    }

    @ViewBuilder private var list: some View {
        let rows = section.flatMap { news.items[$0.id] } ?? []
        let hasSources = section.map { c in c.parent != nil || settings.feeds.contains { $0.category == c.id && $0.enabled } } ?? false
        if rows.isEmpty {
            VStack(spacing: Tok.Space.xs) {
                Text(!hasSources ? "Esta categoría no tiene fuentes todavía."
                     : news.loading ? "Cargando noticias…"
                     : news.updated == nil ? "Sin conexión. No se han podido cargar las noticias."
                     : section?.parent != nil ? "Ahora mismo no hay noticias de este tema." : "Sin noticias.")
                    .font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted)
                if !hasSources {
                    Button("Añadir fuentes") { NewsManagerWindow.show(select: section?.id) }.controlSize(.small)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Tok.Color.surface, in: RoundedRectangle(cornerRadius: Tok.Radius.lg))
        } else {
            ScrollView {
                LazyVStack(spacing: Tok.Space.xxs) { ForEach(rows) { NewsRow(item: $0) } }
            }
            .scrollIndicators(.hidden)
            .background(Tok.Color.surface, in: RoundedRectangle(cornerRadius: Tok.Radius.lg))
        }
    }
}

struct NewsRow: View {
    let item: NewsItem
    @State private var hover = false

    var body: some View {
        Button {
            if let u = URL(string: item.link) { NSWorkspace.shared.open(u) }
        } label: {
            HStack(spacing: Tok.Space.sm) {
                Group {
                    if let s = item.image, let u = URL(string: s) {
                        AsyncImage(url: u) { $0.resizable().scaledToFill() } placeholder: { Tok.Color.surfaceHover }
                    } else {
                        ZStack { Tok.Color.surfaceHover; Image(systemName: "newspaper").foregroundStyle(Tok.Color.textMuted) }
                    }
                }
                .frame(width: 44, height: 44).clipShape(RoundedRectangle(cornerRadius: Tok.Radius.sm))
                VStack(alignment: .leading, spacing: 2) {
                    Text(item.title).font(Tok.Font.sm).foregroundStyle(Tok.Color.text).lineLimit(2).multilineTextAlignment(.leading)
                    Text("\(item.source) · \(item.date.formatted(.relative(presentation: .named, unitsStyle: .abbreviated)))")
                        .font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted).lineLimit(1)
                }
                Spacer(minLength: 0)
            }
            .padding(Tok.Space.xs)
            .background(hover ? Tok.Color.surfaceHover : .clear, in: RoundedRectangle(cornerRadius: Tok.Radius.md))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hover = $0 }
        .accessibilityLabel("\(item.title), \(item.source)")
    }
}
