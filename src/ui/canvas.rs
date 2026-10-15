//! An off-screen pixel buffer that windows are drawn into at their original
//! size (e.g. 275x116), before being scaled up onto the screen.

use crate::skin::{Skin, Sprite};

/// Drawn where a skin is missing a sprite sheet.
const PLACEHOLDER: u32 = 0x3C3C46;

pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Canvas {
        Canvas { width, height, pixels: vec![0; (width * height) as usize] }
    }

    pub fn fill(&mut self, color: u32) {
        self.pixels.fill(color);
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: u32) {
        for row in 0..h as i32 {
            for col in 0..w as i32 {
                self.set(x + col, y + row, color);
            }
        }
    }

    fn set(&mut self, x: i32, y: i32, color: u32) {
        if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
            self.pixels[(y as u32 * self.width + x as u32) as usize] = color;
        }
    }

    /// Copies `sprite` from the skin so its top-left corner lands at (x, y).
    /// Parts outside the canvas or outside a too-small sprite sheet are
    /// skipped. If the skin lacks the sheet entirely, a flat placeholder
    /// rectangle is drawn instead.
    pub fn draw(&mut self, skin: &Skin, sprite: Sprite, x: i32, y: i32) {
        let Some(sheet) = skin.sheet(sprite.sheet) else {
            self.fill_rect(x, y, sprite.w, sprite.h, PLACEHOLDER);
            return;
        };
        for row in 0..sprite.h {
            for col in 0..sprite.w {
                if let Some(color) = sheet.get(sprite.x + col, sprite.y + row) {
                    self.set(x + col as i32, y + row as i32, color);
                }
            }
        }
    }

    /// Copies this canvas onto a larger screen buffer, enlarging each pixel
    /// into a `scale` x `scale` block (nearest-neighbour, so pixel art stays
    /// crisp). Screen pixels outside the enlarged image are painted black.
    pub fn scale_into(&self, screen: &mut [u32], screen_width: u32, screen_height: u32, scale: u32) {
        for sy in 0..screen_height {
            let row = &mut screen[(sy * screen_width) as usize..((sy + 1) * screen_width) as usize];
            let cy = sy / scale;
            for (sx, out) in row.iter_mut().enumerate() {
                let cx = sx as u32 / scale;
                *out = if cx < self.width && cy < self.height {
                    self.pixels[(cy * self.width + cx) as usize]
                } else {
                    0
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_sheet_draws_placeholder_clipped_to_canvas() {
        let mut canvas = Canvas::new(4, 4);
        canvas.draw(&Skin::empty(), crate::skin::PLAY_BUTTON, 2, 2);
        assert_eq!(canvas.pixels[0], 0);
        assert_eq!(canvas.pixels[(3 * 4 + 3) as usize], PLACEHOLDER);
    }

    #[test]
    fn scaling_doubles_pixels() {
        let mut canvas = Canvas::new(2, 1);
        canvas.pixels = vec![1, 2];
        let mut screen = vec![9; 5 * 2];
        canvas.scale_into(&mut screen, 5, 2, 2);
        assert_eq!(screen, vec![1, 1, 2, 2, 0, 1, 1, 2, 2, 0]);
    }
}
