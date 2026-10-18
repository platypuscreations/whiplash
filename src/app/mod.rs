//! The application: owns the windows, the skin, the player and the
//! playlist, and turns mouse and keyboard input into player actions.
//!
//! The main window's behavior lives here; the equalizer and playlist
//! windows have their own files.

mod desktop;
mod dialogs;
mod equalizer;
mod media;
mod menus;
mod milkdrop;
mod playlist;
mod tooltips;
mod windows;

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Result;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Size};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
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
use crate::ui::menu::Menu;
use crate::ui::playlist_window::{self, PlaylistView};
use crate::ui::text::Font;
use crate::vis::Visualizer;
use playlist::PlDrag;
use tooltips::Hover;
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
/// For this long after a pop-up menu opens, losing focus doesn't close it.
/// Opening a menu can briefly move focus around (on macOS, a right-click
/// in an inactive app makes the app active, and macOS then refocuses the
/// window that had focus before), which would otherwise close it at once.
const MENU_GRACE: Duration = Duration::from_millis(500);

/// Messages from other threads that wake up the event loop.
#[derive(Debug)]
pub enum UserEvent {
    /// The metadata thread has read some tracks' details.
    Metadata,
    /// Ctrl+C was pressed in the terminal: quit, saving settings.
    Quit,
    /// A file dialog closed, with the paths chosen (none if cancelled).
    DialogClosed(dialogs::Purpose, Vec<PathBuf>),
    /// A media key, or the desktop's media controls.
    #[cfg_attr(windows, allow(dead_code))] // Windows has no media controls yet
    Media(media::Command),
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
    /// The canvas enlarged to screen size, for drawing sharp text on top
    /// (see `present_with`).
    full_size: Canvas,
    /// Last known mouse position over this window, in physical pixels.
    cursor: PhysicalPosition<f64>,
}

/// How a new window appears.
struct Show {
    visible: bool,
    /// Whether it takes keyboard focus when it appears.
    active: bool,
    /// Where on screen, or `None` to let the system choose.
    position: Option<PhysicalPosition<i32>>,
    /// On Windows, the window this one belongs to: it then has no taskbar
    /// button of its own, stays above its owner, and minimizes with it.
    #[cfg_attr(not(windows), allow(dead_code))]
    owner: Option<RawWindowHandle>,
}

impl Show {
    /// Created hidden, to be shown later.
    const HIDDEN: Show = Show { visible: false, active: true, position: None, owner: None };

    fn at(position: PhysicalPosition<i32>, active: bool) -> Show {
        Show { visible: true, active, position: Some(position), owner: None }
    }

    fn owned_by(self, owner: &Window) -> Show {
        Show { owner: owner.window_handle().ok().map(|h| h.as_raw()), ..self }
    }
}

/// How many screen pixels each skin pixel becomes: the zoom (2 in double
/// size) times the screen's scale factor, rounded to a whole number so every
/// skin pixel is an equal square block. Scale factors aren't always whole
/// numbers: on Linux a 1920x1200 laptop screen is often 1.5.
fn pixels_per_skin_pixel(scale_factor: f64, zoom: u32) -> u32 {
    ((scale_factor * zoom as f64).round() as u32).max(1)
}

/// The window size that shows a `width` x `height` skin layout at `zoom`.
fn skin_window_size(width: u32, height: u32, zoom: u32, scale_factor: f64) -> PhysicalSize<u32> {
    let pixels = pixels_per_skin_pixel(scale_factor, zoom);
    PhysicalSize::new(width * pixels, height * pixels)
}

