import SwiftUI
import AppKit

/// Window to organise the News tab: categories on the left, the sources of the chosen category on the right.
@MainActor
enum NewsManagerWindow {
    private static var window: NSWindow?
    static let selection = NewsManagerSelection()

    static func show(select id: String? = nil) {
        if let id { selection.id = id }
        if window == nil {
            let w = NSWindow(contentViewController: NSHostingController(rootView: NewsManagerView(selection: selection)))
            w.title = "News categories and sources"
            w.styleMask = [.titled, .closable]
            w.isReleasedWhenClosed = false
            window = w
            w.center()
        }
        NSApp.activate(ignoringOtherApps: true)
        window?.makeKeyAndOrderFront(nil)
    }
}

@MainActor
final class NewsManagerSelection: ObservableObject { @Published var id: String? = "tech" }

struct NewsManagerView: View {
    @ObservedObject var settings = Settings.shared
    @ObservedObject var selection: NewsManagerSelection
    @State private var newCategory = ""

    var body: some View {
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 0) {
                Text("CATEGORIES").font(.caption.weight(.semibold)).foregroundStyle(.secondary).padding(.horizontal, 14).padding(.top, 14).padding(.bottom, 6)
                List(selection: $selection.id) {
                    ForEach(settings.categories) { c in
                        Label(c.name, systemImage: c.parent == nil ? "folder" : "line.3.horizontal.decrease.circle").tag(c.id as String?)
                    }
                }
                .listStyle(.sidebar)
                Divider()
                HStack {
                    TextField("New category", text: $newCategory).textFieldStyle(.roundedBorder).onSubmit(addCategory)
                    Button { addCategory() } label: { Image(systemName: "plus") }
                        .disabled(newCategory.trimmingCharacters(in: .whitespaces).isEmpty).help("Create the category")
                }.padding(10)
                Button("Restore default sources") { settings.restoreNewsDefaults() }
                    .buttonStyle(.link).font(.caption).padding(.horizontal, 12).padding(.bottom, 10)
                    .help("Brings back the categories and sources that came with NotchHub, without touching yours")
            }
            .frame(width: 210)
            Divider()
            Group {
                if let c = settings.categories.first(where: { $0.id == selection.id }) {
                    CategoryDetail(category: c, onDeleted: { selection.id = settings.categories.first?.id })
                        .id(c.id)
                } else {
                    Text("Pick a category on the left, or create a new one.").foregroundStyle(.secondary).frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
        }
        .frame(width: 720, height: 480)
    }

    private func addCategory() {
        let name = newCategory.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty else { return }
        let c = NewsCategory(id: UUID().uuidString, name: name)
        settings.categories.append(c)
        selection.id = c.id
        newCategory = ""
    }
}

private struct CategoryDetail: View {
    let category: NewsCategory
    var onDeleted: () -> Void
    @ObservedObject var settings = Settings.shared
    @State private var url = ""
    @State private var name = ""
    @State private var checking = false
    @State private var error: String?
    @State private var confirmDelete = false

    private var nameBinding: Binding<String> {
        Binding(get: { settings.categories.first { $0.id == category.id }?.name ?? "" },
                set: { v in if let i = settings.categories.firstIndex(where: { $0.id == category.id }) { settings.categories[i].name = v } })
    }
    private var feedIndexes: [Int] { settings.feeds.indices.filter { settings.feeds[$0].category == category.id } }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                TextField("Category name", text: nameBinding).textFieldStyle(.plain).font(.title2.weight(.semibold))
                Spacer()
                Button("Delete category", role: .destructive) { confirmDelete = true }
                    .confirmationDialog("Delete \"\(category.name)\" and its sources?", isPresented: $confirmDelete) {
                        Button("Delete", role: .destructive) { settings.deleteCategory(category.id); onDeleted() }
                    }
            }
            if let parent = category.parent {
                Label("This tab has no sources of its own. It shows only the stories about artificial intelligence from \"\(settings.categories.first { $0.id == parent }?.name ?? "")\".",
                      systemImage: "info.circle").foregroundStyle(.secondary)
                Spacer()
            } else {
                Text("SOURCES IN THIS CATEGORY").font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                if feedIndexes.isEmpty {
                    Text("Nothing here yet. Add the first source below.").foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, minHeight: 60)
                } else {
                    List {
                        ForEach(feedIndexes, id: \.self) { i in
                            HStack {
                                Toggle("", isOn: $settings.feeds[i].enabled).labelsHidden().toggleStyle(.switch).controlSize(.small)
                                VStack(alignment: .leading, spacing: 1) {
                                    Text(settings.feeds[i].name)
                                    Text(settings.feeds[i].url).font(.caption).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                                }
                                Spacer()
                                Button { settings.feeds.remove(at: i) } label: { Image(systemName: "trash") }
                                    .buttonStyle(.borderless).help("Remove this source")
                            }
                        }
                    }
                    .frame(minHeight: 120)
                }
                GroupBox {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Add a source").font(.headline)
                        TextField("Website or RSS address, e.g. https://www.informacion.es", text: $url).textFieldStyle(.roundedBorder).onSubmit(add)
                        HStack {
                            TextField("Name (optional)", text: $name).textFieldStyle(.roundedBorder)
                            Button("Add") { add() }.keyboardShortcut(.defaultAction)
                                .disabled(url.trimmingCharacters(in: .whitespaces).isEmpty || checking)
                            if checking { ProgressView().controlSize(.small) }
                        }
                        if let e = error { Text(e).font(.caption).foregroundStyle(.red) }
                        else { Text("Just paste the web address. NotchHub finds the news feed by itself.").font(.caption).foregroundStyle(.secondary) }
                    }.padding(6)
                }
            }
        }
        .padding(18)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private func add() {
        var u = url.trimmingCharacters(in: .whitespacesAndNewlines)
        if !u.contains("://") { u = "https://" + u }
        guard let parsed = URL(string: u), let host = parsed.host, parsed.scheme?.hasPrefix("http") == true else { error = "That is not a valid address."; return }
        let n = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let label = n.isEmpty ? host.replacingOccurrences(of: "www.", with: "") : n
        checking = true; error = nil
        let cat = category.id
        Task {
            let found = await NewsStore.discover(u, category: cat, name: label)
            checking = false
            guard let (feed, _) = found else {
                error = "No news feed found at that address. Look for an RSS link on the site and paste that instead."
                return
            }
            if let other = settings.feeds.first(where: { $0.url == feed.url }) {
                let where_ = settings.categories.first { $0.id == other.category }?.name ?? "another category"
                error = "You already have that source in \"\(where_)\"."; return
            }
            settings.feeds.append(feed)
            url = ""; name = ""
        }
    }
}
