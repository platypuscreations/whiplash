//! Tooltips: after the mouse rests on a button or slider for a moment, a
//! small box names it (and shows its setting), since some skins make the
//! controls hard to recognize.

use std::time::{Duration, Instant};

use winit::dpi::PhysicalPosition;
use winit::event_loop::ActiveEventLoop;

use winit::dpi::PhysicalSize;

use super::{App, Kind, Show, View, format_time};
use crate::ui::canvas::Canvas;
use crate::audio;
use crate::ui::eq_window::{self, EqControl};
use crate::ui::main_window::{self, Control, Slider};
use crate::ui::playlist_window::{self, MenuButton, MenuItem, PlControl};
use crate::ui::text::Font;
use crate::ui::tooltip;
use crate::vis::Mode;

/// How long the mouse must rest on a control before its tooltip appears.
pub(super) const DELAY: Duration = Duration::from_millis(600);

/// A control that can have a tooltip, in one of the windows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Target {
    Main(main_window::Hit),
    Equalizer(eq_window::Hit),
    Playlist(playlist_window::Hit),
}

/// What the mouse is resting on, and since when.
pub(super) struct Hover {
    target: Target,
    since: Instant,
    /// Set by a click: the tooltip stays away until the mouse moves on.
    dismissed: bool,
}

impl App {
    /// Notes which control the mouse is over in `kind`'s window. Moving to
    /// a different control hides the tooltip and restarts the wait.
    pub(super) fn update_hover(&mut self, kind: Kind) {
        let target = self.view(kind).and_then(|view| {
            let (x, y) = view.cursor_in_skin();
            let target = match kind {
                Kind::Main => Target::Main(main_window::hit_test(x, y, self.state.shaded)),
                Kind::Equalizer => Target::Equalizer(eq_window::hit_test(x, y, self.eq.shaded)),
                Kind::Playlist => Target::Playlist(playlist_window::hit_test(&self.pl, self.pl_rows(), x, y)),
                Kind::Menu => return None,
            };
            self.tooltip_text(target).map(|_| target)
        });
        if self.hover.as_ref().map(|h| h.target) != target {
            self.hide_tooltip();
            self.hover = target.map(|target| Hover { target, since: Instant::now(), dismissed: false });
        }
    }

    /// The mouse left a window, or something else should clear the tooltip.
    pub(super) fn clear_hover(&mut self) {
        self.hide_tooltip();
        self.hover = None;
    }

    /// A click hides the tooltip until the mouse moves to another control.
    pub(super) fn dismiss_tooltip(&mut self) {
        self.hide_tooltip();
        if let Some(hover) = &mut self.hover {
            hover.dismissed = true;
        }
    }

    fn hide_tooltip(&mut self) {
        self.tooltip_view = None;
        self.tooltip_text.clear();
    }

    /// When the waiting tooltip should appear, if one is waiting.
    pub(super) fn tooltip_due(&self) -> Option<Instant> {
        let hover = self.hover.as_ref()?;
        (!hover.dismissed && self.tooltip_view.is_none() && self.menu.is_none()).then_some(hover.since + DELAY)
    }

