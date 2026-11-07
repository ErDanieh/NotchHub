//! News tab and the window to manage categories and sources.
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use notchhub_core::news::Feed;
use notchhub_core::newsfeed::{self, NewsItem};
use notchhub_core::settings::{Settings, Tab};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use crate::panel::Panel;
use crate::runtime::{on_rt, rt};
use crate::ui;

pub struct News {
    settings: Rc<RefCell<Settings>>,
    app: gtk::Application,
    by_feed: RefCell<HashMap<String, Vec<NewsItem>>>,
    view: RefCell<HashMap<String, Vec<NewsItem>>>,
    updated: Cell<Option<i64>>,
    loading: Cell<bool>,
    pending: Cell<bool>,
    cache: PathBuf,
    // widgets
    chips: gtk::Box,
    list: gtk::Box,
    msg: gtk::Label,
    status: gtk::Label,
    spinner: gtk::Spinner,
    manager: RefCell<Option<gtk::Window>>,
    manager_selected: RefCell<String>,
    pending_change: RefCell<Option<glib::SourceId>>,
}

fn now() -> i64 {
    glib::real_time() / 1_000_000
}

fn ago(ts: i64) -> String {
    let d = (now() - ts).max(0);
    match d {
        0..=59 => "now".into(),
        60..=3599 => format!("{} min", d / 60),
        3600..=86399 => format!("{} h", d / 3600),
        _ => format!("{} d", d / 86400),
    }
}

thread_local! {
    static TEXTURES: RefCell<HashMap<String, gdk::Texture>> = RefCell::new(HashMap::new());
}

async fn texture(url: String) -> Option<gdk::Texture> {
    if let Some(t) = TEXTURES.with(|m| m.borrow().get(&url).cloned()) {
        return Some(t);
    }
    let u = url.clone();
    let bytes = on_rt(async move {
        let c = notchhub_core::http::client()?;
        let r = c.get(&u).send().await.ok()?;
        if !r.status().is_success() {
            return None;
        }
        r.bytes().await.ok().map(|b| b.to_vec())
    })
    .await
    .flatten()?;
    // centre-crop to a square so every thumbnail has the same size
    let stream = gtk::gio::MemoryInputStream::from_bytes(&glib::Bytes::from_owned(bytes));
    let pb = gtk::gdk_pixbuf::Pixbuf::from_stream(&stream, gtk::gio::Cancellable::NONE).ok()?;
    let side = pb.width().min(pb.height());
    let sq = pb.new_subpixbuf((pb.width() - side) / 2, (pb.height() - side) / 2, side, side);
    let sq = sq.scale_simple(96, 96, gtk::gdk_pixbuf::InterpType::Bilinear)?;
    let t = gdk::Texture::for_pixbuf(&sq);
    TEXTURES.with(|m| m.borrow_mut().insert(url, t.clone()));
    Some(t)
}

