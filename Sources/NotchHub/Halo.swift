import SwiftUI
import AppKit

/// The neon glow, drawn with Core Animation layers. Shadows and the pulse run in the render server,
/// so they cost almost no CPU. (Doing the same with a repeating SwiftUI animation re-rendered the blur
/// every frame and used 30% of a core.)
struct HaloLayer: NSViewRepresentable {
    var color: NSColor
    var intensity: Double          // 0.3 ... 1.6
    var pulseSeconds: Double
    var cornerRadius: CGFloat
    var beat: Bool = false           // follow the music instead of a slow pulse

    func makeNSView(context: Context) -> HaloView { HaloView() }
    func updateNSView(_ v: HaloView, context: Context) {
        v.configure(color: color, intensity: intensity, pulseSeconds: pulseSeconds, cornerRadius: cornerRadius, beat: beat)
    }
}

final class HaloView: NSView {
    private let glow = CALayer(), wide = CALayer(), tight = CALayer(), edge = CAShapeLayer(), cutout = CAShapeLayer()
    private var radius: CGFloat = 9
    private var appliedKey = ""

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.masksToBounds = false
        for l in [wide, tight] as [CALayer] {
            l.backgroundColor = NSColor.clear.cgColor
            l.shadowOffset = .zero
            glow.addSublayer(l)
        }
        // the soft glow is cut away inside the notch, whatever the view order, so it only shines around it
        cutout.fillRule = .evenOdd
        cutout.fillColor = NSColor.black.cgColor
        glow.mask = cutout
        layer?.addSublayer(glow)
        edge.fillColor = nil
        layer?.addSublayer(edge)
    }
    required init?(coder: NSCoder) { fatalError() }
    override var isOpaque: Bool { false }

    func configure(color: NSColor, intensity: Double, pulseSeconds: Double, cornerRadius: CGFloat, beat: Bool) {
        let key = "\(color.hashValue)-\(intensity)-\(pulseSeconds)-\(beat)"
        if cornerRadius != radius { radius = cornerRadius; needsLayout = true }
        guard key != appliedKey else { return }       // do not restart the pulse on every SwiftUI update
        appliedKey = key
        CATransaction.begin(); CATransaction.setDisableActions(true)
        let c = color.cgColor
        wide.shadowColor = c;  wide.shadowRadius = 9 * intensity;  wide.shadowOpacity = Float(min(1, 0.9 * intensity))
        tight.shadowColor = c; tight.shadowRadius = 3 * intensity; tight.shadowOpacity = Float(min(1, 0.9 * intensity))
        edge.strokeColor = c; edge.lineWidth = 1.5
        CATransaction.commit()
        for l in [wide, tight] as [CALayer] { l.removeAnimation(forKey: "pulse") }
        edge.removeAnimation(forKey: "pulse")
        beatTimer?.invalidate(); beatTimer = nil
        if beat {
            // brightness follows the music level, read 40 times a second (only the layer opacity changes)
            beatTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 40, repeats: true) { [weak self] _ in self?.applyLevel() }
            applyLevel()
            return
        }
        glow.opacity = 1; edge.opacity = 1
        for l in [wide, tight] as [CALayer] {
            let a = CABasicAnimation(keyPath: "shadowOpacity")
            a.fromValue = l.shadowOpacity; a.toValue = l.shadowOpacity * 0.45
            a.duration = max(0.3, pulseSeconds); a.autoreverses = true; a.repeatCount = .infinity
            a.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            l.add(a, forKey: "pulse")
        }
        let e = CABasicAnimation(keyPath: "opacity")
        e.fromValue = 1.0; e.toValue = 0.6; e.duration = max(0.3, pulseSeconds); e.autoreverses = true; e.repeatCount = .infinity
        e.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        edge.add(e, forKey: "pulse")
    }

    private var beatTimer: Timer?
    private func applyLevel() {
        let l = CGFloat(AudioLevel.shared.level)
        CATransaction.begin(); CATransaction.setDisableActions(true)
        glow.opacity = Float(0.12 + 0.88 * l)
        edge.opacity = Float(0.45 + 0.55 * l)
        CATransaction.commit()
    }
    override func viewDidMoveToWindow() { super.viewDidMoveToWindow(); if window == nil { beatTimer?.invalidate(); beatTimer = nil; appliedKey = "" } }

    /// Flat top (it touches the top of the screen), rounded bottom corners. AppKit layers have y pointing up.
    private func path(_ b: CGRect, _ r: CGFloat) -> CGPath {
        let r = min(r, b.width / 2, b.height)
        let p = CGMutablePath()
        p.move(to: CGPoint(x: b.minX, y: b.maxY))
        p.addLine(to: CGPoint(x: b.maxX, y: b.maxY))
        p.addLine(to: CGPoint(x: b.maxX, y: b.minY + r))
        p.addArc(tangent1End: CGPoint(x: b.maxX, y: b.minY), tangent2End: CGPoint(x: b.maxX - r, y: b.minY), radius: r)
        p.addLine(to: CGPoint(x: b.minX + r, y: b.minY))
        p.addArc(tangent1End: CGPoint(x: b.minX, y: b.minY), tangent2End: CGPoint(x: b.minX, y: b.minY + r), radius: r)
        p.closeSubpath()
        return p
    }

    override func layout() {
        super.layout()
        CATransaction.begin(); CATransaction.setDisableActions(true)
        let b = bounds, p = path(b, radius)
        glow.frame = b; cutout.frame = b
        let big = CGMutablePath(); big.addRect(b.insetBy(dx: -90, dy: -90)); big.addPath(p)
        cutout.path = big
        for l in [wide, tight] as [CALayer] { l.frame = glow.bounds; l.shadowPath = p }
        edge.frame = b; edge.path = p
        CATransaction.commit()
    }
}
