//! Settings saved between runs.
//!
//! They live in a small text file, `settings.toml`, in the system's usual
//! place for app settings (`~/Library/Application Support/whiplash` on
//! macOS, `~/.config/whiplash` on Linux). The playlist is saved next to it
//! as `playlist.m3u`, and skins dropped into the `Skins` folder there show
//! up in the skin browser.
//!
//! `serde` writes and reads the struct automatically. `#[serde(default)]`
//! means a missing or unknown setting just falls back to its default, so
//! older settings files keep working as settings are added.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::vis::Mode;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The skin in use; none means Whiplash's built-in skin.
    pub skin: Option<PathBuf>,
    pub volume: f32,
    pub balance: f32,
    pub shuffle: bool,
    pub repeat: bool,
    pub time_remaining: bool,
    pub double_size: bool,
    pub always_on_top: bool,
    pub visualizer: Mode,
    /// Where MilkDrop presets are; none means the `Presets` folder here.
    pub milkdrop_presets: Option<PathBuf>,
    pub milkdrop_open: bool,
    /// Where the music comes from.
    pub player_mode: PlayerMode,
    /// Which track in the saved playlist was current (counting from 0).
    pub playlist_current: Option<usize>,
    pub equalizer: EqualizerConfig,
    pub windows: WindowsConfig,
}

/// Where the music comes from: files on this computer, or a music server's
/// library. Each mode has its own playlist.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayerMode {
    #[default]
    Local,
    Emby,
    /// Logitech Media Server, which Squeezebox players play from.
    Squeeze,
}

impl PlayerMode {
    pub const ALL: [PlayerMode; 3] = [PlayerMode::Local, PlayerMode::Emby, PlayerMode::Squeeze];

    pub fn name(self) -> &'static str {
        match self {
            PlayerMode::Local => "local files",
            PlayerMode::Emby => "emby",
            PlayerMode::Squeeze => "squeeze",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EqualizerConfig {
    pub enabled: bool,
    pub auto: bool,
    /// Slider positions from 0 (-12 dB) to 100 (+12 dB).
    pub preamp: f32,
    pub bands: [f32; 10],
    /// Per-song presets for "auto": file name → preamp then ten bands.
    pub auto_presets: HashMap<String, [f32; 11]>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowsConfig {
    /// Screen positions (in physical pixels) of each window's top-left.
    pub main: Option<[i32; 2]>,
    pub equalizer: Option<[i32; 2]>,
    pub playlist: Option<[i32; 2]>,
    pub equalizer_open: bool,
    pub playlist_open: bool,
    pub playlist_size: Option<[u32; 2]>,
    pub main_shaded: bool,
    pub equalizer_shaded: bool,
    pub playlist_shaded: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            skin: None,
            volume: 0.75,
            balance: 0.0,
            shuffle: false,
            repeat: false,
            time_remaining: false,
            double_size: false,
            always_on_top: false,
            visualizer: Mode::Analyzer,
            milkdrop_presets: None,
            milkdrop_open: false,
            player_mode: PlayerMode::Local,
            playlist_current: None,
            equalizer: EqualizerConfig::default(),
            windows: WindowsConfig { playlist_open: true, ..WindowsConfig::default() },
        }
    }
}

impl Default for EqualizerConfig {
    fn default() -> Self {
        EqualizerConfig { enabled: true, auto: false, preamp: 50.0, bands: [50.0; 10], auto_presets: HashMap::new() }
    }
}

/// The folder that holds the settings, playlist and skins. Setting
/// `WHIPLASH_CONFIG_DIR` uses another folder (handy for testing).
pub fn folder() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("WHIPLASH_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("", "", "whiplash").map(|dirs| dirs.config_dir().to_path_buf())
}

pub fn playlist_path() -> Option<PathBuf> {
    folder().map(|f| f.join("playlist.m3u"))
}

/// Where a music server mode's playlist is kept. (Local files' playlist
/// is `playlist_path`.)
pub fn queue_path(mode: PlayerMode) -> Option<PathBuf> {
    let name = match mode {
        PlayerMode::Local => return playlist_path(),
        PlayerMode::Emby => "queue-emby.toml",
        PlayerMode::Squeeze => "queue-squeeze.toml",
    };
    folder().map(|f| f.join(name))
}

/// The default place for MilkDrop presets. Created on first use.
#[cfg_attr(not(feature = "milkdrop"), allow(dead_code))]
pub fn presets_folder() -> Option<PathBuf> {
    let presets = folder()?.join("Presets");
    fs::create_dir_all(&presets).ok()?;
    Some(presets)
}

/// Where users can put skins for the skin browser. Created on first use.
pub fn skins_folder() -> Option<PathBuf> {
    let skins = folder()?.join("Skins");
    fs::create_dir_all(&skins).ok()?;
    Some(skins)
}

/// Moves a skin into `folder` so it keeps working wherever the original
/// was, and returns its new place. A skin already in the folder stays put;
/// one with the same name as a different skin there gets a numbered name,
/// and a copy of a skin already there is just removed.
pub fn move_skin_into(path: &Path, folder: &Path) -> Result<PathBuf> {
    let name = path.file_name().context("not a file")?;
    if path.parent().and_then(|p| p.canonicalize().ok()) == folder.canonicalize().ok() {
        return Ok(path.to_path_buf());
    }
    let mut dest = folder.join(name);
    let stem = path.file_stem().unwrap_or(name).to_string_lossy().into_owned();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    for n in 2.. {
        if !dest.exists() {
            break;
        }
        if path.is_file() && dest.is_file() && fs::read(path)? == fs::read(&dest)? {
            fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
            return Ok(dest);
        }
        dest = folder.join(format!("{stem} ({n}){ext}"));
    }
    // Renaming fails across disks, so fall back to copying and deleting.
    if fs::rename(path, &dest).is_err() {
        copy_all(path, &dest).with_context(|| format!("copying {} to {}", path.display(), folder.display()))?;
        let removed = if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) };
        removed.with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(dest)
}

