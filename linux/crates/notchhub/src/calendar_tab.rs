//! Calendar tab: a month, the events of the chosen day, and a heads-up before each event.
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, Timelike};
use gtk::glib;
use gtk::prelude::*;
use notchhub_core::calendar::{self, Event, RawEvent};
use notchhub_core::settings::{Settings, Tab};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration as StdDuration;

use crate::panel::Panel;
use crate::runtime::on_rt;
use crate::ui;

pub struct Cal {
    settings: Rc<RefCell<Settings>>,
    panel: Panel,
    raw: RefCell<Vec<RawEvent>>,
    loaded: Cell<bool>,
    cal: gtk::Calendar,
    day_label: gtk::Label,
    list: gtk::Box,
    empty: gtk::Label,
    alerted: RefCell<HashSet<String>>,
}

async fn load_source(src: String) -> Option<Vec<RawEvent>> {
    let text = if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("webcal://") {
        let c = notchhub_core::http::client()?;
        let url = src.replacen("webcal://", "https://", 1);
        c.get(url).send().await.ok()?.text().await.ok()?
    } else {
        std::fs::read_to_string(&src).ok()?
    };
    Some(calendar::parse_ics(&text))
}

fn day_start(d: NaiveDate) -> NaiveDateTime {
    d.and_hms_opt(0, 0, 0).unwrap()
}

pub fn setup(panel: &Panel, settings: Rc<RefCell<Settings>>) -> Option<Rc<Cal>> {
    let page = ui::fresh_page(panel, Tab::Calendar)?;
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    body.set_vexpand(true);
    let cal = gtk::Calendar::new();
    cal.add_css_class("compact-cal");
    cal.set_valign(gtk::Align::Start);
    body.append(&cal);
    let right = gtk::Box::new(gtk::Orientation::Vertical, 4);
    right.set_hexpand(true);
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let day_label = gtk::Label::new(None);
    day_label.add_css_class("title");
    day_label.set_xalign(0.0);
    day_label.set_hexpand(true);
    let add = ui::icon_button("list-add-symbolic", "Add a calendar");
    head.append(&day_label);
    head.append(&add);
    right.append(&head);
    let empty = ui::muted("");
    empty.set_xalign(0.0);
    empty.set_wrap(true);
    right.append(&empty);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    right.append(&ui::scrolled(&list));
    body.append(&right);
    page.append(&body);

    let c = Rc::new(Cal { settings, panel: panel.clone(), raw: RefCell::new(vec![]), loaded: Cell::new(false), cal, day_label, list, empty, alerted: RefCell::new(HashSet::new()) });
    let me = c.clone();
    c.cal.connect_day_selected(move |_| me.show_day());
    let me = c.clone();
    c.cal.connect_next_month(move |_| me.mark_month());
    let me = c.clone();
    c.cal.connect_prev_month(move |_| me.mark_month());
    let me = c.clone();
    add.connect_clicked(move |b| me.sources_popover(b));
    c.reload();
    // reload every 15 minutes, check the alerts every 30 seconds
    let me = c.clone();
    glib::timeout_add_local(StdDuration::from_secs(15 * 60), move || {
        me.reload();
        glib::ControlFlow::Continue
    });
    let me = c.clone();
    glib::timeout_add_local(StdDuration::from_secs(30), move || {
        me.check_alerts();
        glib::ControlFlow::Continue
    });
    Some(c)
}

impl Cal {
    fn reload(self: &Rc<Self>) {
        let sources = self.settings.borrow().calendar_sources.clone();
        let me = self.clone();
        glib::spawn_future_local(async move {
            let mut all = vec![];
            for s in sources {
                if let Some(Some(v)) = on_rt(load_source(s)).await {
                    all.extend(v);
                }
            }
            *me.raw.borrow_mut() = all;
            me.loaded.set(true);
            me.mark_month();
            me.show_day();
            me.check_alerts();
        });
    }

    fn selected(&self) -> NaiveDate {
        let d = self.cal.date();
        NaiveDate::from_ymd_opt(d.year(), d.month() as u32, d.day_of_month() as u32).unwrap_or_else(|| Local::now().date_naive())
    }

    fn mark_month(&self) {
        self.cal.clear_marks();
        let d = self.cal.date();
        let first = NaiveDate::from_ymd_opt(d.year(), d.month() as u32, 1).unwrap();
        let next = if d.month() == 12 { NaiveDate::from_ymd_opt(d.year() + 1, 1, 1) } else { NaiveDate::from_ymd_opt(d.year(), d.month() as u32 + 1, 1) }.unwrap();
        for e in calendar::expand(&self.raw.borrow(), day_start(first), day_start(next)) {
            self.cal.mark_day(e.start.day());
        }
    }

