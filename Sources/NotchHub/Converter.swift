import SwiftUI
import AppKit
import ImageIO
import PDFKit
import AVFoundation
import UniformTypeIdentifiers

enum Target: String, CaseIterable, Identifiable {
    case jpg, png, heic, tiff, shrink, imagesToPDF, pdfToJPG, pdfToPNG, pdfCompress, m4a, mp4, mp3
    var id: String { rawValue }
    var label: String {
        switch self {
        case .jpg: "JPG"; case .png: "PNG"; case .heic: "HEIC"; case .tiff: "TIFF"; case .shrink: "Reduce size"
        case .imagesToPDF: "PDF"; case .pdfToJPG: "JPG pages"; case .pdfToPNG: "PNG pages"; case .pdfCompress: "Smaller PDF"
        case .m4a: "M4A audio"; case .mp4: "MP4 video"; case .mp3: "MP3"
        }
    }
    var usesQuality: Bool { [.jpg, .heic, .pdfToJPG, .pdfCompress].contains(self) }
}

enum FileKind { case image, pdf, media, other }

struct ConversionResult { var outputs: [URL]; var inputBytes: Int64; var outputBytes: Int64; var failures: [String] }

enum ConvertError: LocalizedError {
    case unsupported(String), failed(String)
    var errorDescription: String? { switch self { case .unsupported(let s), .failed(let s): s } }
}

/// All conversions use Apple frameworks (ImageIO, PDFKit, AVFoundation). MP3 additionally needs ffmpeg.
enum Converter {
    static var ffmpegPath: String? {
        ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"].first { FileManager.default.isExecutableFile(atPath: $0) }
    }

    static func kind(of url: URL) -> FileKind {
        guard let t = UTType(filenameExtension: url.pathExtension.lowercased()) else { return .other }
        if t.conforms(to: .pdf) { return .pdf }
        if t.conforms(to: .image) { return .image }
        if t.conforms(to: .audiovisualContent) { return .media }
        return .other
    }

    /// What can be done with this set of files. Empty if they are mixed or unsupported.
    static func targets(for urls: [URL]) -> [Target] {
        let kinds = Set(urls.map(kind))
        guard kinds.count == 1, let k = kinds.first else { return [] }
        switch k {
        case .image: return [.shrink, .jpg, .png, .heic, .tiff, .imagesToPDF]
        case .pdf: return [.pdfToJPG, .pdfToPNG, .pdfCompress]
        case .media: return [.m4a, .mp4, .mp3]
        case .other: return []
        }
    }

    static func uniqueURL(_ dir: URL, _ base: String, _ ext: String) -> URL {
        var u = dir.appendingPathComponent(base).appendingPathExtension(ext), n = 2
        while FileManager.default.fileExists(atPath: u.path) { u = dir.appendingPathComponent("\(base) \(n)").appendingPathExtension(ext); n += 1 }
        return u
    }
    static func size(_ u: URL) -> Int64 {
        var isDir: ObjCBool = false
        FileManager.default.fileExists(atPath: u.path, isDirectory: &isDir)
        if isDir.boolValue {   // folder of page images: add up the files inside
            let files = (try? FileManager.default.contentsOfDirectory(at: u, includingPropertiesForKeys: nil)) ?? []
            return files.reduce(0) { $0 + size($1) }
        }
        return (try? FileManager.default.attributesOfItem(atPath: u.path)[.size] as? Int64) ?? 0
    }

    // MARK: entry point
    static func run(_ urls: [URL], to target: Target, quality: Double, targetBytes: Int = 0,
                    progress: @escaping @Sendable (Double) -> Void) async -> ConversionResult {
        var res = ConversionResult(outputs: [], inputBytes: 0, outputBytes: 0, failures: [])
        if target == .imagesToPDF {
            do {
                let out = try imagesToPDF(urls)
                res.outputs = [out]
            } catch { res.failures.append(error.localizedDescription) }
            progress(1)
        } else {
            for (i, url) in urls.enumerated() {
                do {
                    let outs = try await convertOne(url, target, quality, targetBytes)
                    res.outputs += outs
                } catch { res.failures.append("\(url.lastPathComponent): \(error.localizedDescription)") }
                progress(Double(i + 1) / Double(urls.count))
            }
        }
        res.inputBytes = urls.reduce(0) { $0 + size($1) }
        res.outputBytes = res.outputs.reduce(0) { $0 + size($1) }
        return res
    }

