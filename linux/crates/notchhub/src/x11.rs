//! Window placement on X11 (also XWayland): the window manager is asked to keep the panel on top, on every
//! desktop and out of the taskbar, and the panel is moved to the top centre of the screen.
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

pub struct X {
    conn: RustConnection,
    root: u32,
}

impl X {
    pub fn connect() -> Option<X> {
        let (conn, n) = RustConnection::connect(None).ok()?;
        let root = conn.setup().roots[n].root;
        Some(X { conn, root })
    }

    fn atom(&self, name: &str) -> u32 {
        self.conn.intern_atom(false, name.as_bytes()).ok().and_then(|c| c.reply().ok()).map(|r| r.atom).unwrap_or(0)
    }

    /// Dock-type window that stays above the others, on all desktops, without a taskbar entry.
    pub fn make_dock(&self, xid: u32) {
        let c = &self.conn;
        let _ = c.change_property32(
            PropMode::REPLACE,
            xid,
            self.atom("_NET_WM_WINDOW_TYPE"),
            AtomEnum::ATOM,
            &[self.atom("_NET_WM_WINDOW_TYPE_DOCK")],
        );
        let states = [
            self.atom("_NET_WM_STATE_ABOVE"),
            self.atom("_NET_WM_STATE_STICKY"),
            self.atom("_NET_WM_STATE_SKIP_TASKBAR"),
            self.atom("_NET_WM_STATE_SKIP_PAGER"),
        ];
        let _ = c.change_property32(PropMode::REPLACE, xid, self.atom("_NET_WM_STATE"), AtomEnum::ATOM, &states);
        let _ = c.change_property32(PropMode::REPLACE, xid, self.atom("_NET_WM_DESKTOP"), AtomEnum::CARDINAL, &[0xFFFF_FFFF]);
        let _ = c.flush();
    }

    /// Asks the window manager to add the "above" and "sticky" states to an already mapped window.
    pub fn request_above(&self, xid: u32) {
        let state = self.atom("_NET_WM_STATE");
        for st in [self.atom("_NET_WM_STATE_ABOVE"), self.atom("_NET_WM_STATE_STICKY")] {
            let ev = ClientMessageEvent::new(32, xid, state, [1u32, st, 0, 1, 0]);
            let _ = self.conn.send_event(
                false,
                self.root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                ev,
            );
        }
        let _ = self.conn.flush();
    }

    /// Moves and resizes the window in one step (device pixels), so it never shifts under the pointer.
    pub fn place(&self, xid: u32, x: i32, y: i32, w: u32, h: u32) {
        let aux = ConfigureWindowAux::new().x(x).y(y).width(w).height(h);
        let _ = self.conn.configure_window(xid, &aux);
        let _ = self.conn.flush();
    }

    /// Top of the usable area (below the desktop's top bar), in device pixels.
    pub fn workarea_top(&self) -> i32 {
        let prop = self
            .conn
            .get_property(false, self.root, self.atom("_NET_WORKAREA"), AtomEnum::CARDINAL, 0, 4)
            .ok()
            .and_then(|c| c.reply().ok());
        prop.and_then(|p| p.value32().and_then(|mut v| v.nth(1))).map(|y| y as i32).unwrap_or(0)
    }
}
