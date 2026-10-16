//! The MilkDrop window's behavior. In a build without the `milkdrop`
//! feature, these just explain that it isn't available.

#[cfg(feature = "milkdrop")]
mod enabled {
    use std::time::{Duration, Instant};

    use winit::dpi::PhysicalPosition;
    use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
    use winit::event_loop::ActiveEventLoop;
    use winit::keyboard::{Key, NamedKey};
    use winit::window::WindowId;

    use super::super::{App, Kind};
    use crate::config;
    use crate::vis::milkdrop::MilkDrop;

    const DOUBLE_CLICK: Duration = Duration::from_millis(400);

    impl App {
        pub(in crate::app) fn is_milkdrop_window(&self, id: WindowId) -> bool {
            self.milkdrop.as_ref().is_some_and(|m| m.window_id() == id)
        }

        /// True once the MilkDrop window has been created (its presets are set then).
        pub(in crate::app) fn milkdrop_started(&self) -> bool {
            self.milkdrop.is_some()
        }

        pub(in crate::app) fn set_milkdrop_open(&mut self, event_loop: &ActiveEventLoop, open: bool) {
            if open && self.milkdrop.is_none() {
                // Open to the right of the main window the first time.
                let position = self.rect(Kind::Main).map(|r| PhysicalPosition::new(r.x + r.w, r.y));
                let presets = self.milkdrop_presets.clone().or_else(config::presets_folder);
                match MilkDrop::new(event_loop, presets.as_deref(), position) {
                    Ok(milkdrop) => {
                        if milkdrop.preset_count() == 0 {
                            self.show_message("no milkdrop presets found: choose a folder in the main menu".into());
                        }
                        self.milkdrop = Some(milkdrop);
                    }
                    Err(err) => {
                        self.show_message(format!("milkdrop: {err:#}"));
                        return;
                    }
                }
            }
            let Some(milkdrop) = &self.milkdrop else { return };
            let window = milkdrop.window();
            if open {
                window.set_visible(true);
                window.focus_window();
                window.request_redraw();
            } else {
                if milkdrop.is_fullscreen() {
                    milkdrop.toggle_fullscreen();
                }
                window.set_visible(false);
            }
            self.milkdrop_open = open;
            self.redraw_all();
        }

        pub(in crate::app) fn milkdrop_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
            match event {
                WindowEvent::CloseRequested => self.set_milkdrop_open(event_loop, false),
                WindowEvent::RedrawRequested => {
                    if let Some(milkdrop) = &mut self.milkdrop
                        && self.milkdrop_open
                    {
                        milkdrop.render(&self.player);
                        // Keep animating; drawing waits for the screen's
                        // refresh, so this runs at its frame rate.
                        milkdrop.window().request_redraw();
                    }
                }
                WindowEvent::Resized(size) => {
                    if let Some(milkdrop) = &mut self.milkdrop {
                        milkdrop.resize(size);
                    }
                }
                WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
                WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                    // Double-click toggles full screen.
                    let now = Instant::now();
                    if self.milkdrop_last_click.is_some_and(|at| now - at < DOUBLE_CLICK) {
                        self.milkdrop_last_click = None;
                        if let Some(milkdrop) = &self.milkdrop {
                            milkdrop.toggle_fullscreen();
                        }
                    } else {
                        self.milkdrop_last_click = Some(now);
                    }
                }
                WindowEvent::KeyboardInput {
                    event: KeyEvent { logical_key, state: ElementState::Pressed, .. }, ..
                } => self.milkdrop_key(event_loop, &logical_key),
                _ => {}
            }
        }

        /// MilkDrop's keys: N next preset, P previous, R random, L lock,
        /// F full screen, Esc leaves full screen or closes. Other keys work
        /// as in the main window.
        fn milkdrop_key(&mut self, event_loop: &ActiveEventLoop, key: &Key) {
            let command = self.command_key();
            let Some(milkdrop) = &mut self.milkdrop else { return };
            match key {
                Key::Named(NamedKey::Escape) if milkdrop.is_fullscreen() => milkdrop.toggle_fullscreen(),
                Key::Named(NamedKey::Escape) => self.set_milkdrop_open(event_loop, false),
                Key::Named(NamedKey::Space) => milkdrop.next_preset(),
                Key::Named(NamedKey::Backspace) => milkdrop.previous_preset(),
                Key::Named(NamedKey::Enter) => milkdrop.toggle_fullscreen(),
                Key::Character(c) if !command => match c.to_ascii_lowercase().as_str() {
                    "n" => milkdrop.next_preset(),
                    "p" => milkdrop.previous_preset(),
                    "r" => milkdrop.random_preset(),
                    "f" => milkdrop.toggle_fullscreen(),
                    "l" => {
                        let locked = milkdrop.toggle_lock();
                        let state = if locked { "locked" } else { "unlocked" };
                        self.show_message(format!("milkdrop preset {state}"));
                    }
                    _ => self.key_pressed(key, Kind::Main, event_loop),
                },
                _ => self.key_pressed(key, Kind::Main, event_loop),
            }
            self.redraw_all();
        }
    }
}

#[cfg(not(feature = "milkdrop"))]
mod disabled {
    use winit::event::WindowEvent;
    use winit::event_loop::ActiveEventLoop;
    use winit::window::WindowId;

    use super::super::App;

    impl App {
        pub(in crate::app) fn is_milkdrop_window(&self, _id: WindowId) -> bool {
            false
        }

        pub(in crate::app) fn milkdrop_started(&self) -> bool {
            false
        }

        pub(in crate::app) fn set_milkdrop_open(&mut self, _event_loop: &ActiveEventLoop, _open: bool) {
            self.show_message("this build doesn't include milkdrop".into());
        }

        pub(in crate::app) fn milkdrop_event(&mut self, _event_loop: &ActiveEventLoop, _event: WindowEvent) {}
    }
}
