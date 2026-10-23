//! The playlist window's sheet, pledit.bmp. The window can be resized, so
//! the app builds its frame from tiles that repeat every 25 pixels across
//! and 29 down; patterns here repeat at those steps so the tiles join up.

use super::glyphs;
use super::paint::{Paint, Rect, darken, lighten};
use super::palette::PALETTE;
use super::parts::{self, Icon, rect};
use crate::skin::*;

/// Which part of the top edge a tile is.
#[derive(Clone, Copy, PartialEq)]
enum Top {
    Left,
    Middle,
    Title,
    Right,
}

pub fn pledit() -> Paint {
    let mut p = Paint::new(280, 186, PALETTE.face_bottom);

    for (normal, selected, kind) in [
        (PL_TOP_LEFT, PL_TOP_LEFT_SELECTED, Top::Left),
        (PL_TOP_TILE, PL_TOP_TILE_SELECTED, Top::Middle),
        (PL_TITLE, PL_TITLE_SELECTED, Top::Title),
        (PL_TOP_RIGHT, PL_TOP_RIGHT_SELECTED, Top::Right),
    ] {
        top_piece(&mut p, rect(normal), kind, false);
        top_piece(&mut p, rect(selected), kind, true);
    }
    parts::small_button(&mut p, (PL_CLOSE_SELECTED.x as i32, PL_CLOSE_SELECTED.y as i32), Icon::Close, true);
    parts::small_button(&mut p, (PL_COLLAPSE_SELECTED.x as i32, PL_COLLAPSE_SELECTED.y as i32), Icon::Shade, true);
    parts::small_button(&mut p, (PL_EXPAND_SELECTED.x as i32, PL_EXPAND_SELECTED.y as i32), Icon::Unshade, true);

    sides(&mut p);
    bottom(&mut p);
    parts::thumb(&mut p, rect(PL_SCROLL_HANDLE), false, false);
    parts::thumb(&mut p, rect(PL_SCROLL_HANDLE_SELECTED), true, false);
    shade(&mut p);
    menus(&mut p);
    p
}

/// Plain brushed metal for the frame, the same all the way down.
fn frame_metal(p: &mut Paint, area: Rect) {
    p.metal(area, (area.0, 0), 1);
}

/// A 20-pixel-tall piece of the top edge: a title strip above a band of
/// metal whose bottom row is the top edge of the track list.
fn top_piece(p: &mut Paint, (x, y, w, h): Rect, kind: Top, active: bool) {
    frame_metal(p, (x, y, w, h));
    p.gradient((x, y, w, 14), lighten(PALETTE.face_top, 0.08), PALETTE.face_top);
    p.fill((x, y, w, 1), PALETTE.outline);
    p.fill((x, y + 1, w, 1), PALETTE.edge_light);
    p.fill((x, y + 13, w, 1), darken(PALETTE.face_top, 0.3));

    // The track list starts 12 pixels in from the left and ends 20
    // pixels in from the right; its sunken edge runs along the bottom row.
    let (list_left, list_right) = match kind {
        Top::Left => (11, w),
        Top::Right => (0, 6),
        _ => (0, w),
    };
    p.fill((x + list_left, y + h - 1, list_right - list_left, 1), PALETTE.edge_dark);
    match kind {
        Top::Left => {
            p.fill((x, y, 1, h), PALETTE.outline);
            p.fill((x + 1, y + 1, 1, h - 1), PALETTE.edge_light);
            p.fill((x + 11, y + h - 1, 1, 1), PALETTE.edge_dark);
        }
        Top::Right => {
            p.fill((x + w - 1, y, 1, h), PALETTE.outline);
            p.fill((x + w - 2, y + 2, 1, h - 2), PALETTE.edge_dark);
            p.fill((x + 5, y + h - 1, 1, 1), PALETTE.edge_light);
            parts::small_button(p, (x + 4, y + 3), Icon::Shade, false);
            parts::small_button(p, (x + 14, y + 3), Icon::Close, false);
        }
        _ => {}
    }

    if kind == Top::Title {
        let font = &glyphs::TEXT;
        let title = "WHIPLASH PLAYLIST";
        let left = x + (w - font.width(title)) / 2;
        p.text(font, title, left + 1, y + 5, PALETTE.outline);
        p.text(font, title, left, y + 4, parts::title_color(active));
    } else {
        // Grip dots every 5 pixels, so neighboring tiles line up.
        let dot = if active { darken(PALETTE.title_active, 0.45) } else { PALETTE.edge_light };
        let (from, to) = match kind {
            Top::Left => (5, w),
            Top::Right => (0, 3),
            _ => (0, w),
        };
        for col in (from..to).filter(|c| c % 5 == 0) {
            for row in [4, 8] {
                p.set(x + col, y + row, dot);
                p.set(x + col + 1, y + row + 1, PALETTE.outline);
            }
        }
    }
}

