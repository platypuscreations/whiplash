//! The playlist window: layout, hit-testing and drawing.
//!
//! Unlike the other windows it can be resized (in steps of 25 x 29
//! pixels), so its frame is assembled from tiles in the skin's
//! `pledit.bmp`. Positions follow Webamp's `playlist-window.css` (MIT
//! license, https://github.com/captbaritone/webamp).
//!
//! In a music server mode, tabs along the top of the list switch between
//! the playlist and a browser for the server's library. The frame stays
//! the same.

use super::browse::{Browser, FORM_FIELDS, Field, PASSWORD, Row};
use super::text_field::TextField;
use super::canvas::Canvas;
use super::main_window::{MainState, PlayState};
use super::text::Font;
use crate::playlist::Playlist;
use crate::skin::{self, Skin, Sprite};

pub const MIN_WIDTH: u32 = 275;
pub const MIN_HEIGHT: u32 = 116;
pub const WIDTH_STEP: u32 = 25;
pub const HEIGHT_STEP: u32 = 29;
pub const SHADE_HEIGHT: u32 = 14;

/// Size of the playlist font, in pixels.
pub const FONT_PX: f32 = 9.0;
pub const ROW_HEIGHT: i32 = 13;

/// The frame around the track list.
const TOP: i32 = 20;
const BOTTOM: i32 = 38;
const LEFT: i32 = 12;
const RIGHT: i32 = 20;
/// Space above the first track.
const LIST_PADDING: i32 = 3;
const SCROLL_HANDLE_HEIGHT: i32 = 18;
/// The tabs, in a music server mode.
const TAB_HEIGHT: i32 = 12;
const TAB_WIDTH: i32 = 40;
/// The search box, below the tabs while browsing, and the sign-in form's
/// fields, one below another.
const FIELD_HEIGHT: i32 = 13;
const BUTTON_WIDTH: i32 = 44;
/// Hints in the sign-in form's empty fields.
const FORM_HINTS: [&str; FORM_FIELDS] = ["Server address", "User name", "Password"];

/// What the list shows in a music server mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    /// The server's library.
    Browse,
    /// The playlist.
    Queue,
}

impl Tab {
    const ALL: [Tab; 2] = [Tab::Browse, Tab::Queue];

    fn label(self) -> &'static str {
        match self {
            Tab::Browse => "Browse",
            Tab::Queue => "Queue",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlControl {
    Close,
    Shade,
    ScrollUp,
    ScrollDown,
    Previous,
    Play,
    Pause,
    Stop,
    Next,
    Eject,
}

/// The five buttons along the bottom, each opening a column of choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuButton {
    Add,
    Remove,
    Select,
    Misc,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    AddUrl,
    AddFolder,
    AddFiles,
    RemoveMisc,
    RemoveAll,
    Crop,
    RemoveSelected,
    InvertSelection,
    SelectNone,
    SelectAll,
    SortList,
    FileInfo,
    MiscOptions,
    NewList,
    SaveList,
    LoadList,
}

impl MenuButton {
    /// The button's choices from top to bottom, with their sprites. The
    /// bottom one sits over the button itself.
    fn items(self) -> &'static [(MenuItem, Sprite)] {
        use MenuItem::*;
        use skin::*;
        match self {
            MenuButton::Add => &[(AddUrl, PL_ADD_URL), (AddFolder, PL_ADD_DIR), (AddFiles, PL_ADD_FILE)],
            MenuButton::Remove => &[
                (RemoveMisc, PL_REMOVE_MISC),
                (RemoveAll, PL_REMOVE_ALL),
                (Crop, PL_CROP),
                (RemoveSelected, PL_REMOVE_SELECTED),
            ],
            MenuButton::Select => {
                &[(InvertSelection, PL_INVERT_SELECTION), (SelectNone, PL_SELECT_ZERO), (SelectAll, PL_SELECT_ALL)]
            }
            MenuButton::Misc => &[(SortList, PL_SORT_LIST), (FileInfo, PL_FILE_INFO), (MiscOptions, PL_MISC_OPTIONS)],
            MenuButton::List => &[(NewList, PL_NEW_LIST), (SaveList, PL_SAVE_LIST), (LoadList, PL_LOAD_LIST)],
        }
    }

    fn bar(self) -> Sprite {
        match self {
            MenuButton::Add => skin::PL_ADD_MENU_BAR,
            MenuButton::Remove => skin::PL_REMOVE_MENU_BAR,
            MenuButton::Select => skin::PL_SELECT_MENU_BAR,
            MenuButton::Misc => skin::PL_MISC_MENU_BAR,
            MenuButton::List => skin::PL_LIST_MENU_BAR,
        }
    }

