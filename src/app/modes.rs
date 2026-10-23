//! Player modes: playing files on this computer, or a music server's
//! library. Each mode has its own playlist, saved between runs; the ones
//! not in use wait in `App::parked`.

use super::App;
use crate::config::{self, PlayerMode};
use crate::playlist::Playlist;
use crate::playlist_file;

impl App {
    /// Loads every mode's saved playlist and makes `mode` the one in use.
    /// `local_current` is the local playlist's current track. Without
    /// `restore_local`, the local playlist starts empty (music given on the
    /// command line is about to replace it).
    pub(super) fn restore_playlists(&mut self, mode: PlayerMode, local_current: Option<usize>, restore_local: bool) {
        for each in PlayerMode::ALL {
            let playlist = if each == PlayerMode::Local && !restore_local {
                Playlist::default()
            } else {
                self.load_playlist(each, local_current)
            };
            if each == mode {
                self.playlist = playlist;
            } else {
                self.parked.insert(each, playlist);
            }
        }
        self.mode = mode;
        if let Some(index) = self.playlist.current_index() {
            self.pl.reveal(index, self.playlist.len());
        }
    }

    fn load_playlist(&self, mode: PlayerMode, local_current: Option<usize>) -> Playlist {
        let Some(path) = config::queue_path(mode).filter(|p| p.exists()) else { return Playlist::default() };
        let loaded = match mode {
            PlayerMode::Local => playlist_file::load(&path).map(|tracks| {
                let mut playlist = Playlist::default();
                self.metadata.request(playlist.replace(tracks));
                if let Some(index) = local_current.filter(|&i| i < playlist.len()) {
                    playlist.set_current(index);
                }
                playlist
            }),
            _ => playlist_file::load_queue(&path),
        };
        loaded.unwrap_or_else(|err| {
            eprintln!("could not restore the {} playlist: {err:#}", mode.name());
            Playlist::default()
        })
    }

    /// The playlist of `mode`, whether in use or parked.
    pub(super) fn playlist_of(&self, mode: PlayerMode) -> Option<&Playlist> {
        if mode == self.mode { Some(&self.playlist) } else { self.parked.get(&mode) }
    }

    /// Saves every mode's playlist. A music server's playlist file is only
    /// written once there's something in it.
    pub(super) fn save_playlists(&self) {
        for mode in PlayerMode::ALL {
            let (Some(playlist), Some(path)) = (self.playlist_of(mode), config::queue_path(mode)) else { continue };
            let saved = match mode {
                PlayerMode::Local => playlist_file::save_m3u(&path, playlist.entries()),
                _ if playlist.is_empty() && !path.exists() => Ok(()),
                _ => playlist_file::save_queue(&path, playlist),
            };
            if let Err(err) = saved {
                eprintln!("could not save the {} playlist: {err:#}", mode.name());
            }
        }
    }

    /// Fills in a track's details, in whichever playlist it's in.
    pub(super) fn set_details(&mut self, id: u64, title: String, duration: Option<f64>) {
        if !self.playlist.set_details(id, title.clone(), duration) {
            for playlist in self.parked.values_mut() {
                if playlist.set_details(id, title.clone(), duration) {
                    break;
                }
            }
        }
    }

    /// Switches to another mode's playlist. What was playing stops.
    pub(super) fn set_player_mode(&mut self, mode: PlayerMode) {
        if mode == self.mode {
            return;
        }
        self.player.clear();
        let playlist = self.parked.remove(&mode).unwrap_or_default();
        let previous = std::mem::replace(&mut self.playlist, playlist);
        self.parked.insert(self.mode, previous);
        self.mode = mode;
        self.pl.scroll = 0;
        if let Some(index) = self.playlist.current_index() {
            self.pl.reveal(index, self.playlist.len());
        }
        self.show_message(format!("player mode: {}", mode.name()));
    }
}
