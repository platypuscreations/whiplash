//! The main window's sheets: main.bmp, titlebar.bmp, the buttons, sliders,
//! indicators and the display fonts.

use super::glyphs::{self, SEGMENTS, segment};
use super::paint::{Paint, darken, lighten};
use super::palette::PALETTE;
use super::parts::{self, Icon, rect};
use crate::skin::*;
use crate::ui::main_window;

const HEIGHT: i32 = main_window::HEIGHT as i32;

/// Where the time digits go in the main window, left to right.
const DIGIT_X: [i32; 4] = [48, 60, 78, 90];
const DIGIT_Y: i32 = 26;

pub fn main() -> Paint {
    let (w, h) = (main_window::WIDTH as i32, HEIGHT);
    let mut p = Paint::new(w as u32, h as u32, 0);
    p.metal((0, 0, w, h), (0, 0), h);
    p.edge((0, 0, w, h), PALETTE.outline, PALETTE.outline);
    p.raised((1, 1, w - 2, h - 2));

    // The display: play state, time and visualizer.
    p.lcd_well((21, 24, 82, 37));
    p.fill((24, 29, 3, 7), PALETTE.led_off); // the unlit "working" light
    for x in DIGIT_X {
        seven_segment(&mut p, (x, DIGIT_Y), None);
    }
    p.fill((72, 29, 2, 2), PALETTE.lcd_text);
    p.fill((72, 34, 2, 2), PALETTE.lcd_text);
    let (vx, vy) = main_window::VIS_POSITION;
    for row in (1..16).step_by(2) {
        for col in (0..76).step_by(2) {
            p.set(vx + col, vy + row, PALETTE.lcd_grid);
        }
    }

    // The scrolling title, and bitrate and sample rate with their labels.
    p.lcd_well((109, 24, 159, 12));
    p.lcd_well((109, 41, 19, 10));
    parts::engrave(&mut p, &glyphs::SMALL, "KBPS", (131, 44));
    p.lcd_well((154, 41, 14, 10));
    parts::engrave(&mut p, &glyphs::SMALL, "KHZ", (171, 44));

    // Grooves the sliders sit in.
    p.sunken((106, 56, 70, 15));
    p.sunken((176, 56, 40, 15));
    p.sunken((15, 71, 250, 12));

    parts::brand_mark(&mut p, (245, 89, 23, 16));
    parts::engrave(&mut p, &glyphs::SMALL, "WHIPLASH PLAYER", (16, 108));
    p
}

/// Draws a seven-segment digit with its corner at (x, y): unlit segments
/// faintly, and the segments of `digit` (if any) brightly.
fn seven_segment(p: &mut Paint, (x, y): (i32, i32), digit: Option<usize>) {
    for bit in 0..7 {
        let lit = digit.is_some_and(|d| SEGMENTS[d] & (1 << bit) != 0);
        let color = if lit { PALETTE.lcd_text } else { PALETTE.lcd_ghost };
        for &(sx, sy, sw, sh) in segment(bit) {
            p.fill((x + sx, y + sy, sw, sh), color);
        }
    }
}

pub fn titlebar() -> Paint {
    let mut p = Paint::new(344, 87, PALETTE.face_bottom);
    for (sprite, icon, pressed) in [
        (OPTIONS_BUTTON, Icon::Options, false),
        (OPTIONS_BUTTON_DEPRESSED, Icon::Options, true),
        (MINIMIZE_BUTTON, Icon::Minimize, false),
        (MINIMIZE_BUTTON_DEPRESSED, Icon::Minimize, true),
        (CLOSE_BUTTON, Icon::Close, false),
        (CLOSE_BUTTON_DEPRESSED, Icon::Close, true),
        (SHADE_BUTTON, Icon::Shade, false),
        (SHADE_BUTTON_DEPRESSED, Icon::Shade, true),
        (SHADE_BUTTON_SELECTED, Icon::Unshade, false),
        (SHADE_BUTTON_SELECTED_DEPRESSED, Icon::Unshade, true),
    ] {
        parts::small_button(&mut p, (sprite.x as i32, sprite.y as i32), icon, pressed);
    }

    for (sprite, active) in [(TITLE_BAR_SELECTED, true), (TITLE_BAR, false)] {
        parts::title_strip(&mut p, rect(sprite), "WHIPLASH PLAYER", active, (18, 240));
    }
    for (sprite, active) in [(MAIN_SHADE_BACKGROUND_SELECTED, true), (MAIN_SHADE_BACKGROUND, false)] {
        main_shade(&mut p, sprite, active);
    }

    // The tiny seek bar in shade mode.
    let (x, y, w, h) = rect(SHADE_POSITION_BACKGROUND);
    p.fill((x, y, w, h), PALETTE.groove);
    p.fill((x, y + 3, w, 1), PALETTE.edge_dark);
    for (sprite, shade) in [(SHADE_POSITION_THUMB_LEFT, 0.25), (SHADE_POSITION_THUMB, 0.0), (SHADE_POSITION_THUMB_RIGHT, 0.25)] {
        let (x, y, w, h) = rect(sprite);
        p.fill((x, y, w, h), darken(PALETTE.led_on, shade));
        p.fill((x, y, w, 1), lighten(PALETTE.led_on, 0.4));
    }

    clutter_bar(&mut p);
    p
}