    /// Where the button is in a window of the given width.
    fn position(self, width: i32, height: i32) -> (i32, i32) {
        let x = match self {
            MenuButton::Add => 14,
            MenuButton::Remove => 43,
            MenuButton::Select => 72,
            MenuButton::Misc => 101,
            MenuButton::List => width - 44,
        };
        (x, height - 30)
    }

    const ALL: [MenuButton; 5] = [MenuButton::Add, MenuButton::Remove, MenuButton::Select, MenuButton::Misc, MenuButton::List];
}

/// The playlist window's own state (the tracks live in `Playlist`).
pub struct PlaylistView {
    pub width: u32,
    /// Height when not shaded.
    pub height: u32,
    /// Shrunk to a strip ("windowshade" mode).
    pub shaded: bool,
    /// Index of the first visible row.
    pub scroll: usize,
    /// The tab showing, in a music server mode; none for local files,
    /// which have no tabs.
    pub tab: Option<Tab>,
    /// The playlist's scroll position while the library shows.
    pub queue_scroll: usize,
    /// The Browse tab shows the form for signing in to the server, rather
    /// than the library.
    pub signing_in: bool,
    pub focused: bool,
    pub pressed: Option<PlControl>,
    pub open_menu: Option<MenuButton>,
    /// The menu choice under the mouse.
    pub hovered: Option<MenuItem>,
    pub dragging_scrollbar: bool,
}

impl Default for PlaylistView {
    fn default() -> Self {
        PlaylistView {
            width: MIN_WIDTH,
            height: MIN_HEIGHT + 2 * HEIGHT_STEP,
            shaded: false,
            scroll: 0,
            tab: None,
            queue_scroll: 0,
            signing_in: false,
            focused: false,
            pressed: None,
            open_menu: None,
            hovered: None,
            dragging_scrollbar: false,
        }
    }
}

impl PlaylistView {
    fn size(&self) -> (i32, i32) {
        (self.width as i32, self.height as i32)
    }

    /// The window's current height, allowing for shade mode.
    pub fn window_height(&self) -> u32 {
        if self.shaded { SHADE_HEIGHT } else { self.height }
    }

    /// Where the list's first row is, below the tabs and search box.
    pub fn list_top(&self) -> i32 {
        TOP + LIST_PADDING
            + match self.tab {
                None => 0,
                Some(Tab::Queue) => TAB_HEIGHT,
                Some(Tab::Browse) => TAB_HEIGHT + FIELD_HEIGHT,
            }
    }

    /// How many rows fit in the window.
    pub fn visible_rows(&self) -> usize {
        ((self.height as i32 - BOTTOM - LIST_PADDING - self.list_top()) / ROW_HEIGHT).max(0) as usize
    }

    pub fn max_scroll(&self, track_count: usize) -> usize {
        track_count.saturating_sub(self.visible_rows())
    }

    pub fn scroll_to(&mut self, first: isize, track_count: usize) {
        self.scroll = first.clamp(0, self.max_scroll(track_count) as isize) as usize;
    }

    /// Scrolls just enough to show track `index`.
    pub fn reveal(&mut self, index: usize, track_count: usize) {
        if index < self.scroll {
            self.scroll = index;
        } else if index >= self.scroll + self.visible_rows() {
            self.scroll = index + 1 - self.visible_rows();
        }
        self.scroll_to(self.scroll as isize, track_count);
    }

    /// Snaps a requested size to the playlist's 25 x 29 grid.
    pub fn snap_size(width: f64, height: f64) -> (u32, u32) {
        let snap = |value: f64, min: u32, step: u32| {
            let steps = ((value - min as f64) / step as f64).round().max(0.0) as u32;
            min + steps * step
        };
        (snap(width, MIN_WIDTH, WIDTH_STEP), snap(height, MIN_HEIGHT, HEIGHT_STEP))
    }

    /// Position of the scrollbar handle's top edge.
    fn scroll_handle_y(&self, track_count: usize) -> i32 {
        let travel = self.height as i32 - TOP - BOTTOM - SCROLL_HANDLE_HEIGHT;
        let max = self.max_scroll(track_count);
        let fraction = if max == 0 { 0.0 } else { self.scroll as f32 / max as f32 };
        TOP + (fraction * travel as f32).round() as i32
    }

    /// The scroll position for a scrollbar handle centered on `y`.
    pub fn scroll_for_handle(&self, y: i32, track_count: usize) -> usize {
        let travel = (self.height as i32 - TOP - BOTTOM - SCROLL_HANDLE_HEIGHT).max(1);
        let fraction = ((y - TOP - SCROLL_HANDLE_HEIGHT / 2) as f32 / travel as f32).clamp(0.0, 1.0);
        (fraction * self.max_scroll(track_count) as f32).round() as usize
    }