/// The left and right edges, with the scroll bar's groove on the right.
fn sides(p: &mut Paint) {
    let (x, y, w, h) = rect(PL_LEFT_TILE);
    frame_metal(p, (x, y, w, h));
    p.fill((x, y, 1, h), PALETTE.outline);
    p.fill((x + 1, y, 1, h), PALETTE.edge_light);
    p.fill((x + w - 1, y, 1, h), PALETTE.edge_dark);

    let (x, y, w, h) = rect(PL_RIGHT_TILE);
    frame_metal(p, (x, y, w, h));
    p.fill((x, y, 1, h), PALETTE.edge_light);
    p.fill((x + 5, y, 8, h), PALETTE.groove);
    p.fill((x + 4, y, 1, h), PALETTE.edge_dark);
    p.fill((x + 13, y, 1, h), PALETTE.edge_light);
    p.fill((x + w - 2, y, 1, h), PALETTE.edge_dark);
    p.fill((x + w - 1, y, 1, h), PALETTE.outline);
}

/// A piece of the bottom edge: metal whose top row is the track list's
/// lower edge, with the window's outline along the bottom.
fn bottom_piece(p: &mut Paint, (x, y, w, h): Rect) {
    frame_metal(p, (x, y, w, h));
    p.fill((x, y, w, 1), PALETTE.edge_light);
    p.fill((x, y + h - 2, w, 1), PALETTE.edge_dark);
    p.fill((x, y + h - 1, w, 1), PALETTE.outline);
}

fn bottom(p: &mut Paint) {
    bottom_piece(p, rect(PL_BOTTOM_TILE));

    // Bottom left: the ADD, REM, SEL and MISC buttons.
    let (x, y, w, h) = rect(PL_BOTTOM_LEFT);
    bottom_piece(p, (x, y, w, h));
    p.fill((x, y, 11, 1), PALETTE.face_bottom);
    p.fill((x, y, 1, h), PALETTE.outline);
    p.fill((x + 1, y, 1, h - 1), PALETTE.edge_light);
    p.fill((x + 11, y, 1, 1), PALETTE.edge_dark);
    for (caption, bx) in [("ADD", 14), ("REM", 43), ("SEL", 72), ("MISC", 101)] {
        parts::caption_button(p, (x + bx, y + 8, 22, 18), caption, false);
    }

    // Bottom right: the running time, tiny transport buttons, the
    // track time, the LIST button, scroll arrows and the resize grip.
    let (x, y, w, h) = rect(PL_BOTTOM_RIGHT);
    bottom_piece(p, (x, y, w, h));
    p.fill((x + 131, y, w - 131, 1), PALETTE.face_bottom);
    p.fill((x + 135, y, 8, 15), PALETTE.groove);
    p.fill((x + w - 1, y, 1, h), PALETTE.outline);
    p.fill((x + w - 2, y, 1, h - 1), PALETTE.edge_dark);
    p.lcd_well((x + 6, y + 9, 92, 8));
    p.lcd_well((x + 65, y + 22, 32, 8));
    let symbols: [(&[&str], i32); 6] = [
        (&["#..#", "#.##", "####", "#.##", "#..#"], 6),
        (&["#..", "##.", "###", "##.", "#.."], 15),
        (&["#.#", "#.#", "#.#", "#.#", "#.#"], 24),
        (&["####", "####", "####", "####"], 34),
        (&["#..#", "##.#", "####", "##.#", "#..#"], 43),
        (&["..#..", ".###.", "#####", ".....", "#####"], 53),
    ];
    for (rows, sx) in symbols {
        p.shape(rows, x + sx + 1, y + 25, PALETTE.outline);
        p.shape(rows, x + sx, y + 24, PALETTE.engraved);
    }
    parts::caption_button(p, (x + 106, y + 8, 22, 18), "LIST", false);
    // Scroll up and down arrows.
    p.shape(&["..#..", ".###.", "#####"], x + 136, y + 3, PALETTE.engraved);
    p.shape(&["#####", ".###.", "..#.."], x + 136, y + 9, PALETTE.engraved);
    // The resize grip: diagonal ridges in the corner.
    for i in 0..4 {
        let offset = 4 + i * 4;
        for d in 0..offset {
            p.set(x + w - 3 - d, y + h - 3 - offset + d, PALETTE.edge_light);
            p.set(x + w - 3 - d, y + h - 2 - offset + d, PALETTE.edge_dark);
        }
    }

    // The spare panel shown in wide windows: a screen for the visualizer,
    // where the playlist window draws it.
    let (x, y, w, h) = rect(PL_VISUALIZER_BACKGROUND);
    bottom_piece(p, (x, y, w, h));
    p.lcd_well((x + 2, y + 12, 72, 16));
}