    private static func convertOne(_ url: URL, _ t: Target, _ q: Double, _ targetBytes: Int) async throws -> [URL] {
        let dir = url.deletingLastPathComponent(), base = url.deletingPathExtension().lastPathComponent
        switch t {
        case .jpg, .png, .heic, .tiff:
            let type: UTType = t == .jpg ? .jpeg : t == .png ? .png : t == .heic ? .heic : .tiff
            let out = uniqueURL(dir, base + " converted", t == .jpg ? "jpg" : t.rawValue)
            try convertImage(url, type: type, quality: q, dst: out)
            return [out]
        case .shrink:
            return [try shrinkImage(url, targetBytes: targetBytes)]
        case .pdfToJPG, .pdfToPNG:
            return try pdfToImages(url, png: t == .pdfToPNG, quality: q)
        case .pdfCompress:
            return [try compressPDF(url, quality: q)]
        case .m4a:
            return [try await exportAV(url, preset: AVAssetExportPresetAppleM4A, type: .m4a, ext: "m4a")]
        case .mp4:
            return [try await exportAV(url, preset: AVAssetExportPresetHighestQuality, type: .mp4, ext: "mp4")]
        case .mp3:
            return [try ffmpegMP3(url)]
        case .imagesToPDF: return []
        }
    }

    // MARK: images
    static func convertImage(_ src: URL, type: UTType, quality: Double, dst: URL) throws {
        guard let source = CGImageSourceCreateWithURL(src as CFURL, nil) else { throw ConvertError.failed("Could not read the image") }
        let supported = (CGImageDestinationCopyTypeIdentifiers() as? [String]) ?? []
        guard supported.contains(type.identifier) else { throw ConvertError.unsupported("\(type.preferredFilenameExtension?.uppercased() ?? "That format") is not supported on this Mac") }
        guard let dest = CGImageDestinationCreateWithURL(dst as CFURL, type.identifier as CFString, 1, nil) else { throw ConvertError.failed("Could not create the file") }
        let opts: [CFString: Any] = [kCGImageDestinationLossyCompressionQuality: quality]
        CGImageDestinationAddImageFromSource(dest, source, 0, opts as CFDictionary)
        guard CGImageDestinationFinalize(dest) else { throw ConvertError.failed("Could not write the image (is the folder writable?)") }
    }