    /// The open menu's choices with their positions (x, y).
    fn menu_layout(&self) -> Vec<(MenuItem, Sprite, i32, i32)> {
        let Some(button) = self.open_menu else { return Vec::new() };
        let (width, height) = self.size();
        let (x, bottom_y) = button.position(width, height);
        let items = button.items();
        items
            .iter()
            .enumerate()
            .map(|(i, &(item, sprite))| (item, sprite, x, bottom_y - (items.len() - 1 - i) as i32 * 18))
            .collect()
    }
}

/// What lies under a point in the playlist window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Control(PlControl),
    MenuButton(MenuButton),
    MenuItem(MenuItem),
    /// A track, by its index in the playlist.
    Track(usize),
    /// A row of the library browser.
    BrowseRow(usize),
    /// The empty space below the last row.
    BelowTracks,
    Tab(Tab),
    SearchBox,
    FormField(usize),
    SignInButton,
    ScrollBar,
    Resize,
    TitleBar,
    Nothing,
}

/// How many rows the list shown has: tracks, or the browser's rows.
pub fn rows(view: &PlaylistView, playlist: &Playlist, browser: &Browser) -> usize {
    match view.tab {
        Some(Tab::Browse) if view.signing_in => 0,
        Some(Tab::Browse) => browser.rows(),
        _ => playlist.len(),
    }
}

/// What's at (x, y). `row_count` is how many rows the list has (see `rows`).
pub fn hit_test(view: &PlaylistView, row_count: usize, x: i32, y: i32) -> Hit {
    let (width, height) = view.size();
    let inside = |rx: i32, ry: i32, w: i32, h: i32| x >= rx && x < rx + w && y >= ry && y < ry + h;
    if view.shaded {
        return if inside(width - 11, 3, 9, 9) {
            Hit::Control(PlControl::Close)
        } else if inside(width - 21, 3, 9, 9) {
            Hit::Control(PlControl::Shade)
        } else if inside(width - 29, 3, 8, 9) {
            Hit::Resize
        } else if inside(0, 0, width, SHADE_HEIGHT as i32) {
            Hit::TitleBar
        } else {
            Hit::Nothing
        };
    }

    // An open menu covers what's beneath it.
    for (item, _, ix, iy) in view.menu_layout() {
        if inside(ix, iy, 22, 18) {
            return Hit::MenuItem(item);
        }
    }

    let (right_x, bottom_y) = (width - 150, height - BOTTOM);
    let controls = [
        (PlControl::Close, width - 11, 3, 9, 9),
        (PlControl::Shade, width - 21, 3, 9, 9),
        (PlControl::ScrollUp, width - 15, bottom_y + 2, 8, 5),
        (PlControl::ScrollDown, width - 15, bottom_y + 8, 8, 5),
        (PlControl::Previous, right_x + 4, bottom_y + 22, 8, 10),
        (PlControl::Play, right_x + 12, bottom_y + 22, 10, 10),
        (PlControl::Pause, right_x + 22, bottom_y + 22, 9, 10),
        (PlControl::Stop, right_x + 31, bottom_y + 22, 10, 10),
        (PlControl::Next, right_x + 41, bottom_y + 22, 9, 10),
        (PlControl::Eject, right_x + 50, bottom_y + 22, 11, 10),
    ];
    for (control, cx, cy, w, h) in controls {
        if inside(cx, cy, w, h) {
            return Hit::Control(control);
        }
    }
    for button in MenuButton::ALL {
        let (bx, by) = button.position(width, height);
        if inside(bx, by, 22, 18) {
            return Hit::MenuButton(button);
        }
    }
    if inside(width - 20, height - 20, 20, 20) {
        return Hit::Resize;
    }
    if inside(width - 15, TOP, 8, height - TOP - BOTTOM) {
        return Hit::ScrollBar;
    }
    if inside(LEFT, TOP, width - LEFT - RIGHT, height - TOP - BOTTOM) {
        if let Some(tab) = view.tab {
            if y < TOP + TAB_HEIGHT {
                let column = (x - LEFT) / TAB_WIDTH;
                return Tab::ALL.get(column as usize).map_or(Hit::Nothing, |&tab| Hit::Tab(tab));
            }
            if tab == Tab::Browse && view.signing_in {
                for field in 0..=FORM_FIELDS {
                    let (fx, fy, fw, fh) = field_box(width, field);
                    if inside(fx, fy, fw, fh) {
                        return if field < FORM_FIELDS { Hit::FormField(field) } else { Hit::SignInButton };
                    }
                }
                return Hit::Nothing;
            }
            if tab == Tab::Browse && y < TOP + TAB_HEIGHT + FIELD_HEIGHT {
                return Hit::SearchBox;
            }
        }
        let row = (y - view.list_top()).div_euclid(ROW_HEIGHT);
        let index = view.scroll as i32 + row;
        let shown = row >= 0 && (row as usize) < view.visible_rows() && (index as usize) < row_count;
        return match (shown, view.tab) {
            (true, Some(Tab::Browse)) => Hit::BrowseRow(index as usize),
            (true, _) => Hit::Track(index as usize),
            (false, _) => Hit::BelowTracks,
        };
    }
    if inside(0, 0, width, TOP) { Hit::TitleBar } else { Hit::Nothing }
}

