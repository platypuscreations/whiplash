//! The equalizer window's behavior.

use winit::event_loop::ActiveEventLoop;
use super::{App, Kind};
use crate::ui::eq_window::{self, EqControl};
use crate::ui::main_window::Slider;

impl App {
    /// Sends the equalizer window's settings to the player.
    pub(super) fn apply_equalizer(&mut self) {
        let eq = &self.eq;
        self.player.set_equalizer(eq.enabled, eq_window::to_db(eq.preamp), eq.bands.map(eq_window::to_db));
        self.redraw(Kind::Equalizer);
    }

    pub(super) fn set_equalizer_open(&mut self, open: bool) {
        self.state.equalizer_open = open;
        self.dock_changed = true;
        if open {
            self.place_and_show(Kind::Equalizer);
        } else if let Some(eq) = &self.eq_view {
            eq.window.set_visible(false);
        }
        self.redraw(Kind::Main);
    }

    pub(super) fn eq_mouse_down(&mut self) {
        let Some(view) = &self.eq_view else { return };
        let (x, y) = view.cursor_in_skin();
        match eq_window::hit_test(x, y, self.eq.shaded) {
            eq_window::Hit::TitleBar => self.title_bar_pressed(Kind::Equalizer),
            // The tiny sliders in shade mode set the player's volume and
            // balance, showing the value in the main window like its own.
            eq_window::Hit::ShadeVolume => {
                self.state.dragging = Some(Slider::Volume);
                self.eq_shade_drag_to(x);
            }
            eq_window::Hit::ShadeBalance => {
                self.state.dragging = Some(Slider::Balance);
                self.eq_shade_drag_to(x);
            }
            eq_window::Hit::Control(control) => self.eq.pressed = Some(control),
            eq_window::Hit::Slider(index) => {
                self.eq.dragging = Some(index);
                self.eq_drag_to(y);
            }
            eq_window::Hit::Nothing => {}
        }
    }

    pub(super) fn eq_mouse_up(&mut self, event_loop: &ActiveEventLoop) {
        let Some(view) = &self.eq_view else { return };
        let (x, y) = view.cursor_in_skin();
        self.eq.dragging = None;
        if self.eq.shaded {
            self.state.dragging = None;
        }
        if let Some(control) = self.eq.pressed.take()
            && eq_window::hit_test(x, y, self.eq.shaded) == eq_window::Hit::Control(control)
        {
            self.eq_activate(control, event_loop);
        }
    }

    pub(super) fn eq_shade_drag_to(&mut self, x: i32) {
        match self.state.dragging {
            Some(Slider::Volume) => self.set_volume(eq_window::shade_volume_value(x)),
            Some(Slider::Balance) => self.set_balance(eq_window::shade_balance_value(x)),
            _ => {}
        }
    }

    pub(super) fn eq_drag_to(&mut self, y: i32) {
        if let Some(index) = self.eq.dragging {
            self.eq.set_slider(index, eq_window::slider_value(y));
            self.apply_equalizer();
        }
    }

    fn eq_activate(&mut self, control: EqControl, event_loop: &ActiveEventLoop) {
        match control {
            EqControl::Close => self.set_equalizer_open(false),
            EqControl::Shade => self.toggle_shade(Kind::Equalizer),
            EqControl::On => self.eq.enabled = !self.eq.enabled,
            // "Auto" applies a song's own preset (saved from the presets
            // menu) whenever that song starts.
            EqControl::Auto => {
                self.eq.auto = !self.eq.auto;
                self.apply_auto_preset();
            }
            EqControl::Presets => self.open_presets_menu(event_loop),
            EqControl::AllMax => self.eq.bands = [eq_window::SLIDER_MAX; 10],
            EqControl::AllFlat => self.eq.bands = [eq_window::SLIDER_FLAT; 10],
            EqControl::AllMin => self.eq.bands = [0.0; 10],
        }
        self.apply_equalizer();
    }
}
