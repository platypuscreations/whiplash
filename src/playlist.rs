//! The list of tracks, which one is playing, and which are selected.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::location::Location;

/// Entry ids are never reused, even between playlists, so details read in
/// the background always find their own entry.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// One track in the playlist.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Stays the same when the list is sorted or edited, so the playing
    /// track can be found again.
    pub id: u64,
    pub location: Location,
    /// The file name until the file's tags have been read.
    pub title: String,
    /// Length in seconds, once known.
    pub duration: Option<f64>,
    pub selected: bool,
}

/// How a click changes the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Select {
    /// Plain click: select just this track.
    Only,
    /// Ctrl/Cmd-click: add or remove this track.
    Toggle,
    /// Shift-click: select everything from the last click to here.
    Range,
}

#[derive(Default)]
pub struct Playlist {
    entries: Vec<Entry>,
    /// The id of the track that's playing (or will play).
    current: Option<u64>,
    /// Where the last plain or Ctrl click was, for Shift-click ranges.
    anchor: Option<usize>,
}

impl Playlist {
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn current_index(&self) -> Option<usize> {
        let id = self.current?;
        self.entries.iter().position(|e| e.id == id)
    }

    pub fn current(&self) -> Option<&Location> {
        self.current_index().map(|i| &self.entries[i].location)
    }

    /// Position of the current track, counting from 1 as Winamp displays it.
    pub fn current_number(&self) -> usize {
        self.current_index().map_or(0, |i| i + 1)
    }

    pub fn set_current(&mut self, index: usize) {
        self.current = self.entries.get(index).map(|e| e.id);
    }

    /// Replaces the whole list. Returns the new entries' ids and locations,
    /// so their details can be looked up.
    pub fn replace(&mut self, tracks: Vec<impl Into<Location>>) -> Vec<(u64, Location)> {
        self.entries.clear();
        self.current = None;
        self.anchor = None;
        self.append(tracks)
    }

    /// Adds tracks at the end. Returns the new entries' ids and locations.
    pub fn append(&mut self, tracks: Vec<impl Into<Location>>) -> Vec<(u64, Location)> {
        let added: Vec<(u64, Location)> = tracks
            .into_iter()
            .map(|track| (NEXT_ID.fetch_add(1, Ordering::Relaxed), track.into()))
            .collect();
        for (id, location) in &added {
            let title = location.fallback_title();
            self.entries.push(Entry { id: *id, location: location.clone(), title, duration: None, selected: false });
        }
        if self.current_index().is_none() && !self.entries.is_empty() {
            self.current = Some(self.entries[0].id);
        }
        added
    }

    /// Adds tracks whose details are already known at the end.
    pub fn append_known(&mut self, tracks: Vec<(Location, String, Option<f64>)>) {
        let (locations, details): (Vec<Location>, Vec<_>) = tracks.into_iter().map(|(l, t, d)| (l, (t, d))).unzip();
        for ((id, _), (title, duration)) in self.append(locations).into_iter().zip(details) {
            self.set_details(id, title, duration);
        }
    }

