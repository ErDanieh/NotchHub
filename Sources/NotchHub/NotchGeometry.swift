import AppKit

/// Where the notch is and how big the panels should be.
struct NotchGeometry {
    let screen: NSScreen
    let notchSize: CGSize
    let hasNotch: Bool

    init(screen: NSScreen) {
        self.screen = screen
        let top = screen.safeAreaInsets.top
        if top > 0, let left = screen.auxiliaryTopLeftArea, let right = screen.auxiliaryTopRightArea {
            hasNotch = true
            notchSize = CGSize(width: screen.frame.width - left.width - right.width, height: top)
        } else {
            hasNotch = false
            notchSize = CGSize(width: 180, height: 24)   // fallback pill for Macs without a notch
        }
    }

    /// The notch screen if there is one, otherwise the main screen.
    static func current() -> NotchGeometry {
        let screen = NSScreen.screens.first { $0.safeAreaInsets.top > 0 } ?? NSScreen.main ?? NSScreen.screens[0]
        return NotchGeometry(screen: screen)
    }

    var notchRect: NSRect {
        NSRect(x: screen.frame.midX - notchSize.width / 2, y: screen.frame.maxY - notchSize.height,
               width: notchSize.width, height: notchSize.height)
    }
    /// Slightly larger zone that opens the shelf when a drag comes near.
    var hotRect: NSRect { notchRect.insetBy(dx: -50, dy: -26).intersection(screen.frame) }

    var expandedSize: CGSize {
        CGSize(width: max(Tok.Layout.expandedWidth, notchSize.width + 40),
               height: notchSize.height + Tok.Layout.expandedHeight - 30)
    }
    var windowSize: CGSize {
        CGSize(width: expandedSize.width + Tok.Layout.glowMargin * 2, height: expandedSize.height + Tok.Layout.glowMargin)
    }
    var windowRect: NSRect {
        NSRect(x: screen.frame.midX - windowSize.width / 2, y: screen.frame.maxY - windowSize.height,
               width: windowSize.width, height: windowSize.height)
    }
    /// Panel size for a tab. The mirror can be made bigger by the user (level 0, 1, 2).
    func panelSize(tab: NotchTab, mirrorLevel: Int) -> CGSize {
        if tab == .music, UserDefaults.standard.bool(forKey: "musicShowLyrics") {   // taller while the lyrics are open
            return CGSize(width: expandedSize.width, height: expandedSize.height + 56)
        }
        guard tab == .mirror else { return expandedSize }
        switch mirrorLevel {
        case 1: return CGSize(width: max(expandedSize.width, 560), height: notchSize.height + 300)
        case 2: return CGSize(width: 680, height: notchSize.height + 440)
        default: return expandedSize
        }
    }
    func rect(for size: CGSize) -> NSRect {
        NSRect(x: screen.frame.midX - size.width / 2, y: screen.frame.maxY - size.height, width: size.width, height: size.height)
    }
    /// The window is a bit bigger than the panel so the glow is not clipped.
    /// `collapsed`: the window is wide enough for the song alert, which grows only to the right of the notch.
    /// It is narrower while the panel is open, so it does not cover the menu bar icons next to the notch.
    func windowRect(for size: CGSize, collapsed: Bool = false) -> NSRect {
        var width = size.width + Tok.Layout.glowMargin * 2
        if collapsed { width = max(width, 2 * (notchSize.width / 2 + Tok.Layout.songWing + Tok.Layout.glowMargin)) }
        let w = CGSize(width: width, height: size.height + Tok.Layout.glowMargin)
        return NSRect(x: screen.frame.midX - w.width / 2, y: screen.frame.maxY - w.height, width: w.width, height: w.height)
    }
    var expandedRect: NSRect {
        NSRect(x: screen.frame.midX - expandedSize.width / 2, y: screen.frame.maxY - expandedSize.height,
               width: expandedSize.width, height: expandedSize.height)
    }
}
