import AppKit
import SwiftUI

// MARK: sprites

struct PetSprite {
    let frames: [[String]]          // walk frames, same size
    let palette: [Character: Color]
    let body: Character             // what the eyes turn into when blinking
    var cols: Int { frames[0][0].count }
    var rows: Int { frames[0].count }
}

enum PetSprites {
    static let kinds: [(id: String, name: String)] = [("clawd", "Clawd (orange critter)"), ("cat", "Cat"), ("ghost", "Ghost"), ("robot", "Robot"), ("slime", "Slime"), ("frog", "Frog"), ("invader", "Space invader"), ("penguin", "Penguin"), ("bunny", "Bunny")]

    static func sprite(_ id: String) -> PetSprite {
        switch id {
        case "cat": return cat
        case "ghost": return ghost
        case "robot": return robot
        case "slime": return slime
        case "frog": return frog
        case "invader": return invader
        case "penguin": return penguin
        case "bunny": return bunny
        default: return clawd
        }
    }

    private static let clawdBody = [
        "..OOOOOOOOOO..",
        "..OOEOOOOEOO..",
        "OOOOEOOOOEOOOO",
        "OOOOOOOOOOOOOO",
        "..OOOOOOOOOO..",
        "..OOOOOOOOOO..",
    ]
    static let clawd = PetSprite(
        frames: [clawdBody + ["..O.O....O.O..", "..O.O....O.O.."],
                 clawdBody + ["...O.O..O.O...", "...O.O..O.O..."]],
        palette: ["O": Color(hex: 0xD97757), "E": Color(hex: 0x2A1810)], body: "O")

    private static let catBody = [
        ".O........O.",
        ".OO......OO.",
        ".OOOOOOOOOO.",
        ".OEOOOOOOEO.",
        ".OOOOPPOOOO.",
        "..OOOOOOOO.O",
        "..OOOOOOOO.O",
    ]
    static let cat = PetSprite(
        frames: [catBody + ["..OO....OO..", "..OO....OO.."],
                 catBody + ["...OO..OO...", "...OO..OO..."]],
        palette: ["O": Color(hex: 0xE8A25B), "E": Color(hex: 0x24180E), "P": Color(hex: 0xF08FA0)], body: "O")


    private static let robotBody = [
        ".....RR.....", "..GGGGGGGG..", "..GEEGGEEG..", "..GGGGGGGG..",
        ".DGGGGGGGGD.", "DDGGGGGGGGDD", ".DGGGGGGGGD.", "..GGGGGGGG..",
    ]
    static let robot = PetSprite(
        frames: [robotBody + ["..DD....DD..", "..DD....DD.."], robotBody + ["...DD..DD...", "...DD..DD..."]],
        palette: ["G": Color(hex: 0x9AA5B1), "E": Color(hex: 0x5BE3FF), "R": Color(hex: 0xFF5C5C), "D": Color(hex: 0x4A5560)], body: "G")

    static let slime = PetSprite(
        frames: [["....SSSS....", "..SSSSSSSS..", ".SSSSSSSSSS.", ".SSESSSSESS.", ".SSSSSSSSSS.", "SSSSSSSSSSSS", "SSSSSSSSSSSS"],
                 ["............", "....SSSS....", "..SSSSSSSS..", ".SSESSSSESS.", ".SSSSSSSSSS.", "SSSSSSSSSSSS", "SSSSSSSSSSSS"]],
        palette: ["S": Color(hex: 0x5FD068), "E": Color(hex: 0x12301A)], body: "S")

    private static let frogBody = [
        ".GGG....GGG.", ".GEG....GEG.", ".GGGGGGGGGG.", "GGGGGGGGGGGG", "GLLGGGGGGLLG", ".GGGGGGGGGG.",
    ]
    static let frog = PetSprite(
        frames: [frogBody + ["GG........GG", "GG........GG"], frogBody + [".GG......GG.", ".GG......GG."]],
        palette: ["G": Color(hex: 0x6CBF4B), "E": Color(hex: 0x17210F), "L": Color(hex: 0x2E5A22)], body: "G")

    private static let invaderBody = [
        "..I......I..", "...I....I...", "..IIIIIIII..", ".IIEIIIIEII.", "IIIIIIIIIIII", "I.IIIIIIII.I", "I.I......I.I",
    ]
    static let invader = PetSprite(
        frames: [invaderBody + ["...II..II..."], invaderBody + ["..II....II.."]],
        palette: ["I": Color(hex: 0xC77DFF), "E": Color(hex: 0x24103A)], body: "I")