    fn show_day(&self) {
        let day = self.selected();
        let sources = self.settings.borrow().calendar_sources.is_empty();
        self.day_label.set_text(&day.format("%A %-d %B").to_string());
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        let events: Vec<Event> = calendar::expand(&self.raw.borrow(), day_start(day), day_start(day + Duration::days(1)));
        if sources {
            self.empty.set_text("No calendar yet. Press + and add the .ics file or the \"secret address in iCal format\" of your calendar (Google, Nextcloud, Outlook...).");
            self.empty.set_visible(true);
            return;
        }
        self.empty.set_text("Nothing planned.");
        self.empty.set_visible(events.is_empty());
        for e in events {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("row");
            let time = if e.all_day { "All day".to_string() } else { format!("{:02}:{:02}", e.start.hour(), e.start.minute()) };
            let t = ui::muted(&time);
            t.set_width_chars(7);
            t.set_xalign(0.0);
            let info = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let title = gtk::Label::new(Some(&e.title));
            title.set_xalign(0.0);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            info.append(&title);
            if !e.location.is_empty() {
                let l = ui::muted(&e.location);
                l.set_xalign(0.0);
                l.set_ellipsize(gtk::pango::EllipsizeMode::End);
                info.append(&l);
            }
            row.append(&t);
            row.append(&info);
            self.list.append(&row);
        }
    }

    fn check_alerts(&self) {
        let st = self.settings.borrow();
        if !st.alerts_calendar {
            return;
        }
        let now = Local::now().naive_local();
        let lead = Duration::minutes(st.calendar_lead_minutes as i64);
        for e in calendar::expand(&self.raw.borrow(), now, now + lead + Duration::seconds(1)) {
            if e.all_day || e.start < now {
                continue;
            }
            let key = format!("{}|{}", e.title, e.start);
            if self.alerted.borrow_mut().insert(key) {
                let mins = ((e.start - now).num_seconds() as f64 / 60.0).ceil() as i64;
                self.panel.toast(&format!("📅 {} — in {} min", e.title, mins.max(1)));
            }
        }
    }

    fn sources_popover(self: &Rc<Self>, anchor: &gtk::Button) {
        let pop = gtk::Popover::new();
        pop.set_parent(anchor);
        let col = gtk::Box::new(gtk::Orientation::Vertical, 6);
        col.set_margin_top(8);
        col.set_margin_bottom(8);
        col.set_margin_start(8);
        col.set_margin_end(8);
        col.set_width_request(320);
        for (i, s) in self.settings.borrow().calendar_sources.iter().enumerate() {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let l = gtk::Label::new(Some(s));
            l.set_hexpand(true);
            l.set_xalign(0.0);
            l.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            let del = gtk::Button::from_icon_name("user-trash-symbolic");
            del.add_css_class("flat");
            let (me, pop2) = (self.clone(), pop.clone());
            del.connect_clicked(move |_| {
                me.settings.borrow_mut().calendar_sources.remove(i);
                me.settings.borrow().save();
                pop2.popdown();
                me.reload();
            });
            row.append(&l);
            row.append(&del);
            col.append(&row);
        }
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("https://… .ics link, or a file path"));
        col.append(&entry);
        let btns = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let add = gtk::Button::with_label("Add link");
        let file = gtk::Button::with_label("Choose .ics file…");
        btns.append(&add);
        btns.append(&file);
        col.append(&btns);
        let (me, e, p2) = (self.clone(), entry.clone(), pop.clone());
        let add_fn = move || {
            let t = e.text().trim().to_string();
            if !t.is_empty() {
                me.settings.borrow_mut().calendar_sources.push(t);
                me.settings.borrow().save();
                p2.popdown();
                me.reload();
            }
        };
        let a2 = add_fn.clone();
        add.connect_clicked(move |_| add_fn());
        entry.connect_activate(move |_| a2());
        let (me, p2) = (self.clone(), pop.clone());
        file.connect_clicked(move |b| {
            let dialog = gtk::FileDialog::builder().title("Choose a calendar file").build();
            let (me, p2) = (me.clone(), p2.clone());
            let win = b.root().and_downcast::<gtk::Window>();
            dialog.open(win.as_ref(), gtk::gio::Cancellable::NONE, move |r| {
                if let Some(path) = r.ok().and_then(|f| f.path()) {
                    me.settings.borrow_mut().calendar_sources.push(path.to_string_lossy().to_string());
                    me.settings.borrow().save();
                    p2.popdown();
                    me.reload();
                }
            });
        });
        pop.set_child(Some(&col));
        pop.connect_closed(|p| p.unparent());
        pop.popup();
    }
}
