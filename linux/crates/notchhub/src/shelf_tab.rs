//! Shelf: park files here, drag them out again.
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use notchhub_core::settings::{Settings, Tab};
use notchhub_core::shelf::Shelf;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::panel::Panel;
use crate::ui;

pub struct ShelfTab {
    shelf: RefCell<Shelf>,
    file: PathBuf,
    flow: gtk::FlowBox,
    empty: gtk::Label,
    panel: Panel,
}

fn thumbnail(path: &Path) -> gtk::Widget {
    let file = gio::File::for_path(path);
    let info = file.query_info("standard::content-type,standard::icon,thumbnail::path,standard::size", gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE).ok();
    let is_image = info.as_ref().and_then(|i| i.content_type()).map_or(false, |c| c.starts_with("image/"));
    let size = info.as_ref().map_or(0, |i| i.size());
    if is_image && size < 12 * 1024 * 1024 {
        if let Ok(pb) = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, 120, 120, true) {
            let pic = gtk::Picture::for_paintable(&gdk::Texture::for_pixbuf(&pb));
            pic.set_content_fit(gtk::ContentFit::Cover);
            pic.set_size_request(56, 56);
            pic.set_overflow(gtk::Overflow::Hidden);
            pic.add_css_class("art");
            return pic.upcast();
        }
    }
    let img = match info.and_then(|i| i.icon()) {
        Some(icon) => gtk::Image::from_gicon(&icon),
        None => gtk::Image::from_icon_name("text-x-generic"),
    };
    img.set_pixel_size(48);
    img.upcast()
}

pub fn setup(panel: &Panel, _settings: Rc<RefCell<Settings>>) -> Option<Rc<ShelfTab>> {
    let page = ui::fresh_page(panel, Tab::Shelf)?;
    let file = Settings::data_dir().join("shelf.json");
    let (head, actions) = ui::header("Drop files on the notch to park them here.");
    let clear = ui::flat_button("Clear");
    actions.append(&clear);
    page.append(&head);
    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_max_children_per_line(7);
    flow.set_min_children_per_line(3);
    flow.set_homogeneous(false);
    flow.set_halign(gtk::Align::Start);
    flow.set_valign(gtk::Align::Start);
    let empty = ui::muted("Nothing here yet.");
    empty.set_vexpand(true);
    page.append(&empty);
    page.append(&ui::scrolled(&flow));

    let tab = Rc::new(ShelfTab { shelf: RefCell::new(Shelf::load_from(&file)), file, flow, empty, panel: panel.clone() });
    let t = tab.clone();
    clear.connect_clicked(move |_| {
        t.shelf.borrow_mut().clear();
        t.changed();
    });
    let t = tab.clone();
    panel.connect_drop(Tab::Shelf, move |paths| {
        t.shelf.borrow_mut().add(paths);
        t.changed();
    });
    tab.rebuild();
    Some(tab)
}

impl ShelfTab {
    fn changed(self: &Rc<Self>) {
        self.shelf.borrow().save_to(&self.file);
        self.rebuild();
    }

    fn rebuild(self: &Rc<Self>) {
        while let Some(c) = self.flow.first_child() {
            self.flow.remove(&c);
        }
        let items = self.shelf.borrow().items.clone();
        self.empty.set_visible(items.is_empty());
        for p in items {
            self.flow.append(&self.tile(&p));
        }
    }

    fn tile(self: &Rc<Self>, path: &Path) -> gtk::Widget {
        let b = gtk::Box::new(gtk::Orientation::Vertical, 3);
        b.add_css_class("tile");
        b.set_size_request(72, -1);
        let t = thumbnail(path);
        t.set_halign(gtk::Align::Center);
        b.append(&t);
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let l = gtk::Label::new(Some(&name));
        l.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        l.set_max_width_chars(10);
        b.append(&l);
        b.set_tooltip_text(Some(&path.to_string_lossy()));

        // drag it out to any app
        let drag = gtk::DragSource::new();
        drag.set_actions(gdk::DragAction::COPY | gdk::DragAction::MOVE | gdk::DragAction::LINK);
        let f = gio::File::for_path(path);
        drag.connect_prepare(move |_, _, _| {
            let list = gdk::FileList::from_array(&[f.clone()]);
            Some(gdk::ContentProvider::for_value(&list.to_value()))
        });
        b.add_controller(drag);

        // double click opens, right click shows the menu
        let me = self.clone();
        let p = path.to_path_buf();
        let click = gtk::GestureClick::new();
        click.set_button(0);
        let anchor = b.clone();
        click.connect_pressed(move |g, n, _, _| match g.current_button() {
            1 if n == 2 => open(&p),
            3 => me.menu(&anchor, &p),
            _ => {}
        });
        b.add_controller(click);
        b.upcast()
    }

    fn menu(self: &Rc<Self>, anchor: &gtk::Box, path: &Path) {
        let pop = gtk::Popover::new();
        pop.set_parent(anchor);
        let col = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let add = |label: &str, f: Box<dyn Fn()>| {
            let b = gtk::Button::with_label(label);
            b.add_css_class("flat");
            let pop = pop.clone();
            b.connect_clicked(move |_| {
                pop.popdown();
                f();
            });
            col.append(&b);
        };
        let p = path.to_path_buf();
        add("Open", Box::new(move || open(&p)));
        let p = path.to_path_buf();
        add("Show in folder", Box::new(move || p.parent().map(open).unwrap_or(())));
        if kdeconnect_available() {
            let p = path.to_path_buf();
            add("Send to phone (KDE Connect)", Box::new(move || send_to_phone(&p)));
        }
        let (me, p) = (self.clone(), path.to_path_buf());
        add("Remove", Box::new(move || {
            me.shelf.borrow_mut().remove(&p);
            me.changed();
        }));
        pop.set_child(Some(&col));
        // keep the panel open while the menu is
        let panel = self.panel.clone();
        pop.connect_closed(move |p| {
            p.unparent();
            let _ = &panel;
        });
        pop.popup();
    }
}

fn open(path: &Path) {
    let uri = gio::File::for_path(path).uri();
    let _ = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>);
}

fn kdeconnect_available() -> bool {
    std::env::var_os("PATH").map_or(false, |p| std::env::split_paths(&p).any(|d| d.join("kdeconnect-cli").is_file()))
}

fn send_to_phone(path: &Path) {
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        let ids = std::process::Command::new("kdeconnect-cli").args(["--list-available", "--id-only"]).output();
        if let Ok(o) = ids {
            if let Some(id) = String::from_utf8_lossy(&o.stdout).lines().next() {
                let _ = std::process::Command::new("kdeconnect-cli").args(["--device", id, "--share"]).arg(&path).status();
            }
        }
    });
}

#[allow(dead_code)]
fn _keep(_: glib::Priority) {}
