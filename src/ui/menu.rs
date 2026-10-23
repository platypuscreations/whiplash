//! A small pop-up menu of text choices, drawn in its own borderless window
//! and styled like a classic Windows menu (as Winamp's were).
//!
//! Sizes below are in screen points. Menus are drawn at the screen's full
//! resolution, so every size is multiplied by `scale` (2 on a Retina
//! display) and the font is loaded at `FONT_PX * scale`.

use super::canvas::Canvas;
use super::text::Font;

pub const FONT_PX: f32 = 11.0;
const ITEM_HEIGHT: i32 = 18;
const SEPARATOR_HEIGHT: i32 = 7;
/// Space on the left for checkmarks.
const GUTTER: i32 = 20;
const PADDING_RIGHT: i32 = 12;
/// Space between a label and its keyboard shortcut.
const SHORTCUT_GAP: i32 = 24;
const BORDER: i32 = 2;

const BACKGROUND: u32 = 0xD4D0C8;
const TEXT: u32 = 0x000000;
const HIGHLIGHT: u32 = 0x0A246A;
const HIGHLIGHT_TEXT: u32 = 0xFFFFFF;
const LIGHT_EDGE: u32 = 0xFFFFFF;
const DARK_EDGE: u32 = 0x808080;

/// Scales a size in points to pixels.
fn px(points: i32, scale: f64) -> i32 {
    (points as f64 * scale).round() as i32
}

/// Something a menu choice does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    // Playlist
    SortByTitle,
    SortByFileName,
    SortByPath,
    ReverseList,
    RandomizeList,
    RemoveMissingFiles,
    // Main options
    PlayFiles,
    PlayFolder,
    PlayerMode(crate::config::PlayerMode),
    ServerSettings,
    SkinBrowser,
    ToggleEqualizer,
    TogglePlaylist,
    ToggleDoubleSize,
    ToggleAlwaysOnTop,
    Visualizer(crate::vis::Mode),
    ToggleMilkDrop,
    ChoosePresetsFolder,
    FileInfo,
    About,
    Exit,
    // Skin browser
    /// One of the skins listed in the browser, by its place in the list.
    Skin(usize),
    /// Whiplash's own skin.
    BuiltInSkin,
    LoadSkinFile,
    OpenSkinsFolder,
    // Equalizer presets
    LoadPresetFile,
    SavePresetFile,
    /// One of the built-in presets, by its place in the list.
    Preset(usize),
    SaveAutoPreset,
    DeleteAutoPreset,
}

pub struct Item {
    pub label: String,
    pub shortcut: String,
    /// `None` makes this a separator line.
    pub action: Option<Action>,
    pub checked: bool,
}

impl Item {
    pub fn new(label: impl Into<String>, action: Action) -> Item {
        Item { label: label.into(), shortcut: String::new(), action: Some(action), checked: false }
    }

    pub fn separator() -> Item {
        Item { label: String::new(), shortcut: String::new(), action: None, checked: false }
    }

    pub fn checked(mut self, checked: bool) -> Item {
        self.checked = checked;
        self
    }

    pub fn shortcut(mut self, keys: &str) -> Item {
        self.shortcut = keys.to_string();
        self
    }

    fn height(&self, scale: f64) -> i32 {
        px(if self.action.is_some() { ITEM_HEIGHT } else { SEPARATOR_HEIGHT }, scale)
    }
}

pub struct Menu {
    pub items: Vec<Item>,
    /// The choice under the mouse.
    pub hovered: Option<usize>,
}

impl Menu {
    pub fn new(items: Vec<Item>) -> Menu {
        Menu { items, hovered: None }
    }

    /// The size the menu needs, in pixels.
    pub fn size(&self, font: &mut Font, scale: f64) -> (u32, u32) {
        let labels = self.items.iter().map(|i| font.width(&i.label)).max().unwrap_or(0);
        let shortcuts = self.items.iter().map(|i| font.width(&i.shortcut)).max().unwrap_or(0);
        let gap = if shortcuts > 0 { px(SHORTCUT_GAP, scale) } else { 0 };
        let height: i32 = self.items.iter().map(|i| i.height(scale)).sum();
        let frame = px(GUTTER + PADDING_RIGHT + 2 * BORDER, scale);
        ((labels + gap + shortcuts + frame) as u32, (height + px(2 * BORDER, scale)) as u32)
    }

