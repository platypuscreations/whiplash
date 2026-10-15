//! The application: owns the windows, the skin, the player and the
//! playlist, and turns mouse and keyboard input into player actions.

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::audio::{self, PlayState, Player};
use crate::eqf;
use crate::playlist::Playlist;
use crate::skin::Skin;
use crate::ui::canvas::Canvas;
use crate::ui::eq_window::{self, EqControl, EqState};
use crate::ui::main_window::{self, Control, Hit, MainState, Slider};
use crate::vis::Visualizer;

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

/// Which of the app's windows something refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Main,
    Equalizer,
}

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
    fn new(event_loop: &ActiveEventLoop, title: &str, width: u32, height: u32, zoom: u32, visible: bool) -> View {
        let attributes = Window::default_attributes()
            .with_title(title)
            .with_decorations(false) // the skin draws its own title bar
            .with_resizable(false)
            .with_visible(visible)
            .with_inner_size(LogicalSize::new(width * zoom, height * zoom));
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
    state: MainState,
    eq: EqState,
    main_view: Option<View>,
    eq_view: Option<View>,
    player: Player,
    playlist: Playlist,
    visualizer: Visualizer,
    modifiers: ModifiersState,
    double_size: bool,
    /// A temporary message for the marquee, and when it was set.
    message: Option<(String, Instant)>,
    /// When the app started; drives the blinking time display.
    clock: Instant,
    next_frame: Instant,
    marquee_scrolled_at: Instant,
    last_drop: Option<Instant>,
}

impl App {
    pub fn new(skin: Skin, skin_name: String, tracks: Vec<PathBuf>) -> App {
        let now = Instant::now();
        let mut app = App {
            skin,
            skin_name,
            state: MainState::default(),
            eq: EqState::default(),
            main_view: None,
            eq_view: None,
            player: Player::new(),
            playlist: Playlist::default(),
            visualizer: Visualizer::new(),
            modifiers: ModifiersState::empty(),
            double_size: false,
            message: None,
            clock: now,
            next_frame: now,
            marquee_scrolled_at: now,
            last_drop: None,
        };
        app.player.set_volume(app.state.volume);
        app.apply_equalizer();
        if !tracks.is_empty() {
            app.playlist.replace(tracks);
            app.play_current();
        }
        app
    }

    // ----- Player actions -----

