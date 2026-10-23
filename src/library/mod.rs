//! Music servers' libraries: what's in them, as the playlist window's
//! browser shows it.

/// What a library entry is, which decides what opening it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    /// A way into the library, such as "Artists".
    Folder,
    Artist,
    Album,
    Track,
}

/// One entry in a list from the library.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub kind: ItemKind,
    /// The server's id for it.
    pub id: String,
    pub title: String,
    /// Shown on the right: an album's year, say, or a search result's kind.
    pub detail: String,
    /// A track's length in seconds.
    pub duration: Option<f64>,
}

/// A list to ask the library for.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    Artists,
    /// Every album, or one artist's.
    Albums { artist: Option<String> },
    Tracks { album: String },
    Search(String),
}

impl Item {
    pub fn folder(title: &str, id: &str) -> Item {
        Item { kind: ItemKind::Folder, id: id.to_string(), title: title.to_string(), detail: String::new(), duration: None }
    }

    /// The list that opening this item shows; none for a track.
    pub fn request(&self) -> Option<Request> {
        match self.kind {
            ItemKind::Folder if self.id == "albums" => Some(Request::Albums { artist: None }),
            ItemKind::Folder => Some(Request::Artists),
            ItemKind::Artist => Some(Request::Albums { artist: Some(self.id.clone()) }),
            ItemKind::Album => Some(Request::Tracks { album: self.id.clone() }),
            ItemKind::Track => None,
        }
    }
}

/// Where browsing starts.
pub fn top_level() -> Vec<Item> {
    vec![Item::folder("Artists", "artists"), Item::folder("Albums", "albums")]
}