    /// Makes an image weigh at most `targetBytes`. It keeps the full resolution and lowers the JPEG quality as
    /// far as 40%; only if that is not enough does it start reducing the dimensions, a little at a time.
    /// Transparent areas become white (JPEG has no transparency).
    static func shrinkImage(_ src: URL, targetBytes: Int) throws -> URL {
        guard targetBytes > 0 else { throw ConvertError.failed("Choose a target size first") }
        guard let source = CGImageSourceCreateWithURL(src as CFURL, nil),
              let props = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let w = props[kCGImagePropertyPixelWidth] as? Int, let h = props[kCGImagePropertyPixelHeight] as? Int else {
            throw ConvertError.failed("Could not read the image")
        }
        let fmt = ByteCountFormatter()
        let original = Int(size(src))
        if original > 0 && original <= targetBytes {
            throw ConvertError.failed("It already weighs \(fmt.string(fromByteCount: Int64(original))), under your target")
        }

        func flattened(_ maxPixel: Int) -> CGImage? {
            let opts: [CFString: Any] = [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true,
                                         kCGImageSourceThumbnailMaxPixelSize: maxPixel]
            guard let t = CGImageSourceCreateThumbnailAtIndex(source, 0, opts as CFDictionary),
                  let ctx = CGContext(data: nil, width: t.width, height: t.height, bitsPerComponent: 8, bytesPerRow: 0,
                                      space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { return nil }
            ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
            ctx.fill(CGRect(x: 0, y: 0, width: t.width, height: t.height))
            ctx.draw(t, in: CGRect(x: 0, y: 0, width: t.width, height: t.height))
            return ctx.makeImage()
        }
        func jpeg(_ img: CGImage, _ q: Double) -> Data? {
            let data = NSMutableData()
            guard let d = CGImageDestinationCreateWithData(data, UTType.jpeg.identifier as CFString, 1, nil) else { return nil }
            CGImageDestinationAddImage(d, img, [kCGImageDestinationLossyCompressionQuality: q] as CFDictionary)
            return CGImageDestinationFinalize(d) ? data as Data : nil
        }

        let longest = max(w, h)
        var best: Data?
        var last: Data?
        for scale in [1.0, 0.85, 0.7, 0.55, 0.42, 0.32, 0.24, 0.18, 0.13, 0.09, 0.06] {
            guard let img = flattened(max(16, Int(Double(longest) * scale))) else { continue }
            guard let lowest = jpeg(img, 0.4) else { continue }
            last = lowest
            if lowest.count > targetBytes { continue }          // even at 40% it is too big: shrink the dimensions
            // it fits: find the highest quality that still fits
            var lo = 0.4, hi = 0.95, fit = lowest
            for _ in 0..<7 {
                let mid = (lo + hi) / 2
                if let d = jpeg(img, mid), d.count <= targetBytes { fit = d; lo = mid } else { hi = mid }
            }
            best = fit
            break
        }
        if best == nil, let img = flattened(max(16, Int(Double(longest) * 0.06))), let d = jpeg(img, 0.2) { best = d }   // best effort
        guard let data = best ?? last else { throw ConvertError.failed("Could not reduce this image") }
        let out = uniqueURL(src.deletingLastPathComponent(), src.deletingPathExtension().lastPathComponent + " reduced", "jpg")
        try data.write(to: out)
        return out
    }

    static func imagesToPDF(_ urls: [URL]) throws -> URL {
        let doc = PDFDocument()
        for u in urls {
            guard let img = NSImage(contentsOf: u), let page = PDFPage(image: img) else { throw ConvertError.failed("Could not read \(u.lastPathComponent)") }
            doc.insert(page, at: doc.pageCount)
        }
        let first = urls[0]
        let name = urls.count == 1 ? first.deletingPathExtension().lastPathComponent : "Images"
        let out = uniqueURL(first.deletingLastPathComponent(), name, "pdf")
        guard doc.write(to: out) else { throw ConvertError.failed("Could not write the PDF") }
        return out
    }

    // MARK: pdf
    private static func jpegData(_ img: NSImage, png: Bool, quality: Double) -> Data? {
        guard let tiff = img.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff) else { return nil }
        return png ? rep.representation(using: .png, properties: [:]) : rep.representation(using: .jpeg, properties: [.compressionFactor: quality])
    }

    static func pdfToImages(_ url: URL, png: Bool, quality: Double) throws -> [URL] {
        guard let doc = PDFDocument(url: url), doc.pageCount > 0 else { throw ConvertError.failed("Could not open the PDF") }
        let folder = uniqueURL(url.deletingLastPathComponent(), url.deletingPathExtension().lastPathComponent + " pages", "")
            .deletingPathExtension()
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        var outs: [URL] = []
        let scale = 150.0 / 72.0
        for i in 0..<doc.pageCount {
            guard let page = doc.page(at: i) else { continue }
            let b = page.bounds(for: .mediaBox)
            let img = page.thumbnail(of: CGSize(width: b.width * scale, height: b.height * scale), for: .mediaBox)
            guard let data = jpegData(img, png: png, quality: quality) else { continue }
            let out = folder.appendingPathComponent("page-\(i + 1)").appendingPathExtension(png ? "png" : "jpg")
            try data.write(to: out)
            outs.append(out)
        }
        return [folder]
    }

