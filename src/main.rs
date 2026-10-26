//! Whiplash Player: a music player compatible with classic Winamp 2.x skins.
//!
//! Usage: `whiplash [skin.wsz] [music files, folders or playlists...]`. Skins
//! and music can also be dropped onto the windows. Without a skin, Whiplash
//! uses its own built-in one.
//!
//! Two more ways to run it, which open no windows:
//! - `whiplash --render-to picture.bmp [skin.wsz] [--scale N]` draws the
//!   windows with a skin (or the built-in one) into a picture.
//! - `whiplash --export-skin whiplash.wsz` saves the built-in skin, as a
//!   `.wsz` file or into a folder, for making your own version of it.

// A release build on Windows opens no console window. The two modes above
// borrow the console of the terminal they were run from instead.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod audio;
mod config;
mod eqf;
mod library;
mod location;
mod metadata;
mod net;
mod playlist;
mod playlist_file;
mod preview;
mod skin;
mod ui;
mod vis;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use winit::event_loop::EventLoop;

use skin::Skin;

/// `--render-to OUT [SKIN] [--scale N]`.
fn render_to_file(args: &[PathBuf]) -> Result<()> {
    let mut out = None;
    let mut skin_path = None;
    let mut scale = 1;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg.as_os_str() == "--scale" {
            let value = args.next().context("--scale needs a number")?;
            scale = value.to_string_lossy().parse().context("--scale needs a whole number")?;
        } else if out.is_none() {
            out = Some(arg);
        } else {
            skin_path = Some(arg);
        }
    }
    let Some(out) = out else { bail!("usage: whiplash --render-to picture.bmp [skin.wsz] [--scale N]") };
    let skin = match skin_path {
        Some(path) => app::open_skin(path)?,
        None => Skin::builtin(),
    };
    preview::render(&skin, out, scale)
}

fn main() -> Result<()> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    match args.first().map(|a| a.as_os_str().to_string_lossy()) {
        Some(flag) if flag == "--render-to" => {
            attach_console();
            return render_to_file(&args[1..]);
        }
        Some(flag) if flag == "--export-skin" => {
            attach_console();
            let [_, path] = args.as_slice() else { bail!("usage: whiplash --export-skin whiplash.wsz") };
            return skin::export_builtin(path);
        }
        _ => {}
    }
    let result = run(args);
    if let Err(err) = &result {
        show_error(err);
    }
    result
}

/// Opens the windows and plays music until they're closed.
fn run(args: Vec<PathBuf>) -> Result<()> {
    let config = config::Config::load();

    // A skin on the command line is used; otherwise last time's skin, or
    // the built-in one. Everything else on the command line is music.
    let (skins, tracks): (Vec<PathBuf>, Vec<PathBuf>) = args.into_iter().partition(|p| app::is_skin(p));
    let (skin, skin_path) = match skins.last() {
        // Remember the full path, so it still works from another folder.
        Some(path) => (app::open_skin(path)?, Some(std::fs::canonicalize(path).unwrap_or(path.clone()))),
        None => match config.skin.as_ref().map(|path| (app::open_skin(path), path)) {
            Some((Ok(skin), path)) => (skin, Some(path.clone())),
            Some((Err(err), path)) => {
                eprintln!("could not load last skin {}: {err:#}", path.display());
                (Skin::builtin(), None)
            }
            None => (Skin::builtin(), None),
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

/// Lets a release build on Windows print to the terminal it was run from.
#[cfg(windows)]
fn attach_console() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // Fails when there's no terminal, or already one (a debug build).
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

#[cfg(not(windows))]
fn attach_console() {}

/// Puts an error that stops the app in a message box, since a release build
/// on Windows has no console to print it to.
#[cfg(windows)]
fn show_error(err: &anyhow::Error) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    use windows::core::HSTRING;
    let text = HSTRING::from(format!("{err:#}"));
    unsafe { MessageBoxW(None, &text, &HSTRING::from("Whiplash Player"), MB_OK | MB_ICONERROR) };
}

#[cfg(not(windows))]
fn show_error(_err: &anyhow::Error) {}
