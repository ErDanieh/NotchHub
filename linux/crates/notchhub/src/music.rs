//! Music tab: what is playing (MPRIS), controls, progress, lyrics, the song toast and the halo state.
use gtk::glib;
use gtk::prelude::*;
use notchhub_core::lyrics::{self, Lyrics};
use notchhub_core::settings::{Settings, Tab};
use notchhub_mpris::{Command, Mpris, Track};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use crate::panel::Panel;
use crate::runtime::{on_rt, rt};

#[derive(Clone, PartialEq)]
enum LyricsState {
    Idle,
    Loading,
    Ready(Lyrics),
    NotFound,
    Offline,
}

struct Widgets {
    art: gtk::Picture,
    title: gtk::Label,
    artist: gtk::Label,
    play: gtk::Button,
    pos: gtk::Label,
    dur: gtk::Label,
    scale: gtk::Scale,
    l_prev: gtk::Label,
    l_now: gtk::Label,
    l_next: gtk::Label,
    empty: gtk::Label,
    content: gtk::Box,
}

struct State {
    track: Option<Track>,
    lyrics: LyricsState,
    lyrics_key: String,
    cache: HashMap<String, Option<Lyrics>>,
    art_url: Option<String>,
    dragging: bool,
    last_line: Option<usize>,
}

pub struct Music {
    w: Widgets,
    st: RefCell<State>,
    settings: Rc<RefCell<Settings>>,
    panel: Panel,
    mpris: Mpris,
    updating: Cell<bool>,
}

fn fmt(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn icon_button(icon: &str, tip: &str) -> gtk::Button {
    let b = gtk::Button::from_icon_name(icon);
    b.add_css_class("ctl");
    b.set_tooltip_text(Some(tip));
    b
}

pub fn setup(panel: &Panel, settings: Rc<RefCell<Settings>>) -> Option<Rc<Music>> {
    let page = panel.page(Tab::Music)?;
    while let Some(c) = page.first_child() {
        page.remove(&c);
    }
    let empty = gtk::Label::new(Some("Nothing is playing."));
    empty.add_css_class("subtitle");
    empty.set_vexpand(true);
    page.append(&empty);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.set_visible(false);
    content.set_vexpand(true);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let art = gtk::Picture::new();
    art.set_size_request(88, 88);
    art.set_content_fit(gtk::ContentFit::Cover);
    art.add_css_class("art");
    art.set_overflow(gtk::Overflow::Hidden);
    top.append(&art);
    let info = gtk::Box::new(gtk::Orientation::Vertical, 2);
    info.set_hexpand(true);
    info.set_valign(gtk::Align::Center);
    let title = gtk::Label::new(None);
    title.add_css_class("title");
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let artist = gtk::Label::new(None);
    artist.add_css_class("subtitle");
    artist.set_xalign(0.0);
    artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let ctl = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    let prev = icon_button("media-skip-backward-symbolic", "Previous");
    let play = icon_button("media-playback-pause-symbolic", "Play / pause");
    let next = icon_button("media-skip-forward-symbolic", "Next");
    let lyrics_btn = gtk::ToggleButton::new();
    lyrics_btn.set_icon_name("format-justify-center-symbolic");
    lyrics_btn.add_css_class("ctl");
    lyrics_btn.set_tooltip_text(Some("Lyrics"));
    lyrics_btn.set_active(settings.borrow().music_show_lyrics);
    ctl.append(&prev);
    ctl.append(&play);
    ctl.append(&next);
    ctl.append(&lyrics_btn);
    info.append(&title);
    info.append(&artist);
    info.append(&ctl);
    top.append(&info);
    content.append(&top);

    let prog = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let pos = gtk::Label::new(Some("0:00"));
    pos.add_css_class("time");
    let dur = gtk::Label::new(Some("0:00"));
    dur.add_css_class("time");
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.001);
    scale.set_hexpand(true);
    scale.set_draw_value(false);
    prog.append(&pos);
    prog.append(&scale);
    prog.append(&dur);
    content.append(&prog);

    let lyr = gtk::Box::new(gtk::Orientation::Vertical, 1);
    lyr.set_halign(gtk::Align::Center);
    let mk = |now: bool| {
        let l = gtk::Label::new(None);
        l.add_css_class("lyric");
        if now {
            l.add_css_class("now");
        }
        l.set_ellipsize(gtk::pango::EllipsizeMode::End);
        l.set_max_width_chars(60);
        l
    };
    let (l_prev, l_now, l_next) = (mk(false), mk(true), mk(false));
    lyr.append(&l_prev);
    lyr.append(&l_now);
    lyr.append(&l_next);
    lyr.set_visible(settings.borrow().music_show_lyrics);
    content.append(&lyr);
    page.append(&content);

    let _guard = rt().enter();
    let mpris = notchhub_mpris::spawn();
    let me = Rc::new(Music {
        w: Widgets { art, title, artist, play: play.clone(), pos, dur, scale: scale.clone(), l_prev, l_now, l_next, empty, content },
        st: RefCell::new(State { track: None, lyrics: LyricsState::Idle, lyrics_key: String::new(), cache: HashMap::new(), art_url: None, dragging: false, last_line: None }),
        settings,
        panel: panel.clone(),
        mpris,
        updating: Cell::new(false),
    });

    let send = |m: &Rc<Music>, c: Command| {
        let _ = m.mpris.commands.send(c);
    };
    let m = me.clone();
    prev.connect_clicked(move |_| send(&m, Command::Previous));
    let m = me.clone();
    play.connect_clicked(move |_| send(&m, Command::PlayPause));
    let m = me.clone();
    next.connect_clicked(move |_| send(&m, Command::Next));

    let m = me.clone();
    lyrics_btn.connect_toggled(move |b| {
        m.settings.borrow_mut().music_show_lyrics = b.is_active();
        m.settings.borrow().save();
        lyr.set_visible(b.is_active());
        m.load_lyrics();
        m.tick();
    });

    // dragging the bar seeks when released
    let m = me.clone();
    scale.connect_change_value(move |_, _, v| {
        if m.updating.get() {
            return glib::Propagation::Proceed;
        }
        m.st.borrow_mut().dragging = true;
        let dur = m.st.borrow().track.as_ref().map_or(0.0, |t| t.duration);
        m.w.pos.set_text(&fmt(v.clamp(0.0, 1.0) * dur));
        glib::Propagation::Proceed
    });
    let click = gtk::GestureClick::new();
    let m = me.clone();
    click.connect_released(move |_, _, _, _| {
        let (dragging, dur) = {
            let st = m.st.borrow();
            (st.dragging, st.track.as_ref().map_or(0.0, |t| t.duration))
        };
        if dragging && dur > 0.0 {
            let _ = m.mpris.commands.send(Command::Seek(m.w.scale.value() * dur));
        }
        m.st.borrow_mut().dragging = false;
    });
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    scale.add_controller(click);

    // new readings from the players
    let mut rx = me.mpris.tracks.clone();
    let m = me.clone();
    glib::spawn_future_local(async move {
        while rx.changed().await.is_ok() {
            let t = rx.borrow_and_update().clone();
            m.on_track(t);
        }
    });
    // progress, live lyric line
    let m = me.clone();
    glib::timeout_add_local(Duration::from_millis(250), move || {
        m.tick();
        glib::ControlFlow::Continue
    });
    Some(me)
}

