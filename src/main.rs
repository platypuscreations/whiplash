//! skinamp: a music player compatible with classic Winamp 2.x skins.
//!
//! Usage: `skinamp [skin.wsz] [music files, folders or playlists...]`. Skins
//! and music can also be dropped onto the windows.

mod app;
mod audio;
mod eqf;
mod metadata;
mod playlist;
mod playlist_file;
mod skin;
mod ui;
mod vis;

use std::path::{Path, PathBuf};

use anyhow::Result;
use winit::event_loop::EventLoop;

use skin::Skin;
use ui::canvas::Canvas;
use ui::main_window::{self, MainState};

/// Draws the main window once and saves it as a BMP instead of opening a
/// window. Useful for comparing against Webamp screenshots.
fn render_to_file(skin_path: &Path, out: &Path) -> Result<()> {
    let skin = app::open_skin(skin_path)?;
    let state = MainState { marquee: app::file_name(skin_path), ..MainState::default() };
    let mut canvas = Canvas::new(main_window::WIDTH, main_window::HEIGHT);
    main_window::draw(&mut canvas, &skin, &state);

    let rgb: Vec<u8> = canvas.pixels.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
    image::save_buffer_with_format(out, &rgb, canvas.width, canvas.height, image::ColorType::Rgb8, image::ImageFormat::Bmp)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if let [flag, out, skin] = args.as_slice()
        && flag.as_os_str() == "--render-to"
    {
        return render_to_file(skin, out);
    }

    // Any skin on the command line is used; everything else is music.
    let (skins, tracks): (Vec<PathBuf>, Vec<PathBuf>) = args.into_iter().partition(|p| app::is_skin(p));
    let (skin, skin_name) = match skins.last() {
        Some(path) => (app::open_skin(path)?, app::file_name(path)),
        None => (Skin::empty(), String::new()),
    };

    // "User events" let other threads wake the window loop.
    let event_loop = EventLoop::<app::UserEvent>::with_user_event().build()?;
    let mut app = app::App::new(skin, skin_name, tracks, event_loop.create_proxy());
    event_loop.run_app(&mut app)?;
    Ok(())
}
