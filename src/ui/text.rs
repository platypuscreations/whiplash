//! Drawing text with the computer's own fonts, for the playlist and menus.
//!
//! The main window uses the skin's bitmap font, but Winamp's playlist uses
//! a real font named in the skin's `pledit.txt` (usually Arial). We find it
//! among the system's fonts with `fontdb` and turn letters into pixels with
//! `fontdue`.

use std::collections::HashMap;
use std::sync::OnceLock;

use fontdue::{FontSettings, Metrics};

use super::canvas::Canvas;

/// Fonts to try when the skin's font isn't installed (Arial rarely is on
/// Linux). The first one found wins.
const FALLBACK_FAMILIES: &[&str] = &["Arial", "Helvetica", "Liberation Sans", "DejaVu Sans", "Noto Sans"];

/// Every font installed on the computer. Scanning takes a moment, so it
/// happens once, the first time a font is needed.
fn system_fonts() -> &'static fontdb::Database {
    static DATABASE: OnceLock<fontdb::Database> = OnceLock::new();
    DATABASE.get_or_init(|| {
        let mut database = fontdb::Database::new();
        database.load_system_fonts();
        database
    })
}

/// A font at one size, with each letter's pixels kept after first use.
pub struct Font {
    font: fontdue::Font,
    px: f32,
    ascent: f32,
    descent: f32,
    glyphs: HashMap<char, (Metrics, Vec<u8>)>,
}

impl Font {
    /// Loads `family` at `px` pixels, or a similar font if it's missing.
    /// Returns `None` only if the computer has no usable fonts at all.
    pub fn load(family: &str, px: f32) -> Option<Font> {
        let database = system_fonts();
        let candidates = std::iter::once(family).chain(FALLBACK_FAMILIES.iter().copied());
        let id = candidates
            .filter_map(|name| {
                let families = [fontdb::Family::Name(name)];
                database.query(&fontdb::Query { families: &families, ..Default::default() })
            })
            .next()
            .or_else(|| database.faces().next().map(|face| face.id))?;

        let font = database.with_face_data(id, |data, index| {
            fontdue::Font::from_bytes(data, FontSettings { collection_index: index, ..Default::default() }).ok()
        })??;
        let line = font.horizontal_line_metrics(px)?;
        Some(Font { font, px, ascent: line.ascent, descent: line.descent, glyphs: HashMap::new() })
    }

    fn glyph(&mut self, c: char) -> &(Metrics, Vec<u8>) {
        let (font, px) = (&self.font, self.px);
        self.glyphs.entry(c).or_insert_with(|| font.rasterize(c, px))
    }

    /// Width of `text` in pixels.
    pub fn width(&mut self, text: &str) -> i32 {
        text.chars().map(|c| self.glyph(c).0.advance_width).sum::<f32>().round() as i32
    }

    /// Shortens `text` with "..." so it fits in `max_width` pixels.
    pub fn fit(&mut self, text: &str, max_width: i32) -> String {
        if self.width(text) <= max_width {
            return text.to_string();
        }
        let mut shortened: String = text.to_string();
        while !shortened.is_empty() {
            shortened.pop();
            let candidate = format!("{}...", shortened.trim_end());
            if self.width(&candidate) <= max_width {
                return candidate;
            }
        }
        String::new()
    }

    /// Draws `text` starting at `x`, centered vertically in the line of
    /// `line_height` pixels starting at `top`. Letters are blended into
    /// what's already on the canvas, and nothing is drawn outside the
    /// columns `clip_left..clip_right`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        canvas: &mut Canvas,
        text: &str,
        x: i32,
        top: i32,
        line_height: i32,
        color: u32,
        (clip_left, clip_right): (i32, i32),
    ) {
        let text_height = self.ascent - self.descent;
        let baseline = top + ((line_height as f32 - text_height) / 2.0 + self.ascent).round() as i32;
        let mut pen = x as f32;
        for c in text.chars() {
            let (metrics, coverage) = self.glyph(c).clone();
            let left = pen.round() as i32 + metrics.xmin;
            let glyph_top = baseline - metrics.height as i32 - metrics.ymin;
            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let alpha = coverage[row * metrics.width + col];
                    let (px, py) = (left + col as i32, glyph_top + row as i32);
                    if alpha > 0 && px >= clip_left && px < clip_right {
                        canvas.blend(px, py, color, alpha);
                    }
                }
            }
            pen += metrics.advance_width;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_and_shortens_text() {
        // Any machine running the tests has at least one font installed.
        let mut font = Font::load("Arial", 9.0).expect("no fonts found");
        let wide = font.width("WWWWWWWWWW");
        assert!(wide > font.width("iiiiiiiiii"));
        let fitted = font.fit("A long song title that will not fit", 60);
        assert!(fitted.ends_with("..."), "{fitted}");
        assert!(font.width(&fitted) <= 60);
        assert_eq!(font.fit("short", 60), "short");
    }

    #[test]
    fn missing_fonts_fall_back() {
        assert!(Font::load("A Font Nobody Has Installed", 9.0).is_some());
    }

    #[test]
    fn draws_inside_the_clip_only() {
        let mut font = Font::load("Arial", 12.0).unwrap();
        let mut canvas = Canvas::new(40, 20);
        font.draw(&mut canvas, "MMMMMMMM", 0, 0, 20, 0xFFFFFF, (0, 10));
        let lit = |x: u32| (0..20).any(|y| canvas.pixels[(y * 40 + x) as usize] != 0);
        assert!((0..10).any(lit));
        assert!(!(10..40).any(lit));
    }
}
