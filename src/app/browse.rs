//! The playlist window in a music server mode: its tabs, the library
//! browser and the search box.

use std::time::Instant;

use winit::keyboard::{Key, NamedKey};

use super::playlist::DOUBLE_CLICK;
use super::App;
use crate::library::Request;
use crate::ui::browse::Row;
use crate::ui::playlist_window::{self, Tab};

impl App {
    /// How many rows the list showing has: tracks, or the browser's rows.
    pub(super) fn pl_rows(&self) -> usize {
        playlist_window::rows(&self.pl, &self.playlist, &self.browser)
    }

    /// Shows the library or the playlist, each where it was left.
    pub(super) fn set_tab(&mut self, tab: Tab) {
        let Some(showing) = self.pl.tab.filter(|&t| t != tab) else { return };
        match showing {
            Tab::Browse => self.browser.level_mut().scroll = self.pl.scroll,
            Tab::Queue => self.pl.queue_scroll = self.pl.scroll,
        }
        self.pl.scroll = match tab {
            Tab::Browse => self.browser.level().scroll,
            Tab::Queue => self.pl.queue_scroll,
        };
        self.pl.tab = Some(tab);
        self.browser.searching = false;
        self.last_click = None;
    }

    /// A click on a row of the browser: selects it, or a double-click
    /// opens it. The first row below the top goes back up.
    pub(super) fn browse_row_pressed(&mut self, row: usize) {
        let now = Instant::now();
        let double = self.last_click.is_some_and(|(at, r)| r == row && now - at < DOUBLE_CLICK);
        self.last_click = Some((now, row));
        match self.browser.row(row) {
            Some(Row::Back) => {
                self.last_click = None;
                self.browse_back();
            }
            Some(Row::Item(item, _)) => {
                self.browser.level_mut().selected = Some(item);
                if double {
                    self.last_click = None;
                    self.browse_open(item);
                }
            }
            None => {}
        }
    }

    /// Opens an item: the list it leads to, or for a track, plays it.
    fn browse_open(&mut self, item: usize) {
        let Some(item) = self.browser.level().items.get(item).cloned() else { return };
        match item.request() {
            Some(request) => {
                self.browser.open(item.title, self.pl.scroll);
                self.pl.scroll = 0;
                self.request_library(request);
            }
            None => self.show_message(format!("can't play from {} yet", self.mode.name())),
        }
    }

    fn browse_back(&mut self) {
        if self.browser.back() {
            self.pl.scroll = self.browser.level().scroll;
        }
    }

    /// Searches the library for what's in the search box.
    fn search(&mut self) {
        let query = self.browser.search.text().trim().to_string();
        self.browser.searching = false;
        if query.is_empty() {
            return;
        }
        self.browser.top();
        self.browser.open(format!("Search: {query}"), 0);
        self.pl.scroll = 0;
        self.request_library(Request::Search(query));
    }

    /// Asks the server for a list for the browser.
    fn request_library(&mut self, _request: Request) {
        self.show_message(format!("not connected to {} yet", self.mode.name()));
    }

    /// Keys while typing in the search box. `text` is what the key types.
    /// Returns false for keys left to the usual shortcuts.
    pub(super) fn search_key_pressed(&mut self, key: &Key, text: Option<&str>) -> bool {
        let command = self.command_key();
        let field = &mut self.browser.search;
        match key {
            Key::Named(NamedKey::Enter) => self.search(),
            // Escape clears the search, then leaves the box.
            Key::Named(NamedKey::Escape) if field.text().is_empty() => self.browser.searching = false,
            Key::Named(NamedKey::Escape) => field.clear(),
            Key::Named(NamedKey::Backspace) => field.backspace(),
            Key::Named(NamedKey::Delete) => field.delete(),
            Key::Named(NamedKey::ArrowLeft) => field.left(),
            Key::Named(NamedKey::ArrowRight) => field.right(),
            Key::Named(NamedKey::Home | NamedKey::ArrowUp) => field.home(),
            Key::Named(NamedKey::End | NamedKey::ArrowDown) => field.end(),
            _ if command => return false,
            _ => match text.filter(|t| !t.chars().all(char::is_control)) {
                Some(typed) => field.insert(typed),
                None => return false,
            },
        }
        true
    }

    /// Browser keys: arrows move the selection, Right or Enter opens it,
    /// Left or Backspace goes back up, Ctrl/Cmd+F goes to the search box.
    /// Returns false for other keys.
    pub(super) fn browse_key_pressed(&mut self, key: &Key) -> bool {
        let count = self.browser.level().items.len();
        let selected = self.browser.level().selected;
        match key {
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown) if count > 0 => {
                let up = matches!(key, Key::Named(NamedKey::ArrowUp));
                let to = match selected {
                    None => 0,
                    Some(from) if up => from.saturating_sub(1),
                    Some(from) => (from + 1).min(count - 1),
                };
                self.browser.level_mut().selected = Some(to);
                let (row, rows) = (self.browser.row_of(to), self.pl_rows());
                self.pl.reveal(row, rows);
            }
            Key::Named(NamedKey::Enter | NamedKey::ArrowRight) => {
                if let Some(item) = selected {
                    self.browse_open(item);
                }
            }
            Key::Named(NamedKey::ArrowLeft | NamedKey::Backspace) => self.browse_back(),
            Key::Character(c) if c.eq_ignore_ascii_case("f") && self.command_key() => self.browser.searching = true,
            _ => return false,
        }
        true
    }
}