impl View {
    /// A window showing a `width` x `height` canvas, `size` big on screen.
    fn new(event_loop: &ActiveEventLoop, title: &str, width: u32, height: u32, size: Size, show: Show) -> View {
        let Show { visible, active, position, owner } = show;
        let mut attributes = Window::default_attributes()
            .with_title(title)
            .with_decorations(false) // the skin draws its own title bar
            .with_resizable(false)
            .with_visible(visible)
            .with_active(active)
            .with_inner_size(size);
        if let Some(position) = position {
            attributes = attributes.with_position(position);
        }
        // Square corners and no border from Windows 11, and no maximizing
        // (Win+Up included): the skin is the whole window.
        #[cfg(windows)]
        {
            use winit::platform::windows::{CornerPreference, WindowAttributesExtWindows};
            attributes = attributes
                .with_corner_preference(CornerPreference::DoNotRound)
                .with_border_color(None)
                .with_enabled_buttons(winit::window::WindowButtons::MINIMIZE);
            if let Some(RawWindowHandle::Win32(owner)) = owner {
                attributes = attributes.with_owner_window(owner.hwnd.get());
            }
        }
        #[cfg(not(windows))]
        let _ = owner;
        let window = Rc::new(event_loop.create_window(attributes).expect("failed to create window"));
        let context = softbuffer::Context::new(window.clone()).expect("failed to create graphics context");
        let surface = softbuffer::Surface::new(&context, window.clone()).expect("failed to create surface");
        View {
            window,
            surface,
            canvas: Canvas::new(width, height),
            full_size: Canvas::new(0, 0),
            cursor: PhysicalPosition::new(0.0, 0.0),
        }
    }

    /// A window for a `width` x `height` skin layout at `zoom`.
    fn for_skin(event_loop: &ActiveEventLoop, title: &str, width: u32, height: u32, zoom: u32, show: Show) -> View {
        let size = LogicalSize::new(width * zoom, height * zoom);
        let view = View::new(event_loop, title, width, height, size.into(), show);
        // Only now is the screen's scale known: round the size to whole
        // screen pixels per skin pixel.
        view.resize(skin_window_size(width, height, zoom, view.window.scale_factor()));
        view
    }

    /// Changes the window's size. Its minimum and maximum size change with
    /// it, which keeps the user from resizing it. (On X11, winit puts back
    /// the window's original limits whenever it moves to a screen with a
    /// different scale, unless they're set like this, and the window
    /// manager then holds the window to its original size.)
    fn resize(&self, size: PhysicalSize<u32>) {
        self.window.set_min_inner_size(None::<Size>);
        self.window.set_max_inner_size(Some(size));
        self.window.set_min_inner_size(Some(size));
        let _ = self.window.request_inner_size(size);
    }

