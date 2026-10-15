//! Loading classic (Winamp 2.x-style) skins.
//!
//! A classic skin is a zip archive (usually named `.wsz`) or a plain folder
//! containing BMP sprite sheets plus a few text files. Skins in the wild are
//! messy: file names use any letter case, files are sometimes nested inside a
//! subfolder of the archive, and some files are missing or broken. Loading a
//! skin therefore never fails just because one piece is bad; the problem is
//! recorded in `Skin::warnings` and that piece is left out.

mod sprites;
mod viscolor;

pub use sprites::*;

use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{Context, Result};

/// Largest single file we will read out of a skin archive. Real skin files
/// are well under 1 MB; this guards against "zip bombs".
const MAX_FILE_SIZE: u64 = 16 * 1024 * 1024;

/// One BMP sprite sheet inside a skin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sheet {
    Main,
    Titlebar,
    CButtons,
    PosBar,
    Volume,
    Balance,
    MonoSter,
    ShufRep,
    PlayPaus,
    Numbers,
    NumsEx,
    Text,
}

impl Sheet {
    pub const ALL: [Sheet; 12] = [
        Sheet::Main,
        Sheet::Titlebar,
        Sheet::CButtons,
        Sheet::PosBar,
        Sheet::Volume,
        Sheet::Balance,
        Sheet::MonoSter,
        Sheet::ShufRep,
        Sheet::PlayPaus,
        Sheet::Numbers,
        Sheet::NumsEx,
        Sheet::Text,
    ];

    /// File name inside the skin, lowercased (lookups are case-insensitive).
    pub fn file_name(self) -> &'static str {
        match self {
            Sheet::Main => "main.bmp",
            Sheet::Titlebar => "titlebar.bmp",
            Sheet::CButtons => "cbuttons.bmp",
            Sheet::PosBar => "posbar.bmp",
            Sheet::Volume => "volume.bmp",
            Sheet::Balance => "balance.bmp",
            Sheet::MonoSter => "monoster.bmp",
            Sheet::ShufRep => "shufrep.bmp",
            Sheet::PlayPaus => "playpaus.bmp",
            Sheet::Numbers => "numbers.bmp",
            Sheet::NumsEx => "nums_ex.bmp",
            Sheet::Text => "text.bmp",
        }
    }
}

/// A decoded image. Pixels are stored row by row as `0x00RRGGBB`, which is
/// the format `softbuffer` expects, so drawing is a plain copy.
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
}

impl Bitmap {
    fn decode_bmp(bytes: &[u8]) -> Result<Bitmap> {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Bmp)?.to_rgb8();
        let (width, height) = img.dimensions();
        let pixels = img
            .pixels()
            .map(|p| (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32)
            .collect();
        Ok(Bitmap { width, height, pixels })
    }

    /// The pixel at (x, y), or `None` if that is outside the image.
    pub fn get(&self, x: u32, y: u32) -> Option<u32> {
        if x < self.width && y < self.height {
            Some(self.pixels[(y * self.width + x) as usize])
        } else {
            None
        }
    }
}

pub struct Skin {
    sheets: HashMap<Sheet, Bitmap>,
    /// The 24 visualizer colors from `viscolor.txt`, as `0x00RRGGBB`.
    #[allow(dead_code)] // drawn by the visualizer (phase 3)
    pub viscolor: [u32; 24],
    /// Problems found while loading (missing or unreadable files).
    pub warnings: Vec<String>,
}

impl Skin {
    /// Loads a skin from a `.wsz`/`.zip` archive or from an unpacked folder.
    pub fn load(path: &Path) -> Result<Skin> {
        let files = if path.is_dir() {
            read_folder(path)
        } else {
            let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
            read_archive(&bytes).with_context(|| format!("{} is not a valid zip archive", path.display()))
        }?;
        Ok(Skin::from_files(files))
    }

    /// A skin with no images, drawn as plain placeholder colors.
    pub fn empty() -> Skin {
        Skin::from_files(HashMap::new())
    }

    /// Builds a skin from file contents keyed by lowercased file name.
    pub fn from_files(files: HashMap<String, Vec<u8>>) -> Skin {
        let mut sheets = HashMap::new();
        let mut warnings = Vec::new();

        for sheet in Sheet::ALL {
            let name = sheet.file_name();
            match files.get(name) {
                Some(bytes) => match Bitmap::decode_bmp(bytes) {
                    Ok(bitmap) => {
                        sheets.insert(sheet, bitmap);
                    }
                    Err(err) => warnings.push(format!("{name}: could not decode ({err})")),
                },
                // Plenty of skins leave out nums_ex.bmp because numbers.bmp exists.
                None if sheet == Sheet::NumsEx => {}
                None => warnings.push(format!("{name}: missing")),
            }
        }

        let viscolor = match files.get("viscolor.txt") {
            Some(bytes) => viscolor::parse(&String::from_utf8_lossy(bytes)),
            None => viscolor::DEFAULT,
        };

        Skin { sheets, viscolor, warnings }
    }

