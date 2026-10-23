//! File dialogs and message boxes, opened without stopping the app.
//!
//! A dialog shown the ordinary way doesn't return until it is closed, and
//! while it waits the app can't draw: the visualizers and MilkDrop freeze
//! (the music plays on in its own thread). So dialogs are opened without
//! waiting, and the answer comes back to the event loop later as
//! `UserEvent::DialogClosed`.
//!
//! On macOS the dialogs are AppKit's own panels, opened as free-standing
//! windows (other ways of opening them without waiting attach them to a
//! window as a sheet, which can't be moved). Elsewhere they come from
//! `rfd`, with a helper thread waiting for the answer.

#[cfg(target_os = "macos")]
mod macos;

use std::path::PathBuf;

use anyhow::Context;
use winit::event_loop::EventLoopProxy;

use super::{App, LOCAL_ONLY, UserEvent, file_name, open_skin};
use crate::audio;
use crate::config::PlayerMode;
use crate::eqf;
use crate::playlist_file;

/// What a file dialog was opened for, which decides what its answer does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// Files or a folder that replace the playlist.
    PlayFiles,
    PlayFolder,
    /// Files or a folder added to the end of the playlist.
    AddFiles,
    AddFolder,
    LoadPlaylist,
    SavePlaylist,
    LoadSkin,
    LoadEqPreset,
    SaveEqPreset,
    MilkDropPresetsFolder,
}

impl Purpose {
    /// For music files or playlist files, which only the local files
    /// mode plays.
    fn local_only(self) -> bool {
        use Purpose::*;
        matches!(self, PlayFiles | PlayFolder | AddFiles | AddFolder | LoadPlaylist | SavePlaylist)
    }
}

/// What a dialog lets you choose.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pick {
    File,
    Files,
    Folder,
    /// A name for a new file.
    Save,
}

