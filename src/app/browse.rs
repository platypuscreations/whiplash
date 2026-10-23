//! The playlist window in a music server mode: its tabs, the library
//! browser, searching, signing in to the server, and playing what's chosen.

use std::time::Instant;

use winit::keyboard::{Key, NamedKey};

use super::playlist::DOUBLE_CLICK;
use super::{App, UserEvent};
use crate::audio::Source;
use crate::config::{EmbyConfig, PlayerMode, SqueezeConfig};
use crate::library::{self, Item, ItemKind, Job, Library, Reply, Request, ServerConfig, SignedOut, emby, squeeze};
use crate::location::Location;
use crate::ui::browse::{Browser, FORM_FIELDS, Field, PASSWORD, Row};
use crate::ui::playlist_window::{self, Tab};
use crate::ui::text_field::TextField;

/// What a request to the library was for.
pub(super) enum Wanted {
    /// The list the browser is waiting for.
    List,
    /// Tracks to add to the playlist.
    Queue,
    SignIn,
}

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
        self.browser.focus = None;
        self.last_click = None;
    }

    // ----- The server -----

    /// The mode's server and how we're signed in to it; none for local files.
    fn server_config(&self) -> Option<ServerConfig> {
        match self.mode {
            PlayerMode::Local => None,
            PlayerMode::Emby => Some(ServerConfig::Emby(self.emby.clone())),
            PlayerMode::Squeeze => Some(ServerConfig::Squeeze(self.squeeze.clone())),
        }
    }

    fn signed_in(&self) -> bool {
        match self.mode {
            PlayerMode::Local => false,
            PlayerMode::Emby => !self.emby.token.is_empty(),
            PlayerMode::Squeeze => !self.squeeze.url.is_empty(),
        }
    }

    /// Starts talking to the mode's server, after switching mode or
    /// signing in. Shows the sign-in form if we're not signed in.
    pub(super) fn connect_library(&mut self) {
        self.library_jobs.clear();
        self.library = self.server_config().map(|config| {
            let proxy = self.proxy.clone();
            Library::connect(config, move || {
                let _ = proxy.send_event(UserEvent::Library);
            })
        });
        self.pl.signing_in = false;
        if self.mode != PlayerMode::Local && !self.signed_in() {
            self.show_sign_in(String::new());
        }
    }

    /// Shows the sign-in form, filled in with what's known, with `status`
    /// beside its button.
    pub(super) fn show_sign_in(&mut self, status: String) {
        let (url, user, password) = match self.mode {
            PlayerMode::Emby => (self.emby.url.clone(), self.emby.user.clone(), String::new()),
            PlayerMode::Squeeze => (self.squeeze.url.clone(), self.squeeze.user.clone(), self.squeeze.password.clone()),
            PlayerMode::Local => return,
        };
        self.browser.form_hints = match self.mode {
            PlayerMode::Squeeze => ["Server address (port 9000)", "User name, if it has one", "Password, if it has one"],
            _ => ["Server address (port 8096)", "User name", "Password"],
        };
        for (field, text) in self.browser.form.iter_mut().zip([url, user, password]) {
            *field = TextField::default();
            field.insert(&text);
        }
        self.browser.form_status = status;
        self.pl.signing_in = true;
        self.set_tab(Tab::Browse);
        self.pl.scroll = 0;
        // Typing starts in the first empty field. (A Logitech Media
        // Server usually has no user name or password.)
        let empty = self.browser.form.iter().position(|f| f.text().is_empty());
        let first = if self.mode == PlayerMode::Squeeze { Some(0) } else { empty };
        self.browser.focus = Some(Field::Form(first.unwrap_or(PASSWORD)));
    }

    /// Signs in with what's in the form.
    pub(super) fn sign_in(&mut self) {
        let [url, user, password] = self.browser.form.clone().map(|f| f.text().trim().to_string());
        if url.is_empty() || (user.is_empty() && self.mode == PlayerMode::Emby) {
            self.browser.form_status = "Fill in the address and user name".to_string();
            return;
        }
        let config = match self.mode {
            PlayerMode::Emby => {
                if self.emby.device_id.is_empty() {
                    self.emby.device_id = format!("{:016x}{:016x}", fastrand::u64(..), fastrand::u64(..));
                }
                let device_id = self.emby.device_id.clone();
                ServerConfig::Emby(EmbyConfig { url: library::tidy_url(&url, 8096), user: user.clone(), device_id, ..EmbyConfig::default() })
            }
            PlayerMode::Squeeze => ServerConfig::Squeeze(SqueezeConfig { url: library::tidy_url(&url, 9000), ..SqueezeConfig::default() }),
            PlayerMode::Local => return,
        };
        let proxy = self.proxy.clone();
        let mut library = Library::connect(config, move || {
            let _ = proxy.send_event(UserEvent::Library);
        });
        self.library_jobs.clear();
        self.library_jobs.insert(library.send(Job::SignIn { user, password }), Wanted::SignIn);
        self.library = Some(library);
        self.browser.form_status = "Signing in...".to_string();
        self.browser.focus = None;
    }

    /// Asks the library for a list. Returns the request's ticket.
    fn request_library(&mut self, request: Request, wanted: Wanted) -> Option<u64> {
        let ticket = self.library.as_mut()?.send(Job::List(request));
        self.library_jobs.insert(ticket, wanted);
        Some(ticket)
    }

    /// Acts on the library's answers.
    pub(super) fn library_replies(&mut self) {
        let replies: Vec<_> = self.library.as_ref().map(|l| l.replies().collect()).unwrap_or_default();
        for (ticket, reply) in replies {
            let Some(wanted) = self.library_jobs.remove(&ticket) else { continue };
            match (wanted, reply) {
                (Wanted::SignIn, Ok(Reply::SignedIn(config))) => {
                    match config {
                        ServerConfig::Emby(config) => self.emby = config,
                        ServerConfig::Squeeze(config) => self.squeeze = config,
                    }
                    self.browser = Browser::default();
                    self.connect_library();
                    self.show_message(format!("signed in to {}", self.mode.name()));
                }
                (Wanted::SignIn, Err(err)) => self.browser.form_status = format!("{err:#}"),
                (_, Err(err)) if err.is::<SignedOut>() => self.show_sign_in("Please sign in again".to_string()),
                (Wanted::List, reply) => {
                    let level = self.browser.level_mut();
                    if level.pending != Some(ticket) {
                        continue; // the browser has moved on
                    }
                    level.pending = None;
                    match reply {
                        Ok(Reply::Items(items)) => level.items = items,
                        Ok(Reply::SignedIn(_)) => {}
                        Err(err) => self.show_error(format!("{}: {err:#}", self.mode.name())),
                    }
                }
                (Wanted::Queue, Ok(Reply::Items(items))) => {
                    let added = self.queue_items(&items);
                    self.show_message(format!("added {added} track(s) to the playlist"));
                }
                (Wanted::Queue, Err(err)) => self.show_error(format!("{}: {err:#}", self.mode.name())),
                (Wanted::Queue | Wanted::SignIn, Ok(_)) => {}
            }
        }
        self.pl.scroll_to(self.pl.scroll as isize, self.pl_rows());
    }

    /// Where a track of the mode's library plays from: its address on the
    /// server, with `title` to show if it has no tags. None if we're not
    /// signed in.
    pub(super) fn web_source(&self, location: &Location, title: &str) -> Option<Source> {
        let (url, authorization) = match location {
            Location::Emby(id) if !self.emby.token.is_empty() => (emby::stream_url(&self.emby, id), None),
            Location::Squeeze(id) if !self.squeeze.url.is_empty() => {
                (squeeze::stream_url(&self.squeeze, id), squeeze::authorization(&self.squeeze))
            }
            _ => return None,
        };
        Some(Source::Http { url, authorization, title: title.to_string() })
    }

    /// A playlist entry's location for a track of the mode's library.
    fn track_location(&self, id: &str) -> Location {
        match self.mode {
            PlayerMode::Emby => Location::Emby(id.to_string()),
            PlayerMode::Squeeze => Location::Squeeze(id.to_string()),
            // Local files have no library to browse.
            PlayerMode::Local => Location::File(id.into()),
        }
    }

    /// Adds the tracks among `items` to the end of the playlist. Returns
    /// how many there were.
    fn queue_items(&mut self, items: &[Item]) -> usize {
        let tracks: Vec<_> = items
            .iter()
            .filter(|item| item.kind == ItemKind::Track)
            .map(|item| (self.track_location(&item.id), item.title.clone(), item.duration))
            .collect();
        let count = tracks.len();
        self.playlist.append_known(tracks);
        count
    }

    // ----- Browsing -----

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

    /// Opens an item: the list it leads to, or for a track, plays the
    /// tracks of the list from that one on.
    fn browse_open(&mut self, index: usize) {
        let Some(item) = self.browser.level().items.get(index).cloned() else { return };
        match item.request() {
            Some(request) => {
                let ticket = self.request_library(request, Wanted::List);
                self.browser.open(item.title, self.pl.scroll, ticket);
                self.pl.scroll = 0;
            }
            None => self.play_from_browser(index),
        }
    }

    /// Replaces the playlist with the tracks of the list shown and plays
    /// them, starting with item `index`.
    fn play_from_browser(&mut self, index: usize) {
        let items = self.browser.level().items.clone();
        let start = items[..index].iter().filter(|item| item.kind == ItemKind::Track).count();
        self.player.clear();
        self.playlist.clear();
        self.queue_items(&items);
        self.playlist.set_current(start);
        self.pl.queue_scroll = start.saturating_sub(1);
        self.play_current();
    }

    fn browse_back(&mut self) {
        if self.browser.back() {
            self.pl.scroll = self.browser.level().scroll;
        }
    }

    /// The ADD button in a music server mode: adds what's selected in the
    /// library to the playlist. On the Queue tab, it goes to the library.
    pub(super) fn add_from_library(&mut self) {
        if self.pl.tab == Some(Tab::Queue) || self.pl.signing_in {
            return self.set_tab(Tab::Browse);
        }
        let level = self.browser.level();
        let Some(item) = level.selected.and_then(|i| level.items.get(i)).cloned() else {
            return self.show_message("choose something in the library to add".to_string());
        };
        let request = match item.kind {
            ItemKind::Track => {
                self.queue_items(&[item]);
                return self.show_message("added 1 track to the playlist".to_string());
            }
            ItemKind::Album => Request::Tracks { album: item.id },
            ItemKind::Artist => Request::ArtistTracks { artist: item.id },
            ItemKind::Folder => return self.show_message(format!("open {} to add from it", item.title.to_lowercase())),
        };
        self.request_library(request, Wanted::Queue);
        self.show_message(format!("adding {}...", item.title));
    }

    /// Searches the library for what's in the search box.
    fn search(&mut self) {
        let query = self.browser.search.text().trim().to_string();
        self.browser.focus = None;
        if query.is_empty() {
            return;
        }
        self.browser.top();
        let ticket = self.request_library(Request::Search(query.clone()), Wanted::List);
        self.browser.open(format!("Search: {query}"), 0, ticket);
        self.pl.scroll = 0;
    }

    // ----- Keys -----

    /// Keys while typing in the search box or the sign-in form. `text` is
    /// what the key types; Ctrl+V (Cmd+V) pastes. Returns false for keys
    /// left to the usual shortcuts.
    pub(super) fn field_key_pressed(&mut self, key: &Key, text: Option<&str>) -> bool {
        let Some(focus) = self.browser.focus else { return false };
        let command = self.command_key();
        let shift = self.modifiers.shift_key();
        let next_field = |i: usize, back: bool| Field::Form((i + if back { FORM_FIELDS - 1 } else { 1 }) % FORM_FIELDS);
        let field = match focus {
            Field::Search => &mut self.browser.search,
            Field::Form(i) => &mut self.browser.form[i],
        };
        match (key, focus) {
            (Key::Named(NamedKey::Enter), Field::Search) => self.search(),
            (Key::Named(NamedKey::Enter), Field::Form(_)) => self.sign_in(),
            (Key::Named(NamedKey::Tab), Field::Form(i)) => self.browser.focus = Some(next_field(i, shift)),
            (Key::Named(NamedKey::ArrowDown), Field::Form(i)) => self.browser.focus = Some(next_field(i, false)),
            (Key::Named(NamedKey::ArrowUp), Field::Form(i)) => self.browser.focus = Some(next_field(i, true)),
            // Escape clears the search, then leaves the box.
            (Key::Named(NamedKey::Escape), Field::Search) if !field.text().is_empty() => field.clear(),
            (Key::Named(NamedKey::Escape), _) => self.browser.focus = None,
            (Key::Named(NamedKey::Backspace), _) => field.backspace(),
            (Key::Named(NamedKey::Delete), _) => field.delete(),
            (Key::Named(NamedKey::ArrowLeft), _) => field.left(),
            (Key::Named(NamedKey::ArrowRight), _) => field.right(),
            (Key::Named(NamedKey::Home | NamedKey::ArrowUp), _) => field.home(),
            (Key::Named(NamedKey::End | NamedKey::ArrowDown), _) => field.end(),
            (Key::Named(NamedKey::Tab), _) => {}
            (Key::Character(c), _) if command && c.eq_ignore_ascii_case("v") => field.insert(&clipboard_line()),
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
        if self.pl.signing_in {
            return false;
        }
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
            Key::Character(c) if c.eq_ignore_ascii_case("f") && self.command_key() => {
                self.browser.focus = Some(Field::Search);
            }
            _ => return false,
        }
        true
    }
}

/// The first line of text on the clipboard, trimmed; empty if there's none.
fn clipboard_line() -> String {
    let text = arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()).unwrap_or_default();
    text.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or_default().to_string()
}
