//! Docker: see and control containers. Only shown when Docker is installed.
use gtk::glib;
use gtk::prelude::*;
use notchhub_core::docker::{self, Container, Listing};
use notchhub_core::settings::Tab;
use std::rc::Rc;
use std::time::Duration;

use crate::panel::Panel;
use crate::runtime::rt;
use crate::ui;

pub fn setup(panel: &Panel) {
    if !docker::installed() {
        return;
    }
    let Some(page) = ui::fresh_page(panel, Tab::Docker) else { return };
    let (head, _) = ui::header("Your containers");
    page.append(&head);
    let msg = ui::muted("");
    msg.set_vexpand(true);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.append(&msg);
    page.append(&ui::scrolled(&list));

    let refresh: Rc<dyn Fn()> = {
        let (list, msg) = (list.clone(), msg.clone());
        Rc::new(move || {
            let (list, msg) = (list.clone(), msg.clone());
            glib::spawn_future_local(async move {
                let res = rt().spawn_blocking(docker::list).await;
                while let Some(c) = list.first_child() {
                    list.remove(&c);
                }
                match res {
                    Ok(Listing::Ok(v)) if v.is_empty() => {
                        msg.set_text("No containers.");
                        msg.set_visible(true);
                    }
                    Ok(Listing::Ok(v)) => {
                        msg.set_visible(false);
                        for c in v {
                            list.append(&row(&c));
                        }
                    }
                    Ok(Listing::Error(e)) => {
                        msg.set_text(&format!("Docker is not answering: {e}"));
                        msg.set_visible(true);
                    }
                    Err(_) => {}
                }
            });
        })
    };

    let panel = panel.clone();
    let r = refresh.clone();
    glib::timeout_add_local(Duration::from_secs(3), move || {
        if panel.is_open() && panel.current_tab() == Tab::Docker {
            r();
        }
        glib::ControlFlow::Continue
    });
    refresh();
}

fn row(c: &Container) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("row");
    let dot = gtk::Label::new(Some("●"));
    dot.add_css_class(if c.running { "running" } else { "stopped" });
    row.append(&dot);
    let info = gtk::Box::new(gtk::Orientation::Vertical, 0);
    info.set_hexpand(true);
    let name = gtk::Label::new(Some(&c.name));
    name.set_xalign(0.0);
    let sub = ui::muted(&format!("{} · {}", c.image, c.status));
    sub.set_xalign(0.0);
    sub.set_ellipsize(gtk::pango::EllipsizeMode::End);
    info.append(&name);
    info.append(&sub);
    row.append(&info);
    let act = |icon: &str, tip: &str, verb: &'static str| {
        let b = ui::icon_button(icon, tip);
        let id = c.id.clone();
        b.connect_clicked(move |b| {
            b.set_sensitive(false);
            let id = id.clone();
            let b = b.clone();
            glib::spawn_future_local(async move {
                let _ = rt().spawn_blocking(move || docker::action(verb, &id)).await;
                b.set_sensitive(true);
            });
        });
        b
    };
    if c.running {
        row.append(&act("media-playback-stop-symbolic", "Stop", "stop"));
        row.append(&act("view-refresh-symbolic", "Restart", "restart"));
    } else {
        row.append(&act("media-playback-start-symbolic", "Start", "start"));
    }
    row
}
