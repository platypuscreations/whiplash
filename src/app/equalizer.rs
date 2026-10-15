//! The equalizer window's behavior.

use anyhow::Context;
use winit::event_loop::ActiveEventLoop;
use super::{App, Kind, file_name};
use crate::eqf;
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
        if open {
            self.place_and_show(Kind::Equalizer);
        } else if let Some(eq) = &self.eq_view {
            eq.window.set_visible(false);
        }
        self.redraw(Kind::Main);
    }

    fn preset_file_dialog() -> rfd::FileDialog {
        rfd::FileDialog::new().add_filter("Winamp EQ presets", &["eqf", "q1"])
    }

    pub(super) fn load_preset(&mut self) {
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

    pub(super) fn save_preset(&mut self) {
        let dialog = Self::preset_file_dialog().set_title("Save EQ preset").set_file_name("preset.eqf");
        let Some(path) = dialog.save_file() else { return };
        let preset = eqf::Preset { name: file_name(&path), preamp: self.eq.preamp, bands: self.eq.bands };
        match std::fs::write(&path, eqf::write(&[preset])) {
            Ok(()) => self.show_message(format!("saved eq preset: {}", file_name(&path))),
            Err(err) => self.show_message(format!("could not save preset: {err}")),
        }
    }

    pub(super) fn eq_mouse_down(&mut self) {
        let Some(view) = &self.eq_view else { return };
        let (x, y) = view.cursor_in_skin(self.zoom());
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
        let (x, y) = view.cursor_in_skin(self.zoom());
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
