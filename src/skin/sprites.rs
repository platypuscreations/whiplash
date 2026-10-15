//! Where each sprite lives inside a skin's sprite sheets.
//!
//! Coordinates come from Webamp's `skinSprites.ts` (MIT license,
//! https://github.com/captbaritone/webamp).

// Some sprites are only used by later phases (seeking, sliders, time display).
#![allow(dead_code)]

use super::Sheet;

/// A rectangle inside one sprite sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sprite {
    pub sheet: Sheet,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

const fn s(sheet: Sheet, x: u32, y: u32, w: u32, h: u32) -> Sprite {
    Sprite { sheet, x, y, w, h }
}

use Sheet::*;

// main.bmp
pub const MAIN_BACKGROUND: Sprite = s(Main, 0, 0, 275, 116);

// titlebar.bmp
pub const TITLE_BAR: Sprite = s(Titlebar, 27, 15, 275, 14);
pub const TITLE_BAR_SELECTED: Sprite = s(Titlebar, 27, 0, 275, 14);
pub const OPTIONS_BUTTON: Sprite = s(Titlebar, 0, 0, 9, 9);
pub const OPTIONS_BUTTON_DEPRESSED: Sprite = s(Titlebar, 0, 9, 9, 9);
pub const MINIMIZE_BUTTON: Sprite = s(Titlebar, 9, 0, 9, 9);
pub const MINIMIZE_BUTTON_DEPRESSED: Sprite = s(Titlebar, 9, 9, 9, 9);
pub const SHADE_BUTTON: Sprite = s(Titlebar, 0, 18, 9, 9);
pub const SHADE_BUTTON_DEPRESSED: Sprite = s(Titlebar, 9, 18, 9, 9);
pub const CLOSE_BUTTON: Sprite = s(Titlebar, 18, 0, 9, 9);
pub const CLOSE_BUTTON_DEPRESSED: Sprite = s(Titlebar, 18, 9, 9, 9);
pub const CLUTTER_BAR: Sprite = s(Titlebar, 304, 0, 8, 43);

// cbuttons.bmp
pub const PREVIOUS_BUTTON: Sprite = s(CButtons, 0, 0, 23, 18);
pub const PREVIOUS_BUTTON_ACTIVE: Sprite = s(CButtons, 0, 18, 23, 18);
pub const PLAY_BUTTON: Sprite = s(CButtons, 23, 0, 23, 18);
pub const PLAY_BUTTON_ACTIVE: Sprite = s(CButtons, 23, 18, 23, 18);
pub const PAUSE_BUTTON: Sprite = s(CButtons, 46, 0, 23, 18);
pub const PAUSE_BUTTON_ACTIVE: Sprite = s(CButtons, 46, 18, 23, 18);
pub const STOP_BUTTON: Sprite = s(CButtons, 69, 0, 23, 18);
pub const STOP_BUTTON_ACTIVE: Sprite = s(CButtons, 69, 18, 23, 18);
pub const NEXT_BUTTON: Sprite = s(CButtons, 92, 0, 22, 18);
pub const NEXT_BUTTON_ACTIVE: Sprite = s(CButtons, 92, 18, 22, 18);
pub const EJECT_BUTTON: Sprite = s(CButtons, 114, 0, 22, 16);
pub const EJECT_BUTTON_ACTIVE: Sprite = s(CButtons, 114, 16, 22, 16);

// posbar.bmp
pub const POSITION_BACKGROUND: Sprite = s(PosBar, 0, 0, 248, 10);
pub const POSITION_THUMB: Sprite = s(PosBar, 248, 0, 29, 10);
pub const POSITION_THUMB_SELECTED: Sprite = s(PosBar, 278, 0, 29, 10);

// volume.bmp: 28 background frames stacked every 15 px, then the thumb.
pub const VOLUME_FRAMES: u32 = 28;
pub const fn volume_background(frame: u32) -> Sprite {
    s(Volume, 0, frame * 15, 68, 13)
}
pub const VOLUME_THUMB: Sprite = s(Volume, 15, 422, 14, 11);
pub const VOLUME_THUMB_SELECTED: Sprite = s(Volume, 0, 422, 14, 11);

// balance.bmp: same layout as volume.bmp, but frames start at x = 9.
pub const fn balance_background(frame: u32) -> Sprite {
    s(Balance, 9, frame * 15, 38, 13)
}
pub const BALANCE_THUMB: Sprite = s(Balance, 15, 422, 14, 11);
pub const BALANCE_THUMB_SELECTED: Sprite = s(Balance, 0, 422, 14, 11);

// monoster.bmp
pub const STEREO: Sprite = s(MonoSter, 0, 12, 29, 12);
pub const STEREO_SELECTED: Sprite = s(MonoSter, 0, 0, 29, 12);
pub const MONO: Sprite = s(MonoSter, 29, 12, 27, 12);
pub const MONO_SELECTED: Sprite = s(MonoSter, 29, 0, 27, 12);