    /// The choice at `y` pixels from the top, if any (separators can't be
    /// chosen).
    pub fn item_at(&self, y: i32, scale: f64) -> Option<usize> {
        let mut top = px(BORDER, scale);
        for (i, item) in self.items.iter().enumerate() {
            if y >= top && y < top + item.height(scale) {
                return item.action.map(|_| i);
            }
            top += item.height(scale);
        }
        None
    }

    pub fn action(&self, index: usize) -> Option<Action> {
        self.items.get(index)?.action
    }

    pub fn draw(&self, canvas: &mut Canvas, font: &mut Font, scale: f64) {
        let (width, height) = (canvas.width as i32, canvas.height as i32);
        let line = px(1, scale).max(1) as u32;
        let (border, item_height) = (px(BORDER, scale), px(ITEM_HEIGHT, scale));
        canvas.fill(BACKGROUND);
        // A raised 3D edge: light on the top and left, dark on the bottom and right.
        canvas.fill_rect(0, 0, width as u32, line, LIGHT_EDGE);
        canvas.fill_rect(0, 0, line, height as u32, LIGHT_EDGE);
        canvas.fill_rect(0, height - line as i32, width as u32, line, DARK_EDGE);
        canvas.fill_rect(width - line as i32, 0, line, height as u32, DARK_EDGE);

        let mut top = border;
        for (i, item) in self.items.iter().enumerate() {
            if item.action.is_none() {
                // An etched line: dark above light.
                let y = top + item.height(scale) / 2;
                let (x, w) = (border + line as i32, (width - 2 * border - 2 * line as i32) as u32);
                canvas.fill_rect(x, y - line as i32, w, line, DARK_EDGE);
                canvas.fill_rect(x, y, w, line, LIGHT_EDGE);
                top += item.height(scale);
                continue;
            }
            let color = if self.hovered == Some(i) {
                canvas.fill_rect(border, top, (width - 2 * border) as u32, item_height as u32, HIGHLIGHT);
                HIGHLIGHT_TEXT
            } else {
                TEXT
            };
            if item.checked {
                draw_check(canvas, border + px(6, scale), top + px(6, scale), color, scale);
            }
            font.draw(canvas, &item.label, border + px(GUTTER, scale), top, item_height, color, (0, width));
            if !item.shortcut.is_empty() {
                let x = width - border - px(PADDING_RIGHT, scale) - font.width(&item.shortcut);
                font.draw(canvas, &item.shortcut, x, top, item_height, color, (0, width));
            }
            top += item_height;
        }
    }
}

/// A small checkmark, 7 points wide, drawn as blocks of pixels.
fn draw_check(canvas: &mut Canvas, x: i32, y: i32, color: u32, scale: f64) {
    const ROWS: [&str; 7] = ["      #", "     ##", "#   ###", "## ### ", "#####  ", " ###   ", "  #    "];
    let size = scale.round().max(1.0) as i32;
    for (dy, row) in ROWS.iter().enumerate() {
        for (dx, c) in row.chars().enumerate() {
            if c == '#' {
                canvas.fill_rect(x + dx as i32 * size, y + dy as i32 * size, size as u32, size as u32, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_items_by_height_and_skips_separators() {
        let menu = Menu::new(vec![
            Item::new("One", Action::ReverseList),
            Item::separator(),
            Item::new("Two", Action::RandomizeList),
        ]);
        assert_eq!(menu.item_at(BORDER, 1.0), Some(0));
        assert_eq!(menu.item_at(BORDER + ITEM_HEIGHT, 1.0), None); // the separator
        assert_eq!(menu.item_at(BORDER + ITEM_HEIGHT + SEPARATOR_HEIGHT, 1.0), Some(2));
        assert_eq!(menu.item_at(BORDER + 2 * ITEM_HEIGHT + SEPARATOR_HEIGHT, 1.0), None);
        assert_eq!(menu.item_at(-5, 1.0), None);
        // On a Retina screen everything is twice as many pixels.
        assert_eq!(menu.item_at(2 * (BORDER + ITEM_HEIGHT + SEPARATOR_HEIGHT), 2.0), Some(2));
        assert_eq!(menu.item_at(BORDER + ITEM_HEIGHT + SEPARATOR_HEIGHT, 2.0), Some(0));
        assert_eq!(menu.action(2), Some(Action::RandomizeList));
    }
}
