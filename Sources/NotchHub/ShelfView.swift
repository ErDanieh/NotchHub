import SwiftUI
import AppKit
import UniformTypeIdentifiers

struct ShelfTab: View {
    @ObservedObject var state: PanelState
    @ObservedObject var store: ShelfStore

    var body: some View {
        HStack(spacing: Tok.Space.md) {
            // Shelf drop zone
            ZStack {
                RoundedRectangle(cornerRadius: Tok.Radius.lg)
                    .strokeBorder(state.dragHover ? Tok.Color.accent : Tok.Color.border,
                                  style: StrokeStyle(lineWidth: 1.5, dash: store.items.isEmpty ? [5] : []))
                    .background(RoundedRectangle(cornerRadius: Tok.Radius.lg)
                        .fill(state.dragHover ? Tok.Color.surfaceHover : Tok.Color.surface))
                if store.items.isEmpty {
                    Text("Nothing here yet. Drag a file onto the notch.")
                        .font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted)
                        .multilineTextAlignment(.center).padding(Tok.Space.md)
                } else {
                    ScrollView(.horizontal) {
                        HStack(spacing: Tok.Space.sm) {
                            ForEach(store.items) { item in FileTile(item: item) { store.remove(item.id) } }
                        }.padding(Tok.Space.sm)
                    }.scrollIndicators(.hidden)
                }
            }
            .onDrop(of: [.fileURL], isTargeted: $state.dragHover) { providers in
                loadURLs(providers) { store.add($0) }
                return true
            }
            .accessibilityLabel("Shelf. Drop files here.")

            AirDropTarget()
        }
    }
}

struct FileTile: View {
    let item: ShelfItem
    let onRemove: () -> Void
    @State private var hover = false

    var body: some View {
        let url = item.url
        VStack(spacing: Tok.Space.xs) {
            ZStack(alignment: .topTrailing) {
                ThumbnailView(url: url, size: CGSize(width: Tok.Layout.tile - 8, height: Tok.Layout.tile - 8))
                    .opacity(url == nil ? 0.35 : 1)
                if url == nil {
                    Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(Tok.Color.warning)
                }
                if hover {
                    Button(action: onRemove) {
                        Image(systemName: "xmark.circle.fill").foregroundStyle(Tok.Color.textMuted, Tok.Color.surface)
                    }.buttonStyle(.plain).accessibilityLabel("Remove \(item.name)")
                }
            }
            Text(item.name).font(Tok.Font.xs).foregroundStyle(Tok.Color.text)
                .lineLimit(2).truncationMode(.middle).frame(width: Tok.Layout.tile)
        }
        .padding(Tok.Space.xs)
        .background(hover ? Tok.Color.surfaceHover : .clear, in: RoundedRectangle(cornerRadius: Tok.Radius.md))
        .onHover { hover = $0 }
        .onDrag { url.map { NSItemProvider(object: $0 as NSURL) } ?? NSItemProvider() }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(url == nil ? "\(item.name), file missing" : item.name)
    }

    private func icon(_ url: URL?) -> NSImage {
        if let url { return NSWorkspace.shared.icon(forFile: url.path) }
        return NSImage(systemSymbolName: "doc", accessibilityDescription: nil) ?? NSImage()
    }
}

struct AirDropTarget: View {
    @State private var targeted = false
    var body: some View {
        VStack(spacing: Tok.Space.xs) {
            Image(systemName: "airplayaudio").font(.system(size: 22))   // placeholder SF Symbol
            Text("AirDrop").font(Tok.Font.xs)
        }
        .foregroundStyle(targeted ? Tok.Color.text : Tok.Color.textMuted)
        .frame(width: 84).frame(maxHeight: .infinity)
        .background(RoundedRectangle(cornerRadius: Tok.Radius.lg).fill(targeted ? Tok.Color.surfaceHover : Tok.Color.surface))
        .overlay(RoundedRectangle(cornerRadius: Tok.Radius.lg)
            .strokeBorder(targeted ? Tok.Color.accent : Tok.Color.border, lineWidth: 1.5))
        .onDrop(of: [.fileURL], isTargeted: $targeted) { providers in
            loadURLs(providers) { urls in
                NSSharingService(named: .sendViaAirDrop)?.perform(withItems: urls)
            }
            return true
        }
        .accessibilityLabel("AirDrop. Drop files here to send.")
    }
}

/// Reads file URLs out of drop providers, then calls back on the main actor.
func loadURLs(_ providers: [NSItemProvider], done: @escaping @MainActor ([URL]) -> Void) {
    let group = DispatchGroup()
    var urls: [URL] = []
    let lock = NSLock()
    for p in providers where p.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
        group.enter()
        p.loadItem(forTypeIdentifier: UTType.fileURL.identifier, options: nil) { item, _ in
            if let data = item as? Data, let url = URL(dataRepresentation: data, relativeTo: nil) {
                lock.lock(); urls.append(url); lock.unlock()
            } else if let url = item as? URL {
                lock.lock(); urls.append(url); lock.unlock()
            }
            group.leave()
        }
    }
    group.notify(queue: .main) { MainActor.assumeIsolated { if !urls.isEmpty { done(urls) } } }
}