/// Draws the window at skin size, except the list's text: that's drawn
/// afterwards at the screen's full resolution by `draw_list_text`, so it
/// stays sharp.
pub fn draw(canvas: &mut Canvas, skin: &Skin, view: &PlaylistView, playlist: &Playlist, browser: &Browser, main: &MainState) {
    use skin::*;
    if view.shaded {
        return draw_shaded(canvas, skin, view, playlist);
    }
    let (width, height) = view.size();
    let focused = view.focused;
    let pick = |normal, selected| if focused { selected } else { normal };
    canvas.fill(0);

    // Top: corners, tiles, and the title centered between them.
    canvas.draw(skin, pick(PL_TOP_LEFT, PL_TOP_LEFT_SELECTED), 0, 0);
    for x in (25..width - 25).step_by(25) {
        canvas.draw(skin, pick(PL_TOP_TILE, PL_TOP_TILE_SELECTED), x, 0);
    }
    canvas.draw(skin, pick(PL_TITLE, PL_TITLE_SELECTED), 37 + (width - 175) / 2, 0);
    canvas.draw(skin, pick(PL_TOP_RIGHT, PL_TOP_RIGHT_SELECTED), width - 25, 0);
    if view.pressed == Some(PlControl::Close) {
        canvas.draw(skin, PL_CLOSE_SELECTED, width - 11, 3);
    }
    if view.pressed == Some(PlControl::Shade) {
        canvas.draw(skin, PL_COLLAPSE_SELECTED, width - 21, 3);
    }

    // Sides.
    for y in (TOP..height - BOTTOM).step_by(29) {
        canvas.draw(skin, PL_LEFT_TILE, 0, y);
        canvas.draw(skin, PL_RIGHT_TILE, width - RIGHT, y);
    }

    // Bottom: the two corner pieces with tiles between, plus the spare
    // visualizer panel when the window is wide enough.
    for x in (125..width - 150).step_by(25) {
        canvas.draw(skin, PL_BOTTOM_TILE, x, height - BOTTOM);
    }
    if width >= 350 {
        canvas.draw(skin, PL_VISUALIZER_BACKGROUND, width - 225, height - BOTTOM);
    }
    canvas.draw(skin, PL_BOTTOM_LEFT, 0, height - BOTTOM);
    canvas.draw(skin, PL_BOTTOM_RIGHT, width - 150, height - BOTTOM);

    // Scrollbar handle.
    let handle = if view.dragging_scrollbar { PL_SCROLL_HANDLE_SELECTED } else { PL_SCROLL_HANDLE };
    canvas.draw(skin, handle, width - 15, view.scroll_handle_y(rows(view, playlist, browser)));

    draw_list_backgrounds(canvas, skin, view, playlist, browser);
    draw_status(canvas, skin, view, playlist, main);

    // The open button menu, with its bar to the left.
    let layout = view.menu_layout();
    if let (Some(button), Some(&(_, _, x, _))) = (view.open_menu, layout.first()) {
        let bar = button.bar();
        let menu_bottom = layout.last().map_or(0, |&(_, _, _, y)| y + 18);
        canvas.draw(skin, bar, x - 3, menu_bottom - bar.h as i32);
        for (item, sprite, ix, iy) in layout {
            let sprite = if view.hovered == Some(item) { pl_menu_selected(sprite) } else { sprite };
            canvas.draw(skin, sprite, ix, iy);
        }
    }
}

