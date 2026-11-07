//! Tray icon (StatusNotifierItem). GNOME needs the AppIndicator extension to show it; the app works without it.
use ksni::menu::StandardItem;
use ksni::{MenuItem, Tray, TrayMethods};

#[derive(Clone, Copy, Debug)]
pub enum Cmd {
    Toggle,
    Settings,
    Quit,
}

struct NhTray {
    tx: async_channel::Sender<Cmd>,
}

impl Tray for NhTray {
    fn id(&self) -> String {
        "notchhub".into()
    }
    fn title(&self) -> String {
        "NotchHub".into()
    }
    fn icon_name(&self) -> String {
        "folder-symbolic".into()
    }
    fn menu(&self) -> Vec<MenuItem<Self>> {
        let item = |label: &str, cmd: Cmd| -> MenuItem<Self> {
            StandardItem {
                label: label.into(),
                activate: Box::new(move |t: &mut Self| {
                    let _ = t.tx.try_send(cmd);
                }),
                ..Default::default()
            }
            .into()
        };
        vec![item("Open", Cmd::Toggle), item("Settings…", Cmd::Settings), MenuItem::Separator, item("Quit NotchHub", Cmd::Quit)]
    }
}

/// Starts the tray on its own thread. Failing to show it is not an error for the app.
pub fn spawn(tx: async_channel::Sender<Cmd>) {
    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return };
        rt.block_on(async move {
            let tray = NhTray { tx };
            match tray.spawn().await {
                Ok(_handle) => std::future::pending::<()>().await,
                Err(e) => eprintln!("notchhub: no tray icon ({e})"),
            }
        });
    });
}
