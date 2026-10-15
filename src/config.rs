//! Settings saved between runs.
//!
//! They live in a small text file, `settings.toml`, in the system's usual
//! place for app settings (`~/Library/Application Support/skinamp` on
//! macOS, `~/.config/skinamp` on Linux). The playlist is saved next to it
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
    /// The skin in use; none means the built-in placeholder look.
    pub skin: Option<PathBuf>,
    pub volume: f32,
    pub balance: f32,
    pub shuffle: bool,
    pub repeat: bool,
    pub time_remaining: bool,
    pub double_size: bool,
    pub always_on_top: bool,
    pub visualizer: Mode,
    /// Which track in the saved playlist was current (counting from 0).
    pub playlist_current: Option<usize>,
    pub equalizer: EqualizerConfig,
    pub windows: WindowsConfig,
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
/// `SKINAMP_CONFIG_DIR` uses another folder (handy for testing).
pub fn folder() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("SKINAMP_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("", "", "skinamp").map(|dirs| dirs.config_dir().to_path_buf())
}

pub fn playlist_path() -> Option<PathBuf> {
    folder().map(|f| f.join("playlist.m3u"))
}

/// Where users can put skins for the skin browser. Created on first use.
pub fn skins_folder() -> Option<PathBuf> {
    let skins = folder()?.join("Skins");
    fs::create_dir_all(&skins).ok()?;
    Some(skins)
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
        let path = std::env::temp_dir().join(format!("skinamp-test-settings-{}.toml", std::process::id()));
        let mut config = Config { volume: 0.3, shuffle: true, visualizer: Mode::Oscilloscope, ..Config::default() };
        config.windows.main = Some([100, -20]);
        config.equalizer.bands[3] = 80.0;
        config.equalizer.auto_presets.insert("song.mp3".into(), [50.0; 11]);
        config.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path).unwrap(), Some(config));
        fs::remove_file(path).ok();
    }

    #[test]
    fn missing_and_unknown_settings_are_fine() {
        let config: Config = toml::from_str("volume = 0.5\nsome_future_setting = true\n[windows]\nplaylist_open = false\n").unwrap();
        assert_eq!(config.volume, 0.5);
        assert!(!config.windows.playlist_open);
        assert_eq!(config.equalizer, EqualizerConfig::default());
    }
}
