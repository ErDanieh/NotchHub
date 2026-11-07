//! Neon glow around the pill, drawn as soft concentric outlines (no blur needed).
use gtk::cairo;

pub struct Halo {
    pub rgb: (f64, f64, f64),
    pub intensity: f64,
    /// 0..1 pulse phase value.
    pub pulse: f64,
    /// Extra brightness from the music beat, 0..1.
    pub level: f64,
}

fn rounded_bottom(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h);
    let pi = std::f64::consts::PI;
    cr.new_sub_path();
    cr.move_to(x, y);
    cr.line_to(x + w, y);
    cr.line_to(x + w, y + h - r);
    cr.arc(x + w - r, y + h - r, r, 0.0, pi / 2.0);
    cr.line_to(x + r, y + h);
    cr.arc(x + r, y + h - r, r, pi / 2.0, pi);
    cr.close_path();
}

/// `shape` is the pill (width, height), centred at the top of a `width`-wide area.
pub fn draw(cr: &cairo::Context, width: f64, shape: (f64, f64), radius: f64, halo: &Halo) {
    let (sw, sh) = shape;
    let x0 = (width - sw) / 2.0;
    let layers = 14;
    let base = 0.5 * halo.intensity * (0.62 + 0.38 * halo.pulse) + 0.3 * halo.level;
    cr.set_line_width(2.0);
    for i in (1..=layers).rev() {
        let f = 1.0 - i as f64 / (layers as f64 + 1.0);
        let alpha = (base * f.powf(2.3)).clamp(0.0, 1.0);
        let s = i as f64 * 1.9;
        rounded_bottom(cr, x0 - s, -4.0, sw + 2.0 * s, sh + s + 4.0, radius + s);
        cr.set_source_rgba(halo.rgb.0, halo.rgb.1, halo.rgb.2, alpha);
        let _ = cr.stroke();
    }
}

/// The black pill / panel itself, drawn in the same pass as the glow so the two can never get out of step.
pub fn fill_shape(cr: &cairo::Context, width: f64, shape: (f64, f64), radius: f64) {
    rounded_bottom(cr, (width - shape.0) / 2.0, -4.0, shape.0, shape.1 + 4.0, radius);
    cr.set_source_rgb(0.0, 0.0, 0.0);
    let _ = cr.fill();
}
