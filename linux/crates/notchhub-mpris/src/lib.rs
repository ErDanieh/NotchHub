//! Now playing through MPRIS, the D-Bus interface that Spotify, browsers (YouTube, YouTube Music), VLC and
//! most Linux players expose. One code path for all of them.
use futures_util::StreamExt;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, Proxy};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    /// Bus name of the player, e.g. `org.mpris.MediaPlayer2.spotify`.
    pub bus: String,
    pub app: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art_url: Option<String>,
    /// Seconds; 0 when unknown.
    pub duration: f64,
    /// Position in seconds at the moment of `sampled`.
    pub position: f64,
    pub playing: bool,
    pub can_seek: bool,
    pub track_id: String,
    pub sampled: Instant,
}

impl Track {
    /// Position now, advancing with the clock while playing.
    pub fn estimated_position(&self) -> f64 {
        let p = if self.playing { self.position + self.sampled.elapsed().as_secs_f64() } else { self.position };
        if self.duration > 0.0 { p.min(self.duration) } else { p }
    }
    /// Same song? (a position change is not a new song)
    pub fn same_song(&self, other: &Track) -> bool {
        self.bus == other.bus && self.title == other.title && self.artist == other.artist
    }
}

#[derive(Clone, Debug)]
pub enum Command {
    PlayPause,
    Next,
    Previous,
    /// Absolute position in seconds.
    Seek(f64),
    /// Poll faster while the panel is open.
    Fast(bool),
}

pub struct Mpris {
    pub tracks: watch::Receiver<Option<Track>>,
    pub commands: mpsc::UnboundedSender<Command>,
}

/// Starts the watcher on the current tokio runtime.
pub fn spawn() -> Mpris {
    let (tx, rx) = watch::channel(None);
    let (ctx, crx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        if let Err(e) = run(tx, crx).await {
            eprintln!("notchhub: MPRIS unavailable ({e})");
        }
    });
    Mpris { tracks: rx, commands: ctx }
}

fn text(v: &OwnedValue) -> Option<String> {
    String::try_from(v.try_clone().ok()?).ok()
}
fn number(v: &OwnedValue) -> Option<i64> {
    let v = v.try_clone().ok()?;
    i64::try_from(v.try_clone().ok()?).ok().or_else(|| u64::try_from(v).ok().map(|x| x as i64))
}

