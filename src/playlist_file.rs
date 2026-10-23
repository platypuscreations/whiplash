//! Playlist files (M3U and PLS), finding music in folders, and the saved
//! playlists of the music server modes.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::audio::EXTENSIONS;
use crate::location::Location;
use crate::playlist::{Entry, Playlist};

pub const PLAYLIST_EXTENSIONS: &[&str] = &["m3u", "m3u8", "pls"];

fn extension(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

pub fn is_playlist_file(path: &Path) -> bool {
    PLAYLIST_EXTENSIONS.contains(&extension(path).as_str())
}

pub fn is_audio_file(path: &Path) -> bool {
    EXTENSIONS.contains(&extension(path).as_str())
}

/// Turns dropped or chosen paths into a list of tracks: folders are
/// searched for music, playlist files are read, and other files that
/// aren't music are skipped.
pub fn expand(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut tracks = Vec::new();
    for path in paths {
        if path.is_dir() {
            tracks.extend(scan_folder(&path));
        } else if is_playlist_file(&path) {
            match load(&path) {
                Ok(list) => tracks.extend(list),
                Err(err) => eprintln!("{}: {err:#}", path.display()),
            }
        } else if is_audio_file(&path) {
            tracks.push(path);
        }
    }
    tracks
}

/// Every music file in a folder and its subfolders, sorted by path.
pub fn scan_folder(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut folders = vec![dir.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            // Skip hidden files and folders, like macOS's "._" files.
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if path.is_dir() {
                folders.push(path);
            } else if is_audio_file(&path) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Reads an M3U or PLS playlist file.
pub fn load(path: &Path) -> Result<Vec<PathBuf>> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    // Old playlists are often not UTF-8; this keeps the readable parts.
    let text = String::from_utf8_lossy(&bytes);
    let base = path.parent().unwrap_or(Path::new("."));
    Ok(if extension(path) == "pls" { parse_pls(&text, base) } else { parse_m3u(&text, base) })
}

/// M3U: one path per line; lines starting with "#" are comments or extra
/// info. Relative paths are relative to the playlist file's folder.
pub fn parse_m3u(text: &str, base: &Path) -> Vec<PathBuf> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| resolve(line, base))
        .collect()
}

/// PLS: an INI file with lines like "File1=path", in numbered order.
pub fn parse_pls(text: &str, base: &Path) -> Vec<PathBuf> {
    let mut numbered: Vec<(u32, PathBuf)> = text
        .lines()
        .filter_map(|line| {
            let (key, value) = line.trim().split_once('=')?;
            let number = key.trim().to_ascii_lowercase().strip_prefix("file")?.parse().ok()?;
            Some((number, resolve(value.trim(), base)?))
        })
        .collect();
    numbered.sort_by_key(|(n, _)| *n);
    numbered.into_iter().map(|(_, path)| path).collect()
}

/// Makes a playlist line into a path. Web addresses aren't supported yet.
fn resolve(line: &str, base: &Path) -> Option<PathBuf> {
    let line = match line.strip_prefix("file://") {
        // `file:///C:/x.mp3` is `C:/x.mp3`.
        Some(rest) if matches!(rest.as_bytes(), [b'/', drive, b':', ..] if drive.is_ascii_alphabetic()) => &rest[1..],
        Some(rest) => rest,
        None if line.contains("://") => return None,
        None => line,
    };
    // Playlists made on Windows use backslashes.
    let line = if cfg!(windows) { line.to_string() } else { line.replace('\\', "/") };
    let path = PathBuf::from(line);
    Some(if path.is_absolute() { path } else { base.join(path) })
}

/// Writes an extended M3U playlist. Tracks inside the playlist's folder are
/// saved with relative paths, so the folder can be moved as a whole. Only
/// files are saved.
pub fn save_m3u(path: &Path, entries: &[Entry]) -> Result<()> {
    let base = path.parent().unwrap_or(Path::new("."));
    let mut text = String::from("#EXTM3U\n");
    for entry in entries {
        let Some(file) = entry.location.file() else { continue };
        let seconds = entry.duration.map_or(-1, |d| d.round() as i64);
        text.push_str(&format!("#EXTINF:{seconds},{}\n", entry.title));
        let track = file.strip_prefix(base).unwrap_or(file);
        text.push_str(&format!("{}\n", track.display()));
    }
    fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// A music server mode's playlist, saved between runs. Unlike M3U it
/// keeps the server's own ids for the tracks.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct SavedQueue {
    /// Which track was current (counting from 0).
    current: Option<usize>,
    tracks: Vec<SavedTrack>,
}

#[derive(Serialize, Deserialize)]
struct SavedTrack {
    location: Location,
    title: String,
    duration: Option<f64>,
}

