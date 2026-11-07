//! System: CPU, memory, network, battery, disk and the busiest apps; alerts when something runs hot.
use gtk::glib;
use gtk::prelude::*;
use notchhub_core::settings::{Settings, Tab};
use notchhub_core::sysstats::{human_rate, AlertGate, Monitor};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use crate::panel::Panel;
use crate::ui;

struct Widgets {
    cpu: gtk::Label,
    cpu_bar: gtk::LevelBar,
    cpu_spark: gtk::DrawingArea,
    mem: gtk::Label,
    mem_bar: gtk::LevelBar,
    net: gtk::Label,
    net_spark: gtk::DrawingArea,
    power: gtk::Label,
    disk: gtk::Label,
    top: gtk::Label,
}

fn spark(data: Rc<RefCell<VecDeque<f64>>>, max_fixed: Option<f64>) -> gtk::DrawingArea {
    let a = gtk::DrawingArea::new();
    a.set_size_request(-1, 18);
    a.set_hexpand(true);
    a.set_draw_func(move |_, cr, w, h| {
        let d = data.borrow();
        if d.len() < 2 {
            return;
        }
        let max = max_fixed.unwrap_or_else(|| d.iter().cloned().fold(1.0, f64::max));
        let (w, h) = (w as f64, h as f64);
        let step = w / (crate::SPARK_POINTS as f64 - 1.0);
        let x0 = w - step * (d.len() as f64 - 1.0);
        cr.move_to(x0, h);
        for (i, v) in d.iter().enumerate() {
            cr.line_to(x0 + step * i as f64, h - (v / max).clamp(0.0, 1.0) * (h - 2.0));
        }
        cr.line_to(w, h);
        cr.close_path();
        cr.set_source_rgba(0.24, 0.66, 1.0, 0.28);
        let _ = cr.fill_preserve();
        cr.set_source_rgba(0.24, 0.66, 1.0, 0.9);
        cr.set_line_width(1.4);
        let _ = cr.stroke();
    });
    a
}

pub fn setup(panel: &Panel, settings: Rc<RefCell<Settings>>) {
    let Some(page) = ui::fresh_page(panel, Tab::System) else { return };
    let mon = Rc::new(RefCell::new(Monitor::new()));
    let cpu_hist = Rc::new(RefCell::new(VecDeque::<f64>::new()));
    let net_hist = Rc::new(RefCell::new(VecDeque::<f64>::new()));

    let card = |title: &str| -> (gtk::Box, gtk::Label) {
        let c = gtk::Box::new(gtk::Orientation::Vertical, 3);
        c.add_css_class("stat");
        c.set_hexpand(true);
        let t = ui::muted(title);
        t.set_xalign(0.0);
        let big = gtk::Label::new(Some("–"));
        big.add_css_class("big");
        big.set_ellipsize(gtk::pango::EllipsizeMode::End);
        big.set_xalign(0.0);
        c.append(&t);
        c.append(&big);
        (c, big)
    };
    let bar = || {
        let b = gtk::LevelBar::new();
        b.set_min_value(0.0);
        b.set_max_value(100.0);
        b
    };

    let grid = gtk::Grid::new();
    grid.set_column_spacing(8);
    grid.set_row_spacing(8);
    grid.set_column_homogeneous(true);
    let (c1, cpu) = card("CPU");
    let top_row = ();
    let _ = top_row;
    let cpu_bar = bar();
    let cpu_spark = spark(cpu_hist.clone(), Some(100.0));
    c1.append(&cpu_spark);
    c1.append(&cpu_bar);
    let (c2, mem) = card("Memory");
    let mem_bar = bar();
    c2.append(&mem_bar);
    let (c3, net) = card("Network");
    let net_spark = spark(net_hist.clone(), None);
    c3.append(&net_spark);
    let (c4, power) = card("Power and disk");
    let disk = ui::muted("");
    disk.set_xalign(0.0);
    c4.append(&disk);
    grid.attach(&c1, 0, 0, 1, 1);
    grid.attach(&c2, 1, 0, 1, 1);
    grid.attach(&c3, 0, 1, 1, 1);
    grid.attach(&c4, 1, 1, 1, 1);
    page.append(&grid);
    let top = ui::muted("");
    top.set_xalign(0.0);
    top.set_margin_top(6);
    page.append(&top);

    let w = Widgets { cpu, cpu_bar, cpu_spark, mem, mem_bar, net, net_spark, power, disk, top };
    let gate = RefCell::new(AlertGate::default());
    let hot = Cell::new(0u32);
    let tick_n = Cell::new(0u32);
    let panel = panel.clone();
    glib::timeout_add_local(Duration::from_secs(1), move || {
        let visible = panel.is_open() && panel.current_tab() == Tab::System;
        let alerts = settings.borrow().alerts_performance;
        tick_n.set(tick_n.get() + 1);
        // always sample every 3 s for the alerts; every second while the tab is on screen
        if !visible && !(alerts && tick_n.get() % 3 == 0) {
            return glib::ControlFlow::Continue;
        }
        let mut m = mon.borrow_mut();
        m.refresh(visible);
        let s = m.snap.clone();
        *cpu_hist.borrow_mut() = m.cpu_history.clone();
        *net_hist.borrow_mut() = m.net_history.clone();
        drop(m);
        if visible {
            w.cpu.set_text(&format!("{:.0}%", s.cpu));
            w.cpu_bar.set_value(s.cpu);
            w.mem.set_text(&format!("{:.1} / {:.0} GB", s.mem_used_gb, s.mem_total_gb));
            w.mem_bar.set_value(s.mem);
            w.net.set_text(&format!("↓ {}  ↑ {}", human_rate(s.down), human_rate(s.up)));
            w.power.set_text(&s.battery.as_ref().map_or("AC".to_string(), |b| format!("{:.0}%{}", b.percent, if b.charging { " ⚡" } else { "" })));
            w.disk.set_text(&format!("Disk: {:.0} GB free of {:.0}", s.disk_free_gb, s.disk_total_gb));
            w.top.set_text(&format!("Busiest: {}", s.top.iter().map(|(n, c)| format!("{n} {c:.0}%")).collect::<Vec<_>>().join("  ·  ")));
            w.cpu_spark.queue_draw();
            w.net_spark.queue_draw();
        }
        if alerts {
            let st = settings.borrow();
            let now = glib::monotonic_time() as f64 / 1e6;
            hot.set(if s.cpu >= st.cpu_threshold { hot.get() + 1 } else { 0 });
            if hot.get() >= 2 && gate.borrow_mut().check("cpu", now, 300.0) {
                panel.toast(&format!("⚠ CPU at {:.0}%", s.cpu));
            }
            if s.mem >= st.mem_threshold && gate.borrow_mut().check("mem", now, 300.0) {
                panel.toast(&format!("⚠ Memory at {:.0}%", s.mem));
            }
            if let Some(b) = &s.battery {
                if !b.charging && b.percent <= st.battery_threshold && gate.borrow_mut().check("battery", now, 600.0) {
                    panel.toast(&format!("🔋 Battery at {:.0}%", b.percent));
                }
            }
        }
        glib::ControlFlow::Continue
    });
}
