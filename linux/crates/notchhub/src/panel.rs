//! The pill and the panel it opens into.
use gtk::glib;
use gtk::prelude::*;
use notchhub_core::settings::{Settings, Tab};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::halo::{self, Halo};
use crate::x11::X;

pub const PILL: (f64, f64) = (190.0, 26.0);
pub const PANEL: (f64, f64) = (520.0, 224.0);
const MARGIN: f64 = 36.0;
const RIBBON_H: f64 = 24.0;
const R_COLLAPSED: f64 = 9.0;
const R_EXPANDED: f64 = 24.0;

struct Anim {
    from: (f64, f64),
    to: (f64, f64),
    start: Instant,
}

pub struct Inner {
    win: gtk::ApplicationWindow,
    shape: gtk::Box,
    halo_area: gtk::DrawingArea,
    body: gtk::Box,
    ribbon: gtk::Label,
    collapsed: Cell<(f64, f64)>,
    toast_src: RefCell<Option<glib::SourceId>>,
    toast: RefCell<Option<String>>,
    lyric: RefCell<Option<String>>,
    stack: gtk::Stack,
    buttons: RefCell<Vec<(Tab, gtk::ToggleButton)>>,
    settings: Rc<RefCell<Settings>>,
    x: Option<X>,
    expanded: Cell<bool>,
    cur: Cell<(f64, f64)>,
    anim: RefCell<Option<Anim>>,
    open_src: RefCell<Option<glib::SourceId>>,
    close_src: RefCell<Option<glib::SourceId>>,
    anim_src: RefCell<Option<glib::SourceId>>,
    monitor: Cell<u32>,
    audio: crate::audio::AudioLevel,
    current: Cell<Tab>,
    on_drop: RefCell<std::collections::HashMap<Tab, Box<dyn Fn(Vec<std::path::PathBuf>)>>>,
    spring_src: RefCell<Option<glib::SourceId>>,
    on_open: RefCell<Option<Box<dyn Fn(bool)>>>,
    playing: Cell<bool>,
    level: Cell<f64>,
    t0: Instant,
}

#[derive(Clone)]
pub struct Panel(pub Rc<Inner>);

impl Panel {
    pub fn new(app: &gtk::Application, settings: Rc<RefCell<Settings>>) -> Panel {
        let win = gtk::ApplicationWindow::builder()
            .application(app)
            .decorated(false)
            .resizable(true)
            .title("NotchHub")
            .default_width((PILL.0 + 2.0 * MARGIN) as i32)
            .default_height((PILL.1 + MARGIN) as i32)
            .build();
        win.add_css_class("notchhub");

        let overlay = gtk::Overlay::new();
        let halo_area = gtk::DrawingArea::new();
        halo_area.set_hexpand(true);
        halo_area.set_vexpand(true);
        overlay.set_child(Some(&halo_area));

        let shape = gtk::Box::new(gtk::Orientation::Vertical, 0);
        shape.add_css_class("notch");
        shape.add_css_class("collapsed");
        shape.set_halign(gtk::Align::Center);
        shape.set_valign(gtk::Align::Start);
        shape.set_size_request(PILL.0 as i32, PILL.1 as i32);
        shape.set_overflow(gtk::Overflow::Hidden);
        overlay.add_overlay(&shape);

        let ribbon = gtk::Label::new(None);
        ribbon.add_css_class("ribbon");
        ribbon.set_margin_top(PILL.1 as i32);
        ribbon.set_height_request(RIBBON_H as i32);
        ribbon.set_ellipsize(gtk::pango::EllipsizeMode::End);
        ribbon.set_visible(false);
        shape.append(&ribbon);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.set_visible(false);
        let tabbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        tabbar.add_css_class("tabbar");
        tabbar.set_halign(gtk::Align::Center);
        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(120);
        body.append(&tabbar);
        body.append(&stack);
        shape.append(&body);
        win.set_child(Some(&overlay));

        let inner = Rc::new(Inner {
            win,
            shape,
            halo_area,
            body,
            ribbon,
            collapsed: Cell::new(PILL),
            toast_src: RefCell::new(None),
            toast: RefCell::new(None),
            lyric: RefCell::new(None),
            stack,
            buttons: RefCell::new(vec![]),
            settings,
            x: X::connect(),
            expanded: Cell::new(false),
            cur: Cell::new(PILL),
            anim: RefCell::new(None),
            open_src: RefCell::new(None),
            close_src: RefCell::new(None),
            anim_src: RefCell::new(None),
            monitor: Cell::new(0),
            audio: crate::audio::AudioLevel::new(),
            current: Cell::new(Tab::Shelf),
            on_drop: RefCell::new(std::collections::HashMap::new()),
            spring_src: RefCell::new(None),
            on_open: RefCell::new(None),
            playing: Cell::new(false),
            level: Cell::new(0.0),
            t0: Instant::now(),
        });
        let panel = Panel(inner);
        panel.build_tabs(&tabbar);
        panel.wire();
        panel
    }