// shufrep.bmp
pub const SHUFFLE_BUTTON: Sprite = s(ShufRep, 28, 0, 47, 15);
pub const SHUFFLE_BUTTON_DEPRESSED: Sprite = s(ShufRep, 28, 15, 47, 15);
pub const SHUFFLE_BUTTON_SELECTED: Sprite = s(ShufRep, 28, 30, 47, 15);
pub const SHUFFLE_BUTTON_SELECTED_DEPRESSED: Sprite = s(ShufRep, 28, 45, 47, 15);
pub const REPEAT_BUTTON: Sprite = s(ShufRep, 0, 0, 28, 15);
pub const REPEAT_BUTTON_DEPRESSED: Sprite = s(ShufRep, 0, 15, 28, 15);
pub const REPEAT_BUTTON_SELECTED: Sprite = s(ShufRep, 0, 30, 28, 15);
pub const REPEAT_BUTTON_SELECTED_DEPRESSED: Sprite = s(ShufRep, 0, 45, 28, 15);
pub const EQ_BUTTON: Sprite = s(ShufRep, 0, 61, 23, 12);
pub const EQ_BUTTON_SELECTED: Sprite = s(ShufRep, 0, 73, 23, 12);
pub const EQ_BUTTON_DEPRESSED: Sprite = s(ShufRep, 46, 61, 23, 12);
pub const EQ_BUTTON_DEPRESSED_SELECTED: Sprite = s(ShufRep, 46, 73, 23, 12);
pub const PLAYLIST_BUTTON: Sprite = s(ShufRep, 23, 61, 23, 12);
pub const PLAYLIST_BUTTON_SELECTED: Sprite = s(ShufRep, 23, 73, 23, 12);
pub const PLAYLIST_BUTTON_DEPRESSED: Sprite = s(ShufRep, 69, 61, 23, 12);
pub const PLAYLIST_BUTTON_DEPRESSED_SELECTED: Sprite = s(ShufRep, 69, 73, 23, 12);

// playpaus.bmp
pub const PLAYING_INDICATOR: Sprite = s(PlayPaus, 0, 0, 9, 9);
pub const PAUSED_INDICATOR: Sprite = s(PlayPaus, 9, 0, 9, 9);
pub const STOPPED_INDICATOR: Sprite = s(PlayPaus, 18, 0, 9, 9);
pub const WORKING_INDICATOR: Sprite = s(PlayPaus, 39, 0, 3, 9);

// numbers.bmp and nums_ex.bmp share the digit layout. nums_ex.bmp also has
// a full-size minus sign; numbers.bmp has a small one drawn mid-height.
pub const fn digit(sheet: Sheet, n: u32) -> Sprite {
    s(sheet, n * 9, 0, 9, 13)
}
pub const MINUS_SIGN: Sprite = s(Numbers, 20, 6, 5, 1);
pub const MINUS_SIGN_EX: Sprite = s(NumsEx, 99, 0, 9, 13);

// text.bmp: a 5x6 pixel font laid out in rows of 31 characters.
pub const CHAR_WIDTH: u32 = 5;
pub const CHAR_HEIGHT: u32 = 6;

/// The sprite for one character of the skin font. Letters are uppercase in
/// the font, so case is ignored. Characters the font lacks become spaces.
pub fn character(c: char) -> Sprite {
    let (row, col) = match c.to_ascii_lowercase() {
        c @ 'a'..='z' => (0, c as u32 - 'a' as u32),
        '"' => (0, 26),
        '@' => (0, 27),
        c @ '0'..='9' => (1, c as u32 - '0' as u32),
        '…' => (1, 10),
        '.' => (1, 11),
        ':' => (1, 12),
        '(' => (1, 13),
        ')' => (1, 14),
        '-' => (1, 15),
        '\'' => (1, 16),
        '!' => (1, 17),
        '_' => (1, 18),
        '+' => (1, 19),
        '\\' => (1, 20),
        '/' => (1, 21),
        '[' | '<' | '{' => (1, 22),
        ']' | '>' | '}' => (1, 23),
        '^' => (1, 24),
        '&' => (1, 25),
        '%' => (1, 26),
        ',' => (1, 27),
        '=' => (1, 28),
        '$' => (1, 29),
        '#' => (1, 30),
        'å' | 'Å' => (2, 0),
        'ö' | 'Ö' => (2, 1),
        'ä' | 'Ä' => (2, 2),
        '?' => (2, 3),
        '*' => (2, 4),
        _ => (0, 30), // space
    };
    s(Text, col * CHAR_WIDTH, row * CHAR_HEIGHT, CHAR_WIDTH, CHAR_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_lookup() {
        assert_eq!(character('A'), character('a'));
        assert_eq!((character('c').x, character('c').y), (10, 0));
        assert_eq!((character('9').x, character('9').y), (45, 6));
        assert_eq!(character('~'), character(' '));
    }
}
