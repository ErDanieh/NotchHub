import AppKit
import SwiftUI

/// Small floating dot shown at the top centre of the screens that have no notch.
/// Hovering it opens the notch panel on that screen.
struct BallView: View {
    @ObservedObject var nowPlaying: NowPlaying
    @ObservedObject var settings = Settings.shared
    var body: some View {
        let active = settings.haloEnabled && (nowPlaying.isPlaying || settings.haloAlways)
        ZStack {
            Circle().fill(.black).frame(width: 16, height: 16)
            Circle().strokeBorder(active ? settings.haloColor : Color.white.opacity(0.25), lineWidth: 1.5)
                .frame(width: 16, height: 16)
                .shadow(color: active ? settings.haloColor.opacity(0.9 * settings.haloIntensity) : .clear, radius: 5)
            if nowPlaying.isPlaying { Image(systemName: "music.note").font(.system(size: 7, weight: .bold)).foregroundStyle(settings.haloColor) }
        }
        .frame(width: 40, height: 40, alignment: .top)
        .padding(.top, 4)
        .frame(width: 40, height: 40, alignment: .top)
        .animation(.easeOut(duration: 0.25), value: active)
    }
}

enum Ball {
    static let size: CGFloat = 40
    /// Hover zone of the ball on a screen.
    static func hitRect(on screen: NSScreen) -> NSRect {
        NSRect(x: screen.frame.midX - 18, y: screen.frame.maxY - 32, width: 36, height: 32)
    }
    @MainActor static func makePanel(for screen: NSScreen, nowPlaying: NowPlaying) -> NSPanel {
        let p = NotchPanel(contentRect: NSRect(x: screen.frame.midX - size / 2, y: screen.frame.maxY - size, width: size, height: size),
                           styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        p.level = .statusBar
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        p.isOpaque = false; p.backgroundColor = .clear; p.hasShadow = false
        p.ignoresMouseEvents = true
        p.contentView = NSHostingView(rootView: BallView(nowPlaying: nowPlaying))
        p.orderFrontRegardless()
        return p
    }
}
