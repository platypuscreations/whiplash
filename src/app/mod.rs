//! The application: owns the windows, the skin, the player and the
//! playlist, and turns mouse and keyboard input into player actions.
//!
//! The main window's behavior lives here; the equalizer and playlist
//! windows have their own files.

mod equalizer;
mod menus;
mod playlist;
mod windows;

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Result;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId, WindowLevel};

use crate::audio::{self, PlayState, Player};
use crate::config::{self, Config, WindowsConfig};
use crate::metadata::MetadataReader;
use crate::playlist::Playlist;
use crate::playlist_file;
use crate::skin::Skin;
use crate::ui::canvas::Canvas;
use crate::ui::eq_window::{self, EqState};
use crate::ui::main_window::{self, Control, Hit, MainState, Slider};
use crate::ui::menu::{self, Menu};
use crate::ui::playlist_window::{self, PlaylistView};
use crate::ui::text::Font;
use crate::vis::Visualizer;
use playlist::PlDrag;
use windows::WindowDrag;

/// How often the display updates while something is moving (about 30 fps).
const FRAME: Duration = Duration::from_millis(33);
/// How often a long title scrolls by one character.
const MARQUEE_STEP: Duration = Duration::from_millis(200);
/// How long messages stay in the marquee.
const MESSAGE_TIME: Duration = Duration::from_secs(4);
/// Files dropped this close together count as one multi-file drop.
const DROP_BURST: Duration = Duration::from_millis(300);
/// Arrow keys seek by this many seconds.
const SEEK_STEP: f64 = 5.0;

/// Messages from other threads that wake up the event loop.
#[derive(Debug)]
pub enum UserEvent {
    /// The metadata thread has read some tracks' details.
    Metadata,
    /// Ctrl+C was pressed in the terminal: quit, saving settings.
    Quit,
}

/// Which of the app's windows something refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Main,
    Equalizer,
    Playlist,
    /// A pop-up text menu.
    Menu,
}

const KINDS: [Kind; 4] = [Kind::Main, Kind::Equalizer, Kind::Playlist, Kind::Menu];

/// A window, the surface we draw its pixels onto, and its off-screen
/// canvas. Windows can only be created once the event loop is running, so
/// the app holds these in `Option`s.
struct View {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    canvas: Canvas,
    /// Last known mouse position over this window, in physical pixels.
    cursor: PhysicalPosition<f64>,
}

impl View {
    fn new(
        event_loop: &ActiveEventLoop,
        title: &str,
        width: u32,
        height: u32,
        zoom: u32,
        visible: bool,
        position: Option<PhysicalPosition<i32>>,
    ) -> View {
        let mut attributes = Window::default_attributes()
            .with_title(title)
            .with_decorations(false) // the skin draws its own title bar
            .with_resizable(false)
            .with_visible(visible)
            .with_inner_size(LogicalSize::new(width * zoom, height * zoom));
        if let Some(position) = position {
            attributes = attributes.with_position(position);
        }
        let window = Rc::new(event_loop.create_window(attributes).expect("failed to create window"));
        let context = softbuffer::Context::new(window.clone()).expect("failed to create graphics context");
        let surface = softbuffer::Surface::new(&context, window.clone()).expect("failed to create surface");
        View { window, surface, canvas: Canvas::new(width, height), cursor: PhysicalPosition::new(0.0, 0.0) }
    }

