//! MilkDrop visualizations, drawn by projectM in a window of their own.
//!
//! projectM (https://github.com/projectM-visualizer/projectm, LGPL) is an
//! open-source reimplementation of Winamp's MilkDrop plugin. It draws with
//! OpenGL, so this window gets an OpenGL context from `glutin` instead of
//! the plain pixel buffer the skinned windows use. Each frame we hand it
//! the sound played since the last frame and ask it to draw.
//!
//! Presets (`.milk` files) aren't included; skinamp reads them from a folder
//! you choose. projectM's "Cream of the Crop" pack is a good start:
//! https://github.com/projectM-visualizer/presets-cream-of-the-crop

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, GlProfile, NotCurrentGlContext, PossiblyCurrentContext, Version};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use projectm::core::{MONO, ProjectM};
use projectm::playlist::Playlist;
use raw_window_handle::HasWindowHandle;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Fullscreen, Window, WindowId};

use crate::audio::Player;

/// How long each preset shows before moving on to another, in seconds.
const PRESET_SECONDS: f64 = 30.0;

pub struct MilkDrop {
    window: Window,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    projectm: ProjectM,
    playlist: Playlist,
    /// How far we've read the player's sound, so each sample is sent once.
    sample_cursor: u64,
    samples: Vec<f32>,
}

impl MilkDrop {
    /// Opens the window and starts projectM with the presets in `presets`
    /// (searched recursively). Fails if OpenGL isn't available.
    pub fn new(event_loop: &ActiveEventLoop, presets: Option<&Path>, position: Option<PhysicalPosition<i32>>) -> Result<MilkDrop> {
        let mut attributes =
            Window::default_attributes().with_title("MilkDrop").with_inner_size(LogicalSize::new(640, 480));
        if let Some(position) = position {
            attributes = attributes.with_position(position);
        }

        // Ask for a window with an OpenGL-capable pixel format, preferring
        // the smoothest (most multisampled) one on offer.
        let (window, config) = DisplayBuilder::new()
            .with_window_attributes(Some(attributes))
            .build(event_loop, ConfigTemplateBuilder::new(), |configs| {
                configs.reduce(|best, c| if c.num_samples() > best.num_samples() { c } else { best }).unwrap()
            })
            .map_err(|e| anyhow!("OpenGL is not available: {e}"))?;
        let window = window.context("could not create the MilkDrop window")?;

        // projectM 4 needs OpenGL 3.3 or newer, "core profile".
        let display = config.display();
        let handle = window.window_handle().ok().map(|h| h.as_raw());
        let context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .with_profile(GlProfile::Core)
            .build(handle);
        // Safety: the window handle stays valid, since we keep the window
        // alive for as long as the context and surface.
        let context = unsafe { display.create_context(&config, &context_attributes) }.context("creating OpenGL context")?;
        let surface_attributes = window.build_surface_attributes(Default::default()).context("creating OpenGL surface")?;
        let surface = unsafe { display.create_window_surface(&config, &surface_attributes) }.context("creating OpenGL surface")?;
        let context = context.make_current(&surface).context("activating OpenGL")?;
        // Draw in step with the screen's refresh rate.
        let _ = surface.set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::MIN));

        // projectM must be created while its OpenGL context is current.
        let projectm = ProjectM::create();
        projectm.set_preset_duration(PRESET_SECONDS);
        let size = window.inner_size();
        projectm.set_window_size(size.width as usize, size.height as usize);

        let mut playlist = Playlist::create(&projectm);
        playlist.set_shuffle(true);
        if let Some(folder) = presets {
            playlist.add_path(&folder.to_string_lossy(), true);
            let textures: Vec<String> = texture_folders(folder).iter().map(|p| p.to_string_lossy().into_owned()).collect();
            projectm.set_texture_search_paths(&textures, textures.len());
        }
        if !playlist.is_empty() {
            playlist.play_random();
        }

        Ok(MilkDrop { window, surface, context, projectm, playlist, sample_cursor: 0, samples: Vec::new() })
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    /// How many presets were found.
    pub fn preset_count(&self) -> u32 {
        self.playlist.len()
    }

    /// Feeds projectM the newest sound and draws one frame.
    pub fn render(&mut self, player: &Player) {
        self.samples.clear();
        player.samples_since(&mut self.sample_cursor, &mut self.samples);
        let max = ProjectM::pcm_get_max_samples() as usize;
        for chunk in self.samples.chunks(max.max(1)) {
            self.projectm.pcm_add_float(chunk, MONO);
        }
        self.projectm.render_frame();
        if let Err(err) = self.surface.swap_buffers(&self.context) {
            eprintln!("MilkDrop: {err}");
        }
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if let (Some(width), Some(height)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
            self.surface.resize(&self.context, width, height);
            self.projectm.set_window_size(size.width as usize, size.height as usize);
        }
    }

    pub fn next_preset(&mut self) {
        self.playlist.play_next();
    }

    pub fn previous_preset(&mut self) {
        self.playlist.play_prev();
    }

    pub fn random_preset(&mut self) {
        self.playlist.play_random();
    }

    /// Keeps the current preset instead of moving on. Returns whether it's
    /// now locked.
    pub fn toggle_lock(&self) -> bool {
        let locked = !self.projectm.get_preset_locked();
        self.projectm.set_preset_locked(locked);
        locked
    }

    pub fn toggle_fullscreen(&self) {
        let fullscreen = match self.window.fullscreen() {
            Some(_) => None,
            None => Some(Fullscreen::Borderless(None)),
        };
        self.window.set_fullscreen(fullscreen);
    }

    pub fn is_fullscreen(&self) -> bool {
        self.window.fullscreen().is_some()
    }
}

/// Folders that may hold the textures some presets use: the presets folder
/// itself, a "textures" folder inside it, or one next to it.
fn texture_folders(presets: &Path) -> Vec<PathBuf> {
    let mut folders = vec![presets.to_path_buf(), presets.join("textures")];
    if let Some(parent) = presets.parent() {
        folders.push(parent.join("textures"));
    }
    folders.into_iter().filter(|f| f.is_dir()).collect()
}