    /// Shows the tooltip if it's due, and keeps a shown one's text current
    /// (a value like the volume may have changed).
    pub(super) fn refresh_tooltip(&mut self, event_loop: &ActiveEventLoop) {
        let Some(target) = self.hover.as_ref().filter(|h| !h.dismissed).map(|h| h.target) else { return };
        let due = self.tooltip_due().is_some_and(|due| Instant::now() >= due);
        let Some(text) = self.tooltip_text(target) else { return };
        let kind = match target {
            Target::Main(_) => Kind::Main,
            Target::Equalizer(_) => Kind::Equalizer,
            Target::Playlist(_) => Kind::Playlist,
        };
        let Some(parent) = self.view(kind) else { return };
        let scale = parent.window.scale_factor();
        // Just below the mouse pointer.
        let position = parent.window.outer_position().ok().map(|origin| {
            PhysicalPosition::new(origin.x + parent.cursor.x as i32, origin.y + (parent.cursor.y + 20.0 * scale) as i32)
        });
        // Tooltips never take focus from the window being used.
        let show = position.map(|position| Show::at(position, false).owned_by(&parent.window));

        // The font is loaded at the screen's real pixel size, so text is sharp.
        if self.tooltip_font.as_ref().is_none_or(|(_, s)| *s != scale) {
            self.tooltip_font = Font::load("Tahoma", tooltip::FONT_PX * scale as f32).map(|font| (font, scale));
        }
        let Some((font, _)) = self.tooltip_font.as_mut() else { return };
        let (width, height) = tooltip::size(font, &text, scale);
        let size = PhysicalSize::new(width, height);

        // A tooltip already showing: update it if its value changed.
        if let Some(view) = &mut self.tooltip_view {
            if text != self.tooltip_text {
                view.canvas = Canvas::new(width, height);
                view.resize(size);
                view.window.request_redraw();
                self.tooltip_text = text;
            }
            return;
        }
        let (true, Some(show)) = (due, show) else { return };

        // The window is sized in real pixels and drawn one canvas pixel per
        // screen pixel.
        let view = View::new(event_loop, "tooltip", width, height, size.into(), show);
        view.window.set_window_level(winit::window::WindowLevel::AlwaysOnTop);
        view.window.request_redraw();
        self.tooltip_view = Some(view);
        self.tooltip_text = text;
    }

    pub(super) fn render_tooltip(&mut self) {
        let (Some(view), Some((font, scale))) = (&mut self.tooltip_view, &mut self.tooltip_font) else { return };
        tooltip::draw(&mut view.canvas, font, &self.tooltip_text, *scale);
        if let Err(err) = view.present() {
            eprintln!("drawing failed: {err:#}");
        }
    }