    /// Copies the canvas to the screen, scaled up to fill the window.
    fn present(&mut self) -> Result<()> {
        let size = self.window.inner_size();
        let (Some(width), Some(height)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return Ok(()); // minimized
        };
        self.surface.resize(width, height).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut buffer = self.surface.buffer_mut().map_err(|e| anyhow::anyhow!("{e}"))?;
        // Pick the largest whole-number scale that fits, so every skin pixel
        // becomes an equal square block (2x on Retina, 4x in double size).
        let scale = (size.width / self.canvas.width).min(size.height / self.canvas.height).max(1);
        self.canvas.scale_into(&mut buffer, size.width, size.height, scale);
        buffer.present().map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    /// The mouse position in the window's unscaled skin layout.
    fn cursor_in_skin(&self, zoom: u32) -> (i32, i32) {
        let logical = self.cursor.to_logical::<f64>(self.window.scale_factor());
        let zoom = zoom as f64;
        ((logical.x / zoom).floor() as i32, (logical.y / zoom).floor() as i32)
    }
}

pub struct App {
    skin: Skin,
    skin_name: String,
    /// Where the skin came from, to remember it for next time.
    skin_path: Option<PathBuf>,
    state: MainState,
    eq: EqState,
    pl: PlaylistView,
    main_view: Option<View>,
    eq_view: Option<View>,
    pl_view: Option<View>,
    menu_view: Option<View>,
    /// The open pop-up text menu, if any.
    menu: Option<Menu>,
    /// The skin's playlist font, and the font for pop-up menus.
    pl_font: Option<Font>,
    menu_font: Option<Font>,
    player: Player,
    playlist: Playlist,
    metadata: MetadataReader,
    visualizer: Visualizer,
    modifiers: ModifiersState,
    double_size: bool,
    always_on_top: bool,
    /// Window positions and states from the last run, used as the
    /// windows are first shown.
    saved_positions: WindowsConfig,
    /// Per-song equalizer settings for the EQ's "auto" button.
    auto_presets: std::collections::HashMap<String, [f32; 11]>,
    window_drag: Option<WindowDrag>,
    /// When and where the title bar was last clicked, to spot double-clicks.
    last_title_click: Option<(Kind, Instant)>,
    /// The skins listed in the open skin browser.
    skin_choices: Vec<PathBuf>,
    /// Windows that have been on screen, whose positions are worth saving.
    shown: Vec<Kind>,
    /// A temporary message for the marquee, and when it was set.
    message: Option<(String, Instant)>,
    /// When the app started; drives the blinking time display.
    clock: Instant,
    next_frame: Instant,
    marquee_scrolled_at: Instant,
    last_drop: Option<Instant>,
    /// When and on which track the last playlist click was.
    last_click: Option<(Instant, usize)>,
    pl_drag: Option<PlDrag>,
}

impl App {
    /// Creates the app with the given skin and saved settings. `tracks`
    /// from the command line replace the saved playlist.
    pub fn new(
        skin: Skin,
        skin_path: Option<PathBuf>,
        config: Config,
        tracks: Vec<PathBuf>,
        proxy: EventLoopProxy<UserEvent>,
    ) -> App {
        let now = Instant::now();
        let pl_font = Font::load(&skin.playlist.font, playlist_window::FONT_PX);
        let skin_name = skin_path.as_deref().map(file_name).unwrap_or_default();
        let windows = config.windows.clone();
        let mut visualizer = Visualizer::new();
        visualizer.mode = config.visualizer;
        let (pl_width, pl_height) = match windows.playlist_size {
            Some([w, h]) => PlaylistView::snap_size(w as f64, h as f64),
            None => (playlist_window::MIN_WIDTH, PlaylistView::default().height),
        };
        let mut app = App {
            skin,
            skin_name,
            skin_path,
            state: MainState {
                // The windows are opened once they exist (in `resumed`).
                playlist_open: false,
                volume: config.volume,
                balance: config.balance,
                shuffle: config.shuffle,
                repeat: config.repeat,
                time_remaining: config.time_remaining,
                shaded: windows.main_shaded,
                ..MainState::default()
            },
            eq: EqState {
                enabled: config.equalizer.enabled,
                auto: config.equalizer.auto,
                preamp: config.equalizer.preamp,
                bands: config.equalizer.bands,
                shaded: windows.equalizer_shaded,
                ..EqState::default()
            },
            pl: PlaylistView {
                width: pl_width,
                height: pl_height,
                shaded: windows.playlist_shaded,
                ..PlaylistView::default()
            },
            main_view: None,
            eq_view: None,
            pl_view: None,
            menu_view: None,
            menu: None,
            pl_font,
            menu_font: Font::load("Tahoma", menu::FONT_PX),
            player: Player::new(),
            playlist: Playlist::default(),
            metadata: MetadataReader::new(move || {
                let _ = proxy.send_event(UserEvent::Metadata);
            }),
            visualizer,
            modifiers: ModifiersState::empty(),
            double_size: config.double_size,
            always_on_top: config.always_on_top,
            saved_positions: windows,
            auto_presets: config.equalizer.auto_presets,
            window_drag: None,
            shown: vec![Kind::Main],
            skin_choices: Vec::new(),
            last_title_click: None,
            message: None,
            clock: now,
            next_frame: now,
            marquee_scrolled_at: now,
            last_drop: None,
            last_click: None,
            pl_drag: None,
        };
        app.player.set_volume(app.state.volume);
        app.player.set_balance(app.state.balance);
        app.apply_equalizer();
        if !tracks.is_empty() {
            app.add_tracks(tracks, true);
        } else {
            app.restore_playlist(config.playlist_current);
        }
        app
    }

    /// Reloads the playlist saved last time, without starting playback.
    fn restore_playlist(&mut self, current: Option<usize>) {
        let Some(path) = config::playlist_path().filter(|p| p.exists()) else { return };
        match playlist_file::load(&path) {
            Ok(tracks) => {
                let added = self.playlist.replace(tracks);
                self.metadata.request(added);
                if let Some(index) = current.filter(|&i| i < self.playlist.len()) {
                    self.playlist.set_current(index);
                    self.pl.reveal(index, self.playlist.len());
                }
            }
            Err(err) => eprintln!("could not restore the playlist: {err:#}"),
        }
    }

