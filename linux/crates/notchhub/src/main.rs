mod audio;
mod autostart;
mod balls;
mod calendar_tab;
mod convert_tab;
mod clipboard_tab;
mod docker_tab;
mod halo;
mod music;
mod news_tab;
mod panel;
mod pet_window;
mod pdf;
mod runtime;
mod shelf_tab;
mod system_tab;
mod ui;
mod settings_ui;
mod tray;
mod x11;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use notchhub_core::settings::Settings;
use std::cell::RefCell;
use std::rc::Rc;

pub const SPARK_POINTS: usize = notchhub_core::sysstats::HISTORY;

fn main() -> glib::ExitCode {
    // The panel must sit at an exact place and stay on top. Only X11 (also XWayland on GNOME) allows that.
    if std::env::var_os("DISPLAY").is_some() {
        std::env::set_var("GDK_BACKEND", "x11");
    }
    let app = gtk::Application::builder().application_id("io.github.erdanieh.NotchHub").build();
    let state: Rc<RefCell<Option<Ui>>> = Rc::new(RefCell::new(None));
    app.connect_activate(move |app| {
        // a second launch (for example `notchhub` from a terminal) opens the settings of the running one
        if let Some(ui) = &*state.borrow() {
            ui.show_settings();
            return;
        }
        *state.borrow_mut() = Some(Ui::new(app));
    });
    app.run()
}

struct Ui {
    panel: panel::Panel,
    settings_win: Rc<RefCell<Option<gtk::Window>>>,
    app: gtk::Application,
    settings: Rc<RefCell<Settings>>,
}

impl Ui {
    fn new(app: &gtk::Application) -> Ui {
        let provider = gtk::CssProvider::new();
        provider.load_from_data(include_str!("style.css"));
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
        let settings = Rc::new(RefCell::new(Settings::load()));
        let panel = panel::Panel::new(app, settings.clone());
        panel.window().present();
        let _keep = (
            shelf_tab::setup(&panel, settings.clone()),
            clipboard_tab::setup(&panel, settings.clone()),
        );
        system_tab::setup(&panel, settings.clone());
        docker_tab::setup(&panel);
        std::mem::forget(_keep);
        std::mem::forget(calendar_tab::setup(&panel, settings.clone()));
        std::mem::forget(convert_tab::setup(&panel, settings.clone()));
        std::mem::forget(news_tab::setup(&panel, settings.clone(), app));
        pet_window::setup(app, &panel, settings.clone());
        balls::setup(app, &panel, settings.clone());
        let music = music::setup(&panel, settings.clone());
        if let Some(m) = music {
            panel.connect_open_changed(move |open| m.set_open(open));
        }

        // menu on right click and the tray share the same commands
        let quit = gio::SimpleAction::new("quit", None);
        let a = app.clone();
        quit.connect_activate(move |_, _| a.quit());
        app.add_action(&quit);

        let ui = Ui { panel: panel.clone(), settings_win: Rc::new(RefCell::new(None)), app: app.clone(), settings };

        let (tx, rx) = async_channel::unbounded();
        tray::spawn(tx);
        let p = panel.clone();
        let sw = ui.settings_win.clone();
        let (a, st, pp) = (app.clone(), ui.settings.clone(), panel.clone());
        glib::spawn_future_local(async move {
            while let Ok(cmd) = rx.recv().await {
                match cmd {
                    tray::Cmd::Toggle => p.toggle(),
                    tray::Cmd::Settings => show(&a, &st, &pp, &sw),
                    tray::Cmd::Quit => a.quit(),
                }
            }
        });
        ui
    }

    fn show_settings(&self) {
        show(&self.app, &self.settings, &self.panel, &self.settings_win);
    }
}

fn show(app: &gtk::Application, settings: &Rc<RefCell<Settings>>, panel: &panel::Panel, slot: &Rc<RefCell<Option<gtk::Window>>>) {
    if slot.borrow().is_none() {
        let p = panel.clone();
        let w = settings_ui::build(app, settings.clone(), Rc::new(move || p.refresh()));
        let s = slot.clone();
        w.connect_close_request(move |_| {
            s.borrow_mut().take();
            glib::Propagation::Proceed
        });
        *slot.borrow_mut() = Some(w);
    }
    if let Some(w) = &*slot.borrow() {
        w.present();
    }
}