    /// The tooltip for a control, or `None` for places without one (title
    /// bars, empty space, track names).
    fn tooltip_text(&self, target: Target) -> Option<String> {
        let on_off = |on: bool| if on { "on" } else { "off" };
        let text = match target {
            Target::Main(hit) => match hit {
                main_window::Hit::Control(control) => match control {
                    Control::Options | Control::ClutterO => "Main menu".into(),
                    Control::Minimize => "Minimize".into(),
                    Control::Shade => "Windowshade mode".into(),
                    Control::Close => "Close".into(),
                    Control::Eject => "Open file(s)".into(),
                    Control::Shuffle => format!("Shuffle: {}", on_off(self.state.shuffle)),
                    Control::Repeat => format!("Repeat: {}", on_off(self.state.repeat)),
                    Control::Equalizer => "Equalizer (Alt+G)".into(),
                    Control::Playlist => "Playlist editor (Alt+E)".into(),
                    Control::ClutterA => format!("Always on top: {}", on_off(self.always_on_top)),
                    Control::ClutterI => "File info".into(),
                    Control::ClutterD => format!("Double size: {}", on_off(self.double_size)),
                    Control::ClutterV => "Visualization menu".into(),
                    transport => transport_name(transport).into(),
                },
                main_window::Hit::Slider(slider) => self.slider_tooltip(slider),
                main_window::Hit::Time => {
                    let showing = if self.state.time_remaining { "remaining" } else { "elapsed" };
                    format!("Time {showing} (click to switch)")
                }
                main_window::Hit::Visualizer => {
                    let mode = match self.visualizer.mode {
                        Mode::Analyzer => "spectrum analyzer",
                        Mode::Oscilloscope => "oscilloscope",
                        Mode::Off => "off",
                    };
                    format!("Visualization: {mode} (click to change)")
                }
                main_window::Hit::TitleBar | main_window::Hit::Nothing => return None,
            },
            Target::Equalizer(hit) => match hit {
                eq_window::Hit::Control(control) => match control {
                    EqControl::Close => "Close equalizer".into(),
                    EqControl::Shade => "Windowshade mode".into(),
                    EqControl::On => format!("Equalizer: {}", on_off(self.eq.enabled)),
                    EqControl::Auto => format!("Auto-load song presets: {}", on_off(self.eq.auto)),
                    EqControl::Presets => "Presets".into(),
                    EqControl::AllMax => "Set all bands to +12 dB".into(),
                    EqControl::AllFlat => "Set all bands to 0 dB".into(),
                    EqControl::AllMin => "Set all bands to -12 dB".into(),
                },
                eq_window::Hit::Slider(index) => {
                    let name = match index {
                        0 => "Preamp".to_string(),
                        i => match audio::EQ_BAND_HZ[i - 1] {
                            hz if hz >= 1000.0 => format!("{} kHz", hz / 1000.0),
                            hz => format!("{hz} Hz"),
                        },
                    };
                    format!("{name}: {:+.1} dB", eq_window::to_db(self.eq.slider(index)))
                }
                eq_window::Hit::ShadeVolume => self.slider_tooltip(Slider::Volume),
                eq_window::Hit::ShadeBalance => self.slider_tooltip(Slider::Balance),
                eq_window::Hit::TitleBar | eq_window::Hit::Nothing => return None,
            },
            Target::Playlist(hit) => match hit {
                playlist_window::Hit::Control(control) => match control {
                    PlControl::Close => "Close playlist".into(),
                    PlControl::Shade => "Windowshade mode".into(),
                    PlControl::ScrollUp => "Scroll up".into(),
                    PlControl::ScrollDown => "Scroll down".into(),
                    PlControl::Previous => transport_name(Control::Previous).into(),
                    PlControl::Play => transport_name(Control::Play).into(),
                    PlControl::Pause => transport_name(Control::Pause).into(),
                    PlControl::Stop => transport_name(Control::Stop).into(),
                    PlControl::Next => transport_name(Control::Next).into(),
                    PlControl::Eject => "Open file(s)".into(),
                },
                playlist_window::Hit::MenuButton(button) => match button {
                    MenuButton::Add => "Add files or a folder",
                    MenuButton::Remove => "Remove tracks",
                    MenuButton::Select => "Select tracks",
                    MenuButton::Misc => "Sort, file info and options",
                    MenuButton::List => "New, save or load a playlist",
                }
                .into(),
                playlist_window::Hit::MenuItem(item) => match item {
                    MenuItem::AddUrl => "Add internet address",
                    MenuItem::AddFolder => "Add folder",
                    MenuItem::AddFiles => "Add files",
                    MenuItem::RemoveMisc => "More ways to remove",
                    MenuItem::RemoveAll => "Remove all",
                    MenuItem::Crop => "Crop: keep only the selected tracks",
                    MenuItem::RemoveSelected => "Remove selected",
                    MenuItem::InvertSelection => "Invert selection",
                    MenuItem::SelectNone => "Select none",
                    MenuItem::SelectAll => "Select all",
                    MenuItem::SortList => "Sort list",
                    MenuItem::FileInfo => "File info",
                    MenuItem::MiscOptions => "Playlist options",
                    MenuItem::NewList => "New playlist",
                    MenuItem::SaveList => "Save playlist",
                    MenuItem::LoadList => "Load playlist",
                }
                .into(),
                playlist_window::Hit::ScrollBar => "Scroll".into(),
                playlist_window::Hit::Resize => "Resize".into(),
                playlist_window::Hit::Track(_)
                | playlist_window::Hit::BrowseRow(_)
                | playlist_window::Hit::Tab(_)
                | playlist_window::Hit::SearchBox
                | playlist_window::Hit::BelowTracks
                | playlist_window::Hit::TitleBar
                | playlist_window::Hit::Nothing => return None,
            },
        };
        Some(text)
    }

    fn slider_tooltip(&self, slider: Slider) -> String {
        let state = &self.state;
        match slider {
            Slider::Volume => format!("Volume: {}%", (state.volume * 100.0).round()),
            Slider::Balance if state.balance == 0.0 => "Balance: center".into(),
            Slider::Balance => {
                let side = if state.balance < 0.0 { "left" } else { "right" };
                format!("Balance: {}% {side}", (state.balance.abs() * 100.0).round())
            }
            Slider::Position => match state.duration {
                Some(duration) if state.play_state != audio::PlayState::Stopped => {
                    format!("Seek: {} / {}", format_time(state.elapsed), format_time(duration))
                }
                _ => "Seek bar".into(),
            },
        }
    }
}

fn transport_name(control: Control) -> &'static str {
    match control {
        Control::Previous => "Previous track (Z)",
        Control::Play => "Play (X)",
        Control::Pause => "Pause (C)",
        Control::Stop => "Stop (V)",
        Control::Next => "Next track (B)",
        _ => "",
    }
}