    /// Saves settings, window positions and the playlist for next time.
    fn save_settings(&self) {
        let position = |kind| {
            let view = self.view(kind).filter(|_| self.shown.contains(&kind))?;
            view.window.outer_position().ok().map(|p| [p.x, p.y])
        };
        let config = Config {
            skin: self.skin_path.clone(),
            volume: self.state.volume,
            balance: self.state.balance,
            shuffle: self.state.shuffle,
            repeat: self.state.repeat,
            time_remaining: self.state.time_remaining,
            double_size: self.double_size,
            always_on_top: self.always_on_top,
            visualizer: self.visualizer.mode,
            playlist_current: self.playlist.current_index(),
            equalizer: config::EqualizerConfig {
                enabled: self.eq.enabled,
                auto: self.eq.auto,
                preamp: self.eq.preamp,
                bands: self.eq.bands,
                auto_presets: self.auto_presets.clone(),
            },
            windows: WindowsConfig {
                main: position(Kind::Main),
                equalizer: position(Kind::Equalizer).or(self.saved_positions.equalizer),
                playlist: position(Kind::Playlist).or(self.saved_positions.playlist),
                equalizer_open: self.state.equalizer_open,
                playlist_open: self.state.playlist_open,
                playlist_size: Some([self.pl.width, self.pl.height]),
                main_shaded: self.state.shaded,
                equalizer_shaded: self.eq.shaded,
                playlist_shaded: self.pl.shaded,
            },
        };
        if let Err(err) = config.save() {
            eprintln!("could not save settings: {err:#}");
        }
        if let Some(path) = config::playlist_path()
            && let Err(err) = playlist_file::save_m3u(&path, self.playlist.entries())
        {
            eprintln!("could not save the playlist: {err:#}");
        }
    }

    // ----- Player actions -----

    fn play_current(&mut self) {
        let Some(path) = self.playlist.current().map(Path::to_path_buf) else { return };
        match self.player.open(&path) {
            Ok(()) => {
                // Opening the file read its details; show them in the list.
                if let (Some(info), Some(index)) = (self.player.info(), self.playlist.current_index()) {
                    let id = self.playlist.entries()[index].id;
                    self.playlist.set_details(id, info.title.clone(), info.duration);
                    self.pl.reveal(index, self.playlist.len());
                }
                self.apply_auto_preset();
            }
            Err(err) => self.show_message(format!("{}: {err:#}", file_name(&path))),
        }
        self.redraw(Kind::Playlist);
    }

    fn play(&mut self) {
        if self.playlist.is_empty() {
            self.open_files_dialog();
        } else if self.player.info().is_none() {
            self.play_current();
        } else if let Err(err) = self.player.play() {
            self.show_message(format!("{err:#}"));
        }
    }

    fn next(&mut self) {
        if self.playlist.next(self.state.shuffle, true).is_some() {
            self.play_current();
        }
    }

    fn previous(&mut self) {
        if self.playlist.previous().is_some() {
            self.play_current();
        }
    }

    /// Called when a track plays to the end.
    fn track_finished(&mut self) {
        if self.playlist.next(self.state.shuffle, self.state.repeat).is_some() {
            self.play_current();
        } else {
            self.player.stop();
        }
    }

    fn seek_by(&mut self, seconds: f64) {
        if self.player.state() != PlayState::Stopped {
            let target = self.player.position() + seconds;
            if let Err(err) = self.player.seek(target.max(0.0)) {
                self.show_message(format!("{err:#}"));
            }
        }
    }

    fn set_volume(&mut self, volume: f32) {
        self.state.volume = volume.clamp(0.0, 1.0);
        self.player.set_volume(self.state.volume);
    }

    fn set_balance(&mut self, balance: f32) {
        // Snap to the middle, since hitting exact center by hand is fiddly.
        self.state.balance = if balance.abs() < 0.1 { 0.0 } else { balance };
        self.player.set_balance(self.state.balance);
    }

    /// The eject button: choose files, which replace the playlist.
    fn open_files_dialog(&mut self) {
        let files = rfd::FileDialog::new()
            .set_title("Open file(s)")
            .add_filter("Audio files", audio::EXTENSIONS)
            .pick_files();
        if let Some(files) = files.filter(|f| !f.is_empty()) {
            self.add_tracks(files, true);
        }
    }

    fn show_message(&mut self, text: String) {
        eprintln!("{text}");
        self.message = Some((text, Instant::now()));
    }

    // ----- Display -----

