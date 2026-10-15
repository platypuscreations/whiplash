//! The equalizer window's behavior.

use anyhow::Context;
use winit::dpi::PhysicalPosition;

use super::{App, Kind, file_name};
use crate::eqf;
use crate::ui::eq_window::{self, EqControl};

impl App {
    /// Sends the equalizer window's settings to the player.
    pub(super) fn apply_equalizer(&mut self) {
        let eq = &self.eq;
        self.player.set_equalizer(eq.enabled, eq_window::to_db(eq.preamp), eq.bands.map(eq_window::to_db));
        self.redraw(Kind::Equalizer);
    }

    pub(super) fn set_equalizer_open(&mut self, open: bool) {
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

    pub(super) fn eq_mouse_down(&mut self) {
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

    pub(super) fn eq_mouse_up(&mut self) {
        let Some(view) = &self.eq_view else { return };
        let (x, y) = view.cursor_in_skin(self.zoom());
        self.eq.dragging = None;
        if let Some(control) = self.eq.pressed.take()
            && eq_window::hit_test(x, y) == eq_window::Hit::Control(control)
        {
            self.eq_activate(control);
        }
    }

    pub(super) fn eq_drag_to(&mut self, y: i32) {
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
}