    /// Rebuilds the PDF from JPEG images of each page. Smaller, but the text is no longer selectable.
    static func compressPDF(_ url: URL, quality: Double) throws -> URL {
        guard let doc = PDFDocument(url: url), doc.pageCount > 0 else { throw ConvertError.failed("Could not open the PDF") }
        let out = PDFDocument()
        let scale = 110.0 / 72.0
        for i in 0..<doc.pageCount {
            guard let page = doc.page(at: i) else { continue }
            let b = page.bounds(for: .mediaBox)
            let img = page.thumbnail(of: CGSize(width: b.width * scale, height: b.height * scale), for: .mediaBox)
            guard let data = jpegData(img, png: false, quality: quality), let jpg = NSImage(data: data) else { continue }
            jpg.size = b.size   // page keeps its original size in points
            if let p = PDFPage(image: jpg) { out.insert(p, at: out.pageCount) }
        }
        let dst = uniqueURL(url.deletingLastPathComponent(), url.deletingPathExtension().lastPathComponent + " small", "pdf")
        guard out.write(to: dst) else { throw ConvertError.failed("Could not write the PDF") }
        return dst
    }

    // MARK: audio / video
    private static func exportAV(_ url: URL, preset: String, type: AVFileType, ext: String) async throws -> URL {
        let asset = AVURLAsset(url: url)
        let out = uniqueURL(url.deletingLastPathComponent(), url.deletingPathExtension().lastPathComponent + " converted", ext)
        guard let session = AVAssetExportSession(asset: asset, presetName: preset) else { throw ConvertError.unsupported("This file cannot be converted to \(ext.uppercased())") }
        if #available(macOS 15, *) {
            do { try await session.export(to: out, as: type) } catch { throw ConvertError.failed(error.localizedDescription) }
        } else {
            session.outputURL = out
            session.outputFileType = type
            await withCheckedContinuation { (c: CheckedContinuation<Void, Never>) in session.exportAsynchronously { c.resume() } }
            if session.status != .completed { throw ConvertError.failed(session.error?.localizedDescription ?? "Export failed") }
        }
        return out
    }

    private static func ffmpegMP3(_ url: URL) throws -> URL {
        guard let ff = ffmpegPath else { throw ConvertError.unsupported("MP3 needs ffmpeg. In Terminal: brew install ffmpeg") }
        let out = uniqueURL(url.deletingLastPathComponent(), url.deletingPathExtension().lastPathComponent + " converted", "mp3")
        let p = Process()
        p.executableURL = URL(fileURLWithPath: ff)
        p.arguments = ["-y", "-i", url.path, "-vn", "-codec:a", "libmp3lame", "-q:a", "2", out.path]
        p.standardOutput = FileHandle.nullDevice; p.standardError = FileHandle.nullDevice
        try p.run(); p.waitUntilExit()
        guard p.terminationStatus == 0 else { throw ConvertError.failed("ffmpeg could not convert this file") }
        return out
    }
}

@MainActor
final class ConvertModel: ObservableObject {
    enum Status: Equatable { case idle, working(Double), done(String), failed(String) }
    @Published var files: [URL] = []
    @Published var status: Status = .idle
    @Published var quality = 0.8
    var targetBytes = 0              // for Target.shrink
    @Published private(set) var outputs: [URL] = []

    var targets: [Target] { Converter.targets(for: files) }

    func setFiles(_ urls: [URL]) {
        files = urls; outputs = []; status = .idle
        if !urls.isEmpty && targets.isEmpty { status = .failed("Drop one kind of file at a time: images, PDFs, or audio/video.") }
    }

    func convert(to target: Target) {
        if case .working = status { return }
        do {
            status = .working(0)
            let urls = files, q = quality
            Task {
                let r = await Converter.run(urls, to: target, quality: q, targetBytes: targetBytes) { p in
                    Task { @MainActor in if case .working = self.status { self.status = .working(p) } }
                }
                outputs = r.outputs
                if r.outputs.isEmpty {
                    status = .failed(r.failures.first ?? "Nothing was converted")
                } else {
                    var msg = "\(r.outputs.count == 1 ? "1 file" : "\(r.outputs.count) files") ready"
                    if target == .pdfCompress || target.usesQuality || target == .m4a || target == .shrink {
                        let f = ByteCountFormatter()
                        msg += " · \(f.string(fromByteCount: r.inputBytes)) → \(f.string(fromByteCount: r.outputBytes))"
                    }
                    if !r.failures.isEmpty { msg += " · \(r.failures.count) failed" }
                    status = .done(msg)
                    if Settings.shared.alertsConversion {
                        AlertCenter.shared.post(symbol: "checkmark.circle", title: "Converted", detail: "\(r.outputs.count) file\(r.outputs.count == 1 ? "" : "s") · \(target.label)")
                    }
                }
            }
        }
    }
}