/// Copies a file, or a folder and everything in it.
fn copy_all(from: &Path, to: &Path) -> std::io::Result<()> {
    if !from.is_dir() {
        return fs::copy(from, to).map(|_| ());
    }
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        copy_all(&entry.path(), &to.join(entry.file_name()))?;
    }
    Ok(())
}

impl Config {
    /// Loads the settings, or the defaults if there are none yet. A damaged
    /// file is reported and replaced by defaults rather than stopping the app.
    pub fn load() -> Config {
        let Some(path) = folder().map(|f| f.join("settings.toml")) else { return Config::default() };
        match Config::load_from(&path) {
            Ok(Some(config)) => config,
            Ok(None) => Config::default(),
            Err(err) => {
                eprintln!("ignoring settings: {err:#}");
                Config::default()
            }
        }
    }

    fn load_from(path: &Path) -> Result<Option<Config>> {
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Ok(Some(toml::from_str(&text).with_context(|| format!("reading {}", path.display()))?))
    }

    pub fn save(&self) -> Result<()> {
        let folder = folder().context("no settings folder on this system")?;
        self.save_to(&folder.join("settings.toml"))
    }

    fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml::to_string_pretty(self)?).with_context(|| format!("writing {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let path = std::env::temp_dir().join(format!("whiplash-test-settings-{}.toml", std::process::id()));
        let mut config = Config { volume: 0.3, shuffle: true, visualizer: Mode::Oscilloscope, player_mode: PlayerMode::Squeeze, ..Config::default() };
        config.windows.main = Some([100, -20]);
        config.equalizer.bands[3] = 80.0;
        config.equalizer.auto_presets.insert("song.mp3".into(), [50.0; 11]);
        config.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path).unwrap(), Some(config));
        fs::remove_file(path).ok();
    }

    #[test]
    fn skins_move_into_the_skins_folder() {
        let dir = std::env::temp_dir().join(format!("whiplash-test-skins-{}", std::process::id()));
        let (downloads, skins) = (dir.join("Downloads"), dir.join("Skins"));
        fs::create_dir_all(&downloads).unwrap();
        fs::create_dir_all(&skins).unwrap();
        let dropped = downloads.join("base.wsz");

        fs::write(&dropped, "one").unwrap();
        let first = move_skin_into(&dropped, &skins).unwrap();
        assert_eq!(first, skins.join("base.wsz"));
        assert!(!dropped.exists());
        assert_eq!(move_skin_into(&first, &skins).unwrap(), first); // already there

        fs::write(&dropped, "one").unwrap(); // the same skin again
        assert_eq!(move_skin_into(&dropped, &skins).unwrap(), first);
        assert!(!dropped.exists());

        fs::write(&dropped, "two").unwrap(); // a different skin, same name
        assert_eq!(move_skin_into(&dropped, &skins).unwrap(), skins.join("base (2).wsz"));
        assert_eq!(fs::read_to_string(&first).unwrap(), "one");

        let folder = downloads.join("unpacked");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("main.bmp"), "bmp").unwrap();
        assert_eq!(move_skin_into(&folder, &skins).unwrap(), skins.join("unpacked"));
        assert!(skins.join("unpacked/main.bmp").exists());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn missing_and_unknown_settings_are_fine() {
        let config: Config = toml::from_str("volume = 0.5\nsome_future_setting = true\n[windows]\nplaylist_open = false\n").unwrap();
        assert_eq!(config.volume, 0.5);
        assert_eq!(config.player_mode, PlayerMode::Local);
        assert!(!config.windows.playlist_open);
        assert_eq!(config.equalizer, EqualizerConfig::default());
    }
}
