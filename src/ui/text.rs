//! Drawing text with the computer's own fonts, for the playlist and menus.
//!
//! The main window uses the skin's bitmap font, but Winamp's playlist uses
//! a real font named in the skin's `pledit.txt` (usually Arial). We find it
//! among the system's fonts with `fontdb`, read its outlines with `skrifa`
//! and turn them into pixels with `zeno`.
//!
//! Small text is fitted to whole pixels by the font's hinting, its own
//! instructions for that. We use the strong kind, which moves letters
//! across as well as up and down, as Windows did before ClearType: every
//! stroke lands on a pixel, and the text stays sharp on screens without
//! HiDPI. (Hinting meant for smoothed text only moves letters up and down,
//! which leaves upright strokes smeared across two pixels.)

use std::collections::HashMap;
use std::sync::OnceLock;

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, HintingInstance, HintingOptions, OutlinePen, Target};
use skrifa::{FontRef, MetadataProvider};
use zeno::{Command, Format, Mask};

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
    /// The font file (or the collection it's in), and which font in it.
    data: Vec<u8>,
    index: u32,
    px: f32,
    /// Above and below the baseline, in pixels.
    ascent: f32,
    descent: f32,
    /// The font's hinting at this size; none if it can't be run.
    hinting: Option<HintingInstance>,
    glyphs: HashMap<char, Glyph>,
}

/// One letter's pixels.
#[derive(Clone)]
struct Glyph {
    /// Where its pixels start: right of the pen, and up from the baseline.
    left: i32,
    top: i32,
    width: usize,
    height: usize,
    coverage: Vec<u8>,
    /// How far the pen moves on, in whole pixels so hinted letters stay on
    /// the pixel grid.
    advance: i32,
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

        let (data, index) = database.with_face_data(id, |data, index| {
            FontRef::from_index(data, index).ok()?;
            Some((data.to_vec(), index))
        })??;
        let font = FontRef::from_index(&data, index).ok()?;
        let metrics = font.metrics(Size::new(px), LocationRef::default());
        let (ascent, descent) = (metrics.ascent, -metrics.descent);
        let options = HintingOptions::from(Target::Mono);
        let hinting = HintingInstance::new(&font.outline_glyphs(), Size::new(px), LocationRef::default(), options).ok();
        Some(Font { data, index, px, ascent, descent, hinting, glyphs: HashMap::new() })
    }

    fn glyph(&mut self, c: char) -> &Glyph {
        let Font { data, index, px, hinting, glyphs, .. } = self;
        glyphs.entry(c).or_insert_with(|| {
            let font = FontRef::from_index(data, *index).expect("checked when loaded");
            let id = font.charmap().map(c).unwrap_or_default();
            let size = Size::new(*px);
            let advance = font.glyph_metrics(size, LocationRef::default()).advance_width(id).unwrap_or(0.0);
            let mut outline = Outline(Vec::new());
            let drawn = font.outline_glyphs().get(id).and_then(|glyph| {
                let settings = match hinting {
                    Some(hinting) => DrawSettings::hinted(hinting, false),
                    None => DrawSettings::unhinted(size, LocationRef::default()),
                };
                glyph.draw(settings, &mut outline).ok()
            });
            // Hinting may widen or narrow a letter, to whole pixels.
            let advance = drawn.and_then(|m| m.advance_width).unwrap_or(advance).round() as i32;
            if outline.0.is_empty() {
                // A letter with no outline, like a space.
                return Glyph { left: 0, top: 0, width: 0, height: 0, coverage: Vec::new(), advance };
            }
            let (coverage, placement) = Mask::new(&outline.0[..]).format(Format::Alpha).render();
            Glyph {
                left: placement.left,
                top: -placement.top,
                width: placement.width as usize,
                height: placement.height as usize,
                coverage,
                advance,
            }
        })
    }

    /// Width of `text` in pixels.
    pub fn width(&mut self, text: &str) -> i32 {
        text.chars().map(|c| self.glyph(c).advance).sum()
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
        let text_height = self.ascent + self.descent;
        let baseline = top + ((line_height as f32 - text_height) / 2.0 + self.ascent).round() as i32;
        let mut pen = x;
        for c in text.chars() {
            let glyph = self.glyph(c).clone();
            let left = pen + glyph.left;
            let glyph_top = baseline - glyph.top;
            for row in 0..glyph.height {
                for col in 0..glyph.width {
                    let alpha = glyph.coverage[row * glyph.width + col];
                    let (px, py) = (left + col as i32, glyph_top + row as i32);
                    if alpha > 0 && px >= clip_left && px < clip_right {
                        canvas.blend(px, py, color, alpha);
                    }
                }
            }
            pen += glyph.advance;
        }
    }
}

/// Collects a letter's outline for `zeno`, turned the right way up: fonts
/// count up from the baseline, pixels count down.
struct Outline(Vec<Command>);

impl OutlinePen for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(Command::MoveTo([x, -y].into()));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(Command::LineTo([x, -y].into()));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0.push(Command::QuadTo([cx0, -cy0].into(), [x, -y].into()));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.push(Command::CurveTo([cx0, -cy0].into(), [cx1, -cy1].into(), [x, -y].into()));
    }

    fn close(&mut self) {
        self.0.push(Command::Close);
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