/// The main window shrunk to a strip, with tiny transport symbols, a
/// time display and room for the tiny visualizer.
fn main_shade(p: &mut Paint, sprite: Sprite, active: bool) {
    let (x, y, w, h) = rect(sprite);
    parts::title_strip(p, (x, y, w, h), "", active, (0, 0));
    p.text(&glyphs::TEXT, "WHIPLASH", x + 19, y + 5, PALETTE.outline);
    p.text(&glyphs::TEXT, "WHIPLASH", x + 18, y + 4, parts::title_color(active));
    // The tiny visualizer (38 x 5 at 79, 5) and the time (at 127, 4).
    p.lcd_well((x + 78, y + 4, 40, 7));
    p.lcd_well((x + 126, y + 3, 31, 8));
    let symbol = if active { PALETTE.engraved } else { PALETTE.title_inactive };
    let symbols: [(&[&str], i32); 6] = [
        (&["#..#", "#.##", "####", "#.##", "#..#"], 170),
        (&["#..", "##.", "###", "##.", "#.."], 178),
        (&["#.#", "#.#", "#.#", "#.#", "#.#"], 188),
        (&["####", "####", "####", "####", "...."], 197),
        (&["#..#", "##.#", "####", "##.#", "#..#"], 206),
        (&["..#..", ".###.", "#####", ".....", "#####"], 217),
    ];
    for (rows, sx) in symbols {
        p.shape(rows, x + sx, y + 4, symbol);
    }
}

/// The strip of tiny O, A, I, D and V buttons down the main window's left
/// side, plus lit copies of each in their own places in the sheet.
fn clutter_bar(p: &mut Paint) {
    let (bx, by, bw, bh) = rect(CLUTTER_BAR);
    p.fill((bx, by, bw, bh), PALETTE.groove);
    p.edge((bx, by, bw, bh), PALETTE.edge_dark, PALETTE.edge_light);
    // Each button's letter and its slot: top and height within the bar.
    let slots = [
        ("O", 3, 8, CLUTTER_O_SELECTED),
        ("A", 11, 7, CLUTTER_A_SELECTED),
        ("I", 18, 7, CLUTTER_I_SELECTED),
        ("D", 25, 8, CLUTTER_D_SELECTED),
        ("V", 33, 7, CLUTTER_V_SELECTED),
    ];
    for (letter, top, height, lit) in slots {
        p.text(&glyphs::SMALL, letter, bx + 3, by + top + (height - 5) / 2, PALETTE.title_inactive);
        let (lx, ly, lw, lh) = rect(lit);
        p.fill((lx, ly, lw, lh), darken(PALETTE.led_on, 0.55));
        p.text(&glyphs::SMALL, letter, lx + 3, ly + (lh - 5) / 2, lighten(PALETTE.led_on, 0.3));
    }
}

pub fn cbuttons() -> Paint {
    let mut p = Paint::new(136, 36, PALETTE.face_bottom);
    let symbols: [(Sprite, Sprite, &[&str]); 6] = [
        (PREVIOUS_BUTTON, PREVIOUS_BUTTON_ACTIVE, &["#...#", "#..##", "#.###", "#####", "#.###", "#..##", "#...#"]),
        (PLAY_BUTTON, PLAY_BUTTON_ACTIVE, &["#...", "##..", "###.", "####", "###.", "##..", "#..."]),
        (PAUSE_BUTTON, PAUSE_BUTTON_ACTIVE, &["##.##", "##.##", "##.##", "##.##", "##.##", "##.##", "##.##"]),
        (STOP_BUTTON, STOP_BUTTON_ACTIVE, &["######", "######", "######", "######", "######", "######"]),
        (NEXT_BUTTON, NEXT_BUTTON_ACTIVE, &["#...#", "##..#", "###.#", "#####", "###.#", "##..#", "#...#"]),
        (EJECT_BUTTON, EJECT_BUTTON_ACTIVE, &["...#...", "..###..", ".#####.", "#######", ".......", "#######"]),
    ];
    for (up, down, rows) in symbols {
        for (sprite, pressed) in [(up, false), (down, true)] {
            let (x, y, w, h) = rect(sprite);
            p.button((x, y, w, h), pressed);
            let (sw, sh) = (rows[0].len() as i32, rows.len() as i32);
            let shift = i32::from(pressed);
            p.shape(rows, x + (w - sw) / 2 + shift, y + (h - sh) / 2 + shift, PALETTE.button_symbol);
        }
    }
    p
}