impl Music {
    /// The panel opened or closed: poll faster while it is open.
    pub fn set_open(&self, open: bool) {
        let _ = self.mpris.commands.send(Command::Fast(open));
    }

    fn on_track(self: &Rc<Self>, t: Option<Track>) {
        let (new_song, had_track) = {
            let st = self.st.borrow();
            let new = match (&st.track, &t) {
                (Some(a), Some(b)) => !a.same_song(b),
                (None, Some(_)) => true,
                _ => false,
            };
            (new, st.track.is_some())
        };
        self.panel.set_playing(t.as_ref().map_or(false, |x| x.playing));
        self.st.borrow_mut().track = t.clone();
        match &t {
            None => {
                self.w.content.set_visible(false);
                self.w.empty.set_visible(true);
                self.panel.set_lyric_line(None);
            }
            Some(t) => {
                self.w.content.set_visible(true);
                self.w.empty.set_visible(false);
                self.w.title.set_text(&t.title);
                self.w.artist.set_text(&if t.album.is_empty() { t.artist.clone() } else { format!("{} · {}", t.artist, t.album) });
                self.w.play.set_icon_name(if t.playing { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" });
                self.w.dur.set_text(&fmt(t.duration));
                self.w.scale.set_sensitive(t.can_seek && t.duration > 0.0);
                self.load_art(t.art_url.clone());
                if new_song {
                    self.st.borrow_mut().last_line = None;
                    self.load_lyrics();
                    if had_track && t.playing && self.settings.borrow().alerts_song {
                        self.panel.toast(&format!("♪ {} — {}", t.title, t.artist));
                    }
                }
            }
        }
        self.tick();
    }

    fn load_art(self: &Rc<Self>, url: Option<String>) {
        if self.st.borrow().art_url == url {
            return;
        }
        self.st.borrow_mut().art_url = url.clone();
        let Some(url) = url else {
            self.w.art.set_paintable(None::<&gtk::gdk::Paintable>);
            return;
        };
        let m = self.clone();
        glib::spawn_future_local(async move {
            let u = url.clone();
            let bytes = on_rt(async move {
                if let Some(path) = u.strip_prefix("file://") {
                    std::fs::read(path).ok()
                } else {
                    let r = reqwest_get(&u).await?;
                    Some(r)
                }
            })
            .await
            .flatten();
            if m.st.borrow().art_url.as_deref() != Some(url.as_str()) {
                return;
            }
            if let Some(b) = bytes {
                if let Ok(tex) = gtk::gdk::Texture::from_bytes(&glib::Bytes::from_owned(b)) {
                    m.w.art.set_paintable(Some(&tex));
                }
            }
        });
    }

    fn load_lyrics(self: &Rc<Self>) {
        if !self.settings.borrow().music_show_lyrics {
            return;
        }
        let Some(t) = self.st.borrow().track.clone() else { return };
        let key = format!("{}|{}", t.title, t.artist);
        if self.st.borrow().lyrics_key == key {
            return;
        }
        {
            let mut st = self.st.borrow_mut();
            st.lyrics_key = key.clone();
            st.last_line = None;
            if let Some(hit) = st.cache.get(&key) {
                st.lyrics = hit.clone().map_or(LyricsState::NotFound, LyricsState::Ready);
                return;
            }
            st.lyrics = LyricsState::Loading;
        }
        let m = self.clone();
        glib::spawn_future_local(async move {
            let (title, artist, dur) = (t.title.clone(), t.artist.clone(), t.duration);
            let res = on_rt(async move { lyrics::fetch(&title, &artist, dur).await }).await;
            let mut st = m.st.borrow_mut();
            if st.lyrics_key != key {
                return; // the song changed while we were waiting
            }
            match res {
                Some(Ok(l)) => {
                    st.cache.insert(key, l.clone());
                    st.lyrics = l.map_or(LyricsState::NotFound, LyricsState::Ready);
                }
                _ => st.lyrics = LyricsState::Offline,
            }
            st.last_line = None;
        });
    }

    fn tick(&self) {
        let st = self.st.borrow();
        let Some(t) = &st.track else { return };
        let pos = t.estimated_position();
        if !st.dragging && t.duration > 0.0 {
            self.updating.set(true);
            self.w.scale.set_value((pos / t.duration).clamp(0.0, 1.0));
            self.updating.set(false);
            self.w.pos.set_text(&fmt(pos));
        }
        let on = self.settings.borrow().music_show_lyrics;
        let lead = self.settings.borrow().lyrics_lead;
        if !on {
            drop(st);
            self.panel.set_lyric_line(None);
            return;
        }
        let (prev, now, next, line): (String, String, String, Option<usize>) = match &st.lyrics {
            LyricsState::Ready(l) if l.instrumental => (String::new(), "This song is instrumental.".into(), String::new(), None),
            LyricsState::Ready(l) if !l.synced.is_empty() => {
                let idx = lyrics::current_index(&l.synced, pos + lead);
                let get = |i: Option<usize>| i.and_then(|i| l.synced.get(i)).map(|x| x.text.clone()).unwrap_or_default();
                (get(idx.and_then(|i| i.checked_sub(1))), get(idx), get(Some(idx.map_or(0, |i| i + 1))), idx)
            }
            LyricsState::Ready(l) => (String::new(), l.plain.lines().next().unwrap_or("").into(), String::new(), None),
            LyricsState::Loading | LyricsState::Idle => (String::new(), "Looking for the lyrics…".into(), String::new(), None),
            LyricsState::NotFound => (String::new(), "No lyrics found for this song.".into(), String::new(), None),
            LyricsState::Offline => (String::new(), "Offline. Could not look for the lyrics.".into(), String::new(), None),
        };
        let synced = matches!(&st.lyrics, LyricsState::Ready(l) if !l.synced.is_empty());
        drop(st);
        self.w.l_prev.set_text(&prev);
        self.w.l_now.set_text(&now);
        self.w.l_next.set_text(&next);
        let _ = line;
        self.panel.set_lyric_line(if synced && t_playing(&self.st) { Some(now) } else { None });
    }
}

fn t_playing(st: &RefCell<State>) -> bool {
    st.borrow().track.as_ref().map_or(false, |t| t.playing)
}

async fn reqwest_get(url: &str) -> Option<Vec<u8>> {
    let c = reqwest_client()?;
    let r = c.get(url).send().await.ok()?;
    if !r.status().is_success() {
        return None;
    }
    r.bytes().await.ok().map(|b| b.to_vec())
}

fn reqwest_client() -> Option<notchhub_core::http::Client> {
    notchhub_core::http::client()
}
