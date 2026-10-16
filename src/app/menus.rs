//! Pop-up menus: building them, showing them, and doing what was chosen.

use std::path::{Path, PathBuf};

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;

use super::{App, Kind, Show, View, file_name, is_skin, open_skin};
use crate::audio::{self, PlayState};
use crate::config;
use crate::eqf;
use crate::ui::eq_window;
use crate::ui::canvas::Canvas;
use crate::ui::menu::{self, Action, Item, Menu};
use crate::ui::text::Font;
use crate::vis::Mode;

/// The skin browser lists at most this many skins.
const MAX_SKINS: usize = 40;

/// "Ctrl" on Linux, "Cmd" on macOS, for shortcut labels.
const COMMAND: &str = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };

impl App {
    /// Opens a pop-up menu at the mouse position in window `over`.
    pub(super) fn open_menu(&mut self, event_loop: &ActiveEventLoop, over: Kind, items: Vec<Item>) {
        self.close_text_menu();
        self.clear_hover();
        let menu = Menu::new(items);
        let Some(parent) = self.view(over) else { return };
        let scale = parent.window.scale_factor();
        let Ok(origin) = parent.window.outer_position() else { return };
        let position = PhysicalPosition::new(origin.x + parent.cursor.x as i32, origin.y + parent.cursor.y as i32);

        // Drawn at the screen's real pixel size, so the text is sharp.
        if self.menu_font.as_ref().is_none_or(|(_, s)| *s != scale) {
            self.menu_font = Font::load("Tahoma", menu::FONT_PX * scale as f32).map(|font| (font, scale));
        }
        let Some((font, _)) = self.menu_font.as_mut() else { return };
        let (width, height) = menu.size(font, scale);
        let size = PhysicalSize::new(width, height);
        let points = size.to_logical::<f64>(scale);
        let (w, h) = (points.width.ceil() as u32, points.height.ceil() as u32);
        let mut view = View::new(event_loop, "menu", w, h, 1, Show::at(position, true));
        let _ = view.window.request_inner_size(size);
        view.canvas = Canvas::new(width, height);
        if self.always_on_top {
            view.window.set_window_level(winit::window::WindowLevel::AlwaysOnTop);
        }
        view.window.focus_window();
        self.menu_view = Some(view);
        self.menu_opened_at = Some(std::time::Instant::now());
        self.menu = Some(menu);
    }

    pub(super) fn close_text_menu(&mut self) {
        self.menu_view = None; // dropping the window closes it
        self.menu = None;
    }

    pub(super) fn menu_mouse_moved(&mut self) {
        let (Some(view), Some(menu)) = (&self.menu_view, &mut self.menu) else { return };
        // The menu is drawn one canvas pixel per screen pixel.
        menu.hovered = menu.item_at(view.cursor.y as i32, view.window.scale_factor());
    }

    pub(super) fn menu_mouse_up(&mut self, event_loop: &ActiveEventLoop) {
        let action = self.menu.as_ref().and_then(|m| m.action(m.hovered?));
        self.close_text_menu();
        if let Some(action) = action {
            self.menu_action(action, event_loop);
        }
    }

    // ----- The menus -----

    /// The main menu: from the "O" buttons or a right-click.
    pub(super) fn open_options_menu(&mut self, event_loop: &ActiveEventLoop, over: Kind) {
        let mode = self.visualizer.mode;
        let items = vec![
            Item::new("Play file(s)...", Action::PlayFiles).shortcut("L"),
            Item::new("Play folder...", Action::PlayFolder),
            Item::separator(),
            Item::new("Skin browser...", Action::SkinBrowser).shortcut("Alt+S"),
            Item::separator(),
            Item::new("Equalizer", Action::ToggleEqualizer).checked(self.state.equalizer_open).shortcut("Alt+G"),
            Item::new("Playlist editor", Action::TogglePlaylist).checked(self.state.playlist_open).shortcut("Alt+E"),
            Item::separator(),
            Item::new("Double size", Action::ToggleDoubleSize).checked(self.double_size).shortcut(&format!("{COMMAND}+D")),
            Item::new("Always on top", Action::ToggleAlwaysOnTop).checked(self.always_on_top).shortcut(&format!("{COMMAND}+T")),
            Item::separator(),
            Item::new("Spectrum analyzer", Action::Visualizer(Mode::Analyzer)).checked(mode == Mode::Analyzer),
            Item::new("Oscilloscope", Action::Visualizer(Mode::Oscilloscope)).checked(mode == Mode::Oscilloscope),
            Item::new("No visualization", Action::Visualizer(Mode::Off)).checked(mode == Mode::Off),
            Item::separator(),
            Item::new("File info...", Action::FileInfo),
            Item::new("About skinamp...", Action::About),
            Item::separator(),
            Item::new("Exit", Action::Exit),
        ];
        self.open_menu(event_loop, over, items);
    }