pub fn posbar() -> Paint {
    let mut p = Paint::new(307, 10, PALETTE.face_bottom);
    let (x, y, w, h) = rect(POSITION_BACKGROUND);
    p.metal((x, y, w, h), (16, 72), HEIGHT);
    p.fill((x + 1, y + 3, w - 2, 4), PALETTE.groove);
    p.edge((x + 1, y + 3, w - 2, 4), PALETTE.edge_dark, PALETTE.edge_light);
    for (sprite, selected) in [(POSITION_THUMB, false), (POSITION_THUMB_SELECTED, true)] {
        parts::thumb(&mut p, rect(sprite), selected, true);
    }
    p
}

/// volume.bmp: 28 frames whose groove fills further as the level rises,
/// then the thumbs.
pub fn volume() -> Paint {
    let mut p = Paint::new(68, 433, PALETTE.face_bottom);
    for frame in 0..VOLUME_FRAMES {
        let (x, y, w, h) = rect(volume_background(frame));
        p.metal((x, y, w, h), (107, 57), HEIGHT);
        let groove = (x + 1, y + 4, w - 2, 5);
        p.fill(groove, PALETTE.groove);
        let level = frame as f32 / (VOLUME_FRAMES - 1) as f32;
        let filled = (level * (w - 4) as f32).round() as i32;
        p.fill((x + 2, y + 5, filled, 3), parts::ramp(level));
        p.edge(groove, PALETTE.edge_dark, PALETTE.edge_light);
    }
    parts::thumb(&mut p, rect(VOLUME_THUMB), false, true);
    parts::thumb(&mut p, rect(VOLUME_THUMB_SELECTED), true, true);
    p
}

/// balance.bmp: like volume.bmp, but the groove fills outwards from the
/// middle as the balance moves away from center.
pub fn balance() -> Paint {
    let mut p = Paint::new(47, 433, PALETTE.face_bottom);
    for frame in 0..VOLUME_FRAMES {
        let (x, y, w, h) = rect(balance_background(frame));
        p.metal((x, y, w, h), (177, 57), HEIGHT);
        let groove = (x + 1, y + 4, w - 2, 5);
        p.fill(groove, PALETTE.groove);
        let amount = frame as f32 / (VOLUME_FRAMES - 1) as f32;
        let half = (amount * ((w - 4) / 2) as f32).round() as i32;
        let middle = x + w / 2;
        p.fill((middle - half, y + 5, half * 2, 3), parts::ramp(amount));
        p.edge(groove, PALETTE.edge_dark, PALETTE.edge_light);
        // Ticks marking the middle, above and below the groove.
        p.fill((middle - 1, y + 2, 2, 1), PALETTE.engraved);
        p.fill((middle - 1, y + 10, 2, 1), PALETTE.engraved);
    }
    parts::thumb(&mut p, rect(BALANCE_THUMB), false, true);
    parts::thumb(&mut p, rect(BALANCE_THUMB_SELECTED), true, true);
    p
}

pub fn monoster() -> Paint {
    let mut p = Paint::new(56, 24, PALETTE.face_bottom);
    for (sprite, text, lit) in [(STEREO_SELECTED, "STEREO", true), (STEREO, "STEREO", false), (MONO_SELECTED, "MONO", true), (MONO, "MONO", false)] {
        let (x, y, w, h) = rect(sprite);
        p.fill((x, y, w, h), PALETTE.lcd_background);
        p.edge((x, y, w, h), PALETTE.edge_dark, PALETTE.edge_light);
        let color = if lit { PALETTE.lcd_text } else { PALETTE.lcd_dim };
        p.text_centered(&glyphs::SMALL, text, (x, y, w, h), color);
    }
    p
}

