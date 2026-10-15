//! A small pop-up menu of text choices, drawn in its own borderless window
//! and styled like a classic Windows menu (as Winamp's were).

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
    SkinBrowser,
    ToggleEqualizer,
    TogglePlaylist,
    ToggleDoubleSize,
    ToggleAlwaysOnTop,
    Visualizer(crate::vis::Mode),
    FileInfo,
    About,
    Exit,
    // Skin browser
    /// One of the skins listed in the browser, by its place in the list.
    Skin(usize),
    BaseSkin,
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

    fn height(&self) -> i32 {
        if self.action.is_some() { ITEM_HEIGHT } else { SEPARATOR_HEIGHT }
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
    pub fn size(&self, font: &mut Font) -> (u32, u32) {
        let labels = self.items.iter().map(|i| font.width(&i.label)).max().unwrap_or(0);
        let shortcuts = self.items.iter().map(|i| font.width(&i.shortcut)).max().unwrap_or(0);
        let gap = if shortcuts > 0 { SHORTCUT_GAP } else { 0 };
        let height: i32 = self.items.iter().map(Item::height).sum();
        ((GUTTER + labels + gap + shortcuts + PADDING_RIGHT + 2 * BORDER) as u32, (height + 2 * BORDER) as u32)
    }

    /// The choice at height `y`, if any (separators can't be chosen).
    pub fn item_at(&self, y: i32) -> Option<usize> {
        let mut top = BORDER;
        for (i, item) in self.items.iter().enumerate() {
            if y >= top && y < top + item.height() {
                return item.action.map(|_| i);
            }
            top += item.height();
        }
        None
    }

    pub fn action(&self, index: usize) -> Option<Action> {
        self.items.get(index)?.action
    }

    pub fn draw(&self, canvas: &mut Canvas, font: &mut Font) {
        let (width, height) = (canvas.width as i32, canvas.height as i32);
        canvas.fill(BACKGROUND);
        // A raised 3D edge: light on the top and left, dark on the bottom and right.
        canvas.fill_rect(0, 0, width as u32, 1, LIGHT_EDGE);
        canvas.fill_rect(0, 0, 1, height as u32, LIGHT_EDGE);
        canvas.fill_rect(0, height - 1, width as u32, 1, DARK_EDGE);
        canvas.fill_rect(width - 1, 0, 1, height as u32, DARK_EDGE);

        let mut top = BORDER;
        for (i, item) in self.items.iter().enumerate() {
            if item.action.is_none() {
                // An etched line: dark above light.
                let y = top + SEPARATOR_HEIGHT / 2;
                canvas.fill_rect(BORDER + 1, y - 1, (width - 2 * BORDER - 2) as u32, 1, DARK_EDGE);
                canvas.fill_rect(BORDER + 1, y, (width - 2 * BORDER - 2) as u32, 1, LIGHT_EDGE);
                top += SEPARATOR_HEIGHT;
                continue;
            }
            let color = if self.hovered == Some(i) {
                canvas.fill_rect(BORDER, top, (width - 2 * BORDER) as u32, ITEM_HEIGHT as u32, HIGHLIGHT);
                HIGHLIGHT_TEXT
            } else {
                TEXT
            };
            if item.checked {
                draw_check(canvas, BORDER + 6, top + 6, color);
            }
            font.draw(canvas, &item.label, BORDER + GUTTER, top, ITEM_HEIGHT, color, (0, width));
            if !item.shortcut.is_empty() {
                let x = width - BORDER - PADDING_RIGHT - font.width(&item.shortcut);
                font.draw(canvas, &item.shortcut, x, top, ITEM_HEIGHT, color, (0, width));
            }
            top += ITEM_HEIGHT;
        }
    }
}

/// A small checkmark, 7 pixels wide, drawn pixel by pixel.
fn draw_check(canvas: &mut Canvas, x: i32, y: i32, color: u32) {
    const ROWS: [&str; 7] = ["      #", "     ##", "#   ###", "## ### ", "#####  ", " ###   ", "  #    "];
    for (dy, row) in ROWS.iter().enumerate() {
        for (dx, c) in row.chars().enumerate() {
            if c == '#' {
                canvas.set(x + dx as i32, y + dy as i32, color);
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
        assert_eq!(menu.item_at(BORDER), Some(0));
        assert_eq!(menu.item_at(BORDER + ITEM_HEIGHT), None); // the separator
        assert_eq!(menu.item_at(BORDER + ITEM_HEIGHT + SEPARATOR_HEIGHT), Some(2));
        assert_eq!(menu.item_at(BORDER + 2 * ITEM_HEIGHT + SEPARATOR_HEIGHT), None);
        assert_eq!(menu.item_at(-5), None);
        assert_eq!(menu.action(2), Some(Action::RandomizeList));
    }
}
