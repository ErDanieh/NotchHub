import SwiftUI
import QuickLookThumbnailing

/// Real preview of a file (photo, PDF page, video frame), with the Finder icon until it is ready or if none exists.
struct ThumbnailView: View {
    let url: URL?
    let size: CGSize
    var corner: CGFloat = Tok.Radius.sm
    @State private var image: NSImage?

    private static let cache = NSCache<NSString, NSImage>()

    var body: some View {
        Group {
            if let image {
                Image(nsImage: image).resizable().scaledToFit()
            } else if let url {
                Image(nsImage: NSWorkspace.shared.icon(forFile: url.path)).resizable().scaledToFit()
            } else {
                Image(systemName: "doc").resizable().scaledToFit().foregroundStyle(Tok.Color.textMuted)
            }
        }
        .frame(width: size.width, height: size.height)
        .clipShape(RoundedRectangle(cornerRadius: corner))
        .task(id: url) { await load() }
        .accessibilityHidden(true)
    }

    private func load() async {
        guard let url else { image = nil; return }
        let key = "\(url.path)|\(Int(size.width))x\(Int(size.height))" as NSString
        if let hit = Self.cache.object(forKey: key) { image = hit; return }
        let scale = NSScreen.main?.backingScaleFactor ?? 2
        let req = QLThumbnailGenerator.Request(fileAt: url, size: size, scale: scale, representationTypes: .thumbnail)
        if let rep = try? await QLThumbnailGenerator.shared.generateBestRepresentation(for: req) {
            Self.cache.setObject(rep.nsImage, forKey: key)
            image = rep.nsImage
        }
    }
}
