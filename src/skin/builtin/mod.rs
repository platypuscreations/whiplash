//! Whiplash's own skin, used until another is chosen.
//!
//! It is original artwork, drawn here in code: graphite metal with an
//! amber display, laid out like a classic 2.x skin so that every sprite
//! is where the format expects it. `files` produces the same files a
//! `.wsz` holds, so the skin loads like any other, and `export` saves them
//! for anyone who wants to make their own version.

mod equalizer;
mod glyphs;
mod paint;
mod palette;
mod parts;
mod player;
mod playlist;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;

use anyhow::{Context, Result};

use super::Sheet;
use paint::{Paint, darken, lighten};
use palette::PALETTE;

/// Paints one sprite sheet.
fn paint(sheet: Sheet) -> Paint {
    match sheet {
        Sheet::Main => player::main(),
        Sheet::Titlebar => player::titlebar(),
        Sheet::CButtons => player::cbuttons(),
        Sheet::PosBar => player::posbar(),
        Sheet::Volume => player::volume(),
        Sheet::Balance => player::balance(),
        Sheet::MonoSter => player::monoster(),
        Sheet::ShufRep => player::shufrep(),
        Sheet::PlayPaus => player::playpaus(),
        Sheet::Numbers | Sheet::NumsEx => player::numbers(sheet),
        Sheet::Text => player::text(),
        Sheet::EqMain => equalizer::eqmain(),
        Sheet::EqEx => equalizer::eq_ex(),
        Sheet::PlEdit => playlist::pledit(),
    }
}

/// The skin's files, keyed by file name as in a `.wsz`.
pub fn files() -> HashMap<String, Vec<u8>> {
    let mut files: HashMap<String, Vec<u8>> =
        Sheet::ALL.iter().map(|&sheet| (sheet.file_name().to_string(), paint(sheet).to_bmp())).collect();
    files.insert("viscolor.txt".into(), viscolor().into_bytes());
    files.insert("pledit.txt".into(), pledit().into_bytes());
    files.insert("readme.txt".into(), README.replace('\n', "\r\n").into_bytes());
    files
}

const README: &str = "Whiplash
========

The built-in skin of Whiplash Player (https://github.com/platypuscreations/whiplash).

It is original artwork, drawn by Whiplash's own code, and is part of
Whiplash: you may share and change it under the GNU General Public License,
version 3 or later. It contains no Winamp artwork.
";

/// The visualizer's 24 colors: background, grid dots, 16 analyzer rows
/// from top to bottom, 5 oscilloscope shades from the middle outwards,
/// and the peak marks.
fn viscolor() -> String {
    let mut colors = vec![(PALETTE.lcd_background, "background"), (PALETTE.lcd_grid, "grid dots")];
    for row in 0..16 {
        colors.push((parts::ramp(1.0 - row as f32 / 15.0), if row == 0 { "analyzer, top" } else { "" }));
    }
    let scope = [lighten(PALETTE.lcd_text, 0.5), PALETTE.lcd_text, darken(PALETTE.lcd_text, 0.15), darken(PALETTE.lcd_text, 0.3), darken(PALETTE.lcd_text, 0.45)];
    for (i, color) in scope.into_iter().enumerate() {
        colors.push((color, if i == 0 { "oscilloscope, middle" } else { "" }));
    }
    colors.push((PALETTE.peak, "analyzer peaks"));

    colors
        .iter()
        .map(|&(c, note)| {
            let line = format!("{},{},{},", c >> 16, (c >> 8) & 0xFF, c & 0xFF);
            if note.is_empty() { format!("{line}\r\n") } else { format!("{line} // {note}\r\n") }
        })
        .collect()
}

fn pledit() -> String {
    let hex = |c: u32| format!("#{c:06X}");
    [
        "[Text]".to_string(),
        format!("Normal={}", hex(PALETTE.lcd_text)),
        format!("Current={}", hex(PALETTE.current_track)),
        format!("NormalBG={}", hex(PALETTE.lcd_background)),
        format!("SelectedBG={}", hex(PALETTE.selection)),
        "Font=Arial".to_string(),
        String::new(),
    ]
    .join("\r\n")
}

/// Saves the skin as a `.wsz` (or `.zip`) file, or else into a folder.
pub fn export(path: &Path) -> Result<()> {
    let files: BTreeMap<String, Vec<u8>> = files().into_iter().collect();
    let archive = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("wsz") || e.eq_ignore_ascii_case("zip"));
    if archive {
        fs::write(path, zip(&files)?).with_context(|| format!("writing {}", path.display()))?;
    } else {
        fs::create_dir_all(path).with_context(|| format!("creating {}", path.display()))?;
        for (name, data) in &files {
            let file = path.join(name);
            fs::write(&file, data).with_context(|| format!("writing {}", file.display()))?;
        }
    }
    Ok(())
}

