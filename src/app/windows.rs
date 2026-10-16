//! Moving windows: snapping, docking, and placing them on screen.
//!
//! Winamp's windows snap to each other and to the screen edges when
//! dragged near them, and windows "docked" to the main window (touching it,
//! directly or through each other) move along when it is dragged. To do
//! this we move windows ourselves while the mouse is held, rather than
//! handing the drag to the operating system.

use winit::dpi::{PhysicalPosition, PhysicalSize};

use super::{App, KINDS, Kind};

/// How close (in logical pixels) a window must come to an edge to snap.
const SNAP_DISTANCE: f64 = 10.0;

/// A window's position and size on screen, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    fn right(&self) -> i32 {
        self.x + self.w
    }
    fn bottom(&self) -> i32 {
        self.y + self.h
    }
    fn overlaps_horizontally(&self, other: &Rect) -> bool {
        self.x < other.right() && other.x < self.right()
    }
    fn overlaps_vertically(&self, other: &Rect) -> bool {
        self.y < other.bottom() && other.y < self.bottom()
    }

    /// True if the two windows share an edge.
    pub fn touches(&self, other: &Rect) -> bool {
        let stacked = (self.bottom() == other.y || other.bottom() == self.y) && self.overlaps_horizontally(other);
        let side_by_side = (self.right() == other.x || other.right() == self.x) && self.overlaps_vertically(other);
        stacked || side_by_side
    }
}

/// Moves `moving` so its edges line up with nearby edges of `targets`, if
/// any are within `distance`. Windows snap edge-to-edge (docking) and
/// align with each other's edges; screen edges are given as targets too.
pub(super) fn snap(moving: Rect, targets: &[Rect], distance: i32) -> Rect {
    let mut best_x: Option<i32> = None;
    let mut best_y: Option<i32> = None;
    let consider = |best: &mut Option<i32>, shift: i32| {
        if shift.abs() <= distance && best.is_none_or(|b| shift.abs() < b.abs()) {
            *best = Some(shift);
        }
    };
    for t in targets {
        // Only snap sideways to windows level with us (or nearly), and
        // up/down to windows above or below us.
        let near_vertically = moving.y < t.bottom() + distance && t.y < moving.bottom() + distance;
        let near_horizontally = moving.x < t.right() + distance && t.x < moving.right() + distance;
        if near_vertically {
            for shift in [t.right() - moving.x, t.x - moving.right(), t.x - moving.x, t.right() - moving.right()] {
                consider(&mut best_x, shift);
            }
        }
        if near_horizontally {
            for shift in [t.bottom() - moving.y, t.y - moving.bottom(), t.y - moving.y, t.bottom() - moving.bottom()] {
                consider(&mut best_y, shift);
            }
        }
    }
    Rect { x: moving.x + best_x.unwrap_or(0), y: moving.y + best_y.unwrap_or(0), ..moving }
}

/// A window drag in progress.
pub(super) struct WindowDrag {
    kind: Kind,
    /// Where in the window the mouse grabbed it.
    grab: PhysicalPosition<f64>,
    /// Where we last put the window (the system may report it late).
    position: PhysicalPosition<i32>,
    /// Docked windows moving along, with their offsets from this one.
    followers: Vec<(Kind, PhysicalPosition<i32>)>,
}

impl App {
    /// A visible window's position and size, if the system can tell us.
    pub(super) fn rect(&self, kind: Kind) -> Option<Rect> {
        let view = self.view(kind)?;
        if kind == Kind::Menu || !view.window.is_visible().unwrap_or(true) {
            return None;
        }
        let position = view.window.outer_position().ok()?;
        let size = view.window.outer_size();
        Some(Rect { x: position.x, y: position.y, w: size.width as i32, h: size.height as i32 })
    }