    /// What the marquee should say right now.
    fn marquee_text(&self) -> String {
        let state = &self.state;
        if let Some(slider) = state.dragging {
            return match slider {
                Slider::Volume => format!("volume: {}%", (state.volume * 100.0).round()),
                Slider::Balance if state.balance == 0.0 => "balance: center".to_string(),
                Slider::Balance => {
                    let side = if state.balance < 0.0 { "left" } else { "right" };
                    format!("balance: {}% {side}", (state.balance.abs() * 100.0).round())
                }
                Slider::Position => {
                    let duration = state.duration.unwrap_or(0);
                    let target = (state.position * duration as f32) as u32;
                    format!(
                        "seek to: {}/{} ({}%)",
                        format_time(target),
                        format_time(duration),
                        (state.position * 100.0).round()
                    )
                }
            };
        }
        // Like Winamp, dragging an EQ slider shows its setting.
        if let Some(index) = self.eq.dragging {
            let name = match index {
                0 => "preamp".to_string(),
                i => format_hz(audio::EQ_BAND_HZ[i - 1]),
            };
            return format!("eq: {name}: {:+.1} db", eq_window::to_db(self.eq.slider(index)));
        }
        if let Some((text, _)) = &self.message {
            return text.clone();
        }
        match self.player.info() {
            Some(info) => {
                let length = info.duration.map(|d| format!(" ({})", format_time(d.round() as u32))).unwrap_or_default();
                format!("{}. {}{length}", self.playlist.current_number(), info.title)
            }
            None if self.skin_name.is_empty() => "drop a .wsz skin or music here".to_string(),
            None => self.skin_name.clone(),
        }
    }

    /// Copies the player's state into what the main window shows.
    fn update(&mut self) {
        if self.player.finished() {
            self.track_finished();
        }
        let now = Instant::now();
        if self.message.as_ref().is_some_and(|(_, at)| now - *at > MESSAGE_TIME) {
            self.message = None;
        }
        let marquee = self.marquee_text();

        let state = &mut self.state;
        let info = self.player.info();
        state.always_on_top = self.always_on_top;
        state.double_size = self.double_size;
        state.play_state = self.player.state();
        state.duration = info.and_then(|i| i.duration).map(|d| d.round() as u32);
        state.kbps = info.and_then(|i| i.kbps);
        state.khz = info.map(|i| (i.sample_rate + 500) / 1000);
        state.stereo = info.is_none_or(|i| i.channels > 1);

        // The visualizer freezes while paused and clears when stopped.
        match state.play_state {
            PlayState::Playing => {
                let player = &self.player;
                self.visualizer.update(now, |buffer| player.recent_samples(buffer));
            }
            PlayState::Paused => {}
            PlayState::Stopped => self.visualizer.reset(),
        }

        let position = self.player.position();
        state.elapsed = position as u32;
        if state.dragging != Some(Slider::Position) {
            state.position = match info.and_then(|i| i.duration) {
                Some(duration) if duration > 0.0 => (position / duration) as f32,
                _ => 0.0,
            };
        }
        // While paused, the time shows for one second out of every two.
        state.show_time = state.play_state != PlayState::Paused || (now - self.clock).as_millis() % 2000 < 1000;

        if marquee != state.marquee {
            state.marquee = marquee;
            state.marquee_offset = 0;
            self.marquee_scrolled_at = now;
        } else if main_window::marquee_scrolls(&state.marquee) && now - self.marquee_scrolled_at >= MARQUEE_STEP {
            state.marquee_offset += 1;
            self.marquee_scrolled_at = now;
        }
    }

    /// True while something on screen changes by itself.
    fn animating(&self) -> bool {
        self.player.state() != PlayState::Stopped
            || self.state.dragging.is_some()
            || self.message.is_some()
            || main_window::marquee_scrolls(&self.state.marquee)
    }

    fn view(&self, kind: Kind) -> Option<&View> {
        match kind {
            Kind::Main => self.main_view.as_ref(),
            Kind::Equalizer => self.eq_view.as_ref(),
            Kind::Playlist => self.pl_view.as_ref(),
            Kind::Menu => self.menu_view.as_ref(),
        }
    }

    fn view_mut(&mut self, kind: Kind) -> Option<&mut View> {
        match kind {
            Kind::Main => self.main_view.as_mut(),
            Kind::Equalizer => self.eq_view.as_mut(),
            Kind::Playlist => self.pl_view.as_mut(),
            Kind::Menu => self.menu_view.as_mut(),
        }
    }

    fn kind_of(&self, id: WindowId) -> Option<Kind> {
        KINDS.into_iter().find(|&k| self.view(k).is_some_and(|v| v.window.id() == id))
    }