    private static let penguinBody = [
        "...KKKK...", "..KKKKKK..", "..KEKKEK..", "..KKBBKK..", ".KKWWWWKK.", ".KKWWWWKK.", "KKKWWWWKKK", ".KKWWWWKK.", "..KKKKKK..",
    ]
    static let penguin = PetSprite(
        frames: [penguinBody + ["..BB..BB.."], penguinBody + ["...BB.BB.."]],
        palette: ["K": Color(hex: 0x2B3140), "W": Color(hex: 0xF2F4F8), "B": Color(hex: 0xFF9F1C), "E": Color(hex: 0xFFFFFF)], body: "K")

    private static let bunnyBody = [
        ".WW....WW.", ".WP....PW.", ".WP....PW.", ".WWWWWWWW.", ".WEWWWWEW.", ".WWWWNWWW.", "..WWWWWW..", ".WWWWWWWW.", ".WWWWWWWW.",
    ]
    static let bunny = PetSprite(
        frames: [bunnyBody + ["..WW..WW.."], bunnyBody + ["...WW.WW.."]],
        palette: ["W": Color(hex: 0xF4EFEA), "P": Color(hex: 0xF6A9BB), "E": Color(hex: 0x2A1F24), "N": Color(hex: 0xF08FA0)], body: "W")

    private static let ghostBody = [
        "...WWWWWW...",
        "..WWWWWWWW..",
        ".WWWWWWWWWW.",
        ".WWEEWWEEWW.",
        ".WWEEWWEEWW.",
        ".WWWWWWWWWW.",
        ".WWWWWWWWWW.",
        ".WWWWWWWWWW.",
    ]
    static let ghost = PetSprite(
        frames: [ghostBody + [".WW.WWWW.WW."], ghostBody + ["WW.WWWW.WW.W"]],
        palette: ["W": Color(hex: 0xDDE6FF), "E": Color(hex: 0x1C2340)], body: "W")
}

// MARK: behaviour

@MainActor
final class PetModel: ObservableObject {
    enum Mode { case walk, idle, peekIn, peekHold, peekOut, sleep }
    @Published var x: CGFloat = 300
    @Published var facing: CGFloat = 1
    @Published var step = 0
    @Published var peek: CGFloat = 0
    @Published var mode: Mode = .idle
    @Published var blink = false
    @Published var clock = 0.0
    var width: CGFloat = 1400, menu: CGFloat = 32, notchX: CGFloat = 700
    private var target: CGFloat = 0, timer = 1.0, stepT = 0.0, blinkT = 3.0, wantsPeek = false

    func update(dt: Double, scale: CGFloat) {
        clock += dt
        blinkT -= dt
        if blinkT <= 0 { blink.toggle(); blinkT = blink ? 0.14 : Double.random(in: 2...5) }
        let speed = 16 * scale
        switch mode {
        case .walk:
            let d = target - x
            if abs(d) < 2 {
                if wantsPeek { mode = .peekIn; wantsPeek = false } else { pickNext() }
            } else {
                facing = d > 0 ? 1 : -1
                x += facing * min(abs(d), speed * dt)
                stepT += dt
                if stepT > 0.2 { stepT = 0; step = 1 - step }
            }
        case .idle, .sleep:
            timer -= dt
            if timer <= 0 { pickNext() }
        case .peekIn:
            peek = min(1, peek + CGFloat(dt / 0.7))
            if peek >= 1 { mode = .peekHold; timer = Double.random(in: 2...4) }
        case .peekHold:
            timer -= dt
            if timer <= 0 { mode = .peekOut }
        case .peekOut:
            peek = max(0, peek - CGFloat(dt / 0.6))
            if peek <= 0 { mode = .idle; timer = 1.5; step = 0 }
        }
    }

    private func pickNext() {
        step = 0
        let r = Double.random(in: 0..<1)
        if r < 0.5 {
            target = CGFloat.random(in: 40...(max(width - 40, 41)))
            mode = .walk
        } else if r < 0.68 {
            mode = .idle; timer = Double.random(in: 2...5)
        } else if r < 0.9 {
            target = notchX; wantsPeek = true; mode = .walk
        } else {
            mode = .sleep; timer = Double.random(in: 6...10)
        }
    }
}