    pub fn connect_open_changed(&self, f: impl Fn(bool) + 'static) {
        *self.0.on_open.borrow_mut() = Some(Box::new(f));
    }

    fn notify_open(&self, open: bool) {
        if let Some(f) = &*self.0.on_open.borrow() {
            f(open);
        }
    }

    /// Index of the monitor the panel is on (0 = main screen).
    pub fn monitor(&self) -> u32 {
        self.0.monitor.get()
    }

    /// Opens the panel on another screen (from its ball). It goes back to the main screen when it closes.
    pub fn open_on(&self, monitor: u32) {
        if self.0.expanded.get() {
            return;
        }
        self.0.monitor.set(monitor);
        self.open();
    }

    /// Top of the usable area and the pill's size, for the things that sit next to the pill (the pet).
    pub fn pill_rect(&self) -> (f64, f64) {
        PILL
    }

    pub fn current_tab(&self) -> Tab {
        self.0.current.get()
    }

    /// Files dropped on the pill or the open panel.
    pub fn connect_drop(&self, tab: Tab, f: impl Fn(Vec<std::path::PathBuf>) + 'static) {
        self.0.on_drop.borrow_mut().insert(tab, Box::new(f));
    }

    /// Files dropped: the tab that is showing takes them if it accepts files, otherwise the shelf does.
    fn handle_drop(&self, paths: Vec<std::path::PathBuf>) {
        let tab = if self.0.on_drop.borrow().contains_key(&self.0.current.get()) { self.0.current.get() } else { Tab::Shelf };
        self.select(tab);
        if let Some(f) = self.0.on_drop.borrow().get(&tab) {
            f(paths);
        }
    }

    pub fn is_open(&self) -> bool {
        self.0.expanded.get()
    }

    pub fn window(&self) -> &gtk::ApplicationWindow {
        &self.0.win
    }

    /// Page widget of a tab, so the features can fill it in.
    pub fn page(&self, tab: Tab) -> Option<gtk::Box> {
        self.0.stack.child_by_name(tab.label()).and_downcast::<gtk::Box>()
    }

    pub fn set_playing(&self, on: bool) {
        self.0.playing.set(on);
        self.0.halo_area.queue_draw();
    }

    pub fn is_playing(&self) -> bool {
        self.0.playing.get()
    }


    fn build_tabs(&self, tabbar: &gtk::Box) {
        let i = &self.0;
        let docker = notchhub_core::docker::installed();
        let tabs: Vec<Tab> = i.settings.borrow().visible_tabs().into_iter().filter(|t| *t != Tab::Docker || docker).collect();
        for t in Tab::ALL {
            let page = gtk::Box::new(gtk::Orientation::Vertical, 6);
            page.add_css_class("page");
            let l = gtk::Label::new(Some(t.label()));
            l.add_css_class("muted");
            l.set_vexpand(true);
            page.append(&l);
            i.stack.add_named(&page, Some(t.label()));
        }
        let mut group: Option<gtk::ToggleButton> = None;
        for t in tabs {
            let b = gtk::ToggleButton::new();
            b.set_label(t.label());
            b.set_tooltip_text(Some(t.label()));
            if let Some(g) = &group {
                b.set_group(Some(g));
            } else {
                group = Some(b.clone());
            }
            let me = self.clone();
            b.connect_clicked(move |_| me.select(t));
            // holding a file over a tab switches to it, so it can be dropped there
            let spring = gtk::DropTarget::new(gtk::gdk::FileList::static_type(), gtk::gdk::DragAction::COPY);
            let me = self.clone();
            spring.connect_enter(move |_, _, _| {
                if let Some(s) = me.0.spring_src.borrow_mut().take() {
                    s.remove();
                }
                let me2 = me.clone();
                let id = glib::timeout_add_local_once(Duration::from_millis(450), move || {
                    me2.0.spring_src.borrow_mut().take();
                    me2.select(t);
                });
                *me.0.spring_src.borrow_mut() = Some(id);
                gtk::gdk::DragAction::COPY
            });
            let me = self.clone();
            spring.connect_leave(move |_| {
                if let Some(s) = me.0.spring_src.borrow_mut().take() {
                    s.remove();
                }
            });
            let me = self.clone();
            spring.connect_drop(move |_, value, _, _| {
                let Ok(list) = value.get::<gtk::gdk::FileList>() else { return false };
                let paths: Vec<std::path::PathBuf> = list.files().iter().filter_map(|f| f.path()).collect();
                me.0.current.set(t);
                me.handle_drop(paths);
                true
            });
            b.add_controller(spring);
            tabbar.append(&b);
            i.buttons.borrow_mut().push((t, b));
        }
        let start = {
            let s = i.settings.borrow();
            let vis = s.visible_tabs();
            if s.remember_tab && vis.contains(&s.last_tab) && (s.last_tab != Tab::Docker || docker) { s.last_tab } else { vis[0] }
        };
        self.select(start);
    }

