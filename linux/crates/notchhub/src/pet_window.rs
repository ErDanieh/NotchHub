//! The pet: a click-through window along the top edge. It hangs from the desktop's top bar, walks, sleeps and
//! peeks out from under the pill. It hides while the panel is open.
use gtk::glib;
use gtk::prelude::*;
use notchhub_core::pets::{self, Pet};
use notchhub_core::settings::Settings;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::panel::Panel;
use crate::x11::X;

const HEIGHT: i32 = 110;

pub fn setup(app: &gtk::Application, panel: &Panel, settings: Rc<RefCell<Settings>>) {
    let win = gtk::Window::builder().application(app).decorated(false).resizable(true).title("NotchHub pet").default_width(1200).default_height(HEIGHT).build();
    win.add_css_class("notchhub");
    let area = gtk::DrawingArea::new();
    win.set_child(Some(&area));
    let pet = Rc::new(RefCell::new(Pet::new(glib::real_time() as u32 | 1)));
    let sprite = Rc::new(RefCell::new(pets::sprite(&settings.borrow().pet_kind)));
    let kind = Rc::new(RefCell::new(settings.borrow().pet_kind.clone()));
    let playing = Rc::new(std::cell::Cell::new(false));

    {
        let (pet, sprite, settings, panel, playing) = (pet.clone(), sprite.clone(), settings.clone(), panel.clone(), playing.clone());
        area.set_draw_func(move |_, cr, w, _h| {
            let _ = cr.set_operator(gtk::cairo::Operator::Clear);
            let _ = cr.paint();
            let _ = cr.set_operator(gtk::cairo::Operator::Over);
            let p = pet.borrow();
            let sp = sprite.borrow();
            let px = settings.borrow().pet_size as f64;
            let (cols, rows) = (sp.cols(), sp.rows());
            let (sw, sh) = (cols as f64 * px, rows as f64 * px);
            let dancing = playing.get() && matches!(p.mode, pets::Mode::Idle) || (playing.get() && p.peeking());
            let bounce = if dancing { (p.clock * 7.0).sin().abs() * px * 1.5 } else { 0.0 };
            let frame_i = if p.mode == pets::Mode::Walk { p.step } else if dancing { (p.clock * 4.0) as usize % 2 } else { 0 };
            let frame = &sp.frames[frame_i % sp.frames.len()];
            let pill_h = panel.pill_rect().1;
            // hanging from the bar: the feet touch the top edge. Peeking: slides down out of the pill.
            let y_top = if p.peeking() {
                let _ = cr.save();
                cr.rectangle(0.0, pill_h, w as f64, 1000.0);
                cr.clip();
                (pill_h - sh + p.peek * sh * 0.9).round()
            } else {
                bounce.round()
            };
            let wiggle = if p.mode == pets::Mode::PeekHold { ((p.clock * 3.0).sin() * px).round() } else { 0.0 };
            let x0 = (p.x - sw / 2.0 + wiggle).round();
            let closed = p.blink || p.mode == pets::Mode::Sleep;
            let flip = !p.peeking() && p.facing < 0.0;
            for (r, line) in frame.iter().enumerate() {
                for (c, ch) in line.chars().enumerate() {
                    if ch == '.' {
                        continue;
                    }
                    let key = if ch == 'E' && closed { sp.body } else { ch };
                    let Some((rr, gg, bb)) = sp.color(key) else { continue };
                    let cx = if flip { cols - 1 - c } else { c };
                    let ry = rows - 1 - r; // upside down: hanging by its feet
                    cr.rectangle(x0 + cx as f64 * px, y_top + ry as f64 * px, px, px);
                    cr.set_source_rgb(rr as f64 / 255.0, gg as f64 / 255.0, bb as f64 / 255.0);
                    let _ = cr.fill();
                }
            }
            if p.peeking() {
                let _ = cr.restore();
            }
            if p.mode == pets::Mode::Sleep {
                let t = (p.clock % 2.0) / 2.0;
                cr.set_source_rgba(1.0, 1.0, 1.0, 1.0 - t);
                cr.set_font_size(9.0 + 1.5 * px);
                cr.move_to(x0 + sw + 4.0 + t * 8.0, sh + 14.0 + t * 10.0);
                let _ = cr.show_text("z");
            }
        });
    }

    let x = X::connect();
    let place = {
        let win = win.clone();
        let panel = panel.clone();
        Rc::new(move |pet: &mut Pet| {
            let Some(display) = gtk::gdk::Display::default() else { return };
            let Some(mon) = display.monitors().item(0).and_downcast::<gtk::gdk::Monitor>() else { return };
            let g = mon.geometry();
            let scale = mon.scale_factor().max(1);
            pet.width = g.width() as f64;
            pet.notch_x = g.width() as f64 / 2.0;
            if let (Some(x), Some(surface)) = (&x, win.surface()) {
                if let Some(s) = surface.downcast_ref::<gdk4_x11::X11Surface>() {
                    let xid = s.xid() as u32;
                    x.make_dock(xid);
                    let top = x.workarea_top();
                    let top = if top == 0 && std::env::var("XDG_CURRENT_DESKTOP").map_or(false, |d| d.contains("GNOME")) { 32 * scale } else { top };
                    x.place(xid, g.x() * scale, g.y() * scale + top, (g.width() * scale) as u32, (HEIGHT * scale) as u32);
                }
                surface.set_input_region(&gtk::cairo::Region::create());
            }
            let _ = &panel;
        })
    };
    {
        let (pet, place) = (pet.clone(), place.clone());
        win.connect_map(move |_| place(&mut pet.borrow_mut()));
    }
    win.present();

    let (win2, panel2, settings2) = (win.clone(), panel.clone(), settings.clone());
    glib::timeout_add_local(Duration::from_millis(50), move || {
        let s = settings2.borrow();
        let show = s.pet_enabled && !panel2.is_open();
        playing.set(panel2.is_playing());
        if show {
            if !win2.is_visible() {
                win2.set_visible(true);
            }
            if *kind.borrow() != s.pet_kind {
                *kind.borrow_mut() = s.pet_kind.clone();
                *sprite.borrow_mut() = pets::sprite(&s.pet_kind);
            }
            pet.borrow_mut().update(0.05, s.pet_size as f64 / 2.0);
            area.queue_draw();
        } else if win2.is_visible() {
            win2.set_visible(false);
        }
        glib::ControlFlow::Continue
    });
}
