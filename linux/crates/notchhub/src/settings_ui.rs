use gtk::glib;
use gtk::prelude::*;
use notchhub_core::settings::Settings;
use std::cell::RefCell;
use std::rc::Rc;

use crate::autostart;

type Shared = Rc<RefCell<Settings>>;

fn row(title: &str, hint: Option<&str>, w: &impl IsA<gtk::Widget>) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let l = gtk::Box::new(gtk::Orientation::Vertical, 2);
    l.set_hexpand(true);
    let t = gtk::Label::new(Some(title));
    t.set_xalign(0.0);
    l.append(&t);
    if let Some(h) = hint {
        let s = gtk::Label::new(Some(h));
        s.set_xalign(0.0);
        s.add_css_class("dim-label");
        s.set_wrap(true);
        l.append(&s);
    }
    b.append(&l);
    w.set_valign(gtk::Align::Center);
    b.append(w);
    b
}

fn section(title: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(title));
    l.set_xalign(0.0);
    l.set_margin_top(14);
    l.add_css_class("heading");
    l
}

fn switch(settings: &Shared, on_change: &Rc<dyn Fn()>, get: fn(&Settings) -> bool, set: fn(&mut Settings, bool)) -> gtk::Switch {
    let sw = gtk::Switch::new();
    sw.set_active(get(&settings.borrow()));
    let (s, cb) = (settings.clone(), on_change.clone());
    sw.connect_state_set(move |_, v| {
        set(&mut s.borrow_mut(), v);
        s.borrow().save();
        cb();
        glib::Propagation::Proceed
    });
    sw
}

fn scale(settings: &Shared, on_change: &Rc<dyn Fn()>, min: f64, max: f64, get: fn(&Settings) -> f64, set: fn(&mut Settings, f64)) -> gtk::Scale {
    let sc = gtk::Scale::with_range(gtk::Orientation::Horizontal, min, max, (max - min) / 100.0);
    sc.set_width_request(180);
    sc.set_draw_value(false);
    sc.set_value(get(&settings.borrow()));
    let (s, cb) = (settings.clone(), on_change.clone());
    sc.connect_value_changed(move |sc| {
        set(&mut s.borrow_mut(), sc.value());
        s.borrow().save();
        cb();
    });
    sc
}