    pub fn select(&self, tab: Tab) {
        let i = &self.0;
        if i.stack.child_by_name(tab.label()).is_none() {
            return;
        }
        i.stack.set_visible_child_name(tab.label());
        i.current.set(tab);
        for (t, b) in i.buttons.borrow().iter() {
            b.set_active(*t == tab);
            if *t == tab { b.add_css_class("active") } else { b.remove_css_class("active") }
        }
        if i.settings.borrow().last_tab != tab {
            i.settings.borrow_mut().last_tab = tab;
            i.settings.borrow().save();
        }
    }

    fn wire(&self) {
        let i = &self.0;
        // pointer rests on the pill -> open; pointer leaves the open panel -> close
        let motion = gtk::EventControllerMotion::new();
        let me = self.clone();
        motion.connect_enter(move |_, _, _| me.pointer_in());
        let me = self.clone();
        motion.connect_leave(move |_| me.pointer_out());
        i.shape.add_controller(motion);

        // a file dragged onto the pill opens the panel
        let drop = gtk::DropTarget::new(gtk::gdk::FileList::static_type(), gtk::gdk::DragAction::COPY);
        let me = self.clone();
        drop.connect_enter(move |_, _, _| {
            me.pointer_in_now();
            gtk::gdk::DragAction::COPY
        });
        let me = self.clone();
        drop.connect_leave(move |_| me.pointer_out());
        let me = self.clone();
        drop.connect_drop(move |_, value, _, _| {
            let Ok(list) = value.get::<gtk::gdk::FileList>() else { return false };
            let paths: Vec<std::path::PathBuf> = list.files().iter().filter_map(|f| f.path()).collect();
            if paths.is_empty() {
                return false;
            }
            me.handle_drop(paths);
            true
        });
        i.shape.add_controller(drop);

        let keys = gtk::EventControllerKey::new();
        let me = self.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                me.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        i.win.add_controller(keys);

        let me = self.clone();
        i.halo_area.set_draw_func(move |_, cr, w, _| me.draw_halo(cr, w as f64));

        // pulse / beat redraw, only while the halo is visible
        let me = self.clone();
        glib::timeout_add_local(Duration::from_millis(33), move || {
            // the system audio is only listened to while the beat option is on AND something plays
            let beat = me.0.settings.borrow().halo_beat && me.0.playing.get();
            if beat {
                me.0.audio.start();
                me.0.level.set(me.0.audio.level());
            } else {
                me.0.audio.stop();
                me.0.level.set(0.0);
            }
            if me.halo_visible() {
                me.0.halo_area.queue_draw();
            }
            glib::ControlFlow::Continue
        });

        let me = self.clone();
        i.win.connect_realize(move |_| me.on_realize());
        let me = self.clone();
        i.win.connect_map(move |_| me.on_map());
    }

    fn halo_visible(&self) -> bool {
        let s = self.0.settings.borrow();
        s.halo_enabled && (self.0.playing.get() || s.halo_always)
    }

    fn draw_halo(&self, cr: &gtk::cairo::Context, width: f64) {
        let i = &self.0;
        cr.set_operator(gtk::cairo::Operator::Clear);
        let _ = cr.paint();
        cr.set_operator(gtk::cairo::Operator::Over);
        let k = ((i.cur.get().1 - PILL.1 - RIBBON_H) / (PANEL.1 - PILL.1 - RIBBON_H)).clamp(0.0, 1.0);
        let radius = R_COLLAPSED + (R_EXPANDED - R_COLLAPSED) * k;
        if self.halo_visible() {
            self.draw_glow(cr, width, radius);
        }
        halo::fill_shape(cr, width, i.cur.get(), radius);
    }

