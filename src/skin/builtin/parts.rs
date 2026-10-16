//! Pieces shared by several of the built-in skin's sheets.

use super::glyphs::{self, Font};
use super::paint::{Paint, Rect, darken, lighten, mix};
use super::palette::PALETTE;
use crate::skin::Sprite;

/// A sprite's rectangle in its sheet.
pub fn rect(sprite: Sprite) -> Rect {
    (sprite.x as i32, sprite.y as i32, sprite.w as i32, sprite.h as i32)
}

/// The visualizer's colors from quiet (t = 0) to loud (t = 1).
pub fn ramp(t: f32) -> u32 {
    let [loud, high, mid, quiet] = PALETTE.spectrum;
    let stops = [quiet, mid, high, loud];
    let t = t.clamp(0.0, 1.0) * 3.0;
    let i = (t as usize).min(2);
    mix(stops[i], stops[i + 1], t - i as f32)
}

/// The tiny pictures on the 9 x 9 title bar buttons.
#[derive(Clone, Copy)]
pub enum Icon {
    Options,
    Minimize,
    /// Shrink the window to a strip.
    Shade,
    /// Grow a strip back to a window.
    Unshade,
    Close,
}

impl Icon {
    fn rows(self) -> &'static [&'static str] {
        match self {
            Icon::Options => &[".###.", "#...#", "#...#", "#...#", ".###."],
            Icon::Minimize => &[".....", ".....", ".....", "#####", "#####"],
            Icon::Shade => &["#####", "#####", ".....", ".....", "....."],
            Icon::Unshade => &["#####", "#...#", "#...#", "#...#", "#####"],
            Icon::Close => &["#...#", ".#.#.", "..#..", ".#.#.", "#...#"],
        }
    }
}

/// A 9 x 9 title bar button.
pub fn small_button(p: &mut Paint, (x, y): (i32, i32), icon: Icon, pressed: bool) {
    let (top, bottom) = if pressed { (0x4A4A52, 0x6A6A72) } else { (0x8E8E96, 0x5A5A62) };
    p.gradient((x, y, 9, 9), top, bottom);
    p.edge((x, y, 9, 9), if pressed { PALETTE.outline } else { 0xB4B4BC }, PALETTE.outline);
    let shift = i32::from(pressed);
    let color = if pressed { PALETTE.title_active } else { 0x101012 };
    p.shape(icon.rows(), x + 2 + shift, y + 2 + shift, color);
}

/// The text color of a title bar.
pub fn title_color(active: bool) -> u32 {
    if active { PALETTE.title_active } else { PALETTE.title_inactive }
}

/// A title bar strip, 14 pixels tall: metal with grip dots either side of
/// the centered title. `grip` is the part of the strip the dots may use.
pub fn title_strip(p: &mut Paint, (x, y, w, h): Rect, title: &str, active: bool, grip: (i32, i32)) {
    p.gradient((x, y, w, h), lighten(PALETTE.face_top, 0.08), PALETTE.face_top);
    p.fill((x, y, w, 1), PALETTE.outline);
    p.fill((x, y, 1, h), PALETTE.outline);
    p.fill((x + w - 1, y, 1, h), PALETTE.outline);
    p.fill((x + 1, y + 1, w - 2, 1), PALETTE.edge_light);
    p.fill((x + 1, y + h - 1, w - 2, 1), darken(PALETTE.face_top, 0.3));

    let font = &glyphs::TITLE;
    let text_width = font.width(title);
    let text_left = x + (w - text_width) / 2;
    // Grip dots in two rows, stopping short of the title.
    let dot = if active { mix(PALETTE.title_active, PALETTE.face_top, 0.45) } else { PALETTE.edge_light };
    for col in (grip.0..grip.1).step_by(3) {
        if col > text_left - 6 && col < text_left + text_width + 4 {
            continue;
        }
        for row in [4, 8] {
            p.set(x + col, y + row, dot);
            p.set(x + col + 1, y + row + 1, PALETTE.outline);
        }
    }
    if !title.is_empty() {
        p.text(font, title, text_left + 1, y + 5, PALETTE.outline);
        p.text(font, title, text_left, y + 4, title_color(active));
    }
}

/// A round indicator light, 4 x 4.
pub fn led(p: &mut Paint, (x, y): (i32, i32), on: bool) {
    let color = if on { PALETTE.led_on } else { PALETTE.led_off };
    p.fill((x, y, 4, 4), color);
    for (cx, cy) in [(0, 0), (3, 0), (0, 3), (3, 3)] {
        p.set(x + cx, y + cy, darken(color, 0.4));
    }
    if on {
        p.set(x + 1, y + 1, lighten(color, 0.6));
    }
}

/// A push button with an indicator light and a caption.
pub fn led_button(p: &mut Paint, (x, y, w, h): Rect, caption: &str, on: bool, pressed: bool) {
    p.button((x, y, w, h), pressed);
    let shift = i32::from(pressed);
    led(p, (x + 3 + shift, y + (h - 4) / 2 + shift), on);
    let font = &glyphs::SMALL;
    let left = x + 8 + (w - 8 - font.width(caption)) / 2;
    p.text(font, caption, left + shift, y + (h - 5) / 2 + shift, PALETTE.button_symbol);
}

/// A push button with a caption in the middle.
pub fn caption_button(p: &mut Paint, rect: Rect, caption: &str, pressed: bool) {
    p.button(rect, pressed);
    let shift = i32::from(pressed);
    let (x, y, w, h) = rect;
    p.text_centered(&glyphs::SMALL, caption, (x + shift, y + shift, w, h), PALETTE.button_symbol);
}

/// A slider thumb: a small button with a grip line, amber while dragged.
pub fn thumb(p: &mut Paint, (x, y, w, h): Rect, selected: bool, vertical_grip: bool) {
    p.button((x, y, w, h), false);
    let grip = if selected { PALETTE.led_on } else { PALETTE.button_dark };
    if vertical_grip {
        for gx in [w / 2 - 2, w / 2, w / 2 + 2] {
            p.fill((x + gx, y + 3, 1, h - 6), grip);
        }
    } else {
        p.fill((x + 3, y + h / 2, w - 6, 1), grip);
    }
}

/// Draws `text` with `font` so it looks stamped into the metal.
pub fn engrave(p: &mut Paint, font: &Font, text: &str, (x, y): (i32, i32)) {
    p.text(font, text, x + 1, y + 1, PALETTE.outline);
    p.text(font, text, x, y, PALETTE.engraved);
}

/// The Whiplash mark: five spectrum bars, after the app icon, on a
/// display panel. The panel is `w` x `h` with its corner at (x, y).
pub fn brand_mark(p: &mut Paint, (x, y, w, h): Rect) {
    p.lcd_well((x, y, w, h));
    let heights = [0.95, 0.75, 0.85, 0.55, 0.35];
    let bar_width = 3;
    let gap = 2;
    let total = 5 * bar_width + 4 * gap;
    let left = x + (w - total) / 2;
    let bottom = y + h - 2;
    let max = h - 4;
    for (i, fraction) in heights.iter().enumerate() {
        let bar = (fraction * max as f32).round() as i32;
        let bx = left + i as i32 * (bar_width + gap);
        for row in 0..bar {
            let color = ramp(row as f32 / (max - 1) as f32);
            p.fill((bx, bottom - row, bar_width, 1), color);
        }
        p.fill((bx, bottom - bar - 1, bar_width, 1), PALETTE.peak);
    }
}
