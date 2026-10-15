//! Tooltips: a small box of text naming the control under the mouse,
//! styled like classic Windows tooltips (as Winamp's were).
//!
//! Unlike the skinned windows, tooltips are drawn at the screen's full
//! resolution (`scale` is 2 on a Retina display), so their text is sharp
//! instead of enlarged pixel by pixel.

use super::canvas::Canvas;
use super::text::Font;

/// Font size in screen points; multiply by `scale` for pixels.
pub const FONT_PX: f32 = 10.0;

const BACKGROUND: u32 = 0xFFFFE1;
const BORDER: u32 = 0x000000;
const TEXT: u32 = 0x000000;
const PADDING_X: i32 = 4;
const HEIGHT: i32 = 15;

/// Scales a size in points to pixels.
fn px(points: i32, scale: f64) -> i32 {
    (points as f64 * scale).round() as i32
}

/// The size a tooltip needs for `text`, in pixels, given a font loaded at
/// `FONT_PX * scale`.
pub fn size(font: &mut Font, text: &str, scale: f64) -> (u32, u32) {
    ((font.width(text) + px(2 * PADDING_X + 2, scale)) as u32, px(HEIGHT, scale) as u32)
}

pub fn draw(canvas: &mut Canvas, font: &mut Font, text: &str, scale: f64) {
    let (width, height) = (canvas.width as i32, canvas.height as i32);
    let border = px(1, scale).max(1);
    canvas.fill(BORDER);
    canvas.fill_rect(border, border, (width - 2 * border) as u32, (height - 2 * border) as u32, BACKGROUND);
    let left = px(1 + PADDING_X, scale);
    font.draw(canvas, text, left, 0, height, TEXT, (border, width - border));
}