/// The playlist shrunk to a strip: a display band for the current track,
/// with the buttons and a resize grip at the right.
fn shade(p: &mut Paint) {
    let strip = |p: &mut Paint, (x, y, w, h): Rect| {
        frame_metal(p, (x, y, w, h));
        p.gradient((x, y, w, h), lighten(PALETTE.face_top, 0.08), PALETTE.face_top);
        p.fill((x, y, w, 1), PALETTE.outline);
        p.fill((x, y + 1, w, 1), PALETTE.edge_light);
        p.fill((x, y + h - 1, w, 1), PALETTE.outline);
    };
    let (x, y, w, h) = rect(PL_SHADE_LEFT);
    strip(p, (x, y, w, h));
    p.fill((x, y, 1, h), PALETTE.outline);
    p.lcd_well((x + 3, y + 3, w - 3, 8));

    let (x, y, w, h) = rect(PL_SHADE_TILE);
    strip(p, (x, y, w, h));
    p.fill((x, y + 2, w, 1), PALETTE.edge_dark);
    p.fill((x, y + 3, w, 8), PALETTE.lcd_background);
    p.fill((x, y + 11, w, 1), PALETTE.edge_light);
    // The left piece's well is open on the right, like the tile's.
    let (lx, ly, lw, _) = rect(PL_SHADE_LEFT);
    p.fill((lx + lw - 1, ly + 3, 1, 8), PALETTE.lcd_background);

    for (sprite, active) in [(PL_SHADE_RIGHT, false), (PL_SHADE_RIGHT_SELECTED, true)] {
        let (x, y, w, h) = rect(sprite);
        strip(p, (x, y, w, h));
        p.fill((x + w - 1, y, 1, h), PALETTE.outline);
        p.fill((x, y + 2, 20, 1), PALETTE.edge_dark);
        p.fill((x, y + 3, 20, 8), PALETTE.lcd_background);
        p.fill((x, y + 11, 20, 1), PALETTE.edge_light);
        p.fill((x + 20, y + 3, 1, 8), PALETTE.edge_light);
        // The resize grip.
        let ridge = if active { darken(PALETTE.title_active, 0.2) } else { PALETTE.edge_light };
        for gx in [22, 24, 26] {
            p.fill((x + gx, y + 3, 1, 8), ridge);
            p.fill((x + gx + 1, y + 3, 1, 8), PALETTE.outline);
        }
        parts::small_button(p, (x + 29, y + 3), Icon::Unshade, false);
        parts::small_button(p, (x + 39, y + 3), Icon::Close, false);
    }
}

/// The pop-up menus of the bottom buttons: each choice plain and
/// highlighted, and the bar beside each menu.
fn menus(p: &mut Paint) {
    let items: [(Sprite, &[&str]); 16] = [
        (PL_ADD_URL, &["URL"]),
        (PL_ADD_DIR, &["DIR"]),
        (PL_ADD_FILE, &["FILE"]),
        (PL_REMOVE_ALL, &["REM", "ALL"]),
        (PL_CROP, &["CROP"]),
        (PL_REMOVE_SELECTED, &["REM", "SEL"]),
        (PL_REMOVE_MISC, &["MISC"]),
        (PL_INVERT_SELECTION, &["INV", "SEL"]),
        (PL_SELECT_ZERO, &["SEL", "NONE"]),
        (PL_SELECT_ALL, &["SEL", "ALL"]),
        (PL_SORT_LIST, &["SORT"]),
        (PL_FILE_INFO, &["INFO"]),
        (PL_MISC_OPTIONS, &["OPTS"]),
        (PL_NEW_LIST, &["NEW"]),
        (PL_SAVE_LIST, &["SAVE"]),
        (PL_LOAD_LIST, &["LOAD"]),
    ];
    for (sprite, lines) in items {
        menu_item(p, rect(sprite), lines, false);
        menu_item(p, rect(pl_menu_selected(sprite)), lines, true);
    }
    for bar in [PL_ADD_MENU_BAR, PL_REMOVE_MENU_BAR, PL_SELECT_MENU_BAR, PL_MISC_MENU_BAR, PL_LIST_MENU_BAR] {
        let (x, y, w, h) = rect(bar);
        p.fill((x, y, w, h), PALETTE.outline);
        p.fill((x + 1, y + 1, 1, h - 2), PALETTE.led_on);
    }
}

fn menu_item(p: &mut Paint, (x, y, w, h): Rect, lines: &[&str], highlighted: bool) {
    let text = if highlighted {
        p.gradient((x, y, w, h), lighten(PALETTE.led_on, 0.2), darken(PALETTE.led_on, 0.2));
        p.edge((x, y, w, h), PALETTE.outline, PALETTE.outline);
        PALETTE.outline
    } else {
        p.button((x, y, w, h), false);
        PALETTE.button_symbol
    };
    let font = &glyphs::SMALL;
    let total = lines.len() as i32 * 5 + (lines.len() as i32 - 1) * 2;
    for (i, line) in lines.iter().enumerate() {
        let top = y + (h - total) / 2 + i as i32 * 7;
        p.text(font, line, x + (w - font.width(line)) / 2, top, text);
    }
}
