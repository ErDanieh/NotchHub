//! Small helpers shared by the tabs.
use gtk::prelude::*;
use notchhub_core::settings::Tab;

use crate::panel::Panel;

/// The page of a tab with its placeholder removed.
pub fn fresh_page(panel: &Panel, tab: Tab) -> Option<gtk::Box> {
    let page = panel.page(tab)?;
    while let Some(c) = page.first_child() {
        page.remove(&c);
    }
    Some(page)
}

pub fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    let s = gtk::ScrolledWindow::new();
    s.set_vexpand(true);
    s.set_hscrollbar_policy(gtk::PolicyType::Never);
    s.set_child(Some(child));
    s
}

pub fn flat_button(label: &str) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.add_css_class("ctl");
    b
}

pub fn icon_button(icon: &str, tip: &str) -> gtk::Button {
    let b = gtk::Button::from_icon_name(icon);
    b.add_css_class("ctl");
    b.set_tooltip_text(Some(tip));
    b
}

pub fn muted(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class("subtitle");
    l
}

/// Header line of a tab: a hint on the left, actions on the right.
pub fn header(hint: &str) -> (gtk::Box, gtk::Box) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let l = muted(hint);
    l.set_hexpand(true);
    l.set_xalign(0.0);
    row.append(&l);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    row.append(&actions);
    (row, actions)
}
