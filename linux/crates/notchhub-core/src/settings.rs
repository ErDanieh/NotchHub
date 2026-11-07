//! User settings, stored as TOML in `~/.config/notchhub/settings.toml`.
use crate::news::NewsConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tab {
    Shelf,
    Music,
    Clipboard,
    Calendar,
    System,
    Convert,
    News,
    Docker,
}

impl Tab {
    pub const ALL: [Tab; 8] =
        [Tab::Shelf, Tab::Music, Tab::Clipboard, Tab::Calendar, Tab::System, Tab::Convert, Tab::News, Tab::Docker];
    pub fn label(self) -> &'static str {
        match self {
            Tab::Shelf => "Shelf",
            Tab::Music => "Music",
            Tab::Clipboard => "Clipboard",
            Tab::Calendar => "Calendar",
            Tab::System => "System",
            Tab::Convert => "Convert",
            Tab::News => "News",
            Tab::Docker => "Docker",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Tab::Shelf => "folder-symbolic",
            Tab::Music => "audio-x-generic-symbolic",
            Tab::Clipboard => "edit-paste-symbolic",
            Tab::Calendar => "x-office-calendar-symbolic",
            Tab::System => "utilities-system-monitor-symbolic",
            Tab::Convert => "view-refresh-symbolic",
            Tab::News => "news-subscribe-symbolic",
            Tab::Docker => "package-x-generic-symbolic",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub halo_enabled: bool,
    /// Keep the halo on even when nothing plays.
    pub halo_always: bool,
    /// Make the halo follow the beat of the system audio.
    pub halo_beat: bool,
    pub halo_color_hex: String,
    pub halo_intensity: f64,
    pub pulse_seconds: f64,
    pub open_delay: f64,
    pub balls_enabled: bool,
    pub pet_enabled: bool,
    pub pet_kind: String,
    /// Pixels per sprite pixel (2..=5).
    pub pet_size: u32,
    pub tab_order: Vec<Tab>,
    pub hidden_tabs: Vec<Tab>,
    pub remember_tab: bool,
    pub last_tab: Tab,
    pub music_show_lyrics: bool,
    pub lyrics_lead: f64,
    pub alerts_song: bool,
    pub alerts_calendar: bool,
    pub alerts_conversion: bool,
    pub alerts_performance: bool,
    pub calendar_lead_minutes: f64,
    pub cpu_threshold: f64,
    pub mem_threshold: f64,
    pub battery_threshold: f64,
    /// `.ics` files or http(s) addresses shown in the Calendar tab.
    pub calendar_sources: Vec<String>,
    pub news: NewsConfig,
    /// Category shown in the News tab.
    pub news_section: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            halo_enabled: true,
            halo_always: false,
            halo_beat: false,
            halo_color_hex: "3DA9FF".into(),
            halo_intensity: 1.0,
            pulse_seconds: 1.6,
            open_delay: 0.12,
            balls_enabled: true,
            pet_enabled: true,
            pet_kind: "clawd".into(),
            pet_size: 3,
            tab_order: Tab::ALL.to_vec(),
            hidden_tabs: vec![],
            remember_tab: true,
            last_tab: Tab::Shelf,
            music_show_lyrics: false,
            lyrics_lead: 0.3,
            alerts_song: true,
            alerts_calendar: true,
            alerts_conversion: true,
            alerts_performance: true,
            calendar_lead_minutes: 10.0,
            cpu_threshold: 90.0,
            mem_threshold: 90.0,
            battery_threshold: 15.0,
            calendar_sources: vec![],
            news: NewsConfig::default(),
            news_section: "tech".into(),
        }
    }
}

