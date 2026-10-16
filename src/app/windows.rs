//! Moving windows: snapping, docking, and placing them on screen.
//!
//! Winamp's windows snap to each other and to the screen edges when
//! dragged near them, and windows "docked" to the main window (touching it,
//! directly or through each other) move along when it is dragged. To do
//! this we move windows ourselves while the mouse is held, rather than
//! handing the drag to the operating system.

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;

use super::{App, KINDS, Kind, skin_window_size};

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
        // The mouse's screen position: from the system if it can say (see
        // `desktop`), or else from where we put the window.
        let screen = self.desktop().pointer().unwrap_or_else(|| {
            PhysicalPosition::new(drag.position.x as f64 + view.cursor.x, drag.position.y as f64 + view.cursor.y)
        });
        let wanted = Rect {
            x: (screen.x - drag.grab.x).round() as i32,
            y: (screen.y - drag.grab.y).round() as i32,
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

    pub(super) fn desktop(&self) -> &super::desktop::Desktop {
        self.desktop.get_or_init(super::desktop::Desktop::connect)
    }

    /// Brings the player, equalizer and playlist to the front together, with
    /// `active` on top, so a docked stack isn't split between layers. (The
    /// system brings only the window that was clicked forward.)
    pub(super) fn raise_together(&self, active: Kind) {
        let others = [Kind::Main, Kind::Equalizer, Kind::Playlist].into_iter().filter(|&k| k != active);
        for kind in others.chain(std::iter::once(active)) {
            if let Some(view) = self.view(kind).filter(|v| v.window.is_visible().unwrap_or(true)) {
                self.desktop().raise(&view.window);
            }
        }
        // An open pop-up menu stays above them. Choosing "Skin browser..."
        // closes one menu and opens another, and the window that gets the
        // focus back would otherwise cover the new menu.
        if let Some(menu) = &self.menu_view {
            self.desktop().raise(&menu.window);
        }
    }

    /// True if a window at (x, y) would be on one of the screens, so a saved
    /// position from a now-disconnected monitor isn't used.
    pub(super) fn on_screen(&self, x: i32, y: i32) -> bool {
        let Some(view) = &self.main_view else { return true };
        on_a_screen(view.window.available_monitors(), x, y)
    }

    /// Decides where the main, equalizer and playlist windows start, before
    /// any of them is created: saved places if they're still on a screen,
    /// otherwise stacked under the main window, the stack (and MilkDrop
    /// beside it, if it opens too) centered on the main screen when nothing
    /// is saved. The places become
    /// `saved_positions`, which the windows are created and shown at.
    ///
    /// Each window must get its place when it is created: on X11 the
    /// window manager only respects a position set then (with a hint winit
    /// adds), and puts windows moved just before they appear wherever it
    /// likes, usually piled up in the middle of the screen.
    pub(super) fn plan_window_positions(&mut self, event_loop: &ActiveEventLoop) {
        let monitors: Vec<MonitorHandle> = event_loop.available_monitors().collect();
        let screen = event_loop.primary_monitor().or_else(|| monitors.first().cloned());
        let fits = |p: &[i32; 2]| on_a_screen(monitors.iter().cloned(), p[0], p[1]);
        // Window sizes as they will be on the main screen.
        let scale = screen.as_ref().map_or(1.0, |s| s.scale_factor());
        let size = |kind| {
            let (width, height) = self.skin_size(kind);
            skin_window_size(width, height, self.zoom(), scale)
        };
        let (main, eq, pl) = (size(Kind::Main), size(Kind::Equalizer), size(Kind::Playlist));
        // Room for MilkDrop beside the main window, if it opens too.
        #[cfg(feature = "milkdrop")]
        let beside = if self.reopen_milkdrop { crate::vis::milkdrop::MilkDrop::SIZE.to_physical::<u32>(scale).width } else { 0 };
        #[cfg(not(feature = "milkdrop"))]
        let beside = 0;
        let windows = &mut self.saved_positions;

        let main_at = windows.main.filter(fits).or_else(|| {
            let screen = screen.as_ref()?;
            let (p, s) = (screen.position(), screen.size());
            let open = |is_open, size: PhysicalSize<u32>| if is_open { size.height } else { 0 };
            let stack = main.height + open(windows.equalizer_open, eq) + open(windows.playlist_open, pl);
            let x = p.x + (s.width as i32 - (main.width + beside) as i32) / 2;
            let y = p.y + (s.height as i32 - stack as i32) / 2;
            Some([x, y.max(p.y)])
        });
        let below = |at: Option<[i32; 2]>, size: PhysicalSize<u32>| at.map(|[x, y]| [x, y + size.height as i32]);
        let eq_at = windows.equalizer.filter(fits).or(below(main_at, main));
        let pl_at = windows.playlist.filter(fits).or(if windows.equalizer_open { below(eq_at, eq) } else { below(main_at, main) });
        (windows.main, windows.equalizer, windows.playlist) = (main_at, eq_at, pl_at);
    }

    /// Where a window of `size` should open beside the main window: to its
    /// right, or to its left if it only fits on the screen there.
    #[cfg(feature = "milkdrop")]
    pub(super) fn beside_main(&self, size: winit::dpi::LogicalSize<u32>) -> Option<PhysicalPosition<i32>> {
        let main = self.main_rect()?;
        let right = PhysicalPosition::new(main.right(), main.y);
        let view = self.main_view.as_ref()?;
        let Some(screen) = view.window.available_monitors().find(|m| on_a_screen(std::iter::once(m.clone()), main.x, main.y))
        else {
            return Some(right);
        };
        let width = size.to_physical::<i32>(screen.scale_factor()).width;
        let (left_edge, right_edge) = (screen.position().x, screen.position().x + screen.size().width as i32);
        if main.right() + width > right_edge && main.x - width >= left_edge {
            Some(PhysicalPosition::new(main.x - width, main.y))
        } else {
            Some(right)
        }
    }

    /// The main window's place: where the system says it is, or where it
    /// was put if the system hasn't shown it yet.
    #[cfg(feature = "milkdrop")]
    fn main_rect(&self) -> Option<Rect> {
        self.rect(Kind::Main).or_else(|| {
            let [x, y] = self.saved_positions.main?;
            let size = self.window_size(Kind::Main)?;
            Some(Rect { x, y, w: size.width as i32, h: size.height as i32 })
        })
    }
}

/// True if a window corner at (x, y) is on one of the screens (with a
/// little room to spare, so the window can be grabbed).
fn on_a_screen(mut monitors: impl Iterator<Item = MonitorHandle>, x: i32, y: i32) -> bool {
    monitors.any(|m| {
        let (p, s) = (m.position(), m.size());
        x >= p.x - 20 && y >= p.y - 20 && x < p.x + s.width as i32 - 20 && y < p.y + s.height as i32 - 20
    })
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
