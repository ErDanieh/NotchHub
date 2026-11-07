//! A small ball at the top centre of every other screen. Rest the pointer on it and the panel opens there.
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use notchhub_core::settings::Settings;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::panel::Panel;
use crate::x11::X;

const SIZE: i32 = 44;

struct Ball {
    monitor: u32,
    win: gtk::Window,
}

pub fn setup(app: &gtk::Application, panel: &Panel, settings: Rc<RefCell<Settings>>) {
    let balls: Rc<RefCell<Vec<Ball>>> = Rc::new(RefCell::new(vec![]));
    let rebuild: Rc<dyn Fn()> = {
        let (app, panel, settings, balls) = (app.clone(), panel.clone(), settings.clone(), balls.clone());
        Rc::new(move || {
            for b in balls.borrow_mut().drain(..) {
                b.win.close();
            }
            let Some(display) = gdk::Display::default() else { return };
            if !settings.borrow().balls_enabled {
                return;
            }
            let n = display.monitors().n_items();
            for i in 1..n {
                let Some(mon) = display.monitors().item(i).and_downcast::<gdk::Monitor>() else { continue };
                let win = gtk::Window::builder().application(&app).decorated(false).title("NotchHub ball").default_width(SIZE).default_height(SIZE).build();
                win.add_css_class("notchhub");
                let area = gtk::DrawingArea::new();
                let (settings2, panel2) = (settings.clone(), panel.clone());
                area.set_draw_func(move |_, cr, w, _| {
                    let _ = cr.set_operator(gtk::cairo::Operator::Clear);
                    let _ = cr.paint();
                    let _ = cr.set_operator(gtk::cairo::Operator::Over);
                    let s = settings2.borrow();
                    let active = s.halo_enabled && (panel2.is_playing() || s.halo_always);
                    let (cx, cy, r) = (w as f64 / 2.0, 14.0, 8.0);
                    let (rr, gg, bb) = s.halo_rgb();
                    if active {
                        for k in (1..=8).rev() {
                            let f = 1.0 - k as f64 / 9.0;
                            cr.set_source_rgba(rr, gg, bb, 0.4 * f * f * s.halo_intensity);
                            cr.arc(cx, cy, r + k as f64 * 1.4, 0.0, std::f64::consts::TAU);
                            let _ = cr.fill();
                        }
                    }
                    cr.set_source_rgb(0.0, 0.0, 0.0);
                    cr.arc(cx, cy, r, 0.0, std::f64::consts::TAU);
                    let _ = cr.fill();
                    if active { cr.set_source_rgb(rr, gg, bb) } else { cr.set_source_rgba(1.0, 1.0, 1.0, 0.3) }
                    cr.set_line_width(1.5);
                    cr.arc(cx, cy, r, 0.0, std::f64::consts::TAU);
                    let _ = cr.stroke();
                });
                win.set_child(Some(&area));
                // hover = open on this screen after a short rest
                let motion = gtk::EventControllerMotion::new();
                let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
                let (p2, pend, delay) = (panel.clone(), pending.clone(), settings.clone());
                motion.connect_enter(move |_, _, _| {
                    let p3 = p2.clone();
                    let pend2 = pend.clone();
                    let ms = (delay.borrow().open_delay * 1000.0) as u64;
                    let id = glib::timeout_add_local_once(Duration::from_millis(ms.max(1)), move || {
                        pend2.borrow_mut().take();
                        p3.open_on(i);
                    });
                    *pend.borrow_mut() = Some(id);
                });
                let pend = pending.clone();
                motion.connect_leave(move |_| {
                    if let Some(s) = pend.borrow_mut().take() {
                        s.remove();
                    }
                });
                win.add_controller(motion);
                let g = mon.geometry();
                let scale = mon.scale_factor().max(1);
                let win2 = win.clone();
                win.connect_map(move |_| {
                    if let (Some(x), Some(surface)) = (X::connect(), win2.surface()) {
                        if let Some(s) = surface.downcast_ref::<gdk4_x11::X11Surface>() {
                            let xid = s.xid() as u32;
                            x.make_dock(xid);
                            x.place(xid, (g.x() + (g.width() - SIZE) / 2) * scale, g.y() * scale, (SIZE * scale) as u32, (SIZE * scale) as u32);
                            x.request_above(xid);
                        }
                    }
                });
                win.present();
                balls.borrow_mut().push(Ball { monitor: i, win });
            }
        })
    };
    rebuild();
    if let Some(display) = gdk::Display::default() {
        let r = rebuild.clone();
        display.monitors().connect_items_changed(move |_, _, _, _| r());
    }
    // hide the ball of the screen the panel is open on; redraw when the music state changes
    let (panel, balls, settings) = (panel.clone(), balls.clone(), settings.clone());
    let last = std::cell::Cell::new(settings.borrow().balls_enabled);
    glib::timeout_add_local(Duration::from_millis(300), move || {
        let now = settings.borrow().balls_enabled;
        if now != last.get() {
            last.set(now);
            rebuild();
        }
        for b in balls.borrow().iter() {
            let hide = panel.is_open() && panel.monitor() == b.monitor;
            if b.win.is_visible() == hide {
                b.win.set_visible(!hide);
            }
            if let Some(a) = b.win.child() {
                a.queue_draw();
            }
        }
        glib::ControlFlow::Continue
    });
}
