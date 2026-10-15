//! The application: owns the window, the skin, the player and the playlist,
//! and turns mouse and keyboard input into player actions.

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Result;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::audio::{self, PlayState, Player};
use crate::playlist::Playlist;
use crate::skin::Skin;
use crate::ui::canvas::Canvas;
use crate::ui::main_window::{self, Control, Hit, MainState, Slider};
use crate::vis::Visualizer;

/// How often the display updates while something is moving (about 30 fps).
const FRAME: Duration = Duration::from_millis(33);
/// How often a long title scrolls by one character.
const MARQUEE_STEP: Duration = Duration::from_millis(200);
/// How long error messages stay in the marquee.
const MESSAGE_TIME: Duration = Duration::from_secs(4);
/// Files dropped this close together count as one multi-file drop.
const DROP_BURST: Duration = Duration::from_millis(300);
/// Arrow keys seek by this many seconds.
const SEEK_STEP: f64 = 5.0;

/// The window and the surface we draw pixels onto. These can only be created
/// once the event loop is running, so the app holds them in an `Option`.
struct Graphics {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
}

pub struct App {
    skin: Skin,
    skin_name: String,
    state: MainState,
    canvas: Canvas,
    graphics: Option<Graphics>,
    player: Player,
    playlist: Playlist,
    visualizer: Visualizer,
    /// Last known mouse position, in physical (screen) pixels.
    cursor: PhysicalPosition<f64>,
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
            canvas: Canvas::new(main_window::WIDTH, main_window::HEIGHT),
            graphics: None,
            player: Player::new(),
            playlist: Playlist::default(),
            visualizer: Visualizer::new(),
            cursor: PhysicalPosition::new(0.0, 0.0),
            modifiers: ModifiersState::empty(),
            double_size: false,
            message: None,
            clock: now,
            next_frame: now,
            marquee_scrolled_at: now,
            last_drop: None,
        };
        app.player.set_volume(app.state.volume);
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

    /// Copies the player's state into what the window shows.
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

    fn redraw(&self) {
        if let Some(g) = &self.graphics {
            g.window.request_redraw();
        }
    }

    fn render(&mut self) -> Result<()> {
        self.update();
        let Some(g) = &mut self.graphics else { return Ok(()) };
        let size = g.window.inner_size();
        let (Some(width), Some(height)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return Ok(()); // minimized
        };

        main_window::draw(&mut self.canvas, &self.skin, &self.state);
        if self.state.play_state != PlayState::Stopped {
            let (x, y) = main_window::VIS_POSITION;
            self.visualizer.draw(&mut self.canvas, &self.skin, x, y);
        }

        g.surface.resize(width, height).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut buffer = g.surface.buffer_mut().map_err(|e| anyhow::anyhow!("{e}"))?;
        // Pick the largest whole-number scale that fits, so every skin pixel
        // becomes an equal square block (2x on Retina, 4x in double size).
        let scale = (size.width / self.canvas.width).min(size.height / self.canvas.height).max(1);
        self.canvas.scale_into(&mut buffer, size.width, size.height, scale);
        buffer.present().map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    // ----- Input -----

    fn zoom(&self) -> u32 {
        if self.double_size { 2 } else { 1 }
    }

    fn window_size(&self) -> LogicalSize<u32> {
        LogicalSize::new(main_window::WIDTH * self.zoom(), main_window::HEIGHT * self.zoom())
    }

    /// The mouse position in the 275x116 skin layout.
    fn cursor_in_skin(&self) -> (i32, i32) {
        let scale = self.graphics.as_ref().map_or(1.0, |g| g.window.scale_factor());
        let logical = self.cursor.to_logical::<f64>(scale);
        let zoom = self.zoom() as f64;
        ((logical.x / zoom).floor() as i32, (logical.y / zoom).floor() as i32)
    }

    fn mouse_down(&mut self) {
        let (x, y) = self.cursor_in_skin();
        match main_window::hit_test(x, y) {
            Hit::TitleBar => {
                if let Some(g) = &self.graphics {
                    let _ = g.window.drag_window();
                }
            }
            Hit::Control(control) => self.state.pressed = Some(control),
            Hit::Slider(Slider::Position) if self.state.play_state == PlayState::Stopped || self.state.duration.is_none() => {}
            Hit::Slider(slider) => {
                self.state.dragging = Some(slider);
                self.drag_to(x);
            }
            Hit::Time => self.state.time_remaining = !self.state.time_remaining,
            Hit::Visualizer => self.visualizer.cycle_mode(),
            Hit::Nothing => {}
        }
    }

    fn mouse_up(&mut self, event_loop: &ActiveEventLoop) {
        let (x, y) = self.cursor_in_skin();
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
    fn drag_to(&mut self, x: i32) {
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
                if let Some(g) = &self.graphics {
                    g.window.set_minimized(true);
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
            // The windows these toggle arrive in phases 4 and 5.
            Control::Equalizer => self.state.equalizer_open = !self.state.equalizer_open,
            Control::Playlist => self.state.playlist_open = !self.state.playlist_open,
            Control::Options | Control::Shade => {}
        }
    }

    /// Winamp's keyboard shortcuts.
    fn key_pressed(&mut self, key: &Key) {
        let command = self.modifiers.control_key() || self.modifiers.super_key();
        match key {
            Key::Character(c) => match c.to_ascii_lowercase().as_str() {
                "d" if command => self.toggle_double_size(),
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
        let size = self.window_size();
        if let Some(g) = &self.graphics {
            let _ = g.window.request_inner_size(size);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("skinamp")
            .with_decorations(false) // the skin draws its own title bar
            .with_resizable(false)
            .with_inner_size(self.window_size());
        let window = Rc::new(event_loop.create_window(attributes).expect("failed to create window"));
        let context = softbuffer::Context::new(window.clone()).expect("failed to create graphics context");
        let surface = softbuffer::Surface::new(&context, window.clone()).expect("failed to create surface");
        self.graphics = Some(Graphics { window, surface });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Err(err) = self.render() {
                    eprintln!("drawing failed: {err:#}");
                }
                return;
            }
            WindowEvent::Focused(focused) => self.state.focused = focused,
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                if self.state.dragging.is_none() {
                    return; // nothing changes on screen
                }
                let (x, _) = self.cursor_in_skin();
                self.drag_to(x);
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => self.mouse_down(),
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                self.mouse_up(event_loop)
            }
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
        self.redraw();
    }

    /// Runs when the app has handled all pending events and is about to
    /// sleep. While something is moving, wake up again for the next frame.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.animating() {
            let now = Instant::now();
            if now >= self.next_frame {
                self.redraw();
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