/// The list's background, with selected rows highlighted, and the tabs
/// and search box in a music server mode.
fn draw_list_backgrounds(canvas: &mut Canvas, skin: &Skin, view: &PlaylistView, playlist: &Playlist, browser: &Browser) {
    let (width, height) = view.size();
    let style = &skin.playlist;
    let (left, right) = (LEFT, width - RIGHT);
    canvas.fill_rect(left, TOP, (right - left) as u32, (height - TOP - BOTTOM) as u32, style.normal_bg);

    if let Some(tab) = view.tab {
        // The tab showing is highlighted like a selected row, and joined
        // to a line under the tabs.
        let column = Tab::ALL.iter().position(|&t| t == tab).unwrap_or(0) as i32;
        canvas.fill_rect(left + column * TAB_WIDTH, TOP, TAB_WIDTH as u32, TAB_HEIGHT as u32, style.selected_bg);
        canvas.fill_rect(left, TOP + TAB_HEIGHT - 1, (right - left) as u32, 1, style.selected_bg);
        // Text boxes are outlined, brighter while typing in them.
        let outline = |canvas: &mut Canvas, (x, y, w, h): (i32, i32, i32, i32), focused: bool| {
            let color = if focused { style.normal } else { mix(style.normal, style.normal_bg) };
            canvas.fill_rect(x, y, w as u32, 1, color);
            canvas.fill_rect(x, y + h - 1, w as u32, 1, color);
            canvas.fill_rect(x, y, 1, h as u32, color);
            canvas.fill_rect(x + w - 1, y, 1, h as u32, color);
        };
        if tab == Tab::Browse && view.signing_in {
            for field in 0..FORM_FIELDS {
                outline(canvas, field_box(width, field), browser.focus == Some(Field::Form(field)));
            }
            let (x, y, w, h) = field_box(width, FORM_FIELDS);
            canvas.fill_rect(x, y, w as u32, h as u32, style.selected_bg);
            return;
        }
        if tab == Tab::Browse {
            outline(canvas, field_box(width, 0), browser.focus == Some(Field::Search));
        }
    }

    for row in 0..view.visible_rows() {
        let index = view.scroll + row;
        let selected = if view.tab == Some(Tab::Browse) {
            match browser.row(index) {
                Some(Row::Item(item, _)) => browser.level().selected == Some(item),
                Some(Row::Back) => false,
                None => break,
            }
        } else {
            let Some(entry) = playlist.entries().get(index) else { break };
            entry.selected
        };
        if selected {
            let top = view.list_top() + row as i32 * ROW_HEIGHT;
            canvas.fill_rect(left, top, (right - left) as u32, ROW_HEIGHT as u32, style.selected_bg);
        }
    }
}

/// A text box's outline below the tabs: left, top, width and height. Box
/// 0 is the search box, or the sign-in form's first field; the box after
/// the form's fields is its button.
fn field_box(width: i32, index: usize) -> (i32, i32, i32, i32) {
    let top = TOP + TAB_HEIGHT + 1 + index as i32 * FIELD_HEIGHT;
    let w = if index == FORM_FIELDS { BUTTON_WIDTH } else { width - LEFT - RIGHT - 4 };
    (LEFT + 2, top, w, FIELD_HEIGHT - 1)
}

/// Halfway between two colors, for hints such as the empty search box's.
fn mix(a: u32, b: u32) -> u32 {
    let channel = |shift: u32| (((a >> shift) & 0xFF) + ((b >> shift) & 0xFF)) / 2 << shift;
    channel(16) | channel(8) | channel(0)
}