impl Settings {
    pub fn config_dir() -> PathBuf {
        directories::ProjectDirs::from("", "", "notchhub")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".notchhub"))
    }
    pub fn data_dir() -> PathBuf {
        directories::ProjectDirs::from("", "", "notchhub")
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".notchhub"))
    }
    fn path() -> PathBuf {
        Self::config_dir().join("settings.toml")
    }

    pub fn load() -> Settings {
        Self::load_from(&Self::path())
    }
    pub fn load_from(path: &std::path::Path) -> Settings {
        let mut s: Settings =
            std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        s.news.normalize();
        s.fix_tabs();
        s.pet_size = s.pet_size.clamp(2, 5);
        s
    }
    pub fn save(&self) {
        self.save_to(&Self::path());
    }
    pub fn save_to(&self, path: &std::path::Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(t) = toml::to_string_pretty(self) {
            let tmp = path.with_extension("tmp");
            if std::fs::write(&tmp, t).is_ok() {
                let _ = std::fs::rename(&tmp, path);
            }
        }
    }

    /// New tabs that are not in the saved order go at the end; duplicates are dropped.
    fn fix_tabs(&mut self) {
        let mut seen = Vec::new();
        self.tab_order.retain(|t| {
            let new = !seen.contains(t);
            seen.push(*t);
            new
        });
        for t in Tab::ALL {
            if !self.tab_order.contains(&t) {
                self.tab_order.push(t);
            }
        }
    }

    /// Tabs in the order and visibility the user chose (never empty).
    pub fn visible_tabs(&self) -> Vec<Tab> {
        let v: Vec<Tab> = self.tab_order.iter().copied().filter(|t| !self.hidden_tabs.contains(t)).collect();
        if v.is_empty() {
            vec![Tab::Shelf]
        } else {
            v
        }
    }

    /// Halo colour as 0..1 RGB.
    pub fn halo_rgb(&self) -> (f64, f64, f64) {
        let v = u32::from_str_radix(self.halo_color_hex.trim_start_matches('#'), 16).unwrap_or(0x3DA9FF);
        (((v >> 16) & 255) as f64 / 255.0, ((v >> 8) & 255) as f64 / 255.0, (v & 255) as f64 / 255.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_defaults_for_missing_fields() {
        let dir = std::env::temp_dir().join(format!("nh-test-{}", std::process::id()));
        let p = dir.join("settings.toml");
        let mut s = Settings::default();
        s.halo_always = true;
        s.pet_kind = "cat".into();
        s.save_to(&p);
        let l = Settings::load_from(&p);
        assert!(l.halo_always);
        assert_eq!(l.pet_kind, "cat");
        std::fs::write(&p, "halo_always = true\n").unwrap();
        let l = Settings::load_from(&p);
        assert!(l.halo_always && l.halo_enabled && l.pet_size == 3);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn broken_file_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("nh-test2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("s.toml");
        std::fs::write(&p, "this is = = not toml").unwrap();
        assert!(Settings::load_from(&p).halo_enabled);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn tab_order_gets_new_tabs_and_hidden_are_skipped() {
        let mut s = Settings::default();
        s.tab_order = vec![Tab::Music, Tab::Music];
        s.hidden_tabs = vec![Tab::Shelf];
        s.fix_tabs();
        assert_eq!(s.tab_order[0], Tab::Music);
        assert_eq!(s.tab_order.len(), Tab::ALL.len());
        assert!(!s.visible_tabs().contains(&Tab::Shelf));
    }

    #[test]
    fn partial_file_keeps_the_saved_tab() {
        let dir = std::env::temp_dir().join(format!("nh-test3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("s.toml");
        std::fs::write(&p, "halo_always = false\nlast_tab = \"music\"\nmusic_show_lyrics = true\n").unwrap();
        let s = Settings::load_from(&p);
        assert_eq!(s.last_tab, Tab::Music);
        assert!(s.music_show_lyrics);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn halo_colour_parses() {
        let s = Settings::default();
        let (r, g, b) = s.halo_rgb();
        assert!((r - 0x3D as f64 / 255.0).abs() < 1e-9 && g > 0.6 && b == 1.0);
    }
}
