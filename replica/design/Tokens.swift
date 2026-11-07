import SwiftUI

// Generated from tokens.json. Change values there, then mirror them here.
enum Tok {
    enum Color {
        static let bg = SwiftUI.Color(hex: 0x000000)
        static let surface = SwiftUI.Color(hex: 0x1C1C1E)
        static let surfaceHover = SwiftUI.Color(hex: 0x2C2C2E)
        static let border = SwiftUI.Color(hex: 0x3A3A3C)
        static let borderInput = SwiftUI.Color(hex: 0x8E8E93)
        static let text = SwiftUI.Color.white
        static let textMuted = SwiftUI.Color(hex: 0xAEAEB2)
        static let accent = SwiftUI.Color(hex: 0x1473E6)
        static let onAccent = SwiftUI.Color.white
        static let danger = SwiftUI.Color(hex: 0xFF6961)
        static let success = SwiftUI.Color(hex: 0x32D74B)
        static let warning = SwiftUI.Color(hex: 0xFFD60A)
        static let neon = SwiftUI.Color(hex: 0x3DA9FF)   // halo while music plays
    }
    enum Space { static let xxs: CGFloat = 2, xs: CGFloat = 4, sm: CGFloat = 8, md: CGFloat = 12, lg: CGFloat = 16, xl: CGFloat = 24 }
    enum Radius { static let sm: CGFloat = 6, md: CGFloat = 10, lg: CGFloat = 16, notchBottom: CGFloat = 24, notchCollapsed: CGFloat = 9 }
    enum Font {
        static let xs = SwiftUI.Font.system(size: 11, weight: .medium)
        static let sm = SwiftUI.Font.system(size: 13)
        static let base = SwiftUI.Font.system(size: 15)
        static let lg = SwiftUI.Font.system(size: 18, weight: .semibold)
        static let timer = SwiftUI.Font.system(size: 34, weight: .light, design: .rounded).monospacedDigit()
    }
    enum Motion {
        static let fast = Animation.easeOut(duration: 0.12)
        static let panel = Animation.spring(response: 0.35, dampingFraction: 0.8)
    }
    enum Layout { static let expandedWidth: CGFloat = 520, expandedHeight: CGFloat = 224, tabBarHeight: CGFloat = 28, glowMargin: CGFloat = 36, toastWing: CGFloat = 170, songWing: CGFloat = 260, liveLyricsHeight: CGFloat = 26, tile: CGFloat = 64 }
}

extension Color {
    init(hex: UInt32) {
        self.init(red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255, blue: Double(hex & 0xFF) / 255)
    }
}