    /// Fills in a track's title and length once its tags have been read.
    /// Returns false if no track here has that id.
    pub fn set_details(&mut self, id: u64, title: String, duration: Option<f64>) -> bool {
        let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) else { return false };
        entry.title = title;
        entry.duration = duration;
        true
    }

    /// Moves to the next track. In shuffle mode that is a random other
    /// track. At the end of the list, wraps to the start if `wrap` is set,
    /// otherwise returns `None` and stays put.
    pub fn next(&mut self, shuffle: bool, wrap: bool) -> Option<&Location> {
        let len = self.entries.len();
        if len == 0 {
            return None;
        }
        let index = match self.current_index() {
            None => 0,
            // Pick from the other tracks so the same song doesn't repeat.
            Some(current) if shuffle && len > 1 => {
                let pick = fastrand::usize(..len - 1);
                if pick >= current { pick + 1 } else { pick }
            }
            Some(current) if current + 1 < len => current + 1,
            Some(_) if wrap => 0,
            Some(_) => return None,
        };
        self.set_current(index);
        self.current()
    }

    /// Moves to the previous track, wrapping from the first to the last.
    pub fn previous(&mut self) -> Option<&Location> {
        let len = self.entries.len();
        if len == 0 {
            return None;
        }
        let index = self.current_index().map_or(0, |i| (i + len - 1) % len);
        self.set_current(index);
        self.current()
    }

    // ----- Selection -----

    pub fn click(&mut self, index: usize, mode: Select) {
        if index >= self.entries.len() {
            return;
        }
        match (mode, self.anchor) {
            (Select::Range, Some(anchor)) => {
                let (low, high) = (anchor.min(index), anchor.max(index));
                for (i, entry) in self.entries.iter_mut().enumerate() {
                    entry.selected = (low..=high).contains(&i);
                }
                return; // the anchor stays where the range started
            }
            (Select::Toggle, _) => self.entries[index].selected = !self.entries[index].selected,
            _ => {
                for (i, entry) in self.entries.iter_mut().enumerate() {
                    entry.selected = i == index;
                }
            }
        }
        self.anchor = Some(index);
    }

    pub fn select_all(&mut self, selected: bool) {
        self.entries.iter_mut().for_each(|e| e.selected = selected);
    }

    pub fn invert_selection(&mut self) {
        self.entries.iter_mut().for_each(|e| e.selected = !e.selected);
    }

    /// The first selected track, if any.
    pub fn first_selected(&self) -> Option<usize> {
        self.entries.iter().position(|e| e.selected)
    }

    // ----- Editing -----

    /// Removes tracks for which `remove` returns true. The playing track
    /// keeps playing; if it was removed, the list carries on from the track
    /// that took its place.
    fn remove_where(&mut self, remove: impl Fn(&Entry) -> bool) {
        let current = self.current_index();
        let removed_before_current =
            current.map_or(0, |c| self.entries[..c].iter().filter(|e| remove(e)).count());
        let current_removed = current.is_some_and(|c| remove(&self.entries[c]));
        self.entries.retain(|e| !remove(e));
        self.anchor = None;
        if current_removed {
            let index = current.unwrap() - removed_before_current;
            self.current = self.entries.get(index).or(self.entries.last()).map(|e| e.id);
        }
    }

    pub fn remove_selected(&mut self) {
        self.remove_where(|e| e.selected);
    }

    /// Keeps only the selected tracks.
    pub fn crop(&mut self) {
        self.remove_where(|e| !e.selected);
    }

    pub fn clear(&mut self) {
        self.remove_where(|_| true);
    }

    /// Removes tracks whose files no longer exist; returns how many.
    pub fn remove_missing(&mut self) -> usize {
        let before = self.entries.len();
        self.remove_where(|e| !e.location.exists());
        before - self.entries.len()
    }

    pub fn sort_by_title(&mut self) {
        self.entries.sort_by_cached_key(|e| e.title.to_lowercase());
    }

    pub fn sort_by_file_name(&mut self) {
        self.entries.sort_by_cached_key(|e| e.location.name_sort_key());
    }

    pub fn sort_by_path(&mut self) {
        self.entries.sort_by_cached_key(|e| e.location.path_sort_key());
    }

    pub fn reverse(&mut self) {
        self.entries.reverse();
    }

    pub fn randomize(&mut self) {
        fastrand::shuffle(&mut self.entries);
    }

    /// Moves the selected tracks up (negative) or down by `delta` rows,
    /// keeping them in order. They stop at the ends of the list.
    pub fn move_selected(&mut self, delta: isize) {
        let selected: Vec<usize> = (0..self.entries.len()).filter(|&i| self.entries[i].selected).collect();
        let (Some(&first), Some(&last)) = (selected.first(), selected.last()) else { return };
        let delta = delta.clamp(-(first as isize), (self.entries.len() - 1 - last) as isize);
        if delta == 0 {
            return;
        }
        let moving: Vec<Entry> = selected.iter().map(|&i| self.entries[i].clone()).collect();
        let mut rest: Vec<Entry> = self.entries.drain(..).filter(|e| !e.selected).collect();
        // Put the block back at its new position, measured from the first
        // selected track.
        let insert_at = ((first as isize + delta) as usize).min(rest.len());
        rest.splice(insert_at..insert_at, moving);
        self.entries = rest;
        self.anchor = None;
    }

    /// Total length of the selected tracks and of all tracks, in seconds.
    /// The flag is true if any length is still unknown.
    pub fn durations(&self) -> ((f64, bool), (f64, bool)) {
        let total = |entries: &mut dyn Iterator<Item = &Entry>| {
            entries.fold((0.0, false), |(sum, unknown), e| match e.duration {
                Some(d) => (sum + d, unknown),
                None => (sum, true),
            })
        };
        let selected = total(&mut self.entries.iter().filter(|e| e.selected));
        (selected, total(&mut self.entries.iter()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn file(name: &str) -> Location {
        Location::File(PathBuf::from(name))
    }

    fn playlist(n: usize) -> Playlist {
        let mut list = Playlist::default();
        list.replace((0..n).map(|i| PathBuf::from(format!("{i}.mp3"))).collect());
        list
    }

    fn names(list: &Playlist) -> Vec<String> {
        list.entries().iter().map(|e| e.title.clone()).collect()
    }

    fn selected(list: &Playlist) -> Vec<usize> {
        (0..list.len()).filter(|&i| list.entries()[i].selected).collect()
    }

    #[test]
    fn next_and_previous_in_order() {
        let mut list = playlist(3);
        assert_eq!(list.current(), Some(&file("0.mp3")));
        assert_eq!(list.next(false, false), Some(&file("1.mp3")));
        assert_eq!(list.next(false, false), Some(&file("2.mp3")));
        assert_eq!(list.next(false, false), None);
        assert_eq!(list.current(), Some(&file("2.mp3")));
        assert_eq!(list.next(false, true), Some(&file("0.mp3")));
        assert_eq!(list.previous(), Some(&file("2.mp3")));
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
        assert_eq!(list.current_number(), 0);
    }

    #[test]
    fn clicking_selects() {
        let mut list = playlist(6);
        list.click(1, Select::Only);
        list.click(3, Select::Toggle);
        assert_eq!(selected(&list), [1, 3]);
        list.click(5, Select::Range);
        assert_eq!(selected(&list), [3, 4, 5]);
        list.click(2, Select::Only);
        assert_eq!(selected(&list), [2]);
        list.invert_selection();
        assert_eq!(selected(&list), [0, 1, 3, 4, 5]);
    }

    #[test]
    fn the_playing_track_survives_sorting_and_removal() {
        let mut list = playlist(5);
        list.set_current(2);
        list.reverse();
        assert_eq!(list.current(), Some(&file("2.mp3")));
        assert_eq!(list.current_number(), 3);

        list.click(0, Select::Only); // "4"
        list.remove_selected();
        assert_eq!(names(&list), ["3", "2", "1", "0"]);
        assert_eq!(list.current(), Some(&file("2.mp3")));

        // Removing the playing track moves "current" to the one after it.
        list.click(1, Select::Only);
        list.remove_selected();
        assert_eq!(list.current(), Some(&file("1.mp3")));
    }

    #[test]
    fn crop_and_clear() {
        let mut list = playlist(4);
        list.click(1, Select::Only);
        list.click(2, Select::Toggle);
        list.crop();
        assert_eq!(names(&list), ["1", "2"]);
        list.clear();
        assert!(list.is_empty());
        assert_eq!(list.current(), None);
    }

    #[test]
    fn moving_a_selected_block() {
        let mut list = playlist(5);
        list.click(1, Select::Only);
        list.click(2, Select::Toggle);
        list.move_selected(2);
        assert_eq!(names(&list), ["0", "3", "4", "1", "2"]);
        list.move_selected(-10); // stops at the top
        assert_eq!(names(&list), ["1", "2", "0", "3", "4"]);
        assert_eq!(selected(&list), [0, 1]);
    }

    #[test]
    fn durations_add_up() {
        let mut list = playlist(3);
        list.set_details(list.entries()[0].id, "a".into(), Some(60.0));
        list.set_details(list.entries()[1].id, "b".into(), Some(30.0));
        list.click(0, Select::Only);
        let ((selected, selected_unknown), (total, total_unknown)) = list.durations();
        assert_eq!((selected, selected_unknown), (60.0, false));
        assert_eq!((total, total_unknown), (90.0, true));
    }

    #[test]
    fn sorting() {
        let mut list = Playlist::default();
        list.replace(vec![PathBuf::from("/b/z.mp3"), PathBuf::from("/a/y.mp3"), PathBuf::from("/c/x.mp3")]);
        let ids: Vec<u64> = list.entries().iter().map(|e| e.id).collect();
        list.set_details(ids[0], "Alpha".into(), None);
        list.set_details(ids[1], "charlie".into(), None);
        list.set_details(ids[2], "Bravo".into(), None);
        list.sort_by_title();
        assert_eq!(names(&list), ["Alpha", "Bravo", "charlie"]);
        list.sort_by_file_name();
        assert_eq!(names(&list), ["Bravo", "charlie", "Alpha"]);
        list.sort_by_path();
        assert_eq!(names(&list), ["charlie", "Alpha", "Bravo"]);
    }
}
