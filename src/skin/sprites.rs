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
// Shade mode: the main window shrunk to its title bar.
pub const MAIN_SHADE_BACKGROUND: Sprite = s(Titlebar, 27, 42, 275, 14);
pub const MAIN_SHADE_BACKGROUND_SELECTED: Sprite = s(Titlebar, 27, 29, 275, 14);
pub const SHADE_BUTTON_SELECTED: Sprite = s(Titlebar, 0, 27, 9, 9);
pub const SHADE_BUTTON_SELECTED_DEPRESSED: Sprite = s(Titlebar, 9, 27, 9, 9);
pub const SHADE_POSITION_BACKGROUND: Sprite = s(Titlebar, 0, 36, 17, 7);
pub const SHADE_POSITION_THUMB: Sprite = s(Titlebar, 20, 36, 3, 7);
pub const SHADE_POSITION_THUMB_LEFT: Sprite = s(Titlebar, 17, 36, 3, 7);
pub const SHADE_POSITION_THUMB_RIGHT: Sprite = s(Titlebar, 23, 36, 3, 7);
// The clutter bar's buttons, lit (when on or pressed), in their places on it.
pub const CLUTTER_O_SELECTED: Sprite = s(Titlebar, 304, 47, 8, 8);
pub const CLUTTER_A_SELECTED: Sprite = s(Titlebar, 312, 55, 8, 7);
pub const CLUTTER_I_SELECTED: Sprite = s(Titlebar, 320, 62, 8, 7);
pub const CLUTTER_D_SELECTED: Sprite = s(Titlebar, 328, 69, 8, 8);
pub const CLUTTER_V_SELECTED: Sprite = s(Titlebar, 336, 77, 8, 7);

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

// eqmain.bmp
pub const EQ_BACKGROUND: Sprite = s(EqMain, 0, 0, 275, 116);
pub const EQ_TITLE_BAR: Sprite = s(EqMain, 0, 149, 275, 14);
pub const EQ_TITLE_BAR_SELECTED: Sprite = s(EqMain, 0, 134, 275, 14);
pub const EQ_CLOSE_BUTTON: Sprite = s(EqMain, 0, 116, 9, 9);
pub const EQ_CLOSE_BUTTON_ACTIVE: Sprite = s(EqMain, 0, 125, 9, 9);
pub const EQ_ON_BUTTON: Sprite = s(EqMain, 10, 119, 26, 12);
pub const EQ_ON_BUTTON_DEPRESSED: Sprite = s(EqMain, 128, 119, 26, 12);
pub const EQ_ON_BUTTON_SELECTED: Sprite = s(EqMain, 69, 119, 26, 12);
pub const EQ_ON_BUTTON_SELECTED_DEPRESSED: Sprite = s(EqMain, 187, 119, 26, 12);
pub const EQ_AUTO_BUTTON: Sprite = s(EqMain, 36, 119, 32, 12);
pub const EQ_AUTO_BUTTON_DEPRESSED: Sprite = s(EqMain, 154, 119, 32, 12);
pub const EQ_AUTO_BUTTON_SELECTED: Sprite = s(EqMain, 95, 119, 32, 12);
pub const EQ_AUTO_BUTTON_SELECTED_DEPRESSED: Sprite = s(EqMain, 213, 119, 32, 12);
pub const EQ_PRESETS_BUTTON: Sprite = s(EqMain, 224, 164, 44, 12);
pub const EQ_PRESETS_BUTTON_SELECTED: Sprite = s(EqMain, 224, 176, 44, 12);
pub const EQ_SLIDER_THUMB: Sprite = s(EqMain, 0, 164, 11, 11);
pub const EQ_SLIDER_THUMB_SELECTED: Sprite = s(EqMain, 0, 176, 11, 11);
pub const EQ_GRAPH_BACKGROUND: Sprite = s(EqMain, 0, 294, 113, 19);
pub const EQ_PREAMP_LINE: Sprite = s(EqMain, 0, 314, 113, 1);
/// A 1x19 column: the curve's color at each height of the graph.
pub const EQ_GRAPH_LINE_COLORS: Sprite = s(EqMain, 115, 294, 1, 19);

// eq_ex.bmp: the equalizer in shade mode, with tiny volume and balance
// sliders whose thumbs change look across the left, middle and right.
pub const EQ_SHADE_BACKGROUND: Sprite = s(EqEx, 0, 15, 275, 14);
pub const EQ_SHADE_BACKGROUND_SELECTED: Sprite = s(EqEx, 0, 0, 275, 14);
pub const EQ_SHADE_VOLUME_THUMBS: [Sprite; 3] = [s(EqEx, 1, 30, 3, 7), s(EqEx, 4, 30, 3, 7), s(EqEx, 7, 30, 3, 7)];
pub const EQ_SHADE_BALANCE_THUMBS: [Sprite; 3] = [s(EqEx, 11, 30, 3, 7), s(EqEx, 14, 30, 3, 7), s(EqEx, 17, 30, 3, 7)];
pub const EQ_SHADE_CLOSE_BUTTON: Sprite = s(EqEx, 11, 38, 9, 9);
pub const EQ_SHADE_CLOSE_BUTTON_ACTIVE: Sprite = s(EqEx, 11, 47, 9, 9);
pub const EQ_MAXIMIZE_BUTTON_ACTIVE: Sprite = s(EqEx, 1, 38, 9, 9);