fn zip(files: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in files {
        writer.start_file(name.as_str(), zip::write::SimpleFileOptions::default())?;
        writer.write_all(data)?;
    }
    Ok(writer.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skin::{self, Skin, Sprite};

    fn skin() -> Skin {
        Skin::from_files(files())
    }

    /// The pixels of a sprite.
    fn pixels(skin: &Skin, sprite: Sprite) -> Vec<u32> {
        let sheet = skin.sheet(sprite.sheet).unwrap();
        (0..sprite.h).flat_map(|y| (0..sprite.w).map(move |x| (x, y))).map(|(x, y)| sheet.get(sprite.x + x, sprite.y + y).unwrap()).collect()
    }

    #[test]
    fn loads_without_warnings_and_has_every_sheet() {
        let skin = skin();
        assert!(skin.warnings.is_empty(), "{:?}", skin.warnings);
        for sheet in Sheet::ALL {
            assert!(skin.sheet(sheet).is_some(), "{sheet:?} is missing");
        }
    }

    #[test]
    fn sheets_are_the_standard_sizes() {
        let sizes = [
            (Sheet::Main, 275, 116),
            (Sheet::Titlebar, 344, 87),
            (Sheet::CButtons, 136, 36),
            (Sheet::PosBar, 307, 10),
            (Sheet::Volume, 68, 433),
            (Sheet::Balance, 47, 433),
            (Sheet::MonoSter, 56, 24),
            (Sheet::ShufRep, 92, 85),
            (Sheet::PlayPaus, 42, 9),
            (Sheet::Numbers, 99, 13),
            (Sheet::NumsEx, 108, 13),
            (Sheet::Text, 155, 18),
            (Sheet::EqMain, 275, 315),
            (Sheet::EqEx, 275, 56),
            (Sheet::PlEdit, 280, 186),
        ];
        let skin = skin();
        for (sheet, width, height) in sizes {
            let bitmap = skin.sheet(sheet).unwrap();
            assert_eq!((bitmap.width, bitmap.height), (width, height), "{sheet:?}");
        }
    }

    #[test]
    fn every_sprite_fits_its_sheet() {
        let skin = skin();
        for sprite in skin::all_sprites() {
            let bitmap = skin.sheet(sprite.sheet).unwrap();
            assert!(sprite.x + sprite.w <= bitmap.width && sprite.y + sprite.h <= bitmap.height, "{sprite:?}");
        }
    }

    #[test]
    fn every_character_has_a_glyph() {
        let skin = skin();
        let blank = pixels(&skin, skin::character(' '));
        for (c, glyph) in player::font_characters() {
            assert!(glyphs::TEXT.has(glyph), "no glyph for {c:?}");
            assert_ne!(pixels(&skin, skin::character(c)), blank, "{c:?} is blank");
        }
    }

    #[test]
    fn states_look_different() {
        use skin::*;
        let skin = skin();
        let pairs = [
            (TITLE_BAR, TITLE_BAR_SELECTED),
            (MAIN_SHADE_BACKGROUND, MAIN_SHADE_BACKGROUND_SELECTED),
            (PLAY_BUTTON, PLAY_BUTTON_ACTIVE),
            (EJECT_BUTTON, EJECT_BUTTON_ACTIVE),
            (CLOSE_BUTTON, CLOSE_BUTTON_DEPRESSED),
            (SHUFFLE_BUTTON, SHUFFLE_BUTTON_SELECTED),
            (SHUFFLE_BUTTON, SHUFFLE_BUTTON_DEPRESSED),
            (REPEAT_BUTTON, REPEAT_BUTTON_SELECTED),
            (EQ_BUTTON, EQ_BUTTON_SELECTED),
            (PLAYLIST_BUTTON, PLAYLIST_BUTTON_SELECTED),
            (STEREO, STEREO_SELECTED),
            (MONO, MONO_SELECTED),
            (VOLUME_THUMB, VOLUME_THUMB_SELECTED),
            (POSITION_THUMB, POSITION_THUMB_SELECTED),
            (EQ_TITLE_BAR, EQ_TITLE_BAR_SELECTED),
            (EQ_ON_BUTTON, EQ_ON_BUTTON_SELECTED),
            (EQ_AUTO_BUTTON, EQ_AUTO_BUTTON_SELECTED),
            (EQ_PRESETS_BUTTON, EQ_PRESETS_BUTTON_SELECTED),
            (EQ_SLIDER_THUMB, EQ_SLIDER_THUMB_SELECTED),
            (EQ_SHADE_BACKGROUND, EQ_SHADE_BACKGROUND_SELECTED),
            (PL_TITLE, PL_TITLE_SELECTED),
            (PL_SHADE_RIGHT, PL_SHADE_RIGHT_SELECTED),
            (PL_SCROLL_HANDLE, PL_SCROLL_HANDLE_SELECTED),
            (PL_ADD_FILE, pl_menu_selected(PL_ADD_FILE)),
        ];
        for (a, b) in pairs {
            assert_ne!(pixels(&skin, a), pixels(&skin, b), "{a:?} and {b:?} look the same");
        }
        for frame in 1..VOLUME_FRAMES {
            assert_ne!(pixels(&skin, volume_background(frame - 1)), pixels(&skin, volume_background(frame)));
            assert_ne!(pixels(&skin, balance_background(frame - 1)), pixels(&skin, balance_background(frame)));
        }
        for frame in 1..EQ_SLIDER_FRAMES {
            assert_ne!(pixels(&skin, eq_slider_background(frame - 1)), pixels(&skin, eq_slider_background(frame)));
        }
        for n in 1..10 {
            assert_ne!(pixels(&skin, digit(Sheet::Numbers, n - 1)), pixels(&skin, digit(Sheet::Numbers, n)));
        }
    }

    #[test]
    fn displays_share_one_background_color() {
        let skin = skin();
        let background = PALETTE.lcd_background;
        assert_eq!(skin.viscolor[0], background);
        assert_eq!(skin.playlist.normal_bg, background);
        let space = skin::character(' ');
        assert_eq!(skin.sheet(Sheet::Text).unwrap().get(space.x, space.y), Some(background));
        // Inside the marquee's panel, and around the time digits.
        let main = skin.sheet(Sheet::Main).unwrap();
        assert_eq!(main.get(200, 30), Some(background));
        assert_eq!(main.get(47, 26), Some(background));
        assert_eq!(skin.sheet(Sheet::Numbers).unwrap().get(0, 0), Some(background));
        assert_eq!(skin.sheet(Sheet::PlayPaus).unwrap().get(0, 0), Some(background));
    }

    #[test]
    fn painting_is_repeatable() {
        assert_eq!(files(), files());
    }

    #[test]
    fn exported_archive_loads_back() {
        let files: BTreeMap<String, Vec<u8>> = files().into_iter().collect();
        let archive = zip(&files).unwrap();
        let loaded = Skin::from_files(skin::read_archive(&archive).unwrap());
        let original = skin();
        for sheet in Sheet::ALL {
            assert_eq!(loaded.sheet(sheet).unwrap().pixels, original.sheet(sheet).unwrap().pixels, "{sheet:?}");
        }
        assert_eq!(loaded.viscolor, original.viscolor);
    }
}