fn parse_metadata(md: &HashMap<String, OwnedValue>) -> (String, String, String, Option<String>, f64, String) {
    let title = md.get("xesam:title").and_then(text).unwrap_or_default();
    let artist = md
        .get("xesam:artist")
        .and_then(|v| Vec::<String>::try_from(v.try_clone().ok()?).ok())
        .map(|a| a.join(", "))
        .or_else(|| md.get("xesam:artist").and_then(text))
        .unwrap_or_default();
    let album = md.get("xesam:album").and_then(text).unwrap_or_default();
    let art = md.get("mpris:artUrl").and_then(text).filter(|s| !s.is_empty());
    let len = md.get("mpris:length").and_then(number).unwrap_or(0) as f64 / 1_000_000.0;
    let id = md
        .get("mpris:trackid")
        .and_then(|v| match &**v {
            Value::ObjectPath(p) => Some(p.to_string()),
            Value::Str(s) => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_default();
    (title, artist, album, art, len, id)
}

async fn players(conn: &Connection) -> Vec<String> {
    let Ok(dbus) = zbus::fdo::DBusProxy::new(conn).await else { return vec![] };
    let Ok(names) = dbus.list_names().await else { return vec![] };
    let mut v: Vec<String> = names.into_iter().map(|n| n.to_string()).filter(|n| n.starts_with(PREFIX)).collect();
    v.sort();
    v
}

async fn read(conn: &Connection, bus: &str) -> Option<Track> {
    let p = Proxy::new(conn, bus.to_string(), PATH, PLAYER).await.ok()?;
    let status: String = p.get_property("PlaybackStatus").await.ok()?;
    let md: HashMap<String, OwnedValue> = p.get_property("Metadata").await.unwrap_or_default();
    let (title, artist, album, art_url, duration, track_id) = parse_metadata(&md);
    let pos_us: i64 = p.get_property("Position").await.unwrap_or(0);
    let can_seek: bool = p.get_property("CanSeek").await.unwrap_or(false);
    let app = match Proxy::new(conn, bus.to_string(), PATH, "org.mpris.MediaPlayer2").await {
        Ok(r) => r.get_property::<String>("Identity").await.unwrap_or_default(),
        Err(_) => String::new(),
    };
    Some(Track {
        bus: bus.to_string(),
        app,
        title,
        artist,
        album,
        art_url,
        duration,
        position: pos_us as f64 / 1_000_000.0,
        playing: status == "Playing",
        can_seek,
        track_id,
        sampled: Instant::now(),
    })
}

/// The player to show: one that is playing (keeping the current one if it still is), otherwise the current one
/// while it is paused, otherwise the first with a title.
fn pick(all: Vec<Track>, current: Option<&str>) -> Option<Track> {
    let playing: Vec<&Track> = all.iter().filter(|t| t.playing && !t.title.is_empty()).collect();
    if let Some(cur) = current {
        if let Some(t) = playing.iter().find(|t| t.bus == cur) {
            return Some((*t).clone());
        }
    }
    if let Some(t) = playing.first() {
        return Some((*t).clone());
    }
    if let Some(cur) = current {
        if let Some(t) = all.iter().find(|t| t.bus == cur && !t.title.is_empty()) {
            return Some(t.clone());
        }
    }
    all.into_iter().find(|t| !t.title.is_empty())
}

async fn run(tx: watch::Sender<Option<Track>>, mut cmds: mpsc::UnboundedReceiver<Command>) -> zbus::Result<()> {
    let conn = Connection::session().await?;
    // changes announced by any player trigger an immediate refresh (fast song detection)
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .path(PATH)?
        .build();
    let mut signals = zbus::MessageStream::for_match_rule(rule, &conn, Some(32)).await?;
    let mut fast = false;
    let mut current: Option<String> = None;
    loop {
        let all: Vec<Track> = {
            let mut v = vec![];
            for bus in players(&conn).await {
                if let Some(t) = read(&conn, &bus).await {
                    v.push(t);
                }
            }
            v
        };
        let chosen = pick(all, current.as_deref());
        current = chosen.as_ref().map(|t| t.bus.clone());
        tx.send_if_modified(|old| {
            let changed = match (&*old, &chosen) {
                (Some(a), Some(b)) => {
                    !a.same_song(b) || a.playing != b.playing || (b.position - a.estimated_position()).abs() > 1.5 || a.duration != b.duration
                }
                (None, None) => false,
                _ => true,
            };
            if changed {
                *old = chosen.clone();
            }
            changed
        });
        let wait = Duration::from_millis(if fast { 500 } else { 1500 });
        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            _ = signals.next() => { tokio::time::sleep(Duration::from_millis(60)).await; }
            c = cmds.recv() => {
                let Some(c) = c else { return Ok(()) };
                if let Command::Fast(f) = c { fast = f; continue; }
                if let Some(bus) = &current { send(&conn, bus, c, &tx).await; }
            }
        }
    }
}

async fn send(conn: &Connection, bus: &str, cmd: Command, tx: &watch::Sender<Option<Track>>) {
    let Ok(p) = Proxy::new(conn, bus.to_string(), PATH, PLAYER).await else { return };
    match cmd {
        Command::PlayPause => { let _ = p.call_method("PlayPause", &()).await; }
        Command::Next => { let _ = p.call_method("Next", &()).await; }
        Command::Previous => { let _ = p.call_method("Previous", &()).await; }
        Command::Seek(secs) => {
            let id = tx.borrow().as_ref().map(|t| t.track_id.clone()).unwrap_or_default();
            let us = (secs * 1_000_000.0) as i64;
            match zbus::zvariant::ObjectPath::try_from(id.as_str()) {
                Ok(path) if !id.is_empty() => { let _ = p.call_method("SetPosition", &(path, us)).await; }
                _ => {
                    // no usable track id: seek by the difference instead
                    let now: i64 = p.get_property("Position").await.unwrap_or(0);
                    let _ = p.call_method("Seek", &(us - now)).await;
                }
            }
        }
        Command::Fast(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(bus: &str, title: &str, playing: bool) -> Track {
        Track {
            bus: bus.into(), app: String::new(), title: title.into(), artist: String::new(), album: String::new(),
            art_url: None, duration: 100.0, position: 0.0, playing, can_seek: true, track_id: String::new(), sampled: Instant::now(),
        }
    }

    #[test]
    fn playing_player_wins_and_current_sticks() {
        let all = vec![t("a", "x", true), t("b", "y", true)];
        assert_eq!(pick(all.clone(), Some("b")).unwrap().bus, "b");
        assert_eq!(pick(all, None).unwrap().bus, "a");
    }

    #[test]
    fn paused_current_is_kept_when_nothing_plays() {
        let all = vec![t("a", "x", false), t("b", "y", false)];
        assert_eq!(pick(all, Some("b")).unwrap().bus, "b");
    }

    #[test]
    fn nothing_without_title() {
        assert!(pick(vec![t("a", "", true)], None).is_none());
    }

    #[test]
    fn position_advances_only_while_playing() {
        let mut a = t("a", "x", true);
        a.position = 10.0;
        a.sampled = Instant::now() - Duration::from_secs(5);
        assert!((a.estimated_position() - 15.0).abs() < 0.1);
        a.playing = false;
        assert!((a.estimated_position() - 10.0).abs() < 0.01);
    }
}