    fn redraw(&self, kind: Kind) {
        if let Some(view) = self.view(kind) {
            view.window.request_redraw();
        }
    }

    fn redraw_all(&self) {
        KINDS.into_iter().for_each(|kind| self.redraw(kind));
    }

    fn render(&mut self, kind: Kind) -> Result<()> {
        if kind == Kind::Main {
            self.update();
        }
        // Each arm borrows only the fields it needs, so the view can be
        // changed while the skin and state are read.
        match kind {
            Kind::Main => {
                let Some(view) = &mut self.main_view else { return Ok(()) };
                let height = if self.state.shaded { main_window::SHADE_HEIGHT } else { main_window::HEIGHT };
                if view.canvas.height != height {
                    view.canvas = Canvas::new(main_window::WIDTH, height);
                }
                main_window::draw(&mut view.canvas, &self.skin, &self.state);
                if self.state.play_state != PlayState::Stopped {
                    if self.state.shaded {
                        let (x, y) = main_window::SHADE_VIS_POSITION;
                        self.visualizer.draw_small(&mut view.canvas, &self.skin, x, y);
                    } else {
                        let (x, y) = main_window::VIS_POSITION;
                        self.visualizer.draw(&mut view.canvas, &self.skin, x, y);
                    }
                }
                view.present()
            }
            Kind::Equalizer => {
                let Some(view) = &mut self.eq_view else { return Ok(()) };
                let height = if self.eq.shaded { eq_window::SHADE_HEIGHT } else { eq_window::HEIGHT };
                if view.canvas.height != height {
                    view.canvas = Canvas::new(eq_window::WIDTH, height);
                }
                eq_window::draw(&mut view.canvas, &self.skin, &self.eq, self.state.volume, self.state.balance);
                view.present()
            }
            Kind::Playlist => {
                let Some(view) = &mut self.pl_view else { return Ok(()) };
                if (view.canvas.width, view.canvas.height) != (self.pl.width, self.pl.window_height()) {
                    view.canvas = Canvas::new(self.pl.width, self.pl.window_height());
                }
                playlist_window::draw(&mut view.canvas, &self.skin, self.pl_font.as_mut(), &self.pl, &self.playlist, &self.state);
                view.present()
            }
            Kind::Menu => {
                let (Some(view), Some(menu), Some(font)) = (&mut self.menu_view, &self.menu, &mut self.menu_font) else {
                    return Ok(());
                };
                menu.draw(&mut view.canvas, font);
                view.present()
            }
        }
    }

    // ----- Main window input -----

    fn zoom(&self) -> u32 {
        if self.double_size { 2 } else { 1 }
    }

    fn command_key(&self) -> bool {
        self.modifiers.control_key() || self.modifiers.super_key()
    }

    fn main_mouse_down(&mut self) {
        let Some(view) = &self.main_view else { return };
        let (x, y) = view.cursor_in_skin(self.zoom());
        match main_window::hit_test(x, y, self.state.shaded) {
            Hit::TitleBar => self.title_bar_pressed(Kind::Main),
            Hit::Control(control) => self.state.pressed = Some(control),
            Hit::Slider(Slider::Position) if self.state.play_state == PlayState::Stopped || self.state.duration.is_none() => {}
            Hit::Slider(slider) => {
                self.state.dragging = Some(slider);
                self.main_drag_to(x);
            }
            Hit::Time => self.state.time_remaining = !self.state.time_remaining,
            Hit::Visualizer => self.visualizer.cycle_mode(),
            Hit::Nothing => {}
        }
    }

    fn main_mouse_up(&mut self, event_loop: &ActiveEventLoop) {
        let Some(view) = &self.main_view else { return };
        let (x, y) = view.cursor_in_skin(self.zoom());
        // A click only counts if released over the same control.
        if let Some(control) = self.state.pressed.take()
            && main_window::hit_test(x, y, self.state.shaded) == Hit::Control(control)
        {
            self.activate(control, event_loop);
        }
        if self.state.dragging.take() == Some(Slider::Position)
            && let Some(duration) = self.player.info().and_then(|i| i.duration)
        {
            let target = self.state.position as f64 * duration;
            if let Err(err) = self.player.seek(target) {
                self.show_message(format!("{err:#}"));
            }
        }
    }

    /// Moves the slider being dragged to follow the mouse.
    fn main_drag_to(&mut self, x: i32) {
        let Some(slider) = self.state.dragging else { return };
        let value = main_window::slider_value(slider, x, self.state.shaded);
        match slider {
            Slider::Volume => self.set_volume(value),
            Slider::Balance => self.set_balance(value),
            Slider::Position => self.state.position = value, // seeks on release
        }
    }