    fn draw_glow(&self, cr: &gtk::cairo::Context, width: f64, radius: f64) {
        let i = &self.0;
        let s = i.settings.borrow();
        let t = i.t0.elapsed().as_secs_f64();
        let pulse = 0.5 + 0.5 * (std::f64::consts::TAU * t / s.pulse_seconds.max(0.2)).sin();
        let beat = s.halo_beat && i.playing.get();
        let h = Halo {
            rgb: s.halo_rgb(),
            intensity: s.halo_intensity,
            pulse: if beat { 0.5 } else { pulse },
            level: if beat { i.level.get() } else { 0.0 },
        };
        halo::draw(cr, width, i.cur.get(), radius, &h);
    }

    // MARK: window placement

    fn xid(&self) -> Option<u32> {
        let surface = self.0.win.surface()?;
        surface.downcast_ref::<gdk4_x11::X11Surface>().map(|s| s.xid() as u32)
    }

    fn on_realize(&self) {
        if let (Some(x), Some(xid)) = (&self.0.x, self.xid()) {
            x.make_dock(xid);
        }
        self.place(false);
    }

    fn on_map(&self) {
        if let (Some(x), Some(xid)) = (&self.0.x, self.xid()) {
            x.request_above(xid);
        }
        self.place(self.0.expanded.get());
    }

    /// Moves the window to the top centre of the first monitor, sized for the pill or for the open panel.
    fn place(&self, expanded: bool) {
        let i = &self.0;
        let (w, h) = if expanded { PANEL } else { i.collapsed.get() };
        let (ww, wh) = ((w + 2.0 * MARGIN).round() as i32, (h + MARGIN).round() as i32);
        let Some(display) = gtk::gdk::Display::default() else { return };
        let Some(mon) = display.monitors().item(i.monitor.get()).or_else(|| display.monitors().item(0)).and_downcast::<gtk::gdk::Monitor>() else { return };
        let g = mon.geometry();
        let scale = mon.scale_factor().max(1);
        i.win.set_default_size(ww, wh);
        if let (Some(x), Some(xid)) = (&i.x, self.xid()) {
            // the desktop's top bar is only on the main screen
            let top = if i.monitor.get() != 0 { 0 } else { x.workarea_top() };
            let top = if i.monitor.get() == 0 && top == 0 && std::env::var("XDG_CURRENT_DESKTOP").map_or(false, |d| d.contains("GNOME")) { 32 * scale } else { top };
            let px = (g.x() + (g.width() - ww) / 2) * scale;
            x.place(xid, px, g.y() * scale + top, (ww * scale) as u32, (wh * scale) as u32);
        } else {
            i.win.set_default_size(ww, wh);
        }
        // only the pill (or the whole panel) takes the pointer; the glow margin lets clicks through
        if let Some(surface) = i.win.surface() {
            let rect = if expanded {
                gtk::cairo::RectangleInt::new(0, 0, ww, wh)
            } else {
                gtk::cairo::RectangleInt::new(((ww as f64 - w) / 2.0) as i32, 0, w as i32, h as i32)
            };
            surface.set_input_region(&gtk::cairo::Region::create_rectangle(&rect));
        }
    }

    // MARK: open / close

    fn pointer_in(&self) {
        let i = &self.0;
        if let Some(s) = i.close_src.borrow_mut().take() {
            s.remove();
        }
        if i.expanded.get() || i.open_src.borrow().is_some() {
            return;
        }
        let delay = (i.settings.borrow().open_delay * 1000.0) as u64;
        let me = self.clone();
        let id = glib::timeout_add_local_once(Duration::from_millis(delay.max(1)), move || {
            me.0.open_src.borrow_mut().take();
            me.open();
        });
        *i.open_src.borrow_mut() = Some(id);
    }

    fn pointer_in_now(&self) {
        if let Some(s) = self.0.open_src.borrow_mut().take() {
            s.remove();
        }
        if let Some(s) = self.0.close_src.borrow_mut().take() {
            s.remove();
        }
        self.open();
    }

    fn pointer_out(&self) {
        let i = &self.0;
        if let Some(s) = i.open_src.borrow_mut().take() {
            s.remove();
        }
        if !i.expanded.get() || i.close_src.borrow().is_some() {
            return;
        }
        let me = self.clone();
        let id = glib::timeout_add_local_once(Duration::from_millis(250), move || {
            me.0.close_src.borrow_mut().take();
            // a stray "leave" while the window was moving must not close it: look where the pointer really is
            if !me.pointer_over_shape() {
                me.close();
            }
        });
        *i.close_src.borrow_mut() = Some(id);
    }