    /// The windows docked to `kind`: touching it, or touching a window
    /// that is, and so on.
    pub(super) fn docked_to(&self, kind: Kind) -> Vec<Kind> {
        let mut group = vec![kind];
        let mut i = 0;
        while i < group.len() {
            if let Some(rect) = self.rect(group[i]) {
                for other in KINDS {
                    if !group.contains(&other) && self.rect(other).is_some_and(|r| r.touches(&rect)) {
                        group.push(other);
                    }
                }
            }
            i += 1;
        }
        group.remove(0);
        group
    }

    /// Starts moving a window by its title bar. Dragging the main window
    /// brings docked windows along; other windows move on their own.
    pub(super) fn start_window_drag(&mut self, kind: Kind) {
        let Some(view) = self.view(kind) else { return };
        let Ok(position) = view.window.outer_position() else {
            // Some systems (Wayland) don't let apps place windows; let the
            // system move it instead, without snapping.
            let _ = view.window.drag_window();
            return;
        };
        let grab = view.cursor;
        let followers = if kind == Kind::Main {
            self.docked_to(Kind::Main)
                .into_iter()
                .filter_map(|other| {
                    let p = self.view(other)?.window.outer_position().ok()?;
                    Some((other, PhysicalPosition::new(p.x - position.x, p.y - position.y)))
                })
                .collect()
        } else {
            Vec::new()
        };
        self.window_drag = Some(WindowDrag { kind, grab, position, followers });
    }

    /// Follows the mouse during a window drag. Returns false if no drag
    /// is happening in this window.
    pub(super) fn continue_window_drag(&mut self, kind: Kind) -> bool {
        let Some(drag) = &self.window_drag else { return false };
        if drag.kind != kind {
            return false;
        }
        let Some(view) = self.view(kind) else { return false };
        let size = view.window.outer_size();
        let scale = view.window.scale_factor();
        // The mouse's screen position, from where we put the window.
        let screen_x = drag.position.x as f64 + view.cursor.x;
        let screen_y = drag.position.y as f64 + view.cursor.y;
        let wanted = Rect {
            x: (screen_x - drag.grab.x).round() as i32,
            y: (screen_y - drag.grab.y).round() as i32,
            w: size.width as i32,
            h: size.height as i32,
        };

        // Snap to the other windows that aren't moving along, and to the
        // edges of the screen.
        let moving: Vec<Kind> = std::iter::once(kind).chain(drag.followers.iter().map(|(k, _)| *k)).collect();
        let mut targets: Vec<Rect> = KINDS.iter().filter(|k| !moving.contains(k)).filter_map(|&k| self.rect(k)).collect();
        if let Some(monitor) = view.window.current_monitor() {
            let (p, s) = (monitor.position(), monitor.size());
            let screen = Rect { x: p.x, y: p.y, w: s.width as i32, h: s.height as i32 };
            // Edges of the screen act like windows just outside it.
            targets.push(Rect { x: screen.x - 1, y: screen.y - 10_000, w: 1, h: 20_000 });
            targets.push(Rect { x: screen.right(), y: screen.y - 10_000, w: 1, h: 20_000 });
            targets.push(Rect { x: screen.x - 10_000, y: screen.y - 1, w: 20_000, h: 1 });
            targets.push(Rect { x: screen.x - 10_000, y: screen.bottom(), w: 20_000, h: 1 });
        }
        let placed = snap(wanted, &targets, (SNAP_DISTANCE * scale).round() as i32);

        let position = PhysicalPosition::new(placed.x, placed.y);
        view.window.set_outer_position(position);
        for (other, offset) in &drag.followers {
            if let Some(other_view) = self.view(*other) {
                other_view.window.set_outer_position(PhysicalPosition::new(position.x + offset.x, position.y + offset.y));
            }
        }
        if let Some(drag) = &mut self.window_drag {
            drag.position = position;
        }
        true
    }

    pub(super) fn end_window_drag(&mut self) {
        self.window_drag = None;
    }

