//! A tiny drawing surface for painting the built-in skin's sprite sheets.

use std::io::Cursor;

use super::glyphs::Font;
use super::palette::PALETTE;

/// A rectangle: x, y, width, height.
pub type Rect = (i32, i32, i32, i32);

/// An image being painted, as 0xRRGGBB pixels.
pub struct Paint {
    pub width: u32,
    pub height: u32,
    pixels: Vec<u32>,
}

impl Paint {
    pub fn new(width: u32, height: u32, color: u32) -> Paint {
        Paint { width, height, pixels: vec![color; (width * height) as usize] }
    }

    /// Sets one pixel; points outside the image are ignored.
    pub fn set(&mut self, x: i32, y: i32, color: u32) {
        if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
            self.pixels[(y as u32 * self.width + x as u32) as usize] = color;
        }
    }

    pub fn fill(&mut self, (x, y, w, h): Rect, color: u32) {
        for py in y..y + h {
            for px in x..x + w {
                self.set(px, py, color);
            }
        }
    }

    /// A one-pixel outline, `light` along the top and left and `dark`
    /// along the bottom and right.
    pub fn edge(&mut self, (x, y, w, h): Rect, light: u32, dark: u32) {
        self.fill((x, y, w, 1), light);
        self.fill((x, y, 1, h), light);
        self.fill((x, y + h - 1, w, 1), dark);
        self.fill((x + w - 1, y, 1, h), dark);
    }

    /// The one-pixel edge of a raised part of the metal.
    pub fn raised(&mut self, rect: Rect) {
        self.edge(rect, PALETTE.edge_light, PALETTE.edge_dark);
    }

    /// The one-pixel edge of a part sunk into the metal.
    pub fn sunken(&mut self, rect: Rect) {
        self.edge(rect, PALETTE.edge_dark, PALETTE.edge_light);
    }

    /// Fades from `top` to `bottom` down the rectangle.
    pub fn gradient(&mut self, (x, y, w, h): Rect, top: u32, bottom: u32) {
        for row in 0..h {
            let t = if h > 1 { row as f32 / (h - 1) as f32 } else { 0.0 };
            self.fill((x, y + row, w, 1), mix(top, bottom, t));
        }
    }

    /// Brushed metal: the face gradient with faint horizontal streaks.
    /// (`wx`, `wy`) is where the rectangle's corner sits in its window, and
    /// `window_height` the window's height: pieces from different sheets
    /// then match where the app puts them side by side.
    pub fn metal(&mut self, (x, y, w, h): Rect, (wx, wy): (i32, i32), window_height: i32) {
        for row in 0..h {
            let t = ((wy + row) as f32 / (window_height - 1).max(1) as f32).clamp(0.0, 1.0);
            let base = mix(PALETTE.face_top, PALETTE.face_bottom, t);
            for col in 0..w {
                let streak = noise((wx + col).div_euclid(7), wy + row) % 7;
                let color = match streak {
                    0 => lighten(base, 0.06),
                    1 => darken(base, 0.06),
                    _ => base,
                };
                self.set(x + col, y + row, color);
            }
        }
    }

    /// A display panel: the background with a grid of faint dots.
    pub fn lcd(&mut self, (x, y, w, h): Rect) {
        self.fill((x, y, w, h), PALETTE.lcd_background);
        for row in (1..h).step_by(2) {
            for col in (0..w).step_by(2) {
                self.set(x + col, y + row, PALETTE.lcd_grid);
            }
        }
    }

    /// A display panel set into the metal, with a sunken edge around it.
    pub fn lcd_well(&mut self, (x, y, w, h): Rect) {
        self.sunken((x - 1, y - 1, w + 2, h + 2));
        self.fill((x, y, w, h), PALETTE.lcd_background);
    }

    /// A push button's face and edges.
    pub fn button(&mut self, (x, y, w, h): Rect, pressed: bool) {
        let (top, bottom) = if pressed {
            (darken(PALETTE.button_bottom, 0.15), darken(PALETTE.button_top, 0.15))
        } else {
            (PALETTE.button_top, PALETTE.button_bottom)
        };
        self.gradient((x, y, w, h), top, bottom);
        self.fill((x, y, w, 1), PALETTE.outline);
        self.fill((x, y + h - 1, w, 1), PALETTE.outline);
        self.fill((x, y, 1, h), PALETTE.outline);
        self.fill((x + w - 1, y, 1, h), PALETTE.outline);
        let inner = (x + 1, y + 1, w - 2, h - 2);
        if pressed {
            self.edge(inner, PALETTE.button_dark, lighten(bottom, 0.1));
        } else {
            self.edge(inner, PALETTE.button_light, PALETTE.button_dark);
        }
    }

    /// Draws text with its top-left corner at (x, y); only the letters'
    /// pixels are painted.
    pub fn text(&mut self, font: &Font, text: &str, x: i32, y: i32, color: u32) {
        let mut pen = x;
        for c in text.chars() {
            let rows = font.glyph(c);
            for (dy, row) in rows.iter().enumerate() {
                for (dx, cell) in row.bytes().enumerate() {
                    if cell == b'#' {
                        self.set(pen + dx as i32, y + dy as i32, color);
                    }
                }
            }
            pen += font.advance(c);
        }
    }

    /// Draws text centered in a rectangle.
    pub fn text_centered(&mut self, font: &Font, text: &str, (x, y, w, h): Rect, color: u32) {
        let left = x + (w - font.width(text)) / 2;
        let top = y + (h - font.height) / 2;
        self.text(font, text, left, top, color);
    }

    /// Draws a shape given as rows of `#` (painted) and `.` (left alone).
    pub fn shape(&mut self, rows: &[&str], x: i32, y: i32, color: u32) {
        for (dy, row) in rows.iter().enumerate() {
            for (dx, cell) in row.bytes().enumerate() {
                if cell == b'#' {
                    self.set(x + dx as i32, y + dy as i32, color);
                }
            }
        }
    }

    /// Encodes the image as a 24-bit BMP file.
    pub fn to_bmp(&self) -> Vec<u8> {
        let rgb: Vec<u8> = self.pixels.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
        let image = image::RgbImage::from_raw(self.width, self.height, rgb).expect("pixel count matches size");
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Bmp).expect("encoding a BMP in memory cannot fail");
        out.into_inner()
    }
}

/// Blends from color `a` (t = 0) to color `b` (t = 1).
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let channel = |shift: u32| {
        let (ca, cb) = ((a >> shift) & 0xFF, (b >> shift) & 0xFF);
        ((ca as f32 + (cb as f32 - ca as f32) * t).round() as u32) << shift
    };
    channel(16) | channel(8) | channel(0)
}

pub fn lighten(color: u32, amount: f32) -> u32 {
    mix(color, 0xFFFFFF, amount)
}

pub fn darken(color: u32, amount: f32) -> u32 {
    mix(color, 0x000000, amount)
}

/// A repeatable pseudo-random number for a position, so the skin comes out
/// the same every time.
fn noise(x: i32, y: i32) -> u32 {
    let mut n = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    n ^ (n >> 16)
}