    /// The clutter bar's "V" button: just the visualizer choices.
    pub(super) fn open_visualizer_menu(&mut self, event_loop: &ActiveEventLoop) {
        let mode = self.visualizer.mode;
        let items = vec![
            Item::new("Spectrum analyzer", Action::Visualizer(Mode::Analyzer)).checked(mode == Mode::Analyzer),
            Item::new("Oscilloscope", Action::Visualizer(Mode::Oscilloscope)).checked(mode == Mode::Oscilloscope),
            Item::new("No visualization", Action::Visualizer(Mode::Off)).checked(mode == Mode::Off),
        ];
        self.open_menu(event_loop, Kind::Main, items);
    }

    /// Lists skins in the skins folder and next to the current skin.
    pub(super) fn open_skin_browser(&mut self, event_loop: &ActiveEventLoop, over: Kind) {
        self.skin_choices = self.find_skins();
        let current = self.skin_path.clone();
        let mut items = vec![Item::new("<no skin>", Action::BaseSkin).checked(current.is_none())];
        for (i, path) in self.skin_choices.iter().enumerate() {
            let checked = current.as_deref().is_some_and(|c| same_file(c, path));
            items.push(Item::new(file_name(path), Action::Skin(i)).checked(checked));
        }
        items.push(Item::separator());
        items.push(Item::new("Load skin file...", Action::LoadSkinFile));
        items.push(Item::new("Open skins folder", Action::OpenSkinsFolder));
        self.open_menu(event_loop, over, items);
    }

    fn find_skins(&self) -> Vec<PathBuf> {
        let current_folder = self.skin_path.as_deref().and_then(Path::parent).map(Path::to_path_buf);
        let folders = config::skins_folder().into_iter().chain(current_folder);
        let mut skins: Vec<PathBuf> = Vec::new();
        for folder in folders {
            let Ok(entries) = std::fs::read_dir(&folder) else { continue };
            for path in entries.flatten().map(|e| e.path()) {
                if is_skin(&path) && !skins.iter().any(|s| same_file(s, &path)) {
                    skins.push(path);
                }
            }
        }
        skins.sort_by_key(|p| file_name(p).to_lowercase());
        skins.truncate(MAX_SKINS);
        skins
    }

    /// The equalizer's presets button.
    pub(super) fn open_presets_menu(&mut self, event_loop: &ActiveEventLoop) {
        let mut items = vec![
            Item::new("Load preset from file...", Action::LoadPresetFile),
            Item::new("Save preset to file...", Action::SavePresetFile),
            Item::separator(),
        ];
        for (i, (name, ..)) in eqf::BUILT_IN.iter().enumerate() {
            items.push(Item::new(*name, Action::Preset(i)));
        }
        if let Some(key) = self.auto_preset_key() {
            items.push(Item::separator());
            items.push(Item::new("Save auto-load preset for this song", Action::SaveAutoPreset));
            if self.auto_presets.contains_key(&key) {
                items.push(Item::new("Delete this song's auto-load preset", Action::DeleteAutoPreset));
            }
        }
        self.open_menu(event_loop, Kind::Equalizer, items);
    }

    /// The name auto-load presets are filed under: the current song's file name.
    pub(super) fn auto_preset_key(&self) -> Option<String> {
        let info = self.player.info()?;
        Some(info.path.file_name()?.to_string_lossy().into_owned())
    }

    /// When "auto" is on, applies the current song's saved preset, if any.
    pub(super) fn apply_auto_preset(&mut self) {
        if !self.eq.auto {
            return;
        }
        if let Some(values) = self.auto_preset_key().and_then(|key| self.auto_presets.get(&key).copied()) {
            self.eq.preamp = values[0];
            self.eq.bands.copy_from_slice(&values[1..]);
            self.apply_equalizer();
        }
    }

    // ----- Doing what was chosen -----