    fn play_current(&mut self) {
        let Some(path) = self.playlist.current().map(Path::to_path_buf) else { return };
        if let Err(err) = self.player.open(&path) {
            self.show_message(format!("{}: {err:#}", file_name(&path)));
        }
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
            self.playlist.replace(files);
            self.play_current();
        }
    }

    fn show_message(&mut self, text: String) {
        eprintln!("{text}");
        self.message = Some((text, Instant::now()));
    }

    // ----- Equalizer -----

    /// Sends the equalizer window's settings to the player.
    fn apply_equalizer(&mut self) {
        let eq = &self.eq;
        self.player.set_equalizer(eq.enabled, eq_window::to_db(eq.preamp), eq.bands.map(eq_window::to_db));
        self.redraw(Kind::Equalizer);
    }

    fn set_equalizer_open(&mut self, open: bool) {
        self.state.equalizer_open = open;
        if let Some(eq) = &self.eq_view {
            if open {
                // Open just below the main window. (Phase 6 makes the
                // windows dock and move together.)
                if let Some(main) = &self.main_view
                    && let Ok(position) = main.window.outer_position()
                {
                    let below = PhysicalPosition::new(position.x, position.y + main.window.outer_size().height as i32);
                    eq.window.set_outer_position(below);
                }
                eq.window.set_visible(true);
                eq.window.focus_window();
            } else {
                eq.window.set_visible(false);
            }
        }
        self.redraw(Kind::Main);
    }

    /// The presets button: asks whether to load or save a preset.
    /// (Phase 6 replaces this with a Winamp-style menu.)
    fn presets_dialog(&mut self) {
        const LOAD: &str = "Load…";
        const SAVE: &str = "Save…";
        let choice = rfd::MessageDialog::new()
            .set_title("EQ presets")
            .set_description("Load a Winamp .eqf preset, or save the current settings as one?")
            .set_buttons(rfd::MessageButtons::YesNoCancelCustom(LOAD.into(), SAVE.into(), "Cancel".into()))
            .show();
        // Some platforms report custom buttons by label, others as Yes/No.
        match choice {
            rfd::MessageDialogResult::Yes => self.load_preset(),
            rfd::MessageDialogResult::No => self.save_preset(),
            rfd::MessageDialogResult::Custom(label) if label == LOAD => self.load_preset(),
            rfd::MessageDialogResult::Custom(label) if label == SAVE => self.save_preset(),
            _ => {}
        }
    }

    fn preset_file_dialog() -> rfd::FileDialog {
        rfd::FileDialog::new().add_filter("Winamp EQ presets", &["eqf", "q1"])
    }

    fn load_preset(&mut self) {
        let Some(path) = Self::preset_file_dialog().set_title("Load EQ preset").pick_file() else { return };
        let loaded = std::fs::read(&path).context("could not read file").and_then(|bytes| eqf::parse(&bytes));
        match loaded {
            // A .q1 library holds several presets; use the first.
            Ok(presets) => {
                let preset = &presets[0];
                self.eq.preamp = preset.preamp;
                self.eq.bands = preset.bands;
                self.apply_equalizer();
                self.show_message(format!("loaded eq preset: {}", preset.name));
            }
            Err(err) => self.show_message(format!("{}: {err:#}", file_name(&path))),
        }
    }

    fn save_preset(&mut self) {
        let dialog = Self::preset_file_dialog().set_title("Save EQ preset").set_file_name("preset.eqf");
        let Some(path) = dialog.save_file() else { return };
        let preset = eqf::Preset { name: file_name(&path), preamp: self.eq.preamp, bands: self.eq.bands };
        match std::fs::write(&path, eqf::write(&[preset])) {
            Ok(()) => self.show_message(format!("saved eq preset: {}", file_name(&path))),
            Err(err) => self.show_message(format!("could not save preset: {err}")),
        }
    }

    fn eq_mouse_down(&mut self) {
        let Some(view) = &self.eq_view else { return };
        let (x, y) = view.cursor_in_skin(self.zoom());
        match eq_window::hit_test(x, y) {
            eq_window::Hit::TitleBar => {
                let _ = view.window.drag_window();
            }
            eq_window::Hit::Control(control) => self.eq.pressed = Some(control),
            eq_window::Hit::Slider(index) => {
                self.eq.dragging = Some(index);
                self.eq_drag_to(y);
            }
            eq_window::Hit::Nothing => {}
        }
    }

    fn eq_mouse_up(&mut self) {
        let Some(view) = &self.eq_view else { return };
        let (x, y) = view.cursor_in_skin(self.zoom());
        self.eq.dragging = None;
        if let Some(control) = self.eq.pressed.take()
            && eq_window::hit_test(x, y) == eq_window::Hit::Control(control)
        {
            self.eq_activate(control);
        }
    }

    fn eq_drag_to(&mut self, y: i32) {
        if let Some(index) = self.eq.dragging {
            self.eq.set_slider(index, eq_window::slider_value(y));
            self.apply_equalizer();
        }
    }

    fn eq_activate(&mut self, control: EqControl) {
        match control {
            EqControl::Close => self.set_equalizer_open(false),
            EqControl::On => self.eq.enabled = !self.eq.enabled,
            // Winamp's "auto" loads a saved preset for each song; that needs
            // the preset library planned for phase 6, so for now it only
            // toggles.
            EqControl::Auto => self.eq.auto = !self.eq.auto,
            EqControl::Presets => self.presets_dialog(),
            EqControl::AllMax => self.eq.bands = [eq_window::SLIDER_MAX; 10],
            EqControl::AllFlat => self.eq.bands = [eq_window::SLIDER_FLAT; 10],
            EqControl::AllMin => self.eq.bands = [0.0; 10],
        }
        self.apply_equalizer();
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
        }
    }

    fn kind_of(&self, id: WindowId) -> Option<Kind> {
        [Kind::Main, Kind::Equalizer].into_iter().find(|&k| self.view(k).is_some_and(|v| v.window.id() == id))
    }

    fn redraw(&self, kind: Kind) {
        if let Some(view) = self.view(kind) {
            view.window.request_redraw();
        }
    }

    fn render(&mut self, kind: Kind) -> Result<()> {
        match kind {
            Kind::Main => {
                self.update();
                let Some(view) = &mut self.main_view else { return Ok(()) };
                main_window::draw(&mut view.canvas, &self.skin, &self.state);
                if self.state.play_state != PlayState::Stopped {
                    let (x, y) = main_window::VIS_POSITION;
                    self.visualizer.draw(&mut view.canvas, &self.skin, x, y);
                }
                view.present()
            }
            Kind::Equalizer => {
                let Some(view) = &mut self.eq_view else { return Ok(()) };
                eq_window::draw(&mut view.canvas, &self.skin, &self.eq);
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
        match main_window::hit_test(x, y) {
            Hit::TitleBar => {
                let _ = view.window.drag_window();
            }
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
            && main_window::hit_test(x, y) == Hit::Control(control)
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
        let value = main_window::slider_value(slider, x);
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
            // The playlist window arrives in phase 5.
            Control::Playlist => self.state.playlist_open = !self.state.playlist_open,
            Control::Options | Control::Shade => {}
        }
    }

    // ----- Input for any window -----

    /// Winamp's keyboard shortcuts, which work in every window.
    fn key_pressed(&mut self, key: &Key) {
        match key {
            Key::Character(c) => match c.to_ascii_lowercase().as_str() {
                "d" if self.command_key() => self.toggle_double_size(),
                "g" if self.modifiers.alt_key() => self.set_equalizer_open(!self.state.equalizer_open),
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

    fn file_dropped(&mut self, path: PathBuf) {
        if is_skin(&path) {
            match open_skin(&path) {
                Ok(skin) => {
                    self.skin = skin;
                    self.skin_name = file_name(&path);
                    self.redraw(Kind::Equalizer);
                }
                Err(err) => self.show_message(format!("{}: {err:#}", file_name(&path))),
            }
            return;
        }
        // A multi-file drop arrives as one event per file: the first
        // replaces the playlist and starts playing, the rest are added.
        let now = Instant::now();
        let same_drop = self.last_drop.is_some_and(|t| now - t < DROP_BURST);
        self.last_drop = Some(now);
        if same_drop {
            self.playlist.append(path);
        } else {
            self.playlist.replace(vec![path]);
            self.play_current();
        }
    }

    fn toggle_double_size(&mut self) {
        self.double_size = !self.double_size;
        let zoom = self.zoom();
        for (view, (width, height)) in [
            (&self.main_view, (main_window::WIDTH, main_window::HEIGHT)),
            (&self.eq_view, (eq_window::WIDTH, eq_window::HEIGHT)),
        ] {
            if let Some(view) = view {
                let _ = view.window.request_inner_size(LogicalSize::new(width * zoom, height * zoom));
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.main_view.is_some() {
            return;
        }
        let zoom = self.zoom();
        self.main_view = Some(View::new(event_loop, "skinamp", main_window::WIDTH, main_window::HEIGHT, zoom, true));
        // The equalizer window is created hidden and shown on request.
        self.eq_view = Some(View::new(event_loop, "Equalizer", eq_window::WIDTH, eq_window::HEIGHT, zoom, false));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(kind) = self.kind_of(id) else { return };
        match event {
            WindowEvent::CloseRequested => match kind {
                Kind::Main => event_loop.exit(),
                Kind::Equalizer => self.set_equalizer_open(false),
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
            },
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved { position, .. } => {
                let zoom = self.zoom();
                let Some(view) = (match kind {
                    Kind::Main => self.main_view.as_mut(),
                    Kind::Equalizer => self.eq_view.as_mut(),
                }) else {
                    return;
                };
                view.cursor = position;
                let (x, y) = view.cursor_in_skin(zoom);
                match kind {
                    Kind::Main if self.state.dragging.is_some() => self.main_drag_to(x),
                    Kind::Equalizer if self.eq.dragging.is_some() => self.eq_drag_to(y),
                    _ => return, // nothing changes on screen
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => match (kind, state) {
                (Kind::Main, ElementState::Pressed) => self.main_mouse_down(),
                (Kind::Main, ElementState::Released) => self.main_mouse_up(event_loop),
                (Kind::Equalizer, ElementState::Pressed) => self.eq_mouse_down(),
                (Kind::Equalizer, ElementState::Released) => self.eq_mouse_up(),
            },
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 20.0,
                };
                self.set_volume(self.state.volume + steps * 0.02);
            }
            WindowEvent::DroppedFile(path) => self.file_dropped(path),
            WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, .. }, .. } => {
                self.key_pressed(&logical_key)
            }
            WindowEvent::Resized(_) => {}
            _ => return,
        }
        // Most changes show in the main window; the equalizer window
        // redraws itself when its own settings change.
        self.redraw(Kind::Main);
        self.redraw(kind);
    }

    /// Runs when the app has handled all pending events and is about to
    /// sleep. While something is moving, wake up again for the next frame.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.animating() {
            let now = Instant::now();
            if now >= self.next_frame {
                self.redraw(Kind::Main);
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
