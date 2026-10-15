//! The list of tracks to play and which one is current.
//!
//! Phase 5 adds the playlist window; for now this just keeps the order for
//! previous/next, shuffle and repeat.

use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Playlist {
    tracks: Vec<PathBuf>,
    current: usize,
}

impl Playlist {
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn current(&self) -> Option<&Path> {
        self.tracks.get(self.current).map(PathBuf::as_path)
    }

    /// Position of the current track, counting from 1 as Winamp displays it.
    pub fn current_number(&self) -> usize {
        self.current + 1
    }

    pub fn replace(&mut self, tracks: Vec<PathBuf>) {
        self.tracks = tracks;
        self.current = 0;
    }

    pub fn append(&mut self, track: PathBuf) {
        self.tracks.push(track);
    }

    /// Moves to the next track. In shuffle mode that is a random other
    /// track. At the end of the list, wraps to the start if `wrap` is set,
    /// otherwise returns `None` and stays put.
    pub fn next(&mut self, shuffle: bool, wrap: bool) -> Option<&Path> {
        let len = self.tracks.len();
        if len == 0 {
            return None;
        }
        if shuffle && len > 1 {
            // Pick from the other tracks so the same song doesn't repeat.
            let pick = fastrand::usize(..len - 1);
            self.current = if pick >= self.current { pick + 1 } else { pick };
        } else if self.current + 1 < len {
            self.current += 1;
        } else if wrap {
            self.current = 0;
        } else {
            return None;
        }
        self.current()
    }

    /// Moves to the previous track, wrapping from the first to the last.
    pub fn previous(&mut self) -> Option<&Path> {
        let len = self.tracks.len();
        if len == 0 {
            return None;
        }
        self.current = (self.current + len - 1) % len;
        self.current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playlist(n: usize) -> Playlist {
        let mut list = Playlist::default();
        list.replace((0..n).map(|i| PathBuf::from(format!("{i}.mp3"))).collect());
        list
    }

    #[test]
    fn next_and_previous_in_order() {
        let mut list = playlist(3);
        assert_eq!(list.current(), Some(Path::new("0.mp3")));
        assert_eq!(list.next(false, false), Some(Path::new("1.mp3")));
        assert_eq!(list.next(false, false), Some(Path::new("2.mp3")));
        assert_eq!(list.next(false, false), None);
        assert_eq!(list.current(), Some(Path::new("2.mp3")));
        assert_eq!(list.next(false, true), Some(Path::new("0.mp3")));
        assert_eq!(list.previous(), Some(Path::new("2.mp3")));
        assert_eq!(list.current_number(), 3);
    }

    #[test]
    fn shuffle_never_repeats_the_current_track() {
        let mut list = playlist(3);
        for _ in 0..100 {
            let before = list.current_number();
            list.next(true, false);
            assert_ne!(list.current_number(), before);
        }
    }

    #[test]
    fn empty_playlist() {
        let mut list = Playlist::default();
        assert!(list.is_empty());
        assert_eq!(list.next(true, true), None);
        assert_eq!(list.previous(), None);
    }
}
