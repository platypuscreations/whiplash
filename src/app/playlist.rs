//! The playlist window's behavior, and the pop-up text menus it opens.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};

use super::{App, Kind, View, file_name};
use crate::audio;
use crate::playlist::Select;
use crate::playlist_file;
use crate::ui::menu::{Action, Menu};
use crate::ui::playlist_window::{self, Hit, MenuItem, PlControl, PlaylistView, format_duration};

/// Two clicks on the same track this close together count as a double-click.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// What a mouse drag in the playlist window is doing.
#[derive(Clone, Copy, Debug)]
pub(super) enum PlDrag {
    /// Moving the selected tracks; `row` is the track under the mouse.
    Reorder { row: usize, moved: bool },
    /// Resizing from the bottom-right corner; the offset keeps the corner
    /// where it was grabbed.
    Resize { offset_x: i32, offset_y: i32 },
}

impl App {
    /// Adds files, folders or playlist files. `replace` starts a new list
    /// and plays it; otherwise they're added at the end.
    pub(super) fn add_tracks(&mut self, paths: Vec<PathBuf>, replace: bool) {
        let tracks = playlist_file::expand(paths);
        if tracks.is_empty() {
            self.show_message("no playable files found".to_string());
            return;
        }
        let added = if replace {
            self.pl.scroll = 0;
            self.playlist.replace(tracks)
        } else {
            self.playlist.append(tracks)
        };
        self.metadata.request(added);
        if replace {
            self.play_current();
        }
        self.redraw(Kind::Playlist);
    }

    pub(super) fn set_playlist_open(&mut self, open: bool) {
        self.state.playlist_open = open;
        if let Some(view) = &self.pl_view {
            if open {
                // Open below the lowest of the main and equalizer windows.
                // (Phase 6 makes the windows dock and move together.)
                let above = if self.state.equalizer_open { &self.eq_view } else { &self.main_view };
                if let Some(above) = above
                    && let Ok(position) = above.window.outer_position()
                {
                    let below = PhysicalPosition::new(position.x, position.y + above.window.outer_size().height as i32);
                    view.window.set_outer_position(below);
                }
                view.window.set_visible(true);
            } else {
                view.window.set_visible(false);
            }
        }
        self.redraw(Kind::Main);
    }

    // ----- Mouse -----

    pub(super) fn pl_mouse_down(&mut self) {
        let Some(view) = &self.pl_view else { return };
        let (x, y) = view.cursor_in_skin(self.zoom());
        let hit = playlist_window::hit_test(&self.pl, self.playlist.len(), x, y);

        // Clicking anywhere outside an open button menu closes it.
        if self.pl.open_menu.is_some() {
            match hit {
                Hit::MenuItem(item) => self.pl.hovered = Some(item),
                Hit::MenuButton(button) if Some(button) == self.pl.open_menu => self.pl.open_menu = None,
                _ => self.pl.open_menu = None,
            }
            return;
        }

        match hit {
            Hit::TitleBar => {
                let _ = view.window.drag_window();
            }
            Hit::Control(control) => self.pl.pressed = Some(control),
            Hit::MenuButton(button) => {
                self.pl.open_menu = Some(button);
                self.pl.hovered = None;
            }
            Hit::MenuItem(_) | Hit::Nothing => {}
            Hit::Track(index) => self.track_pressed(index),
            Hit::BelowTracks => self.playlist.select_all(false),
            Hit::ScrollBar => {
                self.pl.dragging_scrollbar = true;
                self.pl.scroll = self.pl.scroll_for_handle(y, self.playlist.len());
            }
            Hit::Resize => {
                let (width, height) = (self.pl.width as i32, self.pl.height as i32);
                self.pl_drag = Some(PlDrag::Resize { offset_x: width - x, offset_y: height - y });
            }
        }
    }

    fn track_pressed(&mut self, index: usize) {
        let now = Instant::now();
        let double = self.last_click.is_some_and(|(at, i)| i == index && now - at < DOUBLE_CLICK);
        self.last_click = Some((now, index));
        if double {
            self.playlist.click(index, Select::Only);
            self.playlist.set_current(index);
            self.play_current();
            return;
        }

        if self.modifiers.shift_key() {
            self.playlist.click(index, Select::Range);
        } else if self.command_key() {
            self.playlist.click(index, Select::Toggle);
        } else {
            // Leave an existing selection alone so it can be dragged; a
            // click without dragging narrows it on release.
            if !self.playlist.entries()[index].selected {
                self.playlist.click(index, Select::Only);
            }
            self.pl_drag = Some(PlDrag::Reorder { row: index, moved: false });
        }
    }