pub fn save_queue(path: &Path, playlist: &Playlist) -> Result<()> {
    let queue = SavedQueue {
        current: playlist.current_index(),
        tracks: playlist
            .entries()
            .iter()
            .map(|e| SavedTrack { location: e.location.clone(), title: e.title.clone(), duration: e.duration })
            .collect(),
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, toml::to_string(&queue)?).with_context(|| format!("writing {}", path.display()))
}

pub fn load_queue(path: &Path) -> Result<Playlist> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let queue: SavedQueue = toml::from_str(&text).with_context(|| format!("reading {}", path.display()))?;
    let mut playlist = Playlist::default();
    playlist.append_known(queue.tracks.into_iter().map(|t| (t.location, t.title, t.duration)).collect());
    if let Some(index) = queue.current.filter(|&i| i < playlist.len()) {
        playlist.set_current(index);
    }
    Ok(playlist)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An absolute path for these tests: on Windows, one on drive C.
    const ROOT: &str = if cfg!(windows) { "C:" } else { "" };

    fn abs(path: &str) -> PathBuf {
        PathBuf::from(format!("{ROOT}{path}"))
    }

    #[test]
    fn m3u_with_comments_relative_paths_and_urls() {
        let url_root = if cfg!(windows) { "C:/" } else { "" };
        let text = format!("#EXTM3U\n#EXTINF:123,Artist - Song\nmusic/song.mp3\n\n{ROOT}/abs/other.flac\nhttp://radio.example/stream\nfile:///{url_root}abs/third.ogg\nsub\\win.mp3\n");
        let paths = parse_m3u(&text, &abs("/lists"));
        assert_eq!(
            paths,
            [abs("/lists/music/song.mp3"), abs("/abs/other.flac"), abs("/abs/third.ogg"), abs("/lists/sub/win.mp3")]
        );
    }

    #[test]
    #[cfg(windows)]
    fn file_urls_with_drive_letters() {
        assert_eq!(resolve("file:///C:/x.mp3", Path::new("/lists")), Some(PathBuf::from("C:/x.mp3")));
    }

    #[test]
    fn pls_in_numbered_order() {
        let text = "[playlist]\nNumberOfEntries=2\nFile2=b.mp3\nTitle2=B\nfile1=/x/a.mp3\nVersion=2\n";
        assert_eq!(parse_pls(text, Path::new("/lists")), [PathBuf::from("/x/a.mp3"), PathBuf::from("/lists/b.mp3")]);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("whiplash-test-m3u-{}", std::process::id()));
        fs::create_dir_all(dir.join("music")).unwrap();
        let entries = vec![
            Entry { id: 1, location: dir.join("music/a.mp3").into(), title: "A".into(), duration: Some(61.4), selected: false },
            Entry { id: 2, location: abs("/elsewhere/b.flac").into(), title: "B".into(), duration: None, selected: false },
        ];
        let list = dir.join("list.m3u");
        save_m3u(&list, &entries).unwrap();
        let text = fs::read_to_string(&list).unwrap();
        assert!(text.contains("#EXTINF:61,A\nmusic/a.mp3\n"), "{text}");
        assert!(text.contains(&format!("#EXTINF:-1,B\n{ROOT}/elsewhere/b.flac\n")), "{text}");
        assert_eq!(load(&list).unwrap(), [dir.join("music/a.mp3"), abs("/elsewhere/b.flac")]);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn queues_round_trip() {
        let path = std::env::temp_dir().join(format!("whiplash-test-queue-{}.toml", std::process::id()));
        let mut playlist = Playlist::default();
        playlist.append_known(vec![
            (abs("/a.mp3").into(), "A".into(), Some(61.5)),
            (abs("/b.mp3").into(), "B".into(), None),
        ]);
        playlist.set_current(1);
        save_queue(&path, &playlist).unwrap();
        let loaded = load_queue(&path).unwrap();
        let summary = |p: &Playlist| -> Vec<_> { p.entries().iter().map(|e| (e.location.clone(), e.title.clone(), e.duration)).collect() };
        assert_eq!(summary(&loaded), summary(&playlist));
        assert_eq!(loaded.current_index(), Some(1));
        fs::remove_file(path).ok();
    }

    #[test]
    fn folders_are_searched_for_music() {
        let dir = std::env::temp_dir().join(format!("whiplash-test-scan-{}", std::process::id()));
        fs::create_dir_all(dir.join("b")).unwrap();
        for file in ["a.mp3", "b/c.FLAC", "notes.txt", "._hidden.mp3"] {
            fs::write(dir.join(file), b"").unwrap();
        }
        assert_eq!(scan_folder(&dir), [dir.join("a.mp3"), dir.join("b/c.FLAC")]);
        fs::remove_dir_all(dir).ok();
    }
}
