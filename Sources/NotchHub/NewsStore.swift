import SwiftUI

// MARK: model

struct NewsItem: Identifiable, Codable, Hashable {
    var id: String { link }
    var title: String
    var link: String
    var source: String
    var date: Date
    var image: String?
}

enum NewsSection: String, CaseIterable, Identifiable {
    case tech, ai, market, aiMarket
    var id: String { rawValue }
    var label: String {
        switch self { case .tech: "Tecnología"; case .ai: "IA"; case .market: "Bolsa"; case .aiMarket: "Bolsa e IA" }
    }
}

/// Public RSS feeds in Spanish. Add or remove lines here to change the sources.
struct Feed { enum Group { case tech, finance }; let name: String; let url: String; let group: Group; var onlyLinksContaining: String? = nil }
let newsFeeds: [Feed] = [
    Feed(name: "Xataka", url: "https://www.xataka.com/feedburner.xml", group: .tech),
    Feed(name: "Genbeta", url: "https://www.genbeta.com/feedburner.xml", group: .tech),
    Feed(name: "Hipertextual", url: "https://hipertextual.com/feed", group: .tech),
    Feed(name: "El País Tecnología", url: "https://feeds.elpais.com/mrss-s/pages/ep/site/elpais.com/section/tecnologia/portada", group: .tech),
    Feed(name: "Expansión", url: "https://e00-expansion.uecdn.es/rss/mercados.xml", group: .finance),
    Feed(name: "Cinco Días", url: "https://feeds.elpais.com/mrss-s/pages/ep/site/cincodias.elpais.com/portada", group: .finance, onlyLinksContaining: "/mercados-financieros/"),
    Feed(name: "Investing.com", url: "https://es.investing.com/rss/news_25.rss", group: .finance),
]

/// Keyword filters for the "IA" and "Bolsa e IA" sections.
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
    @Published private(set) var items: [NewsSection: [NewsItem]] = [:]
    @Published private(set) var loading = false
    @Published private(set) var updated: Date?
    @Published private(set) var failedSources: [String] = []

    private var tech: [NewsItem] = [], finance: [NewsItem] = []
    private var timer: Timer?
    private let cacheURL: URL

    init() {
        let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("NotchHub", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        cacheURL = dir.appendingPathComponent("news.json")
        if let data = try? Data(contentsOf: cacheURL),
           let c = try? JSONDecoder.news.decode(Cache.self, from: data) {
            tech = c.tech; finance = c.finance; updated = c.updated; rebuild()
        }
    }

    func start() {
        timer = Timer.scheduledTimer(withTimeInterval: 20 * 60, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.refresh() }
        }
    }

    /// Called when the News tab opens: refresh only if the data is older than 10 minutes.
    func refreshIfStale() { if updated == nil || Date().timeIntervalSince(updated!) > 600 { refresh() } }

    func refresh() {
        guard !loading else { return }
        loading = true
        Task {
            var t: [NewsItem] = [], f: [NewsItem] = [], failed: [String] = []
            await withTaskGroup(of: (Feed, [NewsItem]?).self) { group in
                for feed in newsFeeds { group.addTask { (feed, await Self.fetch(feed)) } }
                for await (feed, result) in group {
                    guard let result else { failed.append(feed.name); continue }
                    if feed.group == .tech { t += result } else { f += result }
                }
            }
            // keep the old items of a source that failed this time
            if !failed.isEmpty {
                let keep = Set(failed)
                t += tech.filter { keep.contains($0.source) }; f += finance.filter { keep.contains($0.source) }
            }
            if !(t.isEmpty && f.isEmpty) { tech = t; finance = f; updated = Date(); save() }
            failedSources = failed
            rebuild()
            loading = false
        }
    }

    private func rebuild() {
        func newest(_ a: [NewsItem]) -> [NewsItem] {
            var seen = Set<String>()
            return a.sorted { $0.date > $1.date }.filter { seen.insert($0.link).inserted }.prefix(40).map { $0 }
        }
        items = [
            .tech: newest(tech),
            .ai: newest(tech.filter { matches(aiRegex, $0.title) }),
            .market: newest(finance),
            .aiMarket: newest(finance.filter { matches(aiMarketRegex, $0.title) }),
        ]
    }

    private struct Cache: Codable { var tech: [NewsItem]; var finance: [NewsItem]; var updated: Date }
    private func save() {
        guard let updated, let data = try? JSONEncoder.news.encode(Cache(tech: tech, finance: finance, updated: updated)) else { return }
        try? data.write(to: cacheURL, options: .atomic)
    }

    // MARK: network + parsing (off the main actor)
    nonisolated private static func fetch(_ feed: Feed) async -> [NewsItem]? {
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
    @AppStorage("newsSection") private var sectionRaw = NewsSection.tech.rawValue
    private var section: NewsSection { NewsSection(rawValue: sectionRaw) ?? .tech }

    var body: some View {
        VStack(spacing: Tok.Space.xs) {
            HStack(spacing: Tok.Space.xs) {
                ForEach(NewsSection.allCases) { s in
                    Button { sectionRaw = s.rawValue } label: {
                        Text(s.label).font(Tok.Font.xs)
                            .foregroundStyle(section == s ? Tok.Color.onAccent : Tok.Color.textMuted)
                            .padding(.horizontal, 9).frame(height: 20)
                            .background(section == s ? Tok.Color.accent : Tok.Color.surface, in: Capsule())
                    }.buttonStyle(.plain)
                }
                Spacer()
                if news.loading { ProgressView().controlSize(.mini) }
                else {
                    if let u = news.updated { Text(u.formatted(.relative(presentation: .numeric, unitsStyle: .abbreviated)))
                        .font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted) }
                    Button { news.refresh() } label: { Image(systemName: "arrow.clockwise").font(.system(size: 11)).foregroundStyle(Tok.Color.textMuted) }
                        .buttonStyle(.plain).accessibilityLabel("Refresh news")
                }
            }
            list
        }
        .onAppear { news.refreshIfStale() }
    }

    @ViewBuilder private var list: some View {
        let rows = news.items[section] ?? []
        if rows.isEmpty {
            Text(news.loading ? "Cargando noticias…"
                 : news.updated == nil ? "Sin conexión. No se han podido cargar las noticias."
                 : section == .ai || section == .aiMarket ? "Ahora mismo no hay noticias de este tema." : "Sin noticias.")
                .font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted).frame(maxWidth: .infinity, maxHeight: .infinity)
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