    pub(super) fn pl_mouse_moved(&mut self) {
        let Some(view) = &self.pl_view else { return };
        let (x, y) = view.cursor_in_skin(self.zoom());

        if self.pl.open_menu.is_some() {
            self.pl.hovered = match playlist_window::hit_test(&self.pl, self.playlist.len(), x, y) {
                Hit::MenuItem(item) => Some(item),
                _ => None,
            };
        }
        if self.pl.dragging_scrollbar {
            self.pl.scroll = self.pl.scroll_for_handle(y, self.playlist.len());
        }
        match self.pl_drag {
            Some(PlDrag::Reorder { row, .. }) => {
                // Which track row the mouse is over, even past the ends.
                let over = self.pl.scroll as isize + (y - 23).div_euclid(playlist_window::ROW_HEIGHT) as isize;
                let over = over.clamp(0, self.playlist.len() as isize - 1) as usize;
                if over != row {
                    self.playlist.move_selected(over as isize - row as isize);
                    self.pl_drag = Some(PlDrag::Reorder { row: over, moved: true });
                    self.pl.reveal(over, self.playlist.len());
                }
            }
            Some(PlDrag::Resize { offset_x, offset_y }) => {
                let (width, height) = PlaylistView::snap_size((x + offset_x) as f64, (y + offset_y) as f64);
                if (width, height) != (self.pl.width, self.pl.height) {
                    self.pl.width = width;
                    self.pl.height = height;
                    self.pl.scroll_to(self.pl.scroll as isize, self.playlist.len());
                    let zoom = self.zoom();
                    let _ = view.window.request_inner_size(LogicalSize::new(width * zoom, height * zoom));
                }
            }
            None => {}
        }
    }

    pub(super) fn pl_mouse_up(&mut self, event_loop: &ActiveEventLoop) {
        let Some(view) = &self.pl_view else { return };
        let (x, y) = view.cursor_in_skin(self.zoom());
        let hit = playlist_window::hit_test(&self.pl, self.playlist.len(), x, y);
        self.pl.dragging_scrollbar = false;

        if let Some(PlDrag::Reorder { row, moved: false }) = self.pl_drag.take() {
            self.playlist.click(row, Select::Only);
        }
        if let Some(control) = self.pl.pressed.take()
            && hit == Hit::Control(control)
        {
            self.pl_activate(control);
        }
        // Releasing over a menu choice picks it (whether the menu was opened
        // by this press or an earlier click).
        if self.pl.open_menu.is_some() {
            match hit {
                Hit::MenuItem(item) => {
                    self.pl.open_menu = None;
                    self.pl.hovered = None;
                    self.pl_menu_item(item, event_loop);
                }
                Hit::MenuButton(_) => {} // just opened: stay open
                _ => self.pl.open_menu = None,
            }
        }
    }

    pub(super) fn pl_scroll_by(&mut self, rows: isize) {
        self.pl.scroll_to(self.pl.scroll as isize + rows, self.playlist.len());
    }

    fn pl_activate(&mut self, control: PlControl) {
        match control {
            PlControl::Close => self.set_playlist_open(false),
            PlControl::ScrollUp => self.pl_scroll_by(-1),
            PlControl::ScrollDown => self.pl_scroll_by(1),
            PlControl::Previous => self.previous(),
            PlControl::Play => self.play(),
            PlControl::Pause => self.player.pause(),
            PlControl::Stop => self.player.stop(),
            PlControl::Next => self.next(),
            PlControl::Eject => self.open_files_dialog(),
        }
    }

    fn pl_menu_item(&mut self, item: MenuItem, event_loop: &ActiveEventLoop) {
        match item {
            MenuItem::AddUrl => self.show_message("internet streams aren't supported yet".to_string()),
            MenuItem::AddFolder => {
                if let Some(folder) = rfd::FileDialog::new().set_title("Add folder").pick_folder() {
                    self.add_tracks(vec![folder], false);
                }
            }
            MenuItem::AddFiles => {
                let dialog = rfd::FileDialog::new().set_title("Add file(s)").add_filter("Audio files", audio::EXTENSIONS);
                if let Some(files) = dialog.pick_files() {
                    self.add_tracks(files, false);
                }
            }
            MenuItem::RemoveMisc => {
                self.open_text_menu(event_loop, Kind::Playlist, &[("Remove missing files", Action::RemoveMissingFiles)])
            }
            MenuItem::RemoveAll | MenuItem::NewList => self.playlist.clear(),
            MenuItem::Crop => self.playlist.crop(),
            MenuItem::RemoveSelected => self.playlist.remove_selected(),
            MenuItem::InvertSelection => self.playlist.invert_selection(),
            MenuItem::SelectNone => self.playlist.select_all(false),
            MenuItem::SelectAll => self.playlist.select_all(true),
            MenuItem::SortList => self.open_text_menu(
                event_loop,
                Kind::Playlist,
                &[
                    ("Sort list by title", Action::SortByTitle),
                    ("Sort list by filename", Action::SortByFileName),
                    ("Sort list by path and filename", Action::SortByPath),
                    ("Reverse list", Action::ReverseList),
                    ("Randomize list", Action::RandomizeList),
                ],
            ),
            MenuItem::FileInfo => self.show_file_info(),
            MenuItem::MiscOptions => self.show_message("no playlist options yet".to_string()),
            MenuItem::SaveList => self.save_list(),
            MenuItem::LoadList => {
                let dialog = rfd::FileDialog::new()
                    .set_title("Load playlist")
                    .add_filter("Playlists", playlist_file::PLAYLIST_EXTENSIONS);
                if let Some(path) = dialog.pick_file() {
                    self.add_tracks(vec![path], true);
                }
            }
        }
        self.pl.scroll_to(self.pl.scroll as isize, self.playlist.len());
    }

