//! Things we ask of the desktop that winit doesn't offer: where the mouse
//! pointer is on the screen, and bringing a window to the front without
//! making it the active one.
//!
//! The pointer: mouse events give its place inside a window. During a
//! drag that isn't enough on X11: the window moves a moment after we ask,
//! and events that arrive in between are measured from where it was. When
//! events queue up (MilkDrop's drawing waits for the screen, for one), the
//! error grows and the windows jump about. So on X11 we ask the X server.
//! macOS moves windows at once, so there the window-relative position does.
//!
//! Raising: the player, equalizer and playlist come to the front together,
//! as Winamp's did, so a docked stack isn't split between layers when one
//! of them is clicked or dragged over other windows.

use winit::dpi::PhysicalPosition;
use winit::window::Window;

/// What we can ask of the desktop on this system.
pub struct Desktop {
    #[cfg(all(unix, not(target_os = "macos")))]
    x11: Option<x11::X11>,
}

impl Desktop {
    pub fn connect() -> Desktop {
        Desktop {
            #[cfg(all(unix, not(target_os = "macos")))]
            x11: x11::X11::connect(),
        }
    }

    /// The pointer's position on the screen, in physical pixels, if the
    /// system can say.
    pub fn pointer(&self) -> Option<PhysicalPosition<f64>> {
        #[cfg(all(unix, not(target_os = "macos")))]
        return self.x11.as_ref()?.pointer();
        #[cfg(not(all(unix, not(target_os = "macos"))))]
        None
    }

    /// Brings `window` to the front without making it the active window.
    pub fn raise(&self, window: &Window) {
        #[cfg(target_os = "macos")]
        macos::raise(window);
        #[cfg(all(unix, not(target_os = "macos")))]
        if let Some(x11) = &self.x11 {
            x11.raise(window);
        }
        #[cfg(not(unix))]
        let _ = window;
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2_app_kit::NSView;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    pub fn raise(window: &Window) {
        let Ok(handle) = window.window_handle() else { return };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return };
        // SAFETY: winit's handle points at the window's content view, which
        // lives as long as the window; we're on the main thread.
        let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
        if let Some(window) = view.window() {
            window.orderFront(None);
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod x11 {
    use winit::dpi::PhysicalPosition;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{Atom, ClientMessageEvent, ConnectionExt, EventMask};
    use x11rb::rust_connection::RustConnection;

    /// A connection to the X server.
    pub struct X11 {
        connection: RustConnection,
        root: u32,
        restack: Atom,
    }

    impl X11 {
        /// Connects to the X server; `None` without one (on Wayland, say).
        pub fn connect() -> Option<X11> {
            let (connection, screen) = RustConnection::connect(None).ok()?;
            let root = connection.setup().roots.get(screen)?.root;
            let restack = connection.intern_atom(false, b"_NET_RESTACK_WINDOW").ok()?.reply().ok()?.atom;
            Some(X11 { connection, root, restack })
        }

        pub fn pointer(&self) -> Option<PhysicalPosition<f64>> {
            let reply = self.connection.query_pointer(self.root).ok()?.reply().ok()?;
            Some(PhysicalPosition::new(reply.root_x.into(), reply.root_y.into()))
        }

        /// Asks the window manager to put the window above the others. (The
        /// window manager owns stacking, so raising the window directly does
        /// nothing; this is the standard way to ask it.)
        pub fn raise(&self, window: &Window) {
            let id = match window.window_handle().map(|h| h.as_raw()) {
                Ok(RawWindowHandle::Xlib(handle)) => handle.window as u32,
                Ok(RawWindowHandle::Xcb(handle)) => handle.window.get(),
                _ => return,
            };
            // Source 2 means "on the user's behalf", with no sibling, and
            // stack mode 0: above everything.
            let message = ClientMessageEvent::new(32, id, self.restack, [2u32, 0, 0, 0, 0]);
            let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
            let _ = self.connection.send_event(false, self.root, mask, message);
            let _ = self.connection.flush();
        }
    }
}
