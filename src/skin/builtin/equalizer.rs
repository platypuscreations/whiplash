//! The equalizer window's sheets: eqmain.bmp and eq_ex.bmp.

use super::glyphs;
use super::paint::{Paint, darken, lighten, mix};
use super::palette::PALETTE;
use super::parts::{self, Icon, rect};
use crate::skin::*;
use crate::ui::eq_window;

const HEIGHT: i32 = eq_window::HEIGHT as i32;

/// Left edge of each slider in the window: the preamp, then the bands.
const SLIDER_X: [i32; 11] = [21, 78, 96, 114, 132, 150, 168, 186, 204, 222, 240];
const SLIDER_Y: i32 = 38;
const BAND_LABELS: [&str; 10] = ["60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K"];

pub fn eqmain() -> Paint {
    let mut p = Paint::new(275, 315, PALETTE.face_bottom);
    background(&mut p);

    for (sprite, active) in [(EQ_TITLE_BAR_SELECTED, true), (EQ_TITLE_BAR, false)] {
        let (x, y, w, h) = rect(sprite);
        parts::title_strip(&mut p, (x, y, w, h), "WHIPLASH EQUALIZER", active, (6, 248));
        parts::small_button(&mut p, (x + 254, y + 3), Icon::Shade, false);
        parts::small_button(&mut p, (x + 264, y + 3), Icon::Close, false);
    }
    parts::small_button(&mut p, (EQ_CLOSE_BUTTON.x as i32, EQ_CLOSE_BUTTON.y as i32), Icon::Close, false);
    parts::small_button(&mut p, (EQ_CLOSE_BUTTON_ACTIVE.x as i32, EQ_CLOSE_BUTTON_ACTIVE.y as i32), Icon::Close, true);

    for (sprite, on, pressed) in [
        (EQ_ON_BUTTON, false, false),
        (EQ_ON_BUTTON_DEPRESSED, false, true),
        (EQ_ON_BUTTON_SELECTED, true, false),
        (EQ_ON_BUTTON_SELECTED_DEPRESSED, true, true),
    ] {
        parts::led_button(&mut p, rect(sprite), "ON", on, pressed);
    }
    for (sprite, on, pressed) in [
        (EQ_AUTO_BUTTON, false, false),
        (EQ_AUTO_BUTTON_DEPRESSED, false, true),
        (EQ_AUTO_BUTTON_SELECTED, true, false),
        (EQ_AUTO_BUTTON_SELECTED_DEPRESSED, true, true),
    ] {
        parts::led_button(&mut p, rect(sprite), "AUTO", on, pressed);
    }
    parts::caption_button(&mut p, rect(EQ_PRESETS_BUTTON), "PRESETS", false);
    parts::caption_button(&mut p, rect(EQ_PRESETS_BUTTON_SELECTED), "PRESETS", true);
    parts::thumb(&mut p, rect(EQ_SLIDER_THUMB), false, false);
    parts::thumb(&mut p, rect(EQ_SLIDER_THUMB_SELECTED), true, false);

    for frame in 0..EQ_SLIDER_FRAMES {
        slider_frame(&mut p, eq_slider_background(frame), frame);
    }

    // The curve graph's panel, its preamp line, and the curve's colors
    // from top (+12 dB) to bottom (-12 dB).
    let graph = rect(EQ_GRAPH_BACKGROUND);
    p.lcd(graph);
    let (gx, gy, gw, gh) = graph;
    for col in (0..gw).step_by(2) {
        p.set(gx + col, gy + gh / 2, PALETTE.lcd_dim);
    }
    p.fill(rect(EQ_PREAMP_LINE), mix(PALETTE.lcd_dim, PALETTE.lcd_text, 0.4));
    let (cx, cy, _, ch) = rect(EQ_GRAPH_LINE_COLORS);
    for row in 0..ch {
        let boost = (row - ch / 2).abs() as f32 / (ch / 2) as f32;
        p.set(cx, cy + row, mix(PALETTE.lcd_text, PALETTE.spectrum[0], boost * 0.8));
    }
    p
}