/// Draws the list's text onto the enlarged window, where each skin pixel
/// is `scale` x `scale` screen pixels: track names and lengths, or the
/// browser's rows, and the tabs and search box. `font` must be loaded at
/// `FONT_PX * scale`. Text never covers an open button menu.
pub fn draw_list_text(
    canvas: &mut Canvas,
    skin: &Skin,
    font: &mut Font,
    view: &PlaylistView,
    playlist: &Playlist,
    browser: &Browser,
    scale: u32,
) {
    if view.shaded {
        return;
    }
    let s = scale as i32;
    let (width, _) = view.size();
    let style = &skin.playlist;
    let (left, right) = (LEFT * s, (width - RIGHT) * s);

    // An open button menu (and its bar) covers part of the list. Text may
    // go everywhere on a line, or either side of the menu.
    let layout = view.menu_layout();
    let menu = layout.first().zip(layout.last()).map(|(&(_, _, x, top), &(_, _, _, bottom))| {
        ((x - 3) * s, (x + 22) * s, top * s, (bottom + 18) * s)
    });
    let clips = |top: i32, height: i32| -> Vec<(i32, i32)> {
        match menu {
            Some((menu_left, menu_right, menu_top, menu_bottom)) if top < menu_bottom && top + height > menu_top => {
                vec![(left, menu_left), (menu_right, right)]
            }
            _ => vec![(left, right)],
        }
    };
    let within = |(a, b): (i32, i32), (c, d): (i32, i32)| (a.max(c), b.min(d));

    if let Some(tab) = view.tab {
        let (top, height) = (TOP * s, (TAB_HEIGHT - 1) * s);
        for (column, each) in Tab::ALL.into_iter().enumerate() {
            let x = (LEFT + column as i32 * TAB_WIDTH) * s;
            let label = each.label();
            let color = if each == tab { style.current } else { style.normal };
            let label_x = x + (TAB_WIDTH * s - font.width(label)) / 2;
            for clip in clips(top, height) {
                font.draw(canvas, label, label_x, top, height, color, within(clip, (x, x + TAB_WIDTH * s)));
            }
        }
        if tab == Tab::Browse && view.signing_in {
            for field in 0..FORM_FIELDS {
                let rect = field_box(width, field);
                let focused = browser.focus == Some(Field::Form(field));
                let clips = clips(rect.1 * s, rect.3 * s);
                draw_field(canvas, font, style, &browser.form[field], FORM_HINTS[field], field == PASSWORD, focused, rect, s, &clips);
            }
            // The button, and how signing in is going beside it.
            let (x, y, w, h) = field_box(width, FORM_FIELDS);
            let (top, height) = (y * s, h * s);
            let label = "Sign in";
            let label_x = x * s + (w * s - font.width(label)) / 2;
            for clip in clips(top, height) {
                font.draw(canvas, label, label_x, top, height, style.current, within(clip, (x * s, (x + w) * s)));
                let status = font.fit(&browser.form_status, right - (x + w + 4) * s);
                font.draw(canvas, &status, (x + w + 4) * s, top, height, style.normal, within(clip, (left, right)));
            }
            return;
        }
        if tab == Tab::Browse {
            let rect = field_box(width, 0);
            let focused = browser.focus == Some(Field::Search);
            draw_field(canvas, font, style, &browser.search, "Search", false, focused, rect, s, &clips(top, height));
        }
    }

    let current = playlist.current_index();
    let browsing = view.tab == Some(Tab::Browse);
    let mut drawn = 0;
    for row in 0..view.visible_rows() {
        let index = view.scroll + row;
        let (title, length, color) = if browsing {
            match browser.row(index) {
                None => break,
                Some(Row::Back) => (format!("‹ {}", browser.path()), String::new(), style.current),
                Some(Row::Item(_, item)) => {
                    let length = match (item.duration, item.kind) {
                        (Some(d), _) => format_duration(d.round() as u64),
                        (None, crate::library::ItemKind::Track) => item.detail.clone(),
                        // Lists that open further are marked.
                        (None, _) if item.detail.is_empty() => "›".to_string(),
                        (None, _) => item.detail.clone(),
                    };
                    (item.title.clone(), length, style.normal)
                }
            }
        } else {
            let Some(entry) = playlist.entries().get(index) else { break };
            let length = entry.duration.map(|d| format_duration(d.round() as u64)).unwrap_or_default();
            let color = if Some(index) == current { style.current } else { style.normal };
            (format!("{}. {}", index + 1, entry.title), length, color)
        };
        let top = (view.list_top() + row as i32 * ROW_HEIGHT) * s;
        draw_row(canvas, font, &title, &length, color, top, ROW_HEIGHT * s, (left, right), &clips(top, ROW_HEIGHT * s), s);
        drawn += 1;
    }

    // Below the browser's rows: that more are on the way.
    if browsing && browser.level().pending.is_some() && drawn < view.visible_rows() {
        let top = (view.list_top() + drawn as i32 * ROW_HEIGHT) * s;
        let color = mix(style.normal, style.normal_bg);
        draw_row(canvas, font, "Loading...", "", color, top, ROW_HEIGHT * s, (left, right), &clips(top, ROW_HEIGHT * s), s);
    }
}

/// One row of the list: the title on the left, shortened to fit before
/// the length on the right.
#[allow(clippy::too_many_arguments)]
fn draw_row(
    canvas: &mut Canvas,
    font: &mut Font,
    title: &str,
    length: &str,
    color: u32,
    top: i32,
    height: i32,
    (left, right): (i32, i32),
    clips: &[(i32, i32)],
    s: i32,
) {
    let within = |(a, b): (i32, i32), (c, d): (i32, i32)| (a.max(c), b.min(d));
    let length_x = right - 3 * s - font.width(length);
    let title = font.fit(title, length_x - 6 * s - (left + 2 * s));
    for &clip in clips {
        font.draw(canvas, length, length_x, top, height, color, within(clip, (left, right)));
        font.draw(canvas, &title, left + 2 * s, top, height, color, within(clip, (left, length_x - 2 * s)));
    }
}