    fn menu_action(&mut self, action: Action, event_loop: &ActiveEventLoop) {
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

            Action::PlayFiles => self.open_files_dialog(),
            Action::PlayFolder => {
                if let Some(folder) = rfd::FileDialog::new().set_title("Play folder").pick_folder() {
                    self.add_tracks(vec![folder], true);
                }
            }
            Action::SkinBrowser => self.open_skin_browser(event_loop, Kind::Main),
            Action::ToggleEqualizer => self.set_equalizer_open(!self.state.equalizer_open),
            Action::TogglePlaylist => self.set_playlist_open(!self.state.playlist_open),
            Action::ToggleDoubleSize => self.toggle_double_size(),
            Action::ToggleAlwaysOnTop => self.set_always_on_top(!self.always_on_top),
            Action::Visualizer(mode) => self.visualizer.mode = mode,
            Action::FileInfo => self.show_file_info(),
            Action::About => {
                rfd::MessageDialog::new()
                    .set_title("About skinamp")
                    .set_description(
                        "skinamp: a music player for classic Winamp 2.x skins.\n\n\
                         Skin layout and visualizer behavior follow Webamp by Jordan \
                         Eldredge and contributors (MIT license).\n\n\
                         Not affiliated with Winamp or Llama Group.",
                    )
                    .show();
            }
            Action::Exit => event_loop.exit(),

            Action::Skin(index) => {
                if let Some(path) = self.skin_choices.get(index).cloned() {
                    match open_skin(&path) {
                        Ok(skin) => self.set_skin(skin, path),
                        Err(err) => self.show_message(format!("{}: {err:#}", file_name(&path))),
                    }
                }
            }
            Action::BaseSkin => {
                self.set_skin(crate::skin::Skin::empty(), PathBuf::new());
                self.skin_path = None;
                self.skin_name.clear();
            }
            Action::LoadSkinFile => {
                let dialog = rfd::FileDialog::new().set_title("Load skin").add_filter("Winamp skins", &["wsz", "zip"]);
                if let Some(path) = dialog.pick_file() {
                    match open_skin(&path) {
                        Ok(skin) => self.set_skin(skin, path),
                        Err(err) => self.show_message(format!("{}: {err:#}", file_name(&path))),
                    }
                }
            }
            Action::OpenSkinsFolder => {
                if let Some(folder) = config::skins_folder() {
                    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
                    if let Err(err) = std::process::Command::new(opener).arg(&folder).spawn() {
                        self.show_message(format!("could not open {}: {err}", folder.display()));
                    }
                }
            }

            Action::LoadPresetFile => self.load_preset(),
            Action::SavePresetFile => self.save_preset(),
            Action::Preset(index) => {
                if let Some((name, preamp_db, bands_db)) = eqf::BUILT_IN.get(index) {
                    self.eq.preamp = eq_window::from_db(*preamp_db);
                    self.eq.bands = bands_db.map(eq_window::from_db);
                    self.apply_equalizer();
                    self.show_message(format!("eq preset: {name}"));
                }
            }
            Action::SaveAutoPreset => {
                if let Some(key) = self.auto_preset_key() {
                    let mut values = [self.eq.preamp; 11];
                    values[1..].copy_from_slice(&self.eq.bands);
                    self.auto_presets.insert(key, values);
                    self.show_message("saved auto-load preset for this song".to_string());
                }
            }
            Action::DeleteAutoPreset => {
                if let Some(key) = self.auto_preset_key() {
                    self.auto_presets.remove(&key);
                }
            }
        }
        if self.state.play_state == PlayState::Stopped && matches!(action, Action::Visualizer(_)) {
            self.visualizer.reset();
        }
        self.pl.scroll_to(self.pl.scroll as isize, self.playlist.len());
        self.redraw_all();
    }

    /// Shows details of the first selected track (or the playing one).
    pub(super) fn show_file_info(&mut self) {
        let index = self.playlist.first_selected().or(self.playlist.current_index());
        let Some(entry) = index.map(|i| &self.playlist.entries()[i]) else { return };
        let mut text = format!("{}\n\n{}", entry.title, entry.path.display());
        match audio::read_info(&entry.path) {
            Ok(info) => {
                let length = info.duration.map_or("unknown".into(), |d| {
                    crate::ui::playlist_window::format_duration(d.round() as u64)
                });
                text += &format!(
                    "\n\nLength: {length}\nSample rate: {} Hz\nChannels: {}",
                    info.sample_rate, info.channels
                );
                if let Some(kbps) = info.kbps {
                    text += &format!("\nBitrate: {kbps} kbps");
                }
            }
            Err(err) => text += &format!("\n\nCan't be played: {err:#}"),
        }
        rfd::MessageDialog::new().set_title("File info").set_description(text).show();
    }
}

/// True if two paths name the same file (allowing for different spellings).
fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}