    /// Runs a control after it was clicked (pressed and released over it).
    fn activate(&mut self, control: Control, event_loop: &ActiveEventLoop) {
        match control {
            Control::Close => event_loop.exit(),
            Control::Minimize => {
                if let Some(view) = &self.main_view {
                    view.window.set_minimized(true);
                }
            }
            Control::Previous => self.previous(),
            Control::Play => self.play(),
            Control::Pause => self.player.pause(),
            Control::Stop => self.player.stop(),
            Control::Next => self.next(),
            Control::Eject => self.open_files_dialog(),
            Control::Shuffle => self.state.shuffle = !self.state.shuffle,
            Control::Repeat => self.state.repeat = !self.state.repeat,
            Control::Equalizer => self.set_equalizer_open(!self.state.equalizer_open),
            Control::Playlist => self.set_playlist_open(!self.state.playlist_open),
            Control::Options | Control::ClutterO => self.open_options_menu(event_loop, Kind::Main),
            Control::ClutterA => self.set_always_on_top(!self.always_on_top),
            Control::ClutterI => self.show_file_info(),
            Control::ClutterD => self.toggle_double_size(),
            Control::ClutterV => self.open_visualizer_menu(event_loop),
            Control::Shade => self.toggle_shade(Kind::Main),
        }
    }

    // ----- Input for any window -----

    /// Winamp's keyboard shortcuts, which work in every window.
    fn key_pressed(&mut self, key: &Key, kind: Kind, event_loop: &ActiveEventLoop) {
        match key {
            Key::Character(c) => match c.to_ascii_lowercase().as_str() {
                "d" if self.command_key() => self.toggle_double_size(),
                "t" if self.command_key() => self.set_always_on_top(!self.always_on_top),
                "s" if self.modifiers.alt_key() => self.open_skin_browser(event_loop, kind),
                "g" if self.modifiers.alt_key() => self.set_equalizer_open(!self.state.equalizer_open),
                "e" if self.modifiers.alt_key() => self.set_playlist_open(!self.state.playlist_open),
                "z" => self.previous(),
                "x" => self.play(),
                "c" => self.player.pause(),
                "v" => self.player.stop(),
                "b" => self.next(),
                "l" => self.open_files_dialog(),
                _ => {}
            },
            Key::Named(NamedKey::ArrowLeft) => self.seek_by(-SEEK_STEP),
            Key::Named(NamedKey::ArrowRight) => self.seek_by(SEEK_STEP),
            Key::Named(NamedKey::ArrowUp) => self.set_volume(self.state.volume + 0.05),
            Key::Named(NamedKey::ArrowDown) => self.set_volume(self.state.volume - 0.05),
            _ => {}
        }
    }

    /// Files dropped on a window: skins change the skin; music dropped on
    /// the playlist is added, and anywhere else replaces the playlist.
    fn file_dropped(&mut self, path: PathBuf, kind: Kind) {
        if is_skin(&path) {
            match open_skin(&path) {
                Ok(skin) => self.set_skin(skin, path),
                Err(err) => self.show_message(format!("{}: {err:#}", file_name(&path))),
            }
            return;
        }
        // A multi-file drop arrives as one event per file: the first
        // decides what happens, the rest are added after it.
        let now = Instant::now();
        let same_drop = self.last_drop.is_some_and(|t| now - t < DROP_BURST);
        self.last_drop = Some(now);
        let replace = kind != Kind::Playlist && !same_drop;
        self.add_tracks(vec![path], replace);
    }

    fn set_skin(&mut self, skin: Skin, path: PathBuf) {
        let name = file_name(&path);
        self.skin_path = Some(std::fs::canonicalize(&path).unwrap_or(path));
        if skin.playlist.font != self.skin.playlist.font {
            self.pl_font = Font::load(&skin.playlist.font, playlist_window::FONT_PX);
        }
        self.skin = skin;
        self.skin_name = name;
        self.redraw_all();
    }

    fn toggle_double_size(&mut self) {
        // Windows docked to the main window keep their places relative to
        // it, scaled by the same factor, so the stack stays together.
        let main = self.rect(Kind::Main);
        let docked: Vec<(Kind, windows::Rect)> =
            self.docked_to(Kind::Main).into_iter().filter_map(|k| self.rect(k).map(|r| (k, r))).collect();
        self.double_size = !self.double_size;
        let factor = if self.double_size { 2.0 } else { 0.5 };
        if let Some(main) = main {
            for (kind, rect) in docked {
                let x = main.x + ((rect.x - main.x) as f64 * factor).round() as i32;
                let y = main.y + ((rect.y - main.y) as f64 * factor).round() as i32;
                if let Some(view) = self.view(kind) {
                    view.window.set_outer_position(PhysicalPosition::new(x, y));
                }
            }
        }
        self.apply_window_sizes();
    }