    /// Changes a window's size, moving the windows docked below it so they
    /// stay attached (used for double size and shade mode).
    pub(super) fn resize_keeping_docked(&mut self, kind: Kind, new_size: PhysicalSize<u32>) {
        let Some(rect) = self.rect(kind) else {
            if let Some(view) = self.view(kind) {
                view.resize(new_size);
            }
            return;
        };
        let dy = new_size.height as i32 - rect.h;
        // Windows below this one (docked to it, directly or not) shift by
        // the change in height.
        let below: Vec<(Kind, Rect)> = self
            .docked_to(kind)
            .into_iter()
            .filter_map(|k| self.rect(k).map(|r| (k, r)))
            .filter(|(_, r)| r.y >= rect.bottom())
            .collect();
        if let Some(view) = self.view(kind) {
            view.resize(new_size);
        }
        for (other, r) in below {
            if let Some(view) = self.view(other) {
                view.window.set_outer_position(PhysicalPosition::new(r.x, r.y + dy));
            }
        }
    }

    /// Shows a window at its saved place, or below the lowest of the windows
    /// above it in Winamp's usual stack (main, equalizer, playlist).
    pub(super) fn place_and_show(&mut self, kind: Kind) {
        let saved = match kind {
            Kind::Equalizer => self.saved_positions.equalizer,
            Kind::Playlist => self.saved_positions.playlist,
            _ => None,
        };
        let position = match saved {
            Some([x, y]) if self.on_screen(x, y) => Some(PhysicalPosition::new(x, y)),
            _ => {
                let stack: &[Kind] = match kind {
                    Kind::Playlist => &[Kind::Equalizer, Kind::Main],
                    _ => &[Kind::Main],
                };
                stack.iter().find_map(|&k| self.rect(k)).map(|r| PhysicalPosition::new(r.x, r.bottom()))
            }
        };
        let Some(view) = self.view(kind) else { return };
        if let Some(position) = position {
            view.window.set_outer_position(position);
        }
        view.window.set_visible(true);
        if !self.shown.contains(&kind) {
            self.shown.push(kind);
        }
        // From now on the window stays wherever it was last.
        match kind {
            Kind::Equalizer => self.saved_positions.equalizer = None,
            Kind::Playlist => self.saved_positions.playlist = None,
            _ => {}
        }
    }

    /// True if a window at (x, y) would be on one of the screens, so a saved
    /// position from a now-disconnected monitor isn't used.
    pub(super) fn on_screen(&self, x: i32, y: i32) -> bool {
        let Some(view) = &self.main_view else { return true };
        view.window.available_monitors().any(|m| {
            let (p, s) = (m.position(), m.size());
            x >= p.x - 20 && y >= p.y - 20 && x < p.x + s.width as i32 - 20 && y < p.y + s.height as i32 - 20
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn touching_windows() {
        let main = rect(0, 0, 275, 116);
        assert!(main.touches(&rect(0, 116, 275, 116))); // directly below
        assert!(main.touches(&rect(275, 50, 100, 100))); // to the right
        assert!(!main.touches(&rect(0, 117, 275, 116))); // a pixel apart
        assert!(!main.touches(&rect(300, 116, 275, 116))); // below but off to the side
    }

    #[test]
    fn snaps_below_and_aligns_left_edges() {
        let main = rect(100, 100, 275, 116);
        let placed = snap(rect(106, 222, 275, 116), &[main], 10);
        assert_eq!((placed.x, placed.y), (100, 216));
    }

    #[test]
    fn far_windows_do_not_snap() {
        let main = rect(100, 100, 275, 116);
        let moving = rect(500, 400, 275, 116);
        assert_eq!(snap(moving, &[main], 10), moving);
    }

    #[test]
    fn snaps_side_by_side() {
        let main = rect(100, 100, 275, 116);
        let placed = snap(rect(380, 104, 275, 116), &[main], 10);
        assert_eq!((placed.x, placed.y), (375, 100));
    }
}