pub fn setup(panel: &Panel, settings: Rc<RefCell<Settings>>, app: &gtk::Application) -> Option<Rc<News>> {
    let page = ui::fresh_page(panel, Tab::News)?;
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let chips = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let chips_scroll = gtk::ScrolledWindow::new();
    chips_scroll.set_hexpand(true);
    chips_scroll.set_vscrollbar_policy(gtk::PolicyType::Never);
    chips_scroll.set_child(Some(&chips));
    top.append(&chips_scroll);
    let spinner = gtk::Spinner::new();
    let status = ui::muted("");
    let refresh = ui::icon_button("view-refresh-symbolic", "Refresh");
    let gear = ui::icon_button("emblem-system-symbolic", "Categories and sources");
    top.append(&spinner);
    top.append(&status);
    top.append(&refresh);
    top.append(&gear);
    page.append(&top);
    let msg = ui::muted("");
    msg.set_vexpand(true);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.append(&msg);
    page.append(&ui::scrolled(&list));

    let cache = Settings::data_dir().join("news-v2.json");
    let news = Rc::new(News {
        settings,
        app: app.clone(),
        by_feed: RefCell::new(HashMap::new()),
        view: RefCell::new(HashMap::new()),
        updated: Cell::new(None),
        loading: Cell::new(false),
        pending: Cell::new(false),
        cache,
        chips,
        list,
        msg,
        status,
        spinner,
        manager: RefCell::new(None),
        manager_selected: RefCell::new(String::new()),
        pending_change: RefCell::new(None),
    });
    news.load_cache();
    news.rebuild();
    let n = news.clone();
    refresh.connect_clicked(move |_| n.refresh());
    let n = news.clone();
    gear.connect_clicked(move |_| n.show_manager(None));

    // refresh when the tab opens (if the data is older than 10 minutes) and every 20 minutes
    let (n, p) = (news.clone(), panel.clone());
    let was = Cell::new(false);
    glib::timeout_add_local(Duration::from_millis(500), move || {
        let visible = p.is_open() && p.current_tab() == Tab::News;
        if visible && !was.get() && n.updated.get().map_or(true, |u| now() - u > 600) {
            n.refresh();
        }
        was.set(visible);
        glib::ControlFlow::Continue
    });
    let n = news.clone();
    glib::timeout_add_local(Duration::from_secs(20 * 60), move || {
        n.refresh();
        glib::ControlFlow::Continue
    });
    Some(news)
}

impl News {
    fn load_cache(&self) {
        #[derive(serde::Deserialize)]
        struct Cache {
            by_feed: HashMap<String, Vec<NewsItem>>,
            updated: i64,
        }
        if let Some(c) = std::fs::read_to_string(&self.cache).ok().and_then(|t| serde_json::from_str::<Cache>(&t).ok()) {
            *self.by_feed.borrow_mut() = c.by_feed;
            self.updated.set(Some(c.updated));
        }
    }

