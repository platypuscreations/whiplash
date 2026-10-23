//! The library browser, shown in the playlist window in a music server
//! mode: the lists opened so far, from the top level down, the search box,
//! and the form for signing in to the server. Like `Playlist` for tracks,
//! this is just the state; the playlist window draws it.

use super::text_field::TextField;
use crate::library::{self, Item};

/// One list in the browser, such as an artist's albums.
pub struct Level {
    pub title: String,
    pub items: Vec<Item>,
    pub selected: Option<usize>,
    /// The first row shown, while another list is.
    pub scroll: usize,
    /// The library request it's waiting for, if any.
    pub pending: Option<u64>,
}

/// Where typing goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Search,
    /// The sign-in form's fields: address, user name and password.
    Form(usize),
}

/// The sign-in form's fields, in order.
pub const FORM_FIELDS: usize = 3;
pub const PASSWORD: usize = 2;

/// What a row of the browser shows.
#[derive(Debug, PartialEq)]
pub enum Row<'a> {
    /// The first row, below the top level: where this list is, and
    /// clicking it goes back up.
    Back,
    Item(usize, &'a Item),
}

pub struct Browser {
    /// From the top level down to the list shown.
    levels: Vec<Level>,
    pub search: TextField,
    pub form: [TextField; FORM_FIELDS],
    /// Shown in the form's fields while they're empty.
    pub form_hints: [&'static str; FORM_FIELDS],
    /// How signing in is going, shown beside the button.
    pub form_status: String,
    /// Where typing goes, if anywhere.
    pub focus: Option<Field>,
}

impl Default for Browser {
    fn default() -> Self {
        let top = Level {
            title: "Library".to_string(),
            items: library::top_level(),
            selected: None,
            scroll: 0,
            pending: None,
        };
        Browser {
            levels: vec![top],
            search: TextField::default(),
            form: Default::default(),
            form_hints: ["Server address", "User name", "Password"],
            form_status: String::new(),
            focus: None,
        }
    }
}

impl Browser {
    /// The list shown.
    pub fn level(&self) -> &Level {
        self.levels.last().expect("the top level is never removed")
    }

    pub fn level_mut(&mut self) -> &mut Level {
        self.levels.last_mut().expect("the top level is never removed")
    }

    fn has_back_row(&self) -> bool {
        self.levels.len() > 1
    }

    /// How many rows the list shown has.
    pub fn rows(&self) -> usize {
        self.level().items.len() + self.has_back_row() as usize
    }

    pub fn row(&self, index: usize) -> Option<Row<'_>> {
        match index.checked_sub(self.has_back_row() as usize) {
            None => Some(Row::Back),
            Some(item) => self.level().items.get(item).map(|it| Row::Item(item, it)),
        }
    }

    /// The row an item is on.
    pub fn row_of(&self, item: usize) -> usize {
        item + self.has_back_row() as usize
    }

    /// Where the list shown is, below the top level: "Artists › Radiohead".
    pub fn path(&self) -> String {
        self.levels[1..].iter().map(|l| l.title.as_str()).collect::<Vec<_>>().join(" › ")
    }

    /// Opens a new list below the one shown, to be filled in when the
    /// library answers request `pending`. `scroll` is the shown list's
    /// position, kept for coming back to it.
    pub fn open(&mut self, title: String, scroll: usize, pending: Option<u64>) {
        self.level_mut().scroll = scroll;
        self.levels.push(Level { title, items: Vec::new(), selected: None, scroll: 0, pending });
    }

    /// Goes back up to the list above. Returns false at the top level.
    pub fn back(&mut self) -> bool {
        if !self.has_back_row() {
            return false;
        }
        self.levels.pop();
        true
    }

    /// Goes back to the top level.
    pub fn top(&mut self) {
        self.levels.truncate(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn going_down_and_back_up() {
        let mut browser = Browser::default();
        assert_eq!(browser.rows(), 2);
        assert!(matches!(browser.row(0), Some(Row::Item(0, item)) if item.title == "Artists"));
        assert!(!browser.back());

        browser.open("Artists".into(), 0, Some(1));
        assert_eq!(browser.level().pending, Some(1));
        assert_eq!(browser.rows(), 1);
        assert_eq!(browser.row(0), Some(Row::Back));
        assert_eq!(browser.row(1), None);

        browser.level_mut().items = vec![Item::folder("A", "a"), Item::folder("B", "b")];
        browser.open("B".into(), 1, None);
        assert_eq!(browser.path(), "Artists › B");
        assert!(browser.back());
        assert_eq!(browser.level().scroll, 1);
        assert_eq!(browser.row_of(1), 2);
        assert!(matches!(browser.row(2), Some(Row::Item(1, item)) if item.title == "B"));
        browser.top();
        assert_eq!(browser.level().title, "Library");
    }
}