/// A text box's text, or a hint while it's empty, and the caret while
/// typing in it. `rect` is the box, in skin pixels. Long text is drawn from
/// far enough in that the caret shows; a password shows as dots.
#[allow(clippy::too_many_arguments)]
fn draw_field(
    canvas: &mut Canvas,
    font: &mut Font,
    style: &crate::skin::PlaylistStyle,
    field: &TextField,
    hint: &str,
    masked: bool,
    focused: bool,
    (x, y, w, h): (i32, i32, i32, i32),
    s: i32,
    clips: &[(i32, i32)],
) {
    let (left, right, top, height) = ((x + 2) * s, (x + w - 2) * s, (y + 1) * s, (h - 2) * s);
    let within = |(a, b): (i32, i32)| (a.max(left), b.min(right));
    if field.text().is_empty() && !focused {
        let color = mix(style.normal, style.normal_bg);
        for &clip in clips {
            font.draw(canvas, hint, left, top, height, color, within(clip));
        }
        return;
    }
    let chars: Vec<char> = if masked { vec!['•'; field.text().chars().count()] } else { field.text().chars().collect() };
    let caret = field.caret().min(chars.len());
    let text = |from: usize, to: usize| chars[from..to].iter().collect::<String>();
    let mut start = 0;
    while start < caret && font.width(&text(start, caret)) > right - left - s {
        start += 1;
    }
    for &clip in clips {
        font.draw(canvas, &text(start, chars.len()), left, top, height, style.normal, within(clip));
    }
    if focused {
        let caret_x = left + font.width(&text(start, caret));
        canvas.fill_rect(caret_x, top + s, (s / 2).max(1) as u32, (height - 2 * s).max(1) as u32, style.normal);
    }
}

/// Shade mode: one strip showing the current track and its length.
fn draw_shaded(canvas: &mut Canvas, skin: &Skin, view: &PlaylistView, playlist: &Playlist) {
    use skin::*;
    let width = view.width as i32;
    canvas.fill(0);
    canvas.draw(skin, PL_SHADE_LEFT, 0, 0);
    for x in (25..width - 50).step_by(25) {
        canvas.draw(skin, PL_SHADE_TILE, x, 0);
    }
    canvas.draw(skin, if view.focused { PL_SHADE_RIGHT_SELECTED } else { PL_SHADE_RIGHT }, width - 50, 0);
    if view.pressed == Some(PlControl::Shade) {
        canvas.draw(skin, PL_EXPAND_SELECTED, width - 21, 3);
    }
    if view.pressed == Some(PlControl::Close) {
        canvas.draw(skin, PL_CLOSE_SELECTED, width - 11, 3);
    }

    let current = playlist.current_index().map(|i| (i, &playlist.entries()[i]));
    let (title, time) = match current {
        Some((i, entry)) => {
            (format!("{}. {}", i + 1, entry.title), entry.duration.map(|d| format_duration(d.round() as u64)))
        }
        None => ("[no file]".to_string(), None),
    };
    // Room for the title grows with the window, 5 pixels per character.
    let max_chars = ((205 + width - MIN_WIDTH as i32) / skin::CHAR_WIDTH as i32) as usize;
    let title = if title.chars().count() > max_chars {
        title.chars().take(max_chars - 1).chain(std::iter::once('…')).collect()
    } else {
        title
    };
    draw_bitmap_text(canvas, skin, &title, 5, 4, max_chars);
    if let Some(time) = time {
        let x = width - 30 - time.len() as i32 * skin::CHAR_WIDTH as i32;
        draw_bitmap_text(canvas, skin, &time, x, 4, 8);
    }
}

/// The running-time display ("selected/total") and the mini time.
fn draw_status(canvas: &mut Canvas, skin: &Skin, view: &PlaylistView, playlist: &Playlist, main: &MainState) {
    let (width, height) = view.size();
    let (x, y) = (width - 150, height - BOTTOM);

    let ((selected, selected_unknown), (total, total_unknown)) = playlist.durations();
    let part = |seconds: f64, unknown: bool| format!("{}{}", format_duration(seconds.round() as u64), if unknown { "+" } else { "" });
    let running = format!("{}/{}", part(selected, selected_unknown), part(total, total_unknown));
    draw_bitmap_text(canvas, skin, &running, x + 7, y + 10, 18);

    // Elapsed time, blinking while paused like the main window's.
    if main.play_state != PlayState::Stopped && main.show_time {
        let (seconds, sign) = match (main.time_remaining, main.duration) {
            (true, Some(duration)) => (duration.saturating_sub(main.elapsed), '-'),
            _ => (main.elapsed, ' '),
        };
        let text = format!("{sign}{:>2}:{:02}", (seconds / 60).min(99), seconds % 60);
        draw_bitmap_text(canvas, skin, &text, x + 66, y + 23, 6);
    }
}

/// Draws up to `max_chars` characters in the skin's 5x6 bitmap font.
fn draw_bitmap_text(canvas: &mut Canvas, skin: &Skin, text: &str, x: i32, y: i32, max_chars: usize) {
    for (i, c) in text.chars().take(max_chars).enumerate() {
        canvas.draw(skin, skin::character(c), x + i as i32 * skin::CHAR_WIDTH as i32, y);
    }
}

