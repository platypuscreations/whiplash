//! Where the mouse pointer is on the screen, for dragging windows.
//!
//! Mouse events give the pointer's place inside a window. During a drag
//! that isn't enough on X11: the window moves a moment after we ask, and
//! events that arrive in between are measured from where it was. When
//! events queue up (MilkDrop's drawing waits for the screen, for one),
//! the error grows and the windows jump about. So on X11 we ask the X
//! server where the pointer is. Elsewhere (macOS moves windows at once)
//! `Pointer` isn't available and the window-relative position is used.

#[cfg(all(unix, not(target_os = "macos")))]
pub use x11::Pointer;

#[cfg(all(unix, not(target_os = "macos")))]
mod x11 {
    use winit::dpi::PhysicalPosition;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, Window};
    use x11rb::rust_connection::RustConnection;

    /// A connection to the X server, for asking where the pointer is.
    pub struct Pointer {
        connection: RustConnection,
        root: Window,
    }

    impl Pointer {
        /// Connects to the X server; `None` without one (on Wayland, say).
        pub fn connect() -> Option<Pointer> {
            let (connection, screen) = RustConnection::connect(None).ok()?;
            let root = connection.setup().roots.get(screen)?.root;
            Some(Pointer { connection, root })
        }

        /// The pointer's position on the screen, in physical pixels.
        pub fn position(&self) -> Option<PhysicalPosition<f64>> {
            let reply = self.connection.query_pointer(self.root).ok()?.reply().ok()?;
            Some(PhysicalPosition::new(reply.root_x.into(), reply.root_y.into()))
        }
    }
}

/// Elsewhere there's nothing to ask.
#[cfg(not(all(unix, not(target_os = "macos"))))]
pub struct Pointer;

#[cfg(not(all(unix, not(target_os = "macos"))))]
impl Pointer {
    pub fn connect() -> Option<Pointer> {
        None
    }

    pub fn position(&self) -> Option<winit::dpi::PhysicalPosition<f64>> {
        None
    }
}