    fn save_list(&mut self) {
        let dialog = rfd::FileDialog::new()
            .set_title("Save playlist")
            .add_filter("M3U playlist", &["m3u"])
            .set_file_name("playlist.m3u");
        let Some(path) = dialog.save_file() else { return };
        match playlist_file::save_m3u(&path, self.playlist.entries()) {
            Ok(()) => self.show_message(format!("saved playlist: {}", file_name(&path))),
            Err(err) => self.show_message(format!("{err:#}")),
        }
    }

    /// Shows details of the first selected track (or the playing one).
    fn show_file_info(&mut self) {
        let index = self.playlist.first_selected().or(self.playlist.current_index());
        let Some(entry) = index.map(|i| &self.playlist.entries()[i]) else { return };
        let mut text = format!("{}\n\n{}", entry.title, entry.path.display());
        match audio::read_info(&entry.path) {
            Ok(info) => {
                text += &format!(
                    "\n\nLength: {}\nSample rate: {} Hz\nChannels: {}",
                    info.duration.map_or("unknown".into(), |d| format_duration(d.round() as u64)),
                    info.sample_rate,
                    info.channels
                );
                if let Some(kbps) = info.kbps {
                    text += &format!("\nBitrate: {kbps} kbps");
                }
            }
            Err(err) => text += &format!("\n\nCan't be played: {err:#}"),
        }
        rfd::MessageDialog::new().set_title("File info").set_description(text).show();
    }

    /// Playlist keys: arrows move the selection, Enter plays, Delete
    /// removes, Ctrl/Cmd+A selects all. Returns false for other keys.
    pub(super) fn pl_key_pressed(&mut self, key: &Key) -> bool {
        let len = self.playlist.len();
        match key {
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown) if len > 0 => {
                let up = matches!(key, Key::Named(NamedKey::ArrowUp));
                let from = self.playlist.first_selected().or(self.playlist.current_index()).unwrap_or(0);
                let to = if up { from.saturating_sub(1) } else { (from + 1).min(len - 1) };
                let mode = if self.modifiers.shift_key() { Select::Range } else { Select::Only };
                self.playlist.click(to, mode);
                self.pl.reveal(to, len);
            }
            Key::Named(NamedKey::Enter) => {
                if let Some(index) = self.playlist.first_selected() {
                    self.playlist.set_current(index);
                    self.play_current();
                }
            }
            Key::Named(NamedKey::Delete | NamedKey::Backspace) => {
                self.playlist.remove_selected();
                self.pl.scroll_to(self.pl.scroll as isize, self.playlist.len());
            }
            Key::Character(c) if c.eq_ignore_ascii_case("a") && self.command_key() => self.playlist.select_all(true),
            _ => return false,
        }
        true
    }

    // ----- Text menus -----

    /// Opens a pop-up menu at the mouse position in window `over`.
    fn open_text_menu(&mut self, event_loop: &ActiveEventLoop, over: Kind, items: &[(&str, Action)]) {
        self.close_text_menu();
        let menu = Menu::new(items);
        let Some(font) = self.menu_font.as_mut() else { return };
        let (width, height) = menu.size(font);
        let Some(parent) = self.view(over) else { return };
        let Ok(origin) = parent.window.outer_position() else { return };
        let position = PhysicalPosition::new(origin.x + parent.cursor.x as i32, origin.y + parent.cursor.y as i32);
        let view = View::new(event_loop, "menu", width, height, self.zoom(), true, Some(position));
        view.window.focus_window();
        self.menu_view = Some(view);
        self.menu = Some(menu);
    }

    pub(super) fn close_text_menu(&mut self) {
        self.menu_view = None; // dropping the window closes it
        self.menu = None;
    }

    pub(super) fn menu_mouse_moved(&mut self) {
        let (Some(view), Some(menu)) = (&self.menu_view, &mut self.menu) else { return };
        let (_, y) = view.cursor_in_skin(if self.double_size { 2 } else { 1 });
        menu.hovered = menu.item_at(y);
    }

    pub(super) fn menu_mouse_up(&mut self) {
        let action = self.menu.as_ref().and_then(|m| m.hovered.map(|i| m.items[i].1));
        self.close_text_menu();
        if let Some(action) = action {
            self.menu_action(action);
        }
    }

    fn menu_action(&mut self, action: Action) {
        match action {
            Action::SortByTitle => self.playlist.sort_by_title(),
            Action::SortByFileName => self.playlist.sort_by_file_name(),
            Action::SortByPath => self.playlist.sort_by_path(),
            Action::ReverseList => self.playlist.reverse(),
            Action::RandomizeList => self.playlist.randomize(),
            Action::RemoveMissingFiles => {
                let removed = self.playlist.remove_missing();
                self.show_message(format!("removed {removed} missing file(s)"));
            }
        }
        self.pl.scroll_to(self.pl.scroll as isize, self.playlist.len());
        self.redraw(Kind::Playlist);
    }
}