    fn save_cache(&self) {
        #[derive(serde::Serialize)]
        struct Cache<'a> {
            by_feed: &'a HashMap<String, Vec<NewsItem>>,
            updated: i64,
        }
        if let (Some(u), Some(d)) = (self.updated.get(), self.cache.parent()) {
            let _ = std::fs::create_dir_all(d);
            if let Ok(t) = serde_json::to_string(&Cache { by_feed: &self.by_feed.borrow(), updated: u }) {
                let _ = std::fs::write(&self.cache, t);
            }
        }
    }

    pub fn refresh(self: &Rc<Self>) {
        if self.loading.get() {
            self.pending.set(true);
            return;
        }
        self.loading.set(true);
        self.spinner.start();
        let feeds: Vec<Feed> = self.settings.borrow().news.feeds.iter().filter(|f| f.enabled).cloned().collect();
        let me = self.clone();
        glib::spawn_future_local(async move {
            let results = on_rt(async move {
                let Some(client) = notchhub_core::http::client() else { return vec![] };
                let t = now();
                let handles: Vec<_> = feeds
                    .into_iter()
                    .map(|f| {
                        let c = client.clone();
                        tokio::spawn(async move { (f.url.clone(), newsfeed::fetch_feed(&c, &f, t).await) })
                    })
                    .collect();
                let mut out = vec![];
                for h in handles {
                    if let Ok(r) = h.await {
                        out.push(r);
                    }
                }
                out
            })
            .await
            .unwrap_or_default();
            // a source that failed this time keeps its previous stories
            let any = results.iter().any(|(_, r)| r.is_some());
            for (url, r) in results {
                if let Some(items) = r {
                    me.by_feed.borrow_mut().insert(url, items);
                }
            }
            if any {
                me.updated.set(Some(now()));
                me.save_cache();
            }
            me.loading.set(false);
            me.spinner.stop();
            me.rebuild();
            if me.pending.replace(false) {
                me.refresh();
            }
        });
    }

    /// Called after categories or sources changed: show the effect now, reload a moment later.
    pub fn config_changed(self: &Rc<Self>) {
        self.settings.borrow().save();
        self.rebuild();
        if let Some(s) = self.pending_change.borrow_mut().take() {
            s.remove();
        }
        let me = self.clone();
        let id = glib::timeout_add_local_once(Duration::from_secs(1), move || {
            me.pending_change.borrow_mut().take();
            me.refresh();
        });
        *self.pending_change.borrow_mut() = Some(id);
    }

    fn rebuild(self: &Rc<Self>) {
        let cfg = self.settings.borrow().news.clone();
        *self.view.borrow_mut() = newsfeed::build_view(&cfg, &self.by_feed.borrow());
        // chips
        while let Some(c) = self.chips.first_child() {
            self.chips.remove(&c);
        }
        let cats = cfg.visible_categories();
        let mut selected = self.settings.borrow().news_section.clone();
        if !cats.iter().any(|c| c.id == selected) {
            selected = cats.first().map(|c| c.id.clone()).unwrap_or_default();
        }
        for c in &cats {
            let b = gtk::Button::with_label(&c.name);
            b.add_css_class("ctl");
            b.add_css_class("chipbtn");
            if c.id == selected {
                b.add_css_class("active");
            }
            let (me, id) = (self.clone(), c.id.clone());
            b.connect_clicked(move |_| {
                me.settings.borrow_mut().news_section = id.clone();
                me.settings.borrow().save();
                me.rebuild();
            });
            self.chips.append(&b);
        }
        // stories
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        let items = self.view.borrow().get(&selected).cloned().unwrap_or_default();
        let category = cats.iter().find(|c| c.id == selected);
        let has_sources = category.map_or(false, |c| c.parent.is_some() || cfg.feeds_in(&c.id).any(|f| f.enabled));
        self.status.set_text(&self.updated.get().map(ago).unwrap_or_default());
        if items.is_empty() {
            let text = if !has_sources {
                "This category has no sources yet. Use the gear to add some."
            } else if self.loading.get() {
                "Loading the news…"
            } else if self.updated.get().is_none() {
                "No connection. The news could not be loaded."
            } else if category.map_or(false, |c| c.parent.is_some()) {
                "No stories about this topic right now."
            } else {
                "No stories."
            };
            self.msg.set_text(text);
            self.msg.set_visible(true);
            return;
        }
        self.msg.set_visible(false);
        for it in items {
            self.list.append(&self.row(&it));
        }
    }

    fn row(&self, it: &NewsItem) -> gtk::Widget {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.add_css_class("row");
        let pic = gtk::Image::new();
        pic.set_pixel_size(44);
        pic.set_overflow(gtk::Overflow::Hidden);
        pic.add_css_class("art");
        pic.set_valign(gtk::Align::Center);
        pic.set_icon_name(Some("image-x-generic-symbolic"));
        if let Some(url) = it.image.clone() {
            let pic = pic.clone();
            glib::spawn_future_local(async move {
                if let Some(t) = texture(url).await {
                    pic.set_paintable(Some(&t));
                }
            });
        }
        row.append(&pic);
        let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
        col.set_hexpand(true);
        let t = gtk::Label::new(Some(&it.title));
        t.set_xalign(0.0);
        t.set_wrap(true);
        t.set_lines(2);
        t.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let s = ui::muted(&format!("{} · {}", it.source, ago(it.date)));
        s.set_xalign(0.0);
        col.append(&t);
        col.append(&s);
        row.append(&col);
        let link = it.link.clone();
        let click = gtk::GestureClick::new();
        click.connect_released(move |_, _, _, _| {
            let _ = gtk::gio::AppInfo::launch_default_for_uri(&link, None::<&gtk::gio::AppLaunchContext>);
        });
        row.add_controller(click);
        row.set_cursor_from_name(Some("pointer"));
        row.upcast()
    }

    // MARK: manager window

    pub fn show_manager(self: &Rc<Self>, select: Option<String>) {
        if let Some(s) = select {
            *self.manager_selected.borrow_mut() = s;
        } else if self.manager_selected.borrow().is_empty() {
            *self.manager_selected.borrow_mut() = self.settings.borrow().news_section.clone();
        }
        if self.manager.borrow().is_none() {
            let win = gtk::Window::builder().application(&self.app).title("News categories and sources").default_width(720).default_height(480).build();
            let me = self.clone();
            win.connect_close_request(move |_| {
                me.manager.borrow_mut().take();
                glib::Propagation::Proceed
            });
            *self.manager.borrow_mut() = Some(win);
        }
        self.refresh_manager();
        if let Some(w) = &*self.manager.borrow() {
            w.present();
        }
    }

    fn refresh_manager(self: &Rc<Self>) {
        let Some(win) = self.manager.borrow().clone() else { return };
        let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
        paned.set_position(210);
        paned.set_shrink_start_child(false);
        paned.set_resize_start_child(false);
        paned.set_start_child(Some(&self.manager_sidebar()));
        paned.set_end_child(Some(&self.manager_detail()));
        win.set_child(Some(&paned));
    }

    fn manager_sidebar(self: &Rc<Self>) -> gtk::Box {
        let side = gtk::Box::new(gtk::Orientation::Vertical, 6);
        side.set_margin_top(10);
        side.set_margin_bottom(10);
        side.set_margin_start(10);
        side.set_margin_end(6);
        let h = gtk::Label::new(Some("CATEGORIES"));
        h.add_css_class("dim-label");
        h.set_xalign(0.0);
        side.append(&h);
        let list = gtk::ListBox::new();
        list.add_css_class("navigation-sidebar");
        let cats = self.settings.borrow().news.categories.clone();
        let selected = self.manager_selected.borrow().clone();
        for c in &cats {
            let row = gtk::ListBoxRow::new();
            let l = gtk::Label::new(Some(&format!("{}{}", if c.parent.is_some() { "⛛ " } else { "" }, c.name)));
            l.set_xalign(0.0);
            l.set_margin_top(6);
            l.set_margin_bottom(6);
            l.set_margin_start(8);
            row.set_child(Some(&l));
            row.set_widget_name(&c.id);
            list.append(&row);
            if c.id == selected {
                list.select_row(Some(&row));
            }
        }
        let me = self.clone();
        list.connect_row_selected(move |_, row| {
            if let Some(r) = row {
                let id = r.widget_name().to_string();
                if *me.manager_selected.borrow() != id {
                    *me.manager_selected.borrow_mut() = id;
                    me.refresh_manager();
                }
            }
        });
        let sc = gtk::ScrolledWindow::new();
        sc.set_vexpand(true);
        sc.set_child(Some(&list));
        side.append(&sc);
        let add = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("New category"));
        entry.set_hexpand(true);
        let plus = gtk::Button::from_icon_name("list-add-symbolic");
        add.append(&entry);
        add.append(&plus);
        side.append(&add);
        let restore = gtk::Button::with_label("Restore default sources");
        restore.add_css_class("flat");
        restore.set_tooltip_text(Some("Brings back the categories and sources that came with NotchHub, without touching yours"));
        side.append(&restore);
        let (me, e) = (self.clone(), entry.clone());
        let create = move || {
            let name = e.text().trim().to_string();
            if name.is_empty() {
                return;
            }
            let id = me.settings.borrow_mut().news.add_category(&name);
            *me.manager_selected.borrow_mut() = id;
            e.set_text("");
            me.config_changed();
            me.refresh_manager();
        };
        let c2 = create.clone();
        plus.connect_clicked(move |_| create());
        entry.connect_activate(move |_| c2());
        let me = self.clone();
        restore.connect_clicked(move |_| {
            me.settings.borrow_mut().news.restore_defaults();
            me.config_changed();
            me.refresh_manager();
        });
        side
    }

    fn manager_detail(self: &Rc<Self>) -> gtk::Widget {
        let col = gtk::Box::new(gtk::Orientation::Vertical, 12);
        col.set_margin_top(14);
        col.set_margin_bottom(14);
        col.set_margin_start(16);
        col.set_margin_end(16);
        let id = self.manager_selected.borrow().clone();
        let Some(cat) = self.settings.borrow().news.categories.iter().find(|c| c.id == id).cloned() else {
            col.append(&gtk::Label::new(Some("Pick a category on the left, or create a new one.")));
            return col.upcast();
        };
        // header: name + delete
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let name = gtk::Entry::new();
        name.set_text(&cat.name);
        name.set_hexpand(true);
        name.add_css_class("title-3");
        let (me, cid) = (self.clone(), cat.id.clone());
        name.connect_changed(move |e| {
            let t = e.text().to_string();
            if let Some(c) = me.settings.borrow_mut().news.categories.iter_mut().find(|c| c.id == cid) {
                c.name = t;
            }
            me.settings.borrow().save();
            me.rebuild();
        });
        let del = gtk::Button::with_label("Delete category");
        del.add_css_class("destructive-action");
        head.append(&name);
        head.append(&del);
        col.append(&head);
        let (me, cid, cname) = (self.clone(), cat.id.clone(), cat.name.clone());
        del.connect_clicked(move |b| {
            let dialog = gtk::AlertDialog::builder()
                .message(format!("Delete \"{cname}\" and its sources?"))
                .buttons(["Cancel", "Delete"])
                .cancel_button(0)
                .default_button(0)
                .build();
            let (me, cid) = (me.clone(), cid.clone());
            let win = b.root().and_downcast::<gtk::Window>();
            dialog.choose(win.as_ref(), gtk::gio::Cancellable::NONE, move |r| {
                if r == Ok(1) {
                    me.settings.borrow_mut().news.delete_category(&cid);
                    *me.manager_selected.borrow_mut() = me.settings.borrow().news.categories.first().map(|c| c.id.clone()).unwrap_or_default();
                    me.config_changed();
                    me.refresh_manager();
                }
            });
        });

        if let Some(parent) = &cat.parent {
            let pname = self.settings.borrow().news.categories.iter().find(|c| &c.id == parent).map(|c| c.name.clone()).unwrap_or_default();
            let l = gtk::Label::new(Some(&format!("This tab has no sources of its own. It shows only the stories about artificial intelligence from \"{pname}\".")));
            l.set_wrap(true);
            l.set_xalign(0.0);
            col.append(&l);
            return col.upcast();
        }
        let h = gtk::Label::new(Some("SOURCES IN THIS CATEGORY"));
        h.add_css_class("dim-label");
        h.set_xalign(0.0);
        col.append(&h);
        let feeds: Vec<Feed> = self.settings.borrow().news.feeds_in(&cat.id).cloned().collect();
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        if feeds.is_empty() {
            col.append(&gtk::Label::new(Some("Nothing here yet. Add the first source below.")));
        }
        for f in feeds {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.set_margin_top(6);
            row.set_margin_bottom(6);
            row.set_margin_start(8);
            row.set_margin_end(8);
            let sw = gtk::Switch::new();
            sw.set_active(f.enabled);
            sw.set_valign(gtk::Align::Center);
            let (me, url) = (self.clone(), f.url.clone());
            sw.connect_state_set(move |_, v| {
                if let Some(x) = me.settings.borrow_mut().news.feeds.iter_mut().find(|x| x.url == url) {
                    x.enabled = v;
                }
                me.config_changed();
                glib::Propagation::Proceed
            });
            let info = gtk::Box::new(gtk::Orientation::Vertical, 1);
            info.set_hexpand(true);
            let n = gtk::Label::new(Some(&f.name));
            n.set_xalign(0.0);
            let u = gtk::Label::new(Some(&f.url));
            u.set_xalign(0.0);
            u.add_css_class("dim-label");
            u.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            info.append(&n);
            info.append(&u);
            let trash = gtk::Button::from_icon_name("user-trash-symbolic");
            trash.add_css_class("flat");
            trash.set_tooltip_text(Some("Remove this source"));
            let (me, url) = (self.clone(), f.url.clone());
            trash.connect_clicked(move |_| {
                me.settings.borrow_mut().news.feeds.retain(|x| x.url != url);
                me.config_changed();
                me.refresh_manager();
            });
            row.append(&sw);
            row.append(&info);
            row.append(&trash);
            list.append(&row);
        }
        let sc = gtk::ScrolledWindow::new();
        sc.set_vexpand(true);
        sc.set_child(Some(&list));
        col.append(&sc);

        // add a source
        let frame = gtk::Frame::new(Some("Add a source"));
        let form = gtk::Box::new(gtk::Orientation::Vertical, 8);
        form.set_margin_top(10);
        form.set_margin_bottom(10);
        form.set_margin_start(10);
        form.set_margin_end(10);
        let url = gtk::Entry::new();
        url.set_placeholder_text(Some("Website or RSS address, e.g. https://www.informacion.es"));
        let row2 = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let nm = gtk::Entry::new();
        nm.set_placeholder_text(Some("Name (optional)"));
        nm.set_hexpand(true);
        let add = gtk::Button::with_label("Add");
        add.add_css_class("suggested-action");
        let spin = gtk::Spinner::new();
        row2.append(&nm);
        row2.append(&add);
        row2.append(&spin);
        let hint = gtk::Label::new(Some("Just paste the web address. NotchHub finds the news feed by itself."));
        hint.add_css_class("dim-label");
        hint.set_xalign(0.0);
        hint.set_wrap(true);
        form.append(&url);
        form.append(&row2);
        form.append(&hint);
        frame.set_child(Some(&form));
        col.append(&frame);
        let me = self.clone();
        let cid = cat.id.clone();
        let (url2, nm2, hint2, spin2, add2) = (url.clone(), nm.clone(), hint.clone(), spin.clone(), add.clone());
        let go = move || {
            let mut u = url2.text().trim().to_string();
            if u.is_empty() {
                return;
            }
            if !u.contains("://") {
                u = format!("https://{u}");
            }
            let Some(host) = url::Url::parse(&u).ok().and_then(|p| p.host_str().map(|h| h.trim_start_matches("www.").to_string())) else {
                hint2.set_text("That is not a valid address.");
                hint2.remove_css_class("dim-label");
                hint2.add_css_class("error");
                return;
            };
            let n = nm2.text().trim().to_string();
            let label = if n.is_empty() { host } else { n };
            spin2.start();
            add2.set_sensitive(false);
            hint2.set_text("Looking for the feed…");
            hint2.remove_css_class("error");
            let (me, cid, hint, spin, add, url, nm) = (me.clone(), cid.clone(), hint2.clone(), spin2.clone(), add2.clone(), url2.clone(), nm2.clone());
            glib::spawn_future_local(async move {
                let found = on_rt(async move {
                    let c = notchhub_core::http::client()?;
                    newsfeed::discover(&c, &u, &cid, &label, now()).await
                })
                .await
                .flatten();
                spin.stop();
                add.set_sensitive(true);
                match found {
                    None => {
                        hint.set_text("No news feed found at that address. Look for an RSS link on the site and paste that instead.");
                        hint.add_css_class("error");
                    }
                    Some((feed, items)) => {
                        let res = me.settings.borrow_mut().news.add_feed(feed.clone());
                        match res {
                            Err(other) => {
                                hint.set_text(&format!("You already have that source in \"{other}\"."));
                                hint.add_css_class("error");
                            }
                            Ok(()) => {
                                me.by_feed.borrow_mut().insert(feed.url, items);
                                url.set_text("");
                                nm.set_text("");
                                me.config_changed();
                                me.refresh_manager();
                            }
                        }
                    }
                }
            });
        };
        let go2 = go.clone();
        add.connect_clicked(move |_| go());
        url.connect_activate(move |_| go2());
        col.upcast()
    }
}

#[allow(dead_code)]
fn _unused() {
    let _ = rt();
}