struct ConvertTab: View {
    @ObservedObject var model: ConvertModel
    @ObservedObject var shelf: ShelfStore
    @State private var hover = false
    @State private var showShrink = false
    @AppStorage("shrinkValue") private var shrinkValue = 500.0
    @AppStorage("shrinkUnitMB") private var unitMB = false

    var body: some View {
        HStack(spacing: Tok.Space.md) {
            dropZone
            VStack(alignment: .leading, spacing: Tok.Space.sm) { options }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
    }

    private var dropZone: some View {
        ZStack {
            RoundedRectangle(cornerRadius: Tok.Radius.lg).fill(hover ? Tok.Color.surfaceHover : Tok.Color.surface)
            RoundedRectangle(cornerRadius: Tok.Radius.lg)
                .strokeBorder(hover ? Tok.Color.accent : Tok.Color.border, style: StrokeStyle(lineWidth: 1.5, dash: model.files.isEmpty ? [5] : []))
            VStack(spacing: Tok.Space.xs) {
                if model.files.isEmpty {
                    Image(systemName: "arrow.triangle.2.circlepath").font(.system(size: 22))
                    Text("Drop files to convert").font(Tok.Font.xs).multilineTextAlignment(.center)
                } else {
                    ZStack(alignment: .bottomTrailing) {
                        ThumbnailView(url: model.files[0], size: CGSize(width: 130, height: 74), corner: Tok.Radius.md)
                        if model.files.count > 1 {
                            Text("+\(model.files.count - 1)").font(Tok.Font.xs.weight(.semibold)).foregroundStyle(Tok.Color.onAccent)
                                .padding(.horizontal, 6).padding(.vertical, 2)
                                .background(Tok.Color.accent, in: Capsule()).padding(4)
                        }
                    }
                    Text(model.files.count == 1 ? model.files[0].lastPathComponent : "\(model.files.count) files")
                        .font(Tok.Font.xs).lineLimit(1).truncationMode(.middle)
                }
            }.foregroundStyle(Tok.Color.textMuted).padding(Tok.Space.sm)
        }
        .frame(width: 150).frame(maxHeight: .infinity)
        .onDrop(of: [.fileURL], isTargeted: $hover) { providers in loadURLs(providers) { model.setFiles($0) }; return true }
        .accessibilityLabel("Drop files here to convert")
    }

    @ViewBuilder private var options: some View {
        let targets = model.targets
        if targets.isEmpty {
            if case .failed(let m) = model.status { Text(m).font(Tok.Font.sm).foregroundStyle(Tok.Color.warning) }
            else { Text("Images (JPG, PNG, HEIC, TIFF), PDFs, audio and video. Convert to another format, make a PDF, or shrink a PDF.")
                .font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted) }
        } else {
            Text("Convert to").font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted)
            FlowRow(spacing: Tok.Space.xs) {
                ForEach(targets) { t in
                    let blocked = t == .mp3 && Converter.ffmpegPath == nil
                    Button(t.label) { if t == .shrink { showShrink.toggle() } else { showShrink = false; model.convert(to: t) } }
                        .buttonStyle(.bordered).controlSize(.small).disabled(blocked || isWorking)
                        .tint(t == .shrink && showShrink ? Tok.Color.accent : nil)
                        .help(blocked ? "Needs ffmpeg: brew install ffmpeg" : "")
                }
            }
            if showShrink && targets.contains(.shrink) { shrinkRow }
            if targets.contains(where: \.usesQuality) && !showShrink {
                HStack(spacing: Tok.Space.sm) {
                    Text("Quality").font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted)
                    Slider(value: $model.quality, in: 0.3...1).controlSize(.mini)
                    Text("\(Int(model.quality * 100))%").font(Tok.Font.xs).monospacedDigit().foregroundStyle(Tok.Color.textMuted)
                }
            }
            if targets.contains(.mp3) && Converter.ffmpegPath == nil {
                Text("MP3 needs ffmpeg: brew install ffmpeg").font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted)
            }
            statusRow
        }
    }

    /// "Make it weigh at most ___": the app picks the quality and, if needed, the dimensions.
    private var shrinkRow: some View {
        VStack(alignment: .leading, spacing: Tok.Space.xs) {
            HStack(spacing: Tok.Space.xs) {
                Text("Max size").font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted)
                TextField("500", value: $shrinkValue, format: .number).textFieldStyle(.roundedBorder).frame(width: 64).controlSize(.small)
                Picker("", selection: $unitMB) { Text("KB").tag(false); Text("MB").tag(true) }
                    .pickerStyle(.segmented).labelsHidden().frame(width: 80).controlSize(.small)
                Button("Reduce") {
                    model.targetBytes = Int(max(1, shrinkValue) * (unitMB ? 1_000_000 : 1_000))
                    model.convert(to: .shrink)
                }.buttonStyle(.borderedProminent).tint(Tok.Color.accent).controlSize(.small).disabled(isWorking)
            }
            HStack(spacing: Tok.Space.xs) {
                ForEach([("100 KB", 100.0, false), ("500 KB", 500.0, false), ("1 MB", 1.0, true), ("2 MB", 2.0, true)], id: \.0) { p in
                    Button(p.0) { shrinkValue = p.1; unitMB = p.2 }.buttonStyle(.plain).font(Tok.Font.xs)
                        .foregroundStyle(Tok.Color.textMuted).padding(.horizontal, 6).padding(.vertical, 2)
                        .background(Tok.Color.surfaceHover, in: Capsule())
                }
            }
        }
    }

    private var isWorking: Bool { if case .working = model.status { return true } else { return false } }

    @ViewBuilder private var statusRow: some View {
        switch model.status {
        case .working(let p):
            HStack { ProgressView(value: p).controlSize(.small); Text("Converting…").font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted) }
        case .done(let msg):
            VStack(alignment: .leading, spacing: Tok.Space.xs) {
                Label(msg, systemImage: "checkmark.circle.fill").font(Tok.Font.xs).foregroundStyle(Tok.Color.success)
                HStack {
                    Button("Show in Finder") { NSWorkspace.shared.activateFileViewerSelecting(model.outputs) }
                    Button("Add to shelf") { shelf.add(model.outputs) }
                }.buttonStyle(.bordered).controlSize(.small)
            }
        case .failed(let m): Text(m).font(Tok.Font.xs).foregroundStyle(Tok.Color.warning).lineLimit(3)
        case .idle: EmptyView()
        }
    }
}

/// Simple wrapping row for the format buttons.
struct FlowRow: Layout {
    var spacing: CGFloat = 4
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        layout(proposal.width ?? 300, subviews).size
    }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let r = layout(bounds.width, subviews)
        for (i, f) in r.frames.enumerated() { subviews[i].place(at: CGPoint(x: bounds.minX + f.minX, y: bounds.minY + f.minY), proposal: .unspecified) }
    }
    private func layout(_ width: CGFloat, _ subviews: Subviews) -> (size: CGSize, frames: [CGRect]) {
        var x: CGFloat = 0, y: CGFloat = 0, rowH: CGFloat = 0, frames: [CGRect] = []
        for s in subviews {
            let sz = s.sizeThatFits(.unspecified)
            if x + sz.width > width && x > 0 { x = 0; y += rowH + spacing; rowH = 0 }
            frames.append(CGRect(origin: CGPoint(x: x, y: y), size: sz))
            x += sz.width + spacing; rowH = max(rowH, sz.height)
        }
        return (CGSize(width: width, height: y + rowH), frames)
    }
}