    pub fn sheet(&self, sheet: Sheet) -> Option<&Bitmap> {
        self.sheets.get(&sheet)
    }
}

/// Reads every file in a zip archive, keyed by lowercased file name.
///
/// Folders inside the archive are ignored, because many skins put their files
/// inside a subfolder. If two files share a name, the one nearest the top of
/// the archive wins.
fn read_archive(bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut files: HashMap<String, (usize, Vec<u8>)> = HashMap::new();

    for i in 0..archive.len() {
        // Skip entries we cannot open (unsupported compression, corruption)
        // instead of rejecting the whole skin.
        let Ok(mut entry) = archive.by_index(i) else { continue };
        if entry.is_dir() || entry.size() > MAX_FILE_SIZE {
            continue;
        }
        // Old archives may use non-UTF-8 names; the files we need are ASCII.
        let path = String::from_utf8_lossy(entry.name_raw()).replace('\\', "/");
        // Archives made on macOS carry metadata copies under __MACOSX/.
        if path.starts_with("__MACOSX/") {
            continue;
        }
        let depth = path.matches('/').count();
        let name = path.rsplit('/').next().unwrap_or_default().to_lowercase();

        if files.get(&name).is_some_and(|(d, _)| *d <= depth) {
            continue;
        }
        let mut data = Vec::new();
        if entry.read_to_end(&mut data).is_ok() {
            files.insert(name, (depth, data));
        }
    }

    Ok(files.into_iter().map(|(name, (_, data))| (name, data)).collect())
}

/// Reads the files directly inside an unpacked skin folder.
fn read_folder(dir: &Path) -> Result<HashMap<String, Vec<u8>>> {
    let mut files = HashMap::new();
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        let too_big = fs::metadata(&path).map_or(true, |m| m.len() > MAX_FILE_SIZE);
        if !path.is_file() || too_big {
            continue;
        }
        if let (Some(name), Ok(data)) = (path.file_name(), fs::read(&path)) {
            files.insert(name.to_string_lossy().to_lowercase(), data);
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Encodes a solid-color BMP of the given size.
    fn bmp(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(width, height, image::Rgb(rgb));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Bmp).unwrap();
        out.into_inner()
    }

    /// Builds a zip archive in memory from (path, contents) pairs.
    fn zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (path, data) in entries {
            writer.start_file(*path, zip::write::SimpleFileOptions::default()).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn file_names_are_case_insensitive_and_nested_folders_are_found() {
        let archive = zip(&[
            ("MySkin/MAIN.BMP", bmp(275, 116, [255, 0, 0])),
            ("MySkin/ViSCoLoR.TxT", b"1,2,3\n".to_vec()),
        ]);
        let skin = Skin::from_files(read_archive(&archive).unwrap());

        let main = skin.sheet(Sheet::Main).expect("main.bmp should load");
        assert_eq!((main.width, main.height), (275, 116));
        assert_eq!(main.get(0, 0), Some(0xFF0000));
        assert_eq!(skin.viscolor[0], 0x010203);
    }

    #[test]
    fn top_level_file_wins_over_nested_duplicate() {
        let archive = zip(&[
            ("extra/main.bmp", bmp(1, 1, [0, 0, 255])),
            ("main.bmp", bmp(1, 1, [0, 255, 0])),
        ]);
        let skin = Skin::from_files(read_archive(&archive).unwrap());
        assert_eq!(skin.sheet(Sheet::Main).unwrap().get(0, 0), Some(0x00FF00));
    }

    #[test]
    fn macos_metadata_is_ignored() {
        let archive = zip(&[
            ("main.bmp", bmp(1, 1, [0, 255, 0])),
            ("__MACOSX/main.bmp", b"not a bitmap".to_vec()),
        ]);
        let files = read_archive(&archive).unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn broken_and_missing_files_become_warnings_not_errors() {
        let archive = zip(&[("main.bmp", b"garbage".to_vec())]);
        let skin = Skin::from_files(read_archive(&archive).unwrap());

        assert!(skin.sheet(Sheet::Main).is_none());
        assert!(skin.warnings.iter().any(|w| w.starts_with("main.bmp: could not decode")));
        assert!(skin.warnings.iter().any(|w| w == "cbuttons.bmp: missing"));
        assert!(!skin.warnings.iter().any(|w| w.starts_with("nums_ex.bmp")));
        assert_eq!(skin.viscolor, viscolor::DEFAULT);
    }

    #[test]
    fn non_zip_file_is_an_error() {
        assert!(read_archive(b"definitely not a zip").is_err());
    }
}