pub fn build(app: &gtk::Application, settings: Shared, on_change: Rc<dyn Fn()>) -> gtk::Window {
    let win = gtk::Window::builder().application(app).title("NotchHub Settings").default_width(480).default_height(560).build();
    let col = gtk::Box::new(gtk::Orientation::Vertical, 10);
    col.set_margin_top(16);
    col.set_margin_bottom(16);
    col.set_margin_start(20);
    col.set_margin_end(20);

    col.append(&section("Halo"));
    col.append(&row("Show the halo while something plays", None, &switch(&settings, &on_change, |s| s.halo_enabled, |s, v| s.halo_enabled = v)));
    col.append(&row("Keep the halo on even when nothing plays", None, &switch(&settings, &on_change, |s| s.halo_always, |s, v| s.halo_always = v)));
    col.append(&row("Make the halo follow the beat", Some("Listens to the sound your computer plays. Nothing is recorded."), &switch(&settings, &on_change, |s| s.halo_beat, |s, v| s.halo_beat = v)));

    let color = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
    {
        let (r, g, b) = settings.borrow().halo_rgb();
        color.set_rgba(&gtk::gdk::RGBA::new(r as f32, g as f32, b as f32, 1.0));
        let (s, cb) = (settings.clone(), on_change.clone());
        color.connect_rgba_notify(move |c| {
            let v = c.rgba();
            s.borrow_mut().halo_color_hex =
                format!("{:02X}{:02X}{:02X}", (v.red() * 255.0).round() as u8, (v.green() * 255.0).round() as u8, (v.blue() * 255.0).round() as u8);
            s.borrow().save();
            cb();
        });
    }
    col.append(&row("Colour", None, &color));
    col.append(&row("Intensity", None, &scale(&settings, &on_change, 0.3, 1.6, |s| s.halo_intensity, |s, v| s.halo_intensity = v)));
    col.append(&row("Pulse speed", None, &scale(&settings, &on_change, 0.4, 2.6, |s| 3.2 - s.pulse_seconds, |s, v| s.pulse_seconds = 3.2 - v)));
    col.append(&row("Open delay", Some("How long the pointer rests on the pill before it opens."), &scale(&settings, &on_change, 0.0, 0.6, |s| s.open_delay, |s, v| s.open_delay = v)));

    col.append(&section("Screens and pets"));
    col.append(&row("Show a ball on the other screens", None, &switch(&settings, &on_change, |s| s.balls_enabled, |s, v| s.balls_enabled = v)));
    col.append(&row("Show a pet along the top edge", None, &switch(&settings, &on_change, |s| s.pet_enabled, |s, v| s.pet_enabled = v)));

    let kinds = notchhub_core::pets::all();
    let names: Vec<&str> = kinds.iter().map(|k| k.name).collect();
    let kind_dd = gtk::DropDown::from_strings(&names);
    kind_dd.set_selected(kinds.iter().position(|k| k.id == settings.borrow().pet_kind).unwrap_or(0) as u32);
    {
        let (s, cb) = (settings.clone(), on_change.clone());
        let ids: Vec<&'static str> = kinds.iter().map(|k| k.id).collect();
        kind_dd.connect_selected_notify(move |d| {
            if let Some(id) = ids.get(d.selected() as usize) {
                s.borrow_mut().pet_kind = id.to_string();
                s.borrow().save();
                cb();
            }
        });
    }
    col.append(&row("Pet", None, &kind_dd));
    let size_dd = gtk::DropDown::from_strings(&["Small", "Medium", "Large", "Huge"]);
    size_dd.set_selected(settings.borrow().pet_size.clamp(2, 5) - 2);
    {
        let (s, cb) = (settings.clone(), on_change.clone());
        size_dd.connect_selected_notify(move |d| {
            s.borrow_mut().pet_size = d.selected() + 2;
            s.borrow().save();
            cb();
        });
    }
    col.append(&row("Pet size", Some("It hangs from the top bar, wanders, sleeps, peeks out from under the pill and dances to music."), &size_dd));

    col.append(&section("Alerts"));
    col.append(&row("When the song changes", None, &switch(&settings, &on_change, |s| s.alerts_song, |s, v| s.alerts_song = v)));
    col.append(&row("Before a calendar event", None, &switch(&settings, &on_change, |s| s.alerts_calendar, |s, v| s.alerts_calendar = v)));
    col.append(&row("When a conversion finishes", None, &switch(&settings, &on_change, |s| s.alerts_conversion, |s, v| s.alerts_conversion = v)));
    col.append(&row("When the computer runs hot", Some("High CPU or memory, low battery."), &switch(&settings, &on_change, |s| s.alerts_performance, |s, v| s.alerts_performance = v)));
    col.append(&row("Lyrics timing", Some("Show the lyrics a little early (right) or late (left) if they drift."), &scale(&settings, &on_change, -2.0, 2.0, |s| s.lyrics_lead, |s, v| s.lyrics_lead = v)));

    col.append(&section("General"));
    let login = gtk::Switch::new();
    login.set_active(autostart::is_enabled());
    login.connect_state_set(|_, v| {
        autostart::set_enabled(v);
        glib::Propagation::Proceed
    });
    col.append(&row("Start NotchHub when I log in", None, &login));
    col.append(&row("Remember the last tab", None, &switch(&settings, &on_change, |s| s.remember_tab, |s, v| s.remember_tab = v)));

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_child(Some(&col));
    win.set_child(Some(&scroller));
    win
}