    /// Sets every window's size from its skin size and the zoom.
    fn apply_window_sizes(&self) {
        let zoom = self.zoom();
        for (view, (width, height)) in [
            (&self.main_view, self.skin_size(Kind::Main)),
            (&self.eq_view, self.skin_size(Kind::Equalizer)),
            (&self.pl_view, self.skin_size(Kind::Playlist)),
        ] {
            if let Some(view) = view {
                let _ = view.window.request_inner_size(LogicalSize::new(width * zoom, height * zoom));
            }
        }
    }

    /// A window's size in skin pixels, allowing for shade mode.
    fn skin_size(&self, kind: Kind) -> (u32, u32) {
        match kind {
            Kind::Main if self.state.shaded => (main_window::WIDTH, main_window::SHADE_HEIGHT),
            Kind::Equalizer if self.eq.shaded => (eq_window::WIDTH, eq_window::SHADE_HEIGHT),
            Kind::Main => (main_window::WIDTH, main_window::HEIGHT),
            Kind::Equalizer => (eq_window::WIDTH, eq_window::HEIGHT),
            Kind::Playlist => (self.pl.width, self.pl.window_height()),
            Kind::Menu => self.menu_view.as_ref().map_or((1, 1), |v| (v.canvas.width, v.canvas.height)),
        }
    }

    /// Switches a window between full size and shade mode (a strip),
    /// moving windows docked below it so they stay attached.
    fn toggle_shade(&mut self, kind: Kind) {
        match kind {
            Kind::Main => self.state.shaded = !self.state.shaded,
            Kind::Equalizer => self.eq.shaded = !self.eq.shaded,
            Kind::Playlist => self.pl.shaded = !self.pl.shaded,
            Kind::Menu => return,
        }
        let (width, height) = self.skin_size(kind);
        let zoom = self.zoom();
        let Some(scale) = self.view(kind).map(|v| v.window.scale_factor()) else { return };
        let size = LogicalSize::new(width * zoom, height * zoom).to_physical(scale);
        self.resize_keeping_docked(kind, size);
        self.redraw_all();
    }

    /// A press on a title bar: double-click toggles shade mode, otherwise
    /// it starts moving the window.
    fn title_bar_pressed(&mut self, kind: Kind) {
        let now = Instant::now();
        let double = self.last_title_click.is_some_and(|(k, at)| k == kind && now - at < Duration::from_millis(400));
        if double {
            self.last_title_click = None;
            self.toggle_shade(kind);
        } else {
            self.last_title_click = Some((kind, now));
            self.start_window_drag(kind);
        }
    }