/// The window behind its controls: metal, frame, labels and guide lines.
fn background(p: &mut Paint) {
    let (x, y, w, h) = rect(EQ_BACKGROUND);
    p.metal((x, y, w, h), (0, 0), HEIGHT);
    p.edge((x, y, w, h), PALETTE.outline, PALETTE.outline);
    p.raised((x + 1, y + 1, w - 2, h - 2));

    p.lcd_well((86, 17, 113, 19));
    // The +12 dB, 0 dB and -12 dB labels, which set every band at once,
    // and dotted guide lines at those levels across the bands.
    for (label, row) in [("+12DB", 36), ("0DB", 64), ("-12DB", 95)] {
        let width = glyphs::SMALL.width(label);
        parts::engrave(p, &glyphs::SMALL, label, (45 + 21 - width, row + 1));
    }
    for level in [5, 31, 56] {
        for col in (70..258).step_by(2) {
            p.set(col, SLIDER_Y + level, darken(PALETTE.engraved, 0.45));
        }
    }
    // A groove under every slider, and its label below it.
    for (i, &sx) in SLIDER_X.iter().enumerate() {
        p.sunken((sx - 1, SLIDER_Y - 1, 16, 65));
        let label = if i == 0 { "PREAMP" } else { BAND_LABELS[i - 1] };
        let width = glyphs::SMALL.width(label);
        parts::engrave(p, &glyphs::SMALL, label, (sx + 7 - width / 2, 105));
    }
}

/// One of the 28 slider backgrounds: a groove lit from the 0 dB line to
/// the thumb, amber for a boost and copper for a cut.
fn slider_frame(p: &mut Paint, sprite: Sprite, frame: u32) {
    let (x, y, w, h) = rect(sprite);
    p.metal((x, y, w, h), (21, SLIDER_Y), HEIGHT);
    let groove = (x + 5, y + 2, 4, h - 4);
    p.fill(groove, PALETTE.groove);
    let value = frame as f32 / (EQ_SLIDER_FRAMES - 1) as f32;
    // The thumb's middle, as eq_window places it.
    let thumb = ((1.0 - value) * 51.0).round() as i32 + 5;
    let middle = 31;
    let (top, bottom) = (thumb.min(middle), thumb.max(middle));
    let color = if thumb < middle { PALETTE.led_on } else { PALETTE.cut };
    p.fill((x + 6, y + top, 2, bottom - top + 1), color);
    p.edge(groove, PALETTE.edge_dark, PALETTE.edge_light);
    // Ticks at 0 dB on either side of the groove.
    p.fill((x + 1, y + middle, 3, 1), PALETTE.engraved);
    p.fill((x + 10, y + middle, 3, 1), PALETTE.engraved);
}

pub fn eq_ex() -> Paint {
    let mut p = Paint::new(275, 56, PALETTE.face_bottom);
    for (sprite, active) in [(EQ_SHADE_BACKGROUND_SELECTED, true), (EQ_SHADE_BACKGROUND, false)] {
        let (x, y, w, h) = rect(sprite);
        parts::title_strip(&mut p, (x, y, w, h), "", active, (0, 0));
        p.text(&glyphs::TEXT, "EQ", x + 9, y + 5, PALETTE.outline);
        p.text(&glyphs::TEXT, "EQ", x + 8, y + 4, parts::title_color(active));
        // The tiny volume (61..158) and balance (164..207) sliders.
        for (sx, sw) in [(61, 97), (164, 43)] {
            p.fill((x + sx, y + 6, sw, 2), PALETTE.groove);
            p.fill((x + sx, y + 8, sw, 1), PALETTE.edge_light);
        }
        parts::small_button(&mut p, (x + 254, y + 3), Icon::Unshade, false);
    }
    // The tiny thumbs: dim at the left, bright at the right.
    for thumbs in [EQ_SHADE_VOLUME_THUMBS, EQ_SHADE_BALANCE_THUMBS] {
        for (i, sprite) in thumbs.into_iter().enumerate() {
            let (x, y, w, h) = rect(sprite);
            p.fill((x, y, w, h), darken(PALETTE.led_on, 0.3 - i as f32 * 0.15));
            p.fill((x, y, w, 1), lighten(PALETTE.led_on, 0.4));
        }
    }
    parts::small_button(&mut p, (EQ_SHADE_CLOSE_BUTTON.x as i32, EQ_SHADE_CLOSE_BUTTON.y as i32), Icon::Close, false);
    parts::small_button(&mut p, (EQ_SHADE_CLOSE_BUTTON_ACTIVE.x as i32, EQ_SHADE_CLOSE_BUTTON_ACTIVE.y as i32), Icon::Close, true);
    parts::small_button(&mut p, (EQ_MAXIMIZE_BUTTON_ACTIVE.x as i32, EQ_MAXIMIZE_BUTTON_ACTIVE.y as i32), Icon::Unshade, true);
    p
}
