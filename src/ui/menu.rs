//! A small pop-up menu of text choices, drawn in its own borderless window
//! and styled like a classic Windows menu (as Winamp's were).

use super::canvas::Canvas;
use super::text::Font;

pub const FONT_PX: f32 = 11.0;
const ITEM_HEIGHT: i32 = 18;
const PADDING_X: i32 = 12;
const BORDER: i32 = 2;

const BACKGROUND: u32 = 0xD4D0C8;
const TEXT: u32 = 0x000000;
const HIGHLIGHT: u32 = 0x0A246A;
const HIGHLIGHT_TEXT: u32 = 0xFFFFFF;
const LIGHT_EDGE: u32 = 0xFFFFFF;
const DARK_EDGE: u32 = 0x808080;

/// Something a menu choice does. More are added as menus are needed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    SortByTitle,
    SortByFileName,
    SortByPath,
    ReverseList,
    RandomizeList,
    RemoveMissingFiles,
}

pub struct Menu {
    pub items: Vec<(String, Action)>,
    /// The choice under the mouse.
    pub hovered: Option<usize>,
}

impl Menu {
    pub fn new(items: &[(&str, Action)]) -> Menu {
        Menu { items: items.iter().map(|(label, action)| (label.to_string(), *action)).collect(), hovered: None }
    }

    /// The size the menu needs, in pixels.
    pub fn size(&self, font: &mut Font) -> (u32, u32) {
        let widest = self.items.iter().map(|(label, _)| font.width(label)).max().unwrap_or(0);
        ((widest + 2 * PADDING_X + 2 * BORDER) as u32, (self.items.len() as i32 * ITEM_HEIGHT + 2 * BORDER) as u32)
    }

    /// The choice at height `y`, if any.
    pub fn item_at(&self, y: i32) -> Option<usize> {
        let index = (y - BORDER).div_euclid(ITEM_HEIGHT);
        (index >= 0 && (index as usize) < self.items.len()).then_some(index as usize)
    }

    pub fn draw(&self, canvas: &mut Canvas, font: &mut Font) {
        let (width, height) = (canvas.width as i32, canvas.height as i32);
        canvas.fill(BACKGROUND);
        // A raised 3D edge: light on the top and left, dark on the bottom and right.
        canvas.fill_rect(0, 0, width as u32, 1, LIGHT_EDGE);
        canvas.fill_rect(0, 0, 1, height as u32, LIGHT_EDGE);
        canvas.fill_rect(0, height - 1, width as u32, 1, DARK_EDGE);
        canvas.fill_rect(width - 1, 0, 1, height as u32, DARK_EDGE);

        for (i, (label, _)) in self.items.iter().enumerate() {
            let top = BORDER + i as i32 * ITEM_HEIGHT;
            let color = if self.hovered == Some(i) {
                canvas.fill_rect(BORDER, top, (width - 2 * BORDER) as u32, ITEM_HEIGHT as u32, HIGHLIGHT);
                HIGHLIGHT_TEXT
            } else {
                TEXT
            };
            font.draw(canvas, label, BORDER + PADDING_X, top, ITEM_HEIGHT, color, (0, width));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_items_by_height() {
        let menu = Menu::new(&[("One", Action::ReverseList), ("Two", Action::RandomizeList)]);
        assert_eq!(menu.item_at(BORDER), Some(0));
        assert_eq!(menu.item_at(BORDER + ITEM_HEIGHT), Some(1));
        assert_eq!(menu.item_at(BORDER + 2 * ITEM_HEIGHT), None);
        assert_eq!(menu.item_at(-5), None);
    }
}
