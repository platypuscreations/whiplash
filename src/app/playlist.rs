//! The playlist window's behavior, and the pop-up text menus it opens.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};

use super::dialogs::Purpose;
use super::{App, Kind, LOCAL_ONLY};
use crate::config::PlayerMode;
use crate::playlist::Select;
use crate::playlist_file;
use crate::ui::menu::{Action, Item};
use crate::ui::browse::Field;
use crate::ui::playlist_window::{self, Hit, MenuButton, MenuItem, PlControl, PlaylistView, Tab};

/// Two clicks on the same track this close together count as a double-click.
pub(super) const DOUBLE_CLICK: Duration = Duration::from_millis(400);

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
        if self.mode != PlayerMode::Local {
            return self.show_message(LOCAL_ONLY.to_string());
        }
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
        self.dock_changed = true;
        if open {
            self.place_and_show(Kind::Playlist);
        } else if let Some(view) = &self.pl_view {
            view.window.set_visible(false);
        }
        self.redraw(Kind::Main);
    }

    // ----- Mouse -----

    pub(super) fn pl_mouse_down(&mut self) {
        let Some(view) = &self.pl_view else { return };
        let (x, y) = view.cursor_in_skin();
        let hit = playlist_window::hit_test(&self.pl, self.pl_rows(), x, y);
        // Typing goes where was clicked, or nowhere.
        self.browser.focus = match hit {
            Hit::SearchBox => Some(Field::Search),
            Hit::FormField(field) => Some(Field::Form(field)),
            _ => None,
        };

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
            Hit::TitleBar => self.title_bar_pressed(Kind::Playlist),
            Hit::Control(control) => self.pl.pressed = Some(control),
            // In a music server mode, ADD adds from the library.
            Hit::MenuButton(MenuButton::Add) if self.pl.tab.is_some() => self.add_from_library(),
            Hit::MenuButton(button) => {
                self.pl.open_menu = Some(button);
                self.pl.hovered = None;
            }
            Hit::MenuItem(_) | Hit::Nothing => {}
            Hit::Track(index) => self.track_pressed(index),
            Hit::BrowseRow(row) => self.browse_row_pressed(row),
            Hit::BelowTracks if self.pl.tab == Some(Tab::Browse) => self.browser.level_mut().selected = None,
            Hit::BelowTracks => self.playlist.select_all(false),
            Hit::Tab(tab) => self.set_tab(tab),
            Hit::SearchBox | Hit::FormField(_) => {}
            Hit::SignInButton => self.sign_in(),
            Hit::ScrollBar => {
                self.pl.dragging_scrollbar = true;
                self.pl.scroll = self.pl.scroll_for_handle(y, self.pl_rows());
            }
            Hit::Resize => {
                let (width, height) = (self.pl.width as i32, self.pl.window_height() as i32);
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
        let (x, y) = view.cursor_in_skin();

        if self.pl.open_menu.is_some() {
            self.pl.hovered = match playlist_window::hit_test(&self.pl, self.pl_rows(), x, y) {
                Hit::MenuItem(item) => Some(item),
                _ => None,
            };
        }
        if self.pl.dragging_scrollbar {
            self.pl.scroll = self.pl.scroll_for_handle(y, self.pl_rows());
        }
        match self.pl_drag {
            Some(PlDrag::Reorder { row, .. }) => {
                // Which track row the mouse is over, even past the ends.
                let over = self.pl.scroll as isize + (y - self.pl.list_top()).div_euclid(playlist_window::ROW_HEIGHT) as isize;
                let over = over.clamp(0, self.playlist.len() as isize - 1) as usize;
                if over != row {
                    self.playlist.move_selected(over as isize - row as isize);
                    self.pl_drag = Some(PlDrag::Reorder { row: over, moved: true });
                    self.pl.reveal(over, self.playlist.len());
                }
            }
            Some(PlDrag::Resize { offset_x, offset_y }) => {
                let (width, mut height) = PlaylistView::snap_size((x + offset_x) as f64, (y + offset_y) as f64);
                if self.pl.shaded {
                    height = self.pl.height; // in shade mode only the width changes
                }
                if (width, height) != (self.pl.width, self.pl.height) {
                    self.pl.width = width;
                    self.pl.height = height;
                    self.pl.scroll_to(self.pl.scroll as isize, self.pl_rows());
                    if let Some(size) = self.window_size(Kind::Playlist) {
                        view.resize(size);
                    }
                }
            }
            None => {}
        }
    }

    pub(super) fn pl_mouse_up(&mut self, event_loop: &ActiveEventLoop) {
        let Some(view) = &self.pl_view else { return };
        let (x, y) = view.cursor_in_skin();
        let hit = playlist_window::hit_test(&self.pl, self.pl_rows(), x, y);
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
        self.pl.scroll_to(self.pl.scroll as isize + rows, self.pl_rows());
    }

    fn pl_activate(&mut self, control: PlControl) {
        match control {
            PlControl::Close => self.set_playlist_open(false),
            PlControl::Shade => self.toggle_shade(Kind::Playlist),
            PlControl::ScrollUp => self.pl_scroll_by(-1),
            PlControl::ScrollDown => self.pl_scroll_by(1),
            PlControl::Previous => self.previous(),
            PlControl::Play => self.play(),
            PlControl::Pause => self.player.pause(),
            PlControl::Stop => self.stop(),
            PlControl::Next => self.next(),
            PlControl::Eject => self.open_dialog(Purpose::PlayFiles),
        }
    }

    fn pl_menu_item(&mut self, item: MenuItem, event_loop: &ActiveEventLoop) {
        match item {
            MenuItem::AddUrl => self.show_message("internet streams aren't supported yet".to_string()),
            MenuItem::AddFolder => self.open_dialog(Purpose::AddFolder),
            MenuItem::AddFiles => self.open_dialog(Purpose::AddFiles),
            MenuItem::RemoveMisc => {
                self.open_menu(event_loop, Kind::Playlist, vec![Item::new("Remove missing files", Action::RemoveMissingFiles)])
            }
            MenuItem::RemoveAll | MenuItem::NewList => self.playlist.clear(),
            MenuItem::Crop => self.playlist.crop(),
            MenuItem::RemoveSelected => self.playlist.remove_selected(),
            MenuItem::InvertSelection => self.playlist.invert_selection(),
            MenuItem::SelectNone => self.playlist.select_all(false),
            MenuItem::SelectAll => self.playlist.select_all(true),
            MenuItem::SortList => self.open_menu(
                event_loop,
                Kind::Playlist,
                vec![
                    Item::new("Sort list by title", Action::SortByTitle),
                    Item::new("Sort list by filename", Action::SortByFileName),
                    Item::new("Sort list by path and filename", Action::SortByPath),
                    Item::separator(),
                    Item::new("Reverse list", Action::ReverseList),
                    Item::new("Randomize list", Action::RandomizeList),
                ],
            ),
            MenuItem::FileInfo => self.show_file_info(),
            MenuItem::MiscOptions if self.mode != PlayerMode::Local => self.show_sign_in(String::new()),
            MenuItem::MiscOptions => self.show_message("no playlist options yet".to_string()),
            MenuItem::SaveList => self.open_dialog(Purpose::SavePlaylist),
            MenuItem::LoadList => self.open_dialog(Purpose::LoadPlaylist),
        }
        self.pl.scroll_to(self.pl.scroll as isize, self.pl_rows());
    }

    /// Playlist keys: arrows move the selection, Enter plays, Delete
    /// removes, Ctrl/Cmd+A selects all. Returns false for other keys.
    pub(super) fn pl_key_pressed(&mut self, key: &Key) -> bool {
        if self.pl.tab == Some(Tab::Browse) {
            return self.browse_key_pressed(key);
        }
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
                self.pl.scroll_to(self.pl.scroll as isize, self.pl_rows());
            }
            Key::Character(c) if c.eq_ignore_ascii_case("a") && self.command_key() => self.playlist.select_all(true),
            _ => return false,
        }
        true
    }
}