// MARK: drawing

struct PetView: View {
    @ObservedObject var m: PetModel
    @ObservedObject var nowPlaying: NowPlaying
    @ObservedObject var settings = Settings.shared

    var body: some View {
        Canvas { ctx, size in
            let sp = PetSprites.sprite(settings.petKind)
            let px = CGFloat(settings.petSize)
            let w = CGFloat(sp.cols) * px, h = CGFloat(sp.rows) * px
            let peeking = m.mode == .peekIn || m.mode == .peekHold || m.mode == .peekOut
            let dancing = nowPlaying.isPlaying && (m.mode == .idle || peeking)
            let bounce: CGFloat = dancing ? -round(abs(sin(m.clock * 7)) * px * 1.5) : 0
            let frame = sp.frames[m.mode == .walk ? m.step : (dancing ? Int(m.clock * 4) % 2 : 0)]
            let yTop: CGFloat
            if peeking {
                // hangs upside down from the notch and slides out
                yTop = round(m.menu - h + m.peek * h * 0.9)
                ctx.clip(to: Path(CGRect(x: 0, y: m.menu, width: size.width, height: size.height - m.menu)))
            } else {
                yTop = m.menu - h + bounce
            }
            let wiggle: CGFloat = m.mode == .peekHold ? round(sin(m.clock * 3) * px) : 0
            let x0 = round(m.x - w / 2 + wiggle)
            let eyesClosed = m.blink || m.mode == .sleep
            let flip = peeking ? false : m.facing < 0
            for (r, line) in frame.enumerated() {
                for (c, ch) in line.enumerated() where ch != "." {
                    let key: Character = (ch == "E" && eyesClosed) ? sp.body : ch
                    guard let color = sp.palette[key] else { continue }
                    let cx = flip ? sp.cols - 1 - c : c
                    let ry = peeking ? sp.rows - 1 - r : r
                    ctx.fill(Path(CGRect(x: x0 + CGFloat(cx) * px, y: yTop + CGFloat(ry) * px, width: px, height: px)), with: .color(color))
                }
            }
            if m.mode == .sleep {
                let t = m.clock.truncatingRemainder(dividingBy: 2) / 2
                ctx.draw(Text("z").font(.system(size: 9 + 3 * px / 2, weight: .bold, design: .rounded)).foregroundStyle(Color.white.opacity(1 - t)),
                         at: CGPoint(x: x0 + w + 4 + t * 8, y: yTop - 2 - t * 10))
            }
        }
        .allowsHitTesting(false)
    }
}

// MARK: window

@MainActor
final class PetController {
    private let model = PetModel()
    private var panel: NotchPanel?
    private var timer: Timer?
    private var geo: NotchGeometry?
    var suppressed = false

    func start(geo: NotchGeometry, nowPlaying: NowPlaying) {
        self.geo = geo
        let p = NotchPanel(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        p.level = .statusBar
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        p.isOpaque = false; p.backgroundColor = .clear; p.hasShadow = false
        p.ignoresMouseEvents = true
        p.contentView = NSHostingView(rootView: PetView(m: model, nowPlaying: nowPlaying))
        panel = p
        reconfigure(geo)
        p.orderFrontRegardless()
        timer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
    }

    func reconfigure(_ geo: NotchGeometry) {
        self.geo = geo
        let f = geo.screen.frame
        let menu = geo.hasNotch ? geo.notchSize.height : max(f.maxY - geo.screen.visibleFrame.maxY, 24)
        model.width = f.width; model.menu = menu; model.notchX = f.width / 2
        model.x = min(max(model.x, 40), f.width - 40)
        panel?.setFrame(NSRect(x: f.minX, y: f.maxY - 110, width: f.width, height: 110), display: true)
        panel?.orderFrontRegardless()
    }

    private func tick() {
        guard let panel, let geo else { return }
        let s = Settings.shared
        let fullscreen = geo.screen.visibleFrame.maxY >= geo.screen.frame.maxY - 1
        let show = s.petsEnabled && !suppressed && !(s.hideInFullscreen && fullscreen)
        panel.alphaValue = show ? 1 : 0
        if show { model.update(dt: 0.05, scale: CGFloat(s.petSize) / 2) }
    }
}