/// eqmain.bmp holds 28 slider backgrounds (14x63 each) in two rows of
/// 14, spaced 15 px apart across and 65 px down; frame 0 is the lowest
/// setting.
pub const EQ_SLIDER_FRAMES: u32 = 28;
pub const fn eq_slider_background(frame: u32) -> Sprite {
    s(EqMain, 13 + (frame % 14) * 15, 164 + (frame / 14) * 65, 14, 63)
}

// pledit.bmp: the playlist window's frame, built from tiles.
pub const PL_TOP_LEFT: Sprite = s(PlEdit, 0, 21, 25, 20);
pub const PL_TOP_LEFT_SELECTED: Sprite = s(PlEdit, 0, 0, 25, 20);
pub const PL_TITLE: Sprite = s(PlEdit, 26, 21, 100, 20);
pub const PL_TITLE_SELECTED: Sprite = s(PlEdit, 26, 0, 100, 20);
pub const PL_TOP_TILE: Sprite = s(PlEdit, 127, 21, 25, 20);
pub const PL_TOP_TILE_SELECTED: Sprite = s(PlEdit, 127, 0, 25, 20);
pub const PL_TOP_RIGHT: Sprite = s(PlEdit, 153, 21, 25, 20);
pub const PL_TOP_RIGHT_SELECTED: Sprite = s(PlEdit, 153, 0, 25, 20);
pub const PL_LEFT_TILE: Sprite = s(PlEdit, 0, 42, 12, 29);
pub const PL_RIGHT_TILE: Sprite = s(PlEdit, 31, 42, 20, 29);
pub const PL_BOTTOM_LEFT: Sprite = s(PlEdit, 0, 72, 125, 38);
pub const PL_BOTTOM_RIGHT: Sprite = s(PlEdit, 126, 72, 150, 38);
pub const PL_BOTTOM_TILE: Sprite = s(PlEdit, 179, 0, 25, 38);
pub const PL_VISUALIZER_BACKGROUND: Sprite = s(PlEdit, 205, 0, 75, 38);
pub const PL_SCROLL_HANDLE: Sprite = s(PlEdit, 52, 53, 8, 18);
pub const PL_SCROLL_HANDLE_SELECTED: Sprite = s(PlEdit, 61, 53, 8, 18);
pub const PL_CLOSE_SELECTED: Sprite = s(PlEdit, 52, 42, 9, 9);
pub const PL_COLLAPSE_SELECTED: Sprite = s(PlEdit, 62, 42, 9, 9);
pub const PL_EXPAND_SELECTED: Sprite = s(PlEdit, 150, 42, 9, 9);
// The playlist in shade mode: a left end, tiles, and a right end.
pub const PL_SHADE_LEFT: Sprite = s(PlEdit, 72, 42, 25, 14);
pub const PL_SHADE_TILE: Sprite = s(PlEdit, 72, 57, 25, 14);
pub const PL_SHADE_RIGHT: Sprite = s(PlEdit, 99, 57, 50, 14);
pub const PL_SHADE_RIGHT_SELECTED: Sprite = s(PlEdit, 99, 42, 50, 14);

// The pop-up button menus above the bottom bar: each item is 22x18, with
// the highlighted version 23 px to the right. The bar is drawn to the left.
pub const PL_ADD_URL: Sprite = s(PlEdit, 0, 111, 22, 18);
pub const PL_ADD_DIR: Sprite = s(PlEdit, 0, 130, 22, 18);
pub const PL_ADD_FILE: Sprite = s(PlEdit, 0, 149, 22, 18);
pub const PL_REMOVE_ALL: Sprite = s(PlEdit, 54, 111, 22, 18);
pub const PL_CROP: Sprite = s(PlEdit, 54, 130, 22, 18);
pub const PL_REMOVE_SELECTED: Sprite = s(PlEdit, 54, 149, 22, 18);
pub const PL_REMOVE_MISC: Sprite = s(PlEdit, 54, 168, 22, 18);
pub const PL_INVERT_SELECTION: Sprite = s(PlEdit, 104, 111, 22, 18);
pub const PL_SELECT_ZERO: Sprite = s(PlEdit, 104, 130, 22, 18);
pub const PL_SELECT_ALL: Sprite = s(PlEdit, 104, 149, 22, 18);
pub const PL_SORT_LIST: Sprite = s(PlEdit, 154, 111, 22, 18);
pub const PL_FILE_INFO: Sprite = s(PlEdit, 154, 130, 22, 18);
pub const PL_MISC_OPTIONS: Sprite = s(PlEdit, 154, 149, 22, 18);
pub const PL_NEW_LIST: Sprite = s(PlEdit, 204, 111, 22, 18);
pub const PL_SAVE_LIST: Sprite = s(PlEdit, 204, 130, 22, 18);
pub const PL_LOAD_LIST: Sprite = s(PlEdit, 204, 149, 22, 18);
pub const PL_ADD_MENU_BAR: Sprite = s(PlEdit, 48, 111, 3, 54);
pub const PL_REMOVE_MENU_BAR: Sprite = s(PlEdit, 100, 111, 3, 72);
pub const PL_SELECT_MENU_BAR: Sprite = s(PlEdit, 150, 111, 3, 54);
pub const PL_MISC_MENU_BAR: Sprite = s(PlEdit, 200, 111, 3, 54);
pub const PL_LIST_MENU_BAR: Sprite = s(PlEdit, 250, 111, 3, 54);

/// The highlighted version of a playlist menu item.
pub const fn pl_menu_selected(item: Sprite) -> Sprite {
    s(item.sheet, item.x + 23, item.y, item.w, item.h)
}

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
