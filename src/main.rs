//! skinamp: a music player compatible with classic Winamp 2.x skins.
//!
//! Usage: `skinamp [skin.wsz] [music files, folders or playlists...]`. Skins
//! and music can also be dropped onto the windows.

mod app;
mod audio;
mod config;
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

    let config = config::Config::load();

    // A skin on the command line is used; otherwise last time's skin.
    // Everything else on the command line is music.
    let (skins, tracks): (Vec<PathBuf>, Vec<PathBuf>) = args.into_iter().partition(|p| app::is_skin(p));
    let (skin, skin_path) = match skins.last() {
        // Remember the full path, so it still works from another folder.
        Some(path) => (app::open_skin(path)?, Some(std::fs::canonicalize(path).unwrap_or(path.clone()))),
        None => match config.skin.as_ref().map(|path| (app::open_skin(path), path)) {
            Some((Ok(skin), path)) => (skin, Some(path.clone())),
            Some((Err(err), path)) => {
                eprintln!("could not load last skin {}: {err:#}", path.display());
                (Skin::empty(), None)
            }
            None => (Skin::empty(), None),
        },
    };

    // "User events" let other threads wake the window loop.
    let event_loop = EventLoop::<app::UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let quit = proxy.clone();
    ctrlc::set_handler(move || {
        let _ = quit.send_event(app::UserEvent::Quit);
    })?;
    let mut app = app::App::new(skin, skin_path, config, tracks, proxy);
    event_loop.run_app(&mut app)?;
    Ok(())
}