    /// Copies the canvas to the screen, scaled up to fill the window.
    fn present(&mut self) -> Result<()> {
        let size = self.window.inner_size();
        let (Some(width), Some(height)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return Ok(()); // minimized
        };
        let scale = self.pixel_scale();
        self.surface.resize(width, height).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut buffer = self.surface.buffer_mut().map_err(|e| anyhow::anyhow!("{e}"))?;
        self.canvas.scale_into(&mut buffer, size.width, size.height, scale);
        buffer.present().map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    /// How many screen pixels each canvas pixel becomes: the largest
    /// whole number that fits (2 on Retina, 4 in double size).
    fn pixel_scale(&self) -> u32 {
        let size = self.window.inner_size();
        (size.width / self.canvas.width.max(1)).min(size.height / self.canvas.height.max(1)).max(1)
    }

    /// Like `present`, but after enlarging the canvas lets `overlay` draw
    /// on the screen-sized image (given the scale), for sharp text.
    fn present_with(&mut self, overlay: impl FnOnce(&mut Canvas, u32)) -> Result<()> {
        let size = self.window.inner_size();
        let (Some(width), Some(height)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return Ok(());
        };
        let scale = self.pixel_scale();
        if (self.full_size.width, self.full_size.height) != (size.width, size.height) {
            self.full_size = Canvas::new(size.width, size.height);
        }
        self.canvas.scale_into(&mut self.full_size.pixels, size.width, size.height, scale);
        overlay(&mut self.full_size, scale);

        self.surface.resize(width, height).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut buffer = self.surface.buffer_mut().map_err(|e| anyhow::anyhow!("{e}"))?;
        buffer.copy_from_slice(&self.full_size.pixels);
        buffer.present().map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    /// The mouse position in the window's unscaled skin layout.
    fn cursor_in_skin(&self) -> (i32, i32) {
        let scale = self.pixel_scale() as f64;
        ((self.cursor.x / scale).floor() as i32, (self.cursor.y / scale).floor() as i32)
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
    /// When the pop-up menu opened (see `MENU_GRACE`).
    menu_opened_at: Option<Instant>,
    /// Log window events to the terminal (set `WHIPLASH_DEBUG_EVENTS=1`).
    debug_events: bool,
    /// The skin's playlist font (loaded for the window's pixel scale), and
    /// the font for pop-up menus (loaded for the screen's scale).
    pl_font: Option<(Font, u32)>,
    menu_font: Option<(Font, f64)>,
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
    /// The MilkDrop window, created the first time it's opened and then
    /// kept (hidden when closed), since projectM can't be cleanly restarted.
    #[cfg(feature = "milkdrop")]
    milkdrop: Option<crate::vis::milkdrop::MilkDrop>,
    milkdrop_open: bool,
    /// MilkDrop was open when Whiplash last quit.
    reopen_milkdrop: bool,
    /// The chosen MilkDrop presets folder, if not the default one.
    milkdrop_presets: Option<PathBuf>,
    /// When the MilkDrop window was last clicked, to spot double-clicks.
    #[cfg(feature = "milkdrop")]
    milkdrop_last_click: Option<Instant>,
    /// The control the mouse is resting on, for its tooltip.
    hover: Option<Hover>,
    tooltip_view: Option<View>,
    tooltip_text: String,
    /// The tooltip font, loaded for the screen's scale (2.0 on Retina).
    tooltip_font: Option<(Font, f64)>,
    /// When and where the title bar was last clicked, to spot double-clicks.
    last_title_click: Option<(Kind, Instant)>,
    /// The skins listed in the open skin browser.
    skin_choices: Vec<PathBuf>,
    /// For sending events to the event loop from other threads.
    proxy: EventLoopProxy<UserEvent>,
    /// The system's media keys and now-playing display.
    media: media::MediaControls,
    /// A file dialog is open; only one is opened at a time.
    dialog_open: bool,
    /// For what we ask of the desktop beyond winit, connected when first needed.
    desktop: std::cell::OnceCell<desktop::Desktop>,
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
            menu_opened_at: None,
            debug_events: std::env::var_os("WHIPLASH_DEBUG_EVENTS").is_some(),
            pl_font: None,
            menu_font: None,
            player: Player::new(),
            playlist: Playlist::default(),
            metadata: {
                let proxy = proxy.clone();
                MetadataReader::new(move || {
                    let _ = proxy.send_event(UserEvent::Metadata);
                })
            },
            visualizer,
            modifiers: ModifiersState::empty(),
            double_size: config.double_size,
            always_on_top: config.always_on_top,
            saved_positions: windows,
            auto_presets: config.equalizer.auto_presets,
            window_drag: None,
            shown: vec![Kind::Main],
            skin_choices: Vec::new(),
            media: media::MediaControls::connect(proxy.clone()),
            proxy,
            dialog_open: false,
            desktop: std::cell::OnceCell::new(),
            last_title_click: None,
            #[cfg(feature = "milkdrop")]
            milkdrop: None,
            // Reopened once the other windows exist (in `resumed`).
            milkdrop_open: false,
            milkdrop_presets: config.milkdrop_presets.clone(),
            reopen_milkdrop: config.milkdrop_open,
            #[cfg(feature = "milkdrop")]
            milkdrop_last_click: None,
            hover: None,
            tooltip_view: None,
            tooltip_text: String::new(),
            tooltip_font: None,
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
            milkdrop_presets: self.milkdrop_presets.clone(),
            milkdrop_open: self.milkdrop_open,
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
            self.open_dialog(dialogs::Purpose::PlayFiles);
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

    /// Media keys and the desktop's media controls. Unlike the Play
    /// button, Play with nothing in the playlist does nothing, rather than
    /// opening a file dialog while you're in another app.
    fn media_command(&mut self, command: media::Command) {
        let playing = self.player.state() == PlayState::Playing;
        match command {
            media::Command::Play | media::Command::PlayPause if playing => {
                if command == media::Command::PlayPause {
                    self.player.pause();
                }
            }
            media::Command::Play | media::Command::PlayPause => {
                if !self.playlist.is_empty() {
                    self.play();
                }
            }
            media::Command::Pause => {
                if playing {
                    self.player.pause();
                }
            }
            media::Command::Stop => self.player.stop(),
            media::Command::Next => self.next(),
            media::Command::Previous => self.previous(),
            media::Command::Seek(seconds) => self.seek_by(seconds),
            media::Command::SetPosition(seconds) => self.seek_by(seconds - self.player.position()),
            media::Command::SetShuffle(on) => self.state.shuffle = on,
            media::Command::SetRepeat(on) => self.state.repeat = on,
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
            None if self.skin_name.is_empty() => "Whiplash Player - drop music or skins here".to_string(),
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
                playlist_window::draw(&mut view.canvas, &self.skin, &self.pl, &self.playlist, &self.state);

                // Track names are drawn on the enlarged image, with the font
                // loaded at that size, so they're sharp.
                let scale = view.pixel_scale();
                if self.pl_font.as_ref().is_none_or(|(_, s)| *s != scale) {
                    let px = playlist_window::FONT_PX * scale as f32;
                    self.pl_font = Font::load(&self.skin.playlist.font, px).map(|font| (font, scale));
                }
                let (skin, pl, playlist) = (&self.skin, &self.pl, &self.playlist);
                let font = self.pl_font.as_mut().map(|(font, _)| font);
                view.present_with(|full_size, scale| {
                    if let Some(font) = font {
                        playlist_window::draw_track_text(full_size, skin, font, pl, playlist, scale);
                    }
                })
            }
            Kind::Menu => {
                let (Some(view), Some(menu), Some((font, scale))) = (&mut self.menu_view, &self.menu, &mut self.menu_font)
                else {
                    return Ok(());
                };
                menu.draw(&mut view.canvas, font, *scale);
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
        let (x, y) = view.cursor_in_skin();
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
        let (x, y) = view.cursor_in_skin();
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
            Control::Eject => self.open_dialog(dialogs::Purpose::PlayFiles),
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
                "k" if self.command_key() => self.set_milkdrop_open(event_loop, !self.milkdrop_open),
                "s" if self.modifiers.alt_key() => self.open_skin_browser(event_loop, kind),
                "g" if self.modifiers.alt_key() => self.set_equalizer_open(!self.state.equalizer_open),
                "e" if self.modifiers.alt_key() => self.set_playlist_open(!self.state.playlist_open),
                "z" => self.previous(),
                "x" => self.play(),
                "c" => self.player.pause(),
                "v" => self.player.stop(),
                "b" => self.next(),
                "l" => self.open_dialog(dialogs::Purpose::PlayFiles),
                _ => {}
            },
            // Media keys the desktop didn't take for itself.
            Key::Named(NamedKey::MediaPlayPause) => self.media_command(media::Command::PlayPause),
            Key::Named(NamedKey::MediaPlay) => self.media_command(media::Command::Play),
            Key::Named(NamedKey::MediaPause) => self.media_command(media::Command::Pause),
            Key::Named(NamedKey::MediaStop) => self.media_command(media::Command::Stop),
            Key::Named(NamedKey::MediaTrackNext) => self.media_command(media::Command::Next),
            Key::Named(NamedKey::MediaTrackPrevious) => self.media_command(media::Command::Previous),
            Key::Named(NamedKey::ArrowLeft) => self.seek_by(-SEEK_STEP),
            Key::Named(NamedKey::ArrowRight) => self.seek_by(SEEK_STEP),
            Key::Named(NamedKey::ArrowUp) => self.set_volume(self.state.volume + 0.05),
            Key::Named(NamedKey::ArrowDown) => self.set_volume(self.state.volume - 0.05),
            _ => {}
        }
    }

    /// Files dropped on a window: skins move into the Skins folder and
    /// change the skin; music dropped on the playlist is added, and anywhere
    /// else replaces the playlist.
    fn file_dropped(&mut self, path: PathBuf, kind: Kind) {
        if is_skin(&path) {
            match open_skin(&path) {
                Ok(skin) => {
                    // A skin that can't be moved still works from where it is.
                    let path = match config::skins_folder().map(|folder| config::move_skin_into(&path, &folder)) {
                        Some(Ok(moved)) => moved,
                        Some(Err(err)) => {
                            eprintln!("{}: {err:#}", path.display());
                            path
                        }
                        None => path,
                    };
                    self.set_skin(skin, Some(path));
                }
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

    /// Switches skin: one from a file, or the built-in one without `path`.
    fn set_skin(&mut self, skin: Skin, path: Option<PathBuf>) {
        let name = path.as_deref().map(file_name).unwrap_or_default();
        self.skin_path = path.map(|path| std::fs::canonicalize(&path).unwrap_or(path));
        if skin.playlist.font != self.skin.playlist.font {
            self.pl_font = None; // reloaded with the new skin's font when next drawn
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
        let scale = self.main_view.as_ref().map_or(1.0, |v| v.window.scale_factor());
        let before = pixels_per_skin_pixel(scale, self.zoom());
        self.double_size = !self.double_size;
        let factor = pixels_per_skin_pixel(scale, self.zoom()) as f64 / before as f64;
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
        for kind in [Kind::Main, Kind::Equalizer, Kind::Playlist] {
            if let (Some(view), Some(size)) = (self.view(kind), self.window_size(kind)) {
                view.resize(size);
            }
        }
    }

    /// The size a window should be on screen, from its skin size, the
    /// zoom and its screen's scale.
    fn window_size(&self, kind: Kind) -> Option<PhysicalSize<u32>> {
        let (width, height) = self.skin_size(kind);
        let scale = self.view(kind)?.window.scale_factor();
        Some(skin_window_size(width, height, self.zoom(), scale))
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
        let Some(size) = self.window_size(kind) else { return };
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
        // Every window is created at its place (see `plan_window_positions`).
        self.plan_window_positions(event_loop);
        let at = |position: Option<[i32; 2]>| position.map(|[x, y]| PhysicalPosition::new(x, y));
        let (width, height) = self.skin_size(Kind::Main);
        let show = Show { visible: true, active: true, position: at(self.saved_positions.main), owner: None };
        self.main_view = Some(View::for_skin(event_loop, "Whiplash Player", width, height, zoom, show));
        // The other windows are created hidden and shown when opened.
        let (width, height) = self.skin_size(Kind::Equalizer);
        // They belong to the player window, as in Winamp.
        let main = &self.main_view.as_ref().expect("just created").window;
        let show = Show { position: at(self.saved_positions.equalizer), ..Show::HIDDEN }.owned_by(main);
        self.eq_view = Some(View::for_skin(event_loop, "Equalizer", width, height, zoom, show));
        let (width, height) = self.skin_size(Kind::Playlist);
        let main = &self.main_view.as_ref().expect("just created").window;
        let show = Show { position: at(self.saved_positions.playlist), ..Show::HIDDEN }.owned_by(main);
        self.pl_view = Some(View::for_skin(event_loop, "Playlist", width, height, zoom, show));
        if self.saved_positions.equalizer_open {
            self.set_equalizer_open(true);
        }
        if self.saved_positions.playlist_open {
            self.set_playlist_open(true);
        }
        if self.always_on_top {
            self.set_always_on_top(true);
        }
        if self.reopen_milkdrop && cfg!(feature = "milkdrop") {
            self.set_milkdrop_open(event_loop, true);
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
            UserEvent::DialogClosed(purpose, paths) => {
                self.dialog_closed(purpose, paths);
                self.redraw_all();
            }
            UserEvent::Media(command) => {
                self.media_command(command);
                self.redraw_all();
            }
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
        // The tooltip window only ever needs drawing.
        if self.tooltip_view.as_ref().is_some_and(|v| v.window.id() == id) {
            if matches!(event, WindowEvent::RedrawRequested) {
                self.render_tooltip();
            }
            return;
        }
        if self.is_milkdrop_window(id) {
            self.milkdrop_event(event_loop, event);
            return;
        }
        let Some(kind) = self.kind_of(id) else { return };
        if self.debug_events
            && matches!(
                event,
                WindowEvent::Focused(_) | WindowEvent::MouseInput { .. } | WindowEvent::CloseRequested
            )
        {
            eprintln!("[event] {kind:?}: {event:?}");
        }
        // A click puts the tooltip away until the mouse moves to another control.
        if matches!(event, WindowEvent::MouseInput { state: ElementState::Pressed, .. }) {
            self.dismiss_tooltip();
            // Clicking anywhere outside an open pop-up menu closes it.
            if kind != Kind::Menu && self.menu.is_some() {
                self.close_text_menu();
                self.redraw_all();
                return;
            }
        }
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
                Kind::Main | Kind::Equalizer | Kind::Playlist => {
                    match kind {
                        Kind::Main => self.state.focused = focused,
                        Kind::Equalizer => self.eq.focused = focused,
                        _ => self.pl.focused = focused,
                    }
                    if focused {
                        self.raise_together(kind);
                    }
                }
                // A pop-up menu closes when you click away from it.
                Kind::Menu if !focused => {
                    let just_opened = self.menu_opened_at.is_some_and(|at| at.elapsed() < MENU_GRACE);
                    if !just_opened {
                        self.close_text_menu();
                    }
                }
                Kind::Menu => {}
            },
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved { position, .. } => {
                let Some(view) = self.view_mut(kind) else { return };
                view.cursor = position;
                if self.continue_window_drag(kind) {
                    return;
                }
                self.update_hover(kind);
                let Some(view) = self.view(kind) else { return };
                let (x, y) = view.cursor_in_skin();
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
            WindowEvent::CursorLeft { .. } => {
                self.clear_hover();
                return;
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
            // It opens when the button is released: opened on the press, the
            // click would finish afterwards and hand focus back to this
            // window, closing the menu straight away.
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Right, .. }
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
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                // Option on a Mac types other characters (Option+S is "ß"),
                // so Alt shortcuts go by the key itself.
                let logical_key =
                    if self.modifiers.alt_key() { event.key_without_modifiers() } else { event.logical_key };
                if logical_key == Key::Named(NamedKey::Escape) {
                    self.close_text_menu();
                    self.pl.open_menu = None;
                } else if kind == Kind::Playlist && self.pl_key_pressed(&logical_key) {
                    // handled by the playlist
                } else {
                    self.key_pressed(&logical_key, kind, event_loop);
                }
            }
            // Moved to a screen with a different scale: keep whole screen
            // pixels per skin pixel, rather than the system's choice of size.
            WindowEvent::ScaleFactorChanged { scale_factor, mut inner_size_writer } if kind != Kind::Menu => {
                let (width, height) = self.skin_size(kind);
                let size = skin_window_size(width, height, self.zoom(), scale_factor);
                if let Some(view) = self.view(kind) {
                    view.resize(size);
                }
                let _ = inner_size_writer.request_inner_size(size);
            }
            // Windows lets the equalizer and playlist be minimized on their
            // own (Win+Down), leaving them as stubs with the player still up.
            // Minimize the whole group instead.
            #[cfg(windows)]
            WindowEvent::Resized(_) if matches!(kind, Kind::Equalizer | Kind::Playlist) => {
                if let Some(view) = self.view(kind).filter(|v| v.window.is_minimized() == Some(true)) {
                    view.window.set_minimized(false);
                    if let Some(main) = &self.main_view {
                        main.window.set_minimized(true);
                    }
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
        self.refresh_tooltip(event_loop);
        self.media.sync(&self.player, self.state.shuffle, self.state.repeat);
        let mut wake = None;
        if self.animating() {
            let now = Instant::now();
            if now >= self.next_frame {
                self.redraw(Kind::Main);
                self.redraw(Kind::Playlist); // its mini time display
                self.next_frame = now + FRAME;
            }
            wake = Some(self.next_frame);
        }
        // Also wake up when a tooltip is due to appear.
        if let Some(due) = self.tooltip_due() {
            wake = Some(wake.map_or(due, |w: Instant| w.min(due)));
        }
        event_loop.set_control_flow(match wake {
            Some(time) => ControlFlow::WaitUntil(time),
            None => ControlFlow::Wait,
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_pixels_are_whole_screen_pixels() {
        assert_eq!(pixels_per_skin_pixel(1.0, 1), 1);
        assert_eq!(pixels_per_skin_pixel(2.0, 2), 4); // Retina, double size
        assert_eq!(pixels_per_skin_pixel(1.5, 1), 2);
        assert_eq!(pixels_per_skin_pixel(1.5, 2), 3);
        assert_eq!(pixels_per_skin_pixel(1.25, 1), 1);
        assert_eq!(pixels_per_skin_pixel(0.5, 1), 1); // never smaller than the skin
        assert_eq!(skin_window_size(275, 116, 1, 1.5), PhysicalSize::new(550, 232));
    }
}
