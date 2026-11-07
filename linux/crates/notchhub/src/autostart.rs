//! Start with the session: an XDG autostart entry.
use std::path::PathBuf;

fn path() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.config_dir().join("autostart"))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("notchhub.desktop")
}

pub fn is_enabled() -> bool {
    path().exists()
}

pub fn set_enabled(on: bool) {
    let p = path();
    if on {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let exe = std::env::current_exe().ok().and_then(|e| e.to_str().map(String::from)).unwrap_or_else(|| "notchhub".into());
        let _ = std::fs::write(
            p,
            format!("[Desktop Entry]\nType=Application\nName=NotchHub\nExec={exe}\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n"),
        );
    } else {
        let _ = std::fs::remove_file(p);
    }
}
