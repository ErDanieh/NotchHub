//! Clipboard history.
use gtk::glib;
use gtk::prelude::*;
use notchhub_core::clipboard::{self, History};
use notchhub_core::settings::{Settings, Tab};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use crate::panel::Panel;
use crate::ui;

pub struct ClipboardTab {
    history: RefCell<History>,
    file: PathBuf,
    list: gtk::Box,
    empty: gtk::Label,
    /// Set while we copy something back ourselves, so it is not recorded again.
    own: Cell<bool>,
}

pub fn setup(panel: &Panel, _settings: Rc<RefCell<Settings>>) -> Option<Rc<ClipboardTab>> {
    let page = ui::fresh_page(panel, Tab::Clipboard)?;
    let (head, actions) = ui::header("What you copy shows up here. Click to copy it again.");
    let clear = ui::flat_button("Clear");
    actions.append(&clear);
    page.append(&head);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let empty = ui::muted("Nothing copied yet.");
    empty.set_vexpand(true);
    page.append(&empty);
    page.append(&ui::scrolled(&list));
    let file = Settings::data_dir().join("clipboard.json");
    let tab = Rc::new(ClipboardTab { history: RefCell::new(History::load_from(&file)), file, list, empty, own: Cell::new(false) });
    let t = tab.clone();
    clear.connect_clicked(move |_| {
        t.history.borrow_mut().clear();
        t.changed();
    });

    if let Some(display) = gtk::gdk::Display::default() {
        let cb = display.clipboard();
        let t = tab.clone();
        cb.connect_changed(move |cb| {
            if cb.is_local() || t.own.get() {
                return;
            }
            let mimes: Vec<String> = cb.formats().mime_types().iter().map(|m| m.to_string()).collect();
            if clipboard::is_secret(&mimes) {
                return;
            }
            let (cb, t) = (cb.clone(), t.clone());
            glib::spawn_future_local(async move {
                if let Ok(Some(text)) = cb.read_text_future().await {
                    let now = glib::real_time() / 1_000_000;
                    if t.history.borrow_mut().push(text.as_str(), now) {
                        t.changed();
                    }
                }
            });
        });
    }
    tab.rebuild();
    Some(tab)
}

impl ClipboardTab {
    fn changed(self: &Rc<Self>) {
        self.history.borrow().save_to(&self.file);
        self.rebuild();
    }

    fn rebuild(self: &Rc<Self>) {
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        let entries: Vec<_> = self.history.borrow().ordered().into_iter().cloned().collect();
        self.empty.set_visible(entries.is_empty());
        for e in entries {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            row.add_css_class("row");
            let one_line: String = e.text.split_whitespace().collect::<Vec<_>>().join(" ");
            let l = gtk::Label::new(Some(&one_line));
            l.set_xalign(0.0);
            l.set_hexpand(true);
            l.set_ellipsize(gtk::pango::EllipsizeMode::End);
            l.set_max_width_chars(60);
            row.append(&l);
            let pin = ui::icon_button(if e.pinned { "starred-symbolic" } else { "non-starred-symbolic" }, "Pin");
            if e.pinned {
                pin.add_css_class("on");
            }
            let del = ui::icon_button("window-close-symbolic", "Remove");
            row.append(&pin);
            row.append(&del);
            let (me, text) = (self.clone(), e.text.clone());
            pin.connect_clicked(move |_| {
                me.history.borrow_mut().toggle_pin(&text);
                me.changed();
            });
            let (me, text) = (self.clone(), e.text.clone());
            del.connect_clicked(move |_| {
                me.history.borrow_mut().remove(&text);
                me.changed();
            });
            let (me, text) = (self.clone(), e.text.clone());
            let click = gtk::GestureClick::new();
            click.connect_released(move |g, _, _, _| {
                if g.widget().and_then(|w| w.pick(0.0, 0.0, gtk::PickFlags::DEFAULT)).is_none() {}
                if let Some(display) = gtk::gdk::Display::default() {
                    me.own.set(true);
                    display.clipboard().set_text(&text);
                    let me2 = me.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || me2.own.set(false));
                }
            });
            l.add_controller(click);
            self.list.append(&row);
        }
    }
}