/// "3:07", or "1:02:03" for an hour or more.
pub fn format_duration(seconds: u64) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{seconds:02}") } else { format!("{minutes}:{seconds:02}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn sizes_snap_to_the_grid() {
        assert_eq!(PlaylistView::snap_size(100.0, 50.0), (275, 116));
        assert_eq!(PlaylistView::snap_size(312.0, 160.0), (300, 174));
    }

    #[test]
    fn rows_and_scrolling() {
        let view = PlaylistView { height: MIN_HEIGHT, ..PlaylistView::default() };
        assert_eq!(view.visible_rows(), 4);
        let mut view = PlaylistView { height: MIN_HEIGHT + HEIGHT_STEP, ..view };
        assert_eq!(view.visible_rows(), 6);
        assert_eq!(view.max_scroll(10), 4);
        view.reveal(8, 10);
        assert_eq!(view.scroll, 3);
        view.reveal(1, 10);
        assert_eq!(view.scroll, 1);
        view.scroll_to(99, 10);
        assert_eq!(view.scroll, 4);
    }

    #[test]
    fn hit_testing() {
        let mut list = Playlist::default();
        list.replace((0..10).map(|i| PathBuf::from(format!("{i}.mp3"))).collect());
        let mut view = PlaylistView { width: 300, height: MIN_HEIGHT + HEIGHT_STEP, ..PlaylistView::default() };
        view.scroll = 2;
        assert_eq!(hit_test(&view, 10, 50, TOP + LIST_PADDING + 1), Hit::Track(2));
        assert_eq!(hit_test(&view, 10, 50, TOP + LIST_PADDING + ROW_HEIGHT + 1), Hit::Track(3));
        assert_eq!(hit_test(&view, 2, 50, TOP + LIST_PADDING + 1), Hit::BelowTracks);
        assert_eq!(hit_test(&view, 10, 20, view.height as i32 - 25), Hit::MenuButton(MenuButton::Add));
        assert_eq!(hit_test(&view, 10, 295, 5), Hit::Control(PlControl::Close));
        assert_eq!(hit_test(&view, 10, 295, view.height as i32 - 3), Hit::Resize);
        assert_eq!(hit_test(&view, 10, 100, 5), Hit::TitleBar);
        assert_eq!(hit_test(&view, 10, 282, 5), Hit::Control(PlControl::Shade));
        let shaded = PlaylistView { shaded: true, ..PlaylistView::default() };
        assert_eq!(hit_test(&shaded, 10, 268, 5), Hit::Control(PlControl::Close));
        assert_eq!(hit_test(&shaded, 10, 250, 5), Hit::Resize);
        assert_eq!(hit_test(&shaded, 10, 50, 5), Hit::TitleBar);

        // The open "remove" menu stacks four choices above its button.
        view.open_menu = Some(MenuButton::Remove);
        let bottom = view.height as i32 - 30;
        assert_eq!(hit_test(&view, 10, 50, bottom + 2), Hit::MenuItem(MenuItem::RemoveSelected));
        assert_eq!(hit_test(&view, 10, 50, bottom - 3 * 18 + 2), Hit::MenuItem(MenuItem::RemoveMisc));
    }

    #[test]
    fn tabs_move_the_list_down() {
        let mut view = PlaylistView { tab: Some(Tab::Queue), ..PlaylistView::default() };
        assert_eq!(hit_test(&view, 10, LEFT + 5, TOP + 2), Hit::Tab(Tab::Browse));
        assert_eq!(hit_test(&view, 10, LEFT + TAB_WIDTH + 5, TOP + 2), Hit::Tab(Tab::Queue));
        assert_eq!(hit_test(&view, 10, LEFT + 3 * TAB_WIDTH, TOP + 2), Hit::Nothing);
        let first = TOP + LIST_PADDING + TAB_HEIGHT;
        assert_eq!(hit_test(&view, 10, 50, first + 1), Hit::Track(0));

        view.tab = Some(Tab::Browse);
        assert_eq!(hit_test(&view, 10, 50, first + 1), Hit::SearchBox);
        assert_eq!(hit_test(&view, 10, 50, first + FIELD_HEIGHT + 1), Hit::BrowseRow(0));
        assert_eq!(hit_test(&view, 1, 50, first + FIELD_HEIGHT + ROW_HEIGHT + 1), Hit::BelowTracks);

        // Even the smallest window shows two rows of the library.
        view.height = MIN_HEIGHT;
        assert_eq!(view.visible_rows(), 2);
        assert_eq!(PlaylistView { height: MIN_HEIGHT, tab: Some(Tab::Queue), ..view }.visible_rows(), 3);
    }

    #[test]
    fn durations_format_like_winamp() {
        assert_eq!(format_duration(65), "1:05");
        assert_eq!(format_duration(3723), "1:02:03");
    }
}