/// Everything about how a dialog looks.
struct Spec {
    title: &'static str,
    pick: Pick,
    /// Only files of these kinds can be chosen: a description and the
    /// file extensions.
    filter: Option<(&'static str, &'static [&'static str])>,
    /// The suggested name, when saving.
    file_name: Option<&'static str>,
}

impl Purpose {
    fn spec(self) -> Spec {
        let spec = |title, pick| Spec { title, pick, filter: None, file_name: None };
        let audio_files = Some(("Audio files", audio::EXTENSIONS));
        let eq_presets = Some(("Winamp EQ presets", &["eqf", "q1"][..]));
        match self {
            Purpose::PlayFiles => Spec { filter: audio_files, ..spec("Open file(s)", Pick::Files) },
            Purpose::PlayFolder => spec("Play folder", Pick::Folder),
            Purpose::AddFiles => Spec { filter: audio_files, ..spec("Add file(s)", Pick::Files) },
            Purpose::AddFolder => spec("Add folder", Pick::Folder),
            Purpose::LoadPlaylist => {
                Spec { filter: Some(("Playlists", playlist_file::PLAYLIST_EXTENSIONS)), ..spec("Load playlist", Pick::File) }
            }
            Purpose::SavePlaylist => Spec {
                filter: Some(("M3U playlist", &["m3u"])),
                file_name: Some("playlist.m3u"),
                ..spec("Save playlist", Pick::Save)
            },
            Purpose::LoadSkin => Spec { filter: Some(("Winamp skins", &["wsz", "zip"])), ..spec("Load skin", Pick::File) },
            Purpose::LoadEqPreset => Spec { filter: eq_presets, ..spec("Load EQ preset", Pick::File) },
            Purpose::SaveEqPreset => {
                Spec { filter: eq_presets, file_name: Some("preset.eqf"), ..spec("Save EQ preset", Pick::Save) }
            }
            Purpose::MilkDropPresetsFolder => spec("Choose a folder of MilkDrop presets", Pick::Folder),
        }
    }
}

/// Shows a dialog and sends its answer to the event loop once it closes.
#[cfg(target_os = "macos")]
fn show(purpose: Purpose, proxy: EventLoopProxy<UserEvent>) {
    macos::show(&purpose.spec(), move |paths| {
        let _ = proxy.send_event(UserEvent::DialogClosed(purpose, paths));
    });
}

/// Shows a dialog and sends its answer to the event loop once it closes.
#[cfg(not(target_os = "macos"))]
fn show(purpose: Purpose, proxy: EventLoopProxy<UserEvent>) {
    use rfd::{AsyncFileDialog, FileHandle};
    use std::future::Future;
    use std::pin::Pin;

    type Answer = Pin<Box<dyn Future<Output = Vec<PathBuf>> + Send>>;
    fn many(chosen: impl Future<Output = Option<Vec<FileHandle>>> + Send + 'static) -> Answer {
        Box::pin(async move { chosen.await.unwrap_or_default().iter().map(|f| f.path().to_path_buf()).collect() })
    }
    fn one(chosen: impl Future<Output = Option<FileHandle>> + Send + 'static) -> Answer {
        Box::pin(async move { chosen.await.map(|f| f.path().to_path_buf()).into_iter().collect() })
    }

    let spec = purpose.spec();
    let mut dialog = AsyncFileDialog::new().set_title(spec.title);
    if let Some((name, extensions)) = spec.filter {
        dialog = dialog.add_filter(name, extensions);
    }
    if let Some(file_name) = spec.file_name {
        dialog = dialog.set_file_name(file_name);
    }
    let answer = match spec.pick {
        Pick::File => one(dialog.pick_file()),
        Pick::Files => many(dialog.pick_files()),
        Pick::Folder => one(dialog.pick_folder()),
        Pick::Save => one(dialog.save_file()),
    };
    std::thread::spawn(move || {
        let paths = pollster::block_on(answer);
        let _ = proxy.send_event(UserEvent::DialogClosed(purpose, paths));
    });
}

impl App {
    /// Opens a file dialog; its answer arrives later, in `dialog_closed`.
    /// Only one dialog is open at a time.
    pub(super) fn open_dialog(&mut self, purpose: Purpose) {
        if self.dialog_open {
            return;
        }
        if purpose.local_only() && self.mode != PlayerMode::Local {
            return self.show_message(LOCAL_ONLY.to_string());
        }
        self.dialog_open = true;
        show(purpose, self.proxy.clone());
    }

    /// Acts on a dialog's answer: the chosen paths, or none if cancelled.
    pub(super) fn dialog_closed(&mut self, purpose: Purpose, paths: Vec<PathBuf>) {
        self.dialog_open = false;
        let Some(path) = paths.first().cloned() else { return };
        match purpose {
            Purpose::PlayFiles | Purpose::PlayFolder | Purpose::LoadPlaylist => self.add_tracks(paths, true),
            Purpose::AddFiles | Purpose::AddFolder => self.add_tracks(paths, false),
            Purpose::SavePlaylist => match playlist_file::save_m3u(&path, self.playlist.entries()) {
                Ok(()) => self.show_message(format!("saved playlist: {}", file_name(&path))),
                Err(err) => self.show_message(format!("{err:#}")),
            },
            Purpose::LoadSkin => match open_skin(&path) {
                Ok(skin) => self.set_skin(skin, Some(path)),
                Err(err) => self.show_message(format!("{}: {err:#}", file_name(&path))),
            },
            Purpose::LoadEqPreset => {
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
            Purpose::SaveEqPreset => {
                let preset = eqf::Preset { name: file_name(&path), preamp: self.eq.preamp, bands: self.eq.bands };
                match std::fs::write(&path, eqf::write(&[preset])) {
                    Ok(()) => self.show_message(format!("saved eq preset: {}", file_name(&path))),
                    Err(err) => self.show_message(format!("could not save preset: {err}")),
                }
            }
            Purpose::MilkDropPresetsFolder => {
                self.milkdrop_presets = Some(path);
                let note = if self.milkdrop_started() { " (used from next start)" } else { "" };
                self.show_message(format!("milkdrop presets folder set{note}"));
            }
        }
        self.pl.scroll_to(self.pl.scroll as isize, self.pl_rows());
    }
}

/// Shows a message box, without waiting for it to be closed.
pub fn message_box(title: &str, text: String) {
    let closed = rfd::AsyncMessageDialog::new().set_title(title).set_description(text).show();
    std::thread::spawn(move || pollster::block_on(closed));
}