    fn pointer_over_shape(&self) -> bool {
        let i = &self.0;
        let Some(surface) = i.win.surface() else { return false };
        let Some(pointer) = surface.display().default_seat().and_then(|s| s.pointer()) else { return false };
        let Some((x, y, _)) = surface.device_position(&pointer) else { return false };
        let (w, h) = i.cur.get();
        let left = (i.win.width() as f64 - w) / 2.0;
        x >= left && x <= left + w && y >= 0.0 && y <= h
    }

    pub fn toggle(&self) {
        if self.0.expanded.get() { self.close() } else { self.open() }
    }

    pub fn open(&self) {
        let i = &self.0;
        if i.expanded.get() {
            return;
        }
        i.expanded.set(true);
        self.notify_open(true);
        self.place(true);
        i.shape.remove_css_class("collapsed");
        i.shape.add_css_class("expanded");
        i.body.set_visible(true);
        self.animate_to(PANEL);
    }

    pub fn close(&self) {
        let i = &self.0;
        if !i.expanded.get() {
            return;
        }
        i.expanded.set(false);
        self.notify_open(false);
        i.body.set_visible(false);
        i.shape.remove_css_class("expanded");
        i.shape.add_css_class("collapsed");
        self.refresh_ribbon();
        self.animate_to(i.collapsed.get());
    }

    fn animate_to(&self, to: (f64, f64)) {
        let i = &self.0;
        if let Some(s) = i.anim_src.borrow_mut().take() {
            s.remove();
        }
        *i.anim.borrow_mut() = Some(Anim { from: i.cur.get(), to, start: Instant::now() });
        let me = self.clone();
        let id = glib::timeout_add_local(Duration::from_millis(16), move || me.step());
        *i.anim_src.borrow_mut() = Some(id);
    }

    fn step(&self) -> glib::ControlFlow {
        let i = &self.0;
        let (from, to, t) = match &*i.anim.borrow() {
            Some(a) => (a.from, a.to, (a.start.elapsed().as_secs_f64() / 0.22).min(1.0)),
            None => return glib::ControlFlow::Break,
        };
        let e = 1.0 - (1.0 - t).powi(3);
        let cur = (from.0 + (to.0 - from.0) * e, from.1 + (to.1 - from.1) * e);
        i.cur.set(cur);
        i.shape.set_size_request(cur.0.round() as i32, cur.1.round() as i32);
        i.halo_area.queue_draw();
        if t >= 1.0 {
            *i.anim.borrow_mut() = None;
            i.anim_src.borrow_mut().take();
            if !i.expanded.get() {
                let was = i.monitor.replace(0);
                self.place(false);
                if was != 0 {
                    self.notify_open(false);
                }
            }
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    }

    /// Text under the pill while it is closed: a short toast wins over the line being sung.
    pub fn set_lyric_line(&self, line: Option<String>) {
        *self.0.lyric.borrow_mut() = line.filter(|l| !l.is_empty());
        self.refresh_ribbon();
    }

    pub fn toast(&self, text: &str) {
        let i = &self.0;
        *i.toast.borrow_mut() = Some(text.to_string());
        if let Some(s) = i.toast_src.borrow_mut().take() {
            s.remove();
        }
        let me = self.clone();
        let id = glib::timeout_add_local_once(Duration::from_millis(3500), move || {
            me.0.toast_src.borrow_mut().take();
            *me.0.toast.borrow_mut() = None;
            me.refresh_ribbon();
        });
        *i.toast_src.borrow_mut() = Some(id);
        self.refresh_ribbon();
    }

    fn refresh_ribbon(&self) {
        let i = &self.0;
        let text = i.toast.borrow().clone().or_else(|| i.lyric.borrow().clone());
        let show = text.is_some() && !i.expanded.get();
        i.ribbon.set_text(text.as_deref().unwrap_or(""));
        i.ribbon.set_visible(show);
        let new = match (&text, show) {
            (Some(t), true) => {
                let w = i.ribbon.create_pango_layout(Some(t)).pixel_size().0 as f64;
                ((w + 36.0).clamp(PILL.0, 440.0), PILL.1 + RIBBON_H)
            }
            _ => PILL,
        };
        if i.expanded.get() || new == i.collapsed.get() {
            i.collapsed.set(new);
            return;
        }
        let growing = new.0 > i.collapsed.get().0 || new.1 > i.collapsed.get().1;
        i.collapsed.set(new);
        if growing {
            self.place(false);
        }
        self.animate_to(new);
    }

    /// Applies changed settings (halo, delays...).
    pub fn refresh(&self) {
        self.0.halo_area.queue_draw();
    }
}