    fn set_always_on_top(&mut self, on_top: bool) {
        self.always_on_top = on_top;
        let level = if on_top { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal };
        for kind in [Kind::Main, Kind::Equalizer, Kind::Playlist] {
            if let Some(view) = self.view(kind) {
                view.window.set_window_level(level);
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.main_view.is_some() {
            return;
        }
        let zoom = self.zoom();
        let main_position = self.saved_positions.main.map(|[x, y]| PhysicalPosition::new(x, y));
        let (width, height) = self.skin_size(Kind::Main);
        self.main_view = Some(View::new(event_loop, "skinamp", width, height, zoom, true, main_position));
        // A saved position on a screen that's no longer connected is dropped.
        if let Some([x, y]) = self.saved_positions.main
            && !self.on_screen(x, y)
            && let Some(view) = &self.main_view
        {
            view.window.set_outer_position(PhysicalPosition::new(100, 100));
        }
        // The other windows are created hidden and shown when opened.
        let (width, height) = self.skin_size(Kind::Equalizer);
        self.eq_view = Some(View::new(event_loop, "Equalizer", width, height, zoom, false, None));
        let (width, height) = self.skin_size(Kind::Playlist);
        self.pl_view = Some(View::new(event_loop, "Playlist", width, height, zoom, false, None));
        if self.saved_positions.equalizer_open {
            self.set_equalizer_open(true);
        }
        if self.saved_positions.playlist_open {
            self.set_playlist_open(true);
        }
        if self.always_on_top {
            self.set_always_on_top(true);
        }
        if let Some(view) = &self.main_view {
            view.window.focus_window();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.save_settings();
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Quit => event_loop.exit(),
            UserEvent::Metadata => {
                let results: Vec<_> = self.metadata.results().collect();
                for details in results {
                    self.playlist.set_details(details.id, details.title, details.duration);
                }
                self.redraw(Kind::Playlist);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(kind) = self.kind_of(id) else { return };
        match event {
            WindowEvent::CloseRequested => match kind {
                Kind::Main => event_loop.exit(),
                Kind::Equalizer => self.set_equalizer_open(false),
                Kind::Playlist => self.set_playlist_open(false),
                Kind::Menu => self.close_text_menu(),
            },
            WindowEvent::RedrawRequested => {
                if let Err(err) = self.render(kind) {
                    eprintln!("drawing failed: {err:#}");
                }
                return;
            }
            WindowEvent::Focused(focused) => match kind {
                Kind::Main => self.state.focused = focused,
                Kind::Equalizer => self.eq.focused = focused,
                Kind::Playlist => self.pl.focused = focused,
                // A pop-up menu closes when you click away from it.
                Kind::Menu if !focused => self.close_text_menu(),
                Kind::Menu => {}
            },
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved { position, .. } => {
                let zoom = self.zoom();
                let Some(view) = self.view_mut(kind) else { return };
                view.cursor = position;
                if self.continue_window_drag(kind) {
                    return;
                }
                let Some(view) = self.view(kind) else { return };
                let (x, y) = view.cursor_in_skin(zoom);
                match kind {
                    Kind::Main if self.state.dragging.is_some() => self.main_drag_to(x),
                    Kind::Equalizer if self.eq.dragging.is_some() => self.eq_drag_to(y),
                    Kind::Equalizer if self.eq.shaded && self.state.dragging.is_some() => self.eq_shade_drag_to(x),
                    Kind::Playlist if self.pl.open_menu.is_some() || self.pl.dragging_scrollbar || self.pl_drag.is_some() => {
                        self.pl_mouse_moved()
                    }
                    Kind::Menu => self.menu_mouse_moved(),
                    _ => return, // nothing changes on screen
                }
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. }
                if self.window_drag.is_some() =>
            {
                self.end_window_drag();
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => match (kind, state) {
                (Kind::Main, ElementState::Pressed) => self.main_mouse_down(),
                (Kind::Main, ElementState::Released) => self.main_mouse_up(event_loop),
                (Kind::Equalizer, ElementState::Pressed) => self.eq_mouse_down(),
                (Kind::Equalizer, ElementState::Released) => self.eq_mouse_up(event_loop),
                (Kind::Playlist, ElementState::Pressed) => self.pl_mouse_down(),
                (Kind::Playlist, ElementState::Released) => self.pl_mouse_up(event_loop),
                (Kind::Menu, ElementState::Released) => self.menu_mouse_up(event_loop),
                (Kind::Menu, ElementState::Pressed) => {}
            },
            // Right-clicking the main or equalizer window opens the main menu.
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Right, .. }
                if matches!(kind, Kind::Main | Kind::Equalizer) =>
            {
                self.open_options_menu(event_loop, kind);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 20.0,
                };
                if kind == Kind::Playlist {
                    // Scrolling down the page shows later tracks.
                    self.pl_scroll_by(-(steps * 2.0).round() as isize);
                } else {
                    self.set_volume(self.state.volume + steps * 0.02);
                }
            }
            WindowEvent::DroppedFile(path) => self.file_dropped(path, kind),
            WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, .. }, .. } => {
                if logical_key == Key::Named(NamedKey::Escape) {
                    self.close_text_menu();
                    self.pl.open_menu = None;
                } else if kind == Kind::Playlist && self.pl_key_pressed(&logical_key) {
                    // handled by the playlist
                } else {
                    self.key_pressed(&logical_key, kind, event_loop);
                }
            }
            WindowEvent::Resized(_) => {}
            _ => return,
        }
        // Something changed; windows are small, so just redraw them all.
        self.redraw_all();
    }

    /// Runs when the app has handled all pending events and is about to
    /// sleep. While something is moving, wake up again for the next frame.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.animating() {
            let now = Instant::now();
            if now >= self.next_frame {
                self.redraw(Kind::Main);
                self.redraw(Kind::Playlist); // its mini time display
                self.next_frame = now + FRAME;
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

/// True for skin archives and for folders containing an unpacked skin.
pub fn is_skin(path: &Path) -> bool {
    if path.is_dir() {
        return std::fs::read_dir(path).is_ok_and(|entries| {
            entries.flatten().any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("main.bmp"))
        });
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase());
    matches!(ext.as_deref(), Some("wsz" | "zip"))
}

/// Loads a skin and reports anything that was wrong with it.
pub fn open_skin(path: &Path) -> Result<Skin> {
    let skin = Skin::load(path)?;
    for warning in &skin.warnings {
        eprintln!("{}: {warning}", path.display());
    }
    Ok(skin)
}

pub fn file_name(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn format_time(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// "60hz", "1khz", "14khz" — how Winamp labels its bands.
fn format_hz(hz: f64) -> String {
    if hz >= 1000.0 { format!("{}khz", hz / 1000.0) } else { format!("{hz}hz") }
}
