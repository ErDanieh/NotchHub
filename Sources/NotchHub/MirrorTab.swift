import SwiftUI
import AVFoundation

/// Camera preview, mirrored like a real mirror. Runs only while the Mirror tab is visible.
@MainActor
final class CameraModel: ObservableObject {
    enum State { case idle, running, denied, noCamera }
    @Published var state: State = .idle
    let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "notchhub.camera")
    private var configured = false

    func start() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized: run()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { ok in
                DispatchQueue.main.async { MainActor.assumeIsolated { ok ? self.run() : (self.state = .denied) } }
            }
        default: state = .denied
        }
    }

    private func run() {
        guard let device = AVCaptureDevice.default(for: .video) else { state = .noCamera; return }
        state = .running
        let session = self.session, needsSetup = !configured
        configured = true
        queue.async {
            if needsSetup {
                session.beginConfiguration()
                session.sessionPreset = .medium
                if let input = try? AVCaptureDeviceInput(device: device), session.canAddInput(input) { session.addInput(input) }
                session.commitConfiguration()
            }
            if !session.isRunning { session.startRunning() }
        }
    }

    func stop() {
        let session = self.session
        queue.async { if session.isRunning { session.stopRunning() } }
        if state == .running { state = .idle }
    }
}

final class PreviewView: NSView {
    let previewLayer = AVCaptureVideoPreviewLayer()
    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer = previewLayer
        previewLayer.videoGravity = .resizeAspectFill
        previewLayer.cornerRadius = Tok.Radius.md
        previewLayer.masksToBounds = true
    }
    required init?(coder: NSCoder) { fatalError() }
}

struct CameraPreview: NSViewRepresentable {
    let session: AVCaptureSession
    func makeNSView(context: Context) -> PreviewView {
        let v = PreviewView(frame: .zero)
        v.previewLayer.session = session
        if let c = v.previewLayer.connection, c.isVideoMirroringSupported {
            c.automaticallyAdjustsVideoMirroring = false
            c.isVideoMirrored = true
        }
        return v
    }
    func updateNSView(_ v: PreviewView, context: Context) {
        if let c = v.previewLayer.connection, c.isVideoMirroringSupported {
            c.automaticallyAdjustsVideoMirroring = false
            c.isVideoMirrored = true
        }
    }
}

struct MirrorTab: View {
    @StateObject private var camera = CameraModel()
    @ObservedObject var settings = Settings.shared

    var body: some View {
        ZStack {
            Tok.Color.surface
            switch camera.state {
            case .running:
                // 4:3 like a real mirror, centred, instead of stretched across the whole panel
                CameraPreview(session: camera.session)
                    .aspectRatio(4.0 / 3.0, contentMode: .fit)
                    .clipShape(RoundedRectangle(cornerRadius: Tok.Radius.md))
                    .padding(Tok.Space.xs)
            case .denied:
                VStack(spacing: Tok.Space.sm) {
                    Text("Camera access is off. Turn it on in System Settings > Privacy & Security > Camera.")
                        .font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted).multilineTextAlignment(.center)
                    Button("Open System Settings") {
                        NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Camera")!)
                    }.buttonStyle(.borderedProminent).tint(Tok.Color.accent).controlSize(.small)
                }.padding(Tok.Space.md)
            case .noCamera:
                Text("No camera found.").font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted)
            case .idle:
                ProgressView().controlSize(.small)
            }
        }
        .overlay(alignment: .bottomTrailing) { sizePicker.padding(Tok.Space.sm) }
        .clipShape(RoundedRectangle(cornerRadius: Tok.Radius.lg))
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onAppear { camera.start() }
        .onDisappear { camera.stop() }
    }

    /// S / M / L: the panel itself grows with the choice.
    private var sizePicker: some View {
        HStack(spacing: 2) {
            ForEach(Array(["S", "M", "L"].enumerated()), id: \.offset) { i, label in
                Button { withAnimation(Tok.Motion.panel) { settings.mirrorLevel = i } } label: {
                    Text(label).font(.system(size: 10, weight: .semibold))
                        .foregroundStyle(settings.mirrorLevel == i ? Tok.Color.onAccent : Tok.Color.text)
                        .frame(width: 20, height: 18)
                        .background(settings.mirrorLevel == i ? Tok.Color.accent : .clear, in: Capsule())
                }.buttonStyle(.plain).accessibilityLabel("Mirror size \(["small", "medium", "large"][i])")
            }
        }
        .padding(2)
        .background(.black.opacity(0.55), in: Capsule())
    }
}
