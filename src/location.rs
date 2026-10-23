//! Where a track comes from: a file on this computer, or a track in a
//! music server's library.
//!
//! Only what identifies the track is kept here, never a web address with
//! a password or token in it, so locations can be saved and shown safely.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Location {
    File(PathBuf),
    /// A track in an Emby server's library, by its id there.
    Emby(String),
    /// A track in a Logitech Media Server's library, by its id there.
    Squeeze(String),
}

impl Location {
    /// The file, for a track on this computer.
    pub fn file(&self) -> Option<&Path> {
        match self {
            Location::File(path) => Some(path),
            _ => None,
        }
    }

    /// A name to show until the track's tags have been read: the file name
    /// without its extension, or for a server's track, its id.
    pub fn fallback_title(&self) -> String {
        match self {
            Location::File(path) => path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            Location::Emby(_) | Location::Squeeze(_) => self.to_string(),
        }
    }

    /// The name equalizer auto-load presets are filed under: the file
    /// name, or the server's id for the track.
    pub fn preset_key(&self) -> Option<String> {
        match self {
            Location::File(path) => Some(path.file_name()?.to_string_lossy().into_owned()),
            Location::Emby(id) => Some(format!("emby:{id}")),
            Location::Squeeze(id) => Some(format!("squeeze:{id}")),
        }
    }

    /// For "sort by filename".
    pub fn name_sort_key(&self) -> Option<String> {
        match self {
            Location::File(path) => path.file_name().map(|n| n.to_string_lossy().to_lowercase()),
            Location::Emby(_) | Location::Squeeze(_) => None,
        }
    }

    /// For "sort by path and filename".
    pub fn path_sort_key(&self) -> String {
        match self {
            Location::File(path) => path.to_string_lossy().to_lowercase(),
            Location::Emby(id) | Location::Squeeze(id) => id.clone(),
        }
    }

    /// False for a file that no longer exists.
    pub fn exists(&self) -> bool {
        match self {
            Location::File(path) => path.exists(),
            Location::Emby(_) | Location::Squeeze(_) => true,
        }
    }
}

impl From<PathBuf> for Location {
    fn from(path: PathBuf) -> Location {
        Location::File(path)
    }
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Location::File(path) => write!(f, "{}", path.display()),
            Location::Emby(id) => write!(f, "Emby track {id}"),
            Location::Squeeze(id) => write!(f, "Logitech Media Server track {id}"),
        }
    }
}