pub fn shufrep() -> Paint {
    let mut p = Paint::new(92, 85, PALETTE.face_bottom);
    let states = |off, off_pressed, on, on_pressed| [(off, false, false), (off_pressed, false, true), (on, true, false), (on_pressed, true, true)];
    for (sprite, on, pressed) in states(SHUFFLE_BUTTON, SHUFFLE_BUTTON_DEPRESSED, SHUFFLE_BUTTON_SELECTED, SHUFFLE_BUTTON_SELECTED_DEPRESSED) {
        parts::led_button(&mut p, rect(sprite), "SHUFFLE", on, pressed);
    }
    for (sprite, on, pressed) in states(REPEAT_BUTTON, REPEAT_BUTTON_DEPRESSED, REPEAT_BUTTON_SELECTED, REPEAT_BUTTON_SELECTED_DEPRESSED) {
        let (x, y, w, h) = rect(sprite);
        parts::led_button(&mut p, (x, y, w, h), "", on, pressed);
        // A circular arrow.
        let rows = ["..####.#", ".#....##", "#....###", "#.......", "#......#", ".#....#.", "..####.."];
        let shift = i32::from(pressed);
        p.shape(&rows, x + 13 + shift, y + 4 + shift, PALETTE.button_symbol);
    }
    for (sprite, on, pressed) in states(EQ_BUTTON, EQ_BUTTON_DEPRESSED, EQ_BUTTON_SELECTED, EQ_BUTTON_DEPRESSED_SELECTED) {
        parts::led_button(&mut p, rect(sprite), "EQ", on, pressed);
    }
    for (sprite, on, pressed) in
        states(PLAYLIST_BUTTON, PLAYLIST_BUTTON_DEPRESSED, PLAYLIST_BUTTON_SELECTED, PLAYLIST_BUTTON_DEPRESSED_SELECTED)
    {
        parts::led_button(&mut p, rect(sprite), "PL", on, pressed);
    }
    p
}

pub fn playpaus() -> Paint {
    let mut p = Paint::new(42, 9, PALETTE.lcd_background);
    let color = PALETTE.lcd_text;
    p.shape(&["#...", "##..", "###.", "####", "###.", "##..", "#..."], PLAYING_INDICATOR.x as i32 + 3, 1, color);
    p.shape(&["##.##"; 7], PAUSED_INDICATOR.x as i32 + 2, 1, color);
    p.shape(&["#####"; 5], STOPPED_INDICATOR.x as i32 + 2, 2, color);
    p.fill((WORKING_INDICATOR.x as i32, 1, 3, 7), PALETTE.led_on);
    p
}

/// numbers.bmp or nums_ex.bmp: the digits 0-9, a blank, and (in nums_ex)
/// a minus sign.
pub fn numbers(sheet: Sheet) -> Paint {
    let width = if sheet == Sheet::NumsEx { 108 } else { 99 };
    let mut p = Paint::new(width, 13, PALETTE.lcd_background);
    for n in 0..10 {
        seven_segment(&mut p, (n as i32 * 9, 0), Some(n));
    }
    seven_segment(&mut p, (90, 0), None);
    if sheet == Sheet::NumsEx {
        let (x, y, _, _) = rect(MINUS_SIGN_EX);
        for &(sx, sy, sw, sh) in segment(6) {
            p.fill((x + sx, y + sy, sw, sh), PALETTE.lcd_text);
        }
    }
    p
}

/// text.bmp: the display font, one character per 5 x 6 cell.
pub fn text() -> Paint {
    let mut p = Paint::new(155, 18, PALETTE.lcd_background);
    for (c, glyph) in font_characters() {
        let sprite = character(c);
        // Narrow glyphs sit in the middle of their cell, so text spaced
        // a cell per character looks even.
        let width = glyphs::TEXT.glyph(glyph)[0].len() as u32;
        let left = sprite.x + (sprite.w - 1).saturating_sub(width).div_ceil(2);
        p.text(&glyphs::TEXT, &glyph.to_string(), left as i32, sprite.y as i32, PALETTE.lcd_text);
    }
    p
}

/// Every character text.bmp has a cell for, with the glyph to draw in it.
pub fn font_characters() -> Vec<(char, char)> {
    let mut chars: Vec<(char, char)> = ('a'..='z').chain('0'..='9').map(|c| (c, c)).collect();
    chars.extend("\"@….:()-'!_+\\/[]^&%,=$#?*".chars().map(|c| (c, c)));
    chars.extend([('å', 'Å'), ('ö', 'Ö'), ('ä', 'Ä')]);
    chars
}
