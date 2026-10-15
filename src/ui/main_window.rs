//! The main player window: layout, hit-testing and drawing.
//!
//! On-screen positions come from Webamp's `main-window.css` (MIT license,
//! https://github.com/captbaritone/webamp).

use super::canvas::Canvas;
pub use crate::audio::PlayState;
use crate::skin::{self, Sheet, Skin};

pub const WIDTH: u32 = 275;
pub const HEIGHT: u32 = 116;

/// Height of the title bar, which drags the window.
const TITLE_BAR_HEIGHT: i32 = 14;

/// The marquee is 154 px wide: 31 characters of the 5 px skin font.
const MARQUEE_CHARS: usize = 31;
/// Shown between repeats of a scrolling title.
const MARQUEE_SEPARATOR: &str = "  ***  ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Options,
    Minimize,
    Shade,
    Close,
    Previous,
    Play,
    Pause,
    Stop,
    Next,
    Eject,
    Shuffle,
    Repeat,
    Equalizer,
    Playlist,
}

/// Where each clickable control sits in the window: (control, x, y, w, h).
const CONTROLS: [(Control, i32, i32, i32, i32); 14] = [
    (Control::Options, 6, 3, 9, 9),
    (Control::Minimize, 244, 3, 9, 9),
    (Control::Shade, 254, 3, 9, 9),
    (Control::Close, 264, 3, 9, 9),
    (Control::Previous, 16, 88, 23, 18),
    (Control::Play, 39, 88, 23, 18),
    (Control::Pause, 62, 88, 23, 18),
    (Control::Stop, 85, 88, 23, 18),
    (Control::Next, 108, 88, 22, 18),
    (Control::Eject, 136, 89, 22, 16),
    (Control::Shuffle, 164, 89, 47, 15),
    (Control::Repeat, 210, 89, 28, 15),
    (Control::Equalizer, 219, 58, 23, 12),
    (Control::Playlist, 242, 58, 23, 12),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slider {
    Volume,
    Balance,
    Position,
}

/// Each slider's track: (slider, x, y, width, height, thumb width).
const SLIDERS: [(Slider, i32, i32, i32, i32, i32); 3] = [
    (Slider::Volume, 107, 57, 68, 13, 14),
    (Slider::Balance, 177, 57, 38, 13, 14),
    (Slider::Position, 16, 72, 248, 10, 29),
];

/// The time display; clicking it switches between elapsed and remaining.
const TIME_AREA: (i32, i32, i32, i32) = (39, 26, 59, 13);

/// Everything the main window shows that isn't part of the skin.
pub struct MainState {
    pub focused: bool,
    /// The button currently held down with the mouse, if any.
    pub pressed: Option<Control>,
    /// The slider currently being dragged, if any.
    pub dragging: Option<Slider>,
    pub play_state: PlayState,
    /// False during the "off" half of the blink while paused.
    pub show_time: bool,
    /// Elapsed time of the current track in seconds.
    pub elapsed: u32,
    /// Track length in seconds, if known.
    pub duration: Option<u32>,
    /// Show time remaining (with a minus sign) instead of elapsed.
    pub time_remaining: bool,
    /// Track position from 0.0 to 1.0.
    pub position: f32,
    /// 0.0 to 1.0.
    pub volume: f32,
    /// -1.0 (left) to 1.0 (right).
    pub balance: f32,
    pub stereo: bool,
    pub kbps: Option<u32>,
    pub khz: Option<u32>,
    pub shuffle: bool,
    pub repeat: bool,
    pub equalizer_open: bool,
    pub playlist_open: bool,
    /// The scrolling text display.
    pub marquee: String,
    /// How many characters the marquee has scrolled.
    pub marquee_offset: usize,
}

impl Default for MainState {
    fn default() -> Self {
        MainState {
            focused: true,
            pressed: None,
            dragging: None,
            play_state: PlayState::Stopped,
            show_time: true,
            elapsed: 0,
            duration: None,
            time_remaining: false,
            position: 0.0,
            volume: 0.75,
            balance: 0.0,
            stereo: true,
            kbps: None,
            khz: None,
            shuffle: false,
            repeat: false,
            equalizer_open: false,
            playlist_open: false,
            marquee: String::new(),
            marquee_offset: 0,
        }
    }
}

/// What lies under a point in the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Control(Control),
    Slider(Slider),
    Time,
    TitleBar,
    Nothing,
}

pub fn hit_test(x: i32, y: i32) -> Hit {
    let inside = |rx: i32, ry: i32, w: i32, h: i32| x >= rx && x < rx + w && y >= ry && y < ry + h;

    for (control, cx, cy, w, h) in CONTROLS {
        if inside(cx, cy, w, h) {
            return Hit::Control(control);
        }
    }
    for (slider, sx, sy, w, h, _) in SLIDERS {
        if inside(sx, sy, w, h) {
            return Hit::Slider(slider);
        }
    }
    let (tx, ty, tw, th) = TIME_AREA;
    if inside(tx, ty, tw, th) {
        Hit::Time
    } else if inside(0, 0, WIDTH as i32, TITLE_BAR_HEIGHT) {
        Hit::TitleBar
    } else {
        Hit::Nothing
    }
}

/// The value a slider takes when the mouse is at window x-coordinate `x`:
/// the thumb is centered on the mouse. Volume and position are 0.0 to 1.0;
/// balance is -1.0 to 1.0.
pub fn slider_value(slider: Slider, x: i32) -> f32 {
    let (_, sx, _, width, _, thumb) = SLIDERS.iter().find(|s| s.0 == slider).copied().unwrap();
    let fraction = ((x - sx - thumb / 2) as f32 / (width - thumb) as f32).clamp(0.0, 1.0);
    match slider {
        Slider::Balance => fraction * 2.0 - 1.0,
        _ => fraction,
    }
}

/// The marquee text with a separator, as it cycles when scrolling.
fn marquee_cycle(text: &str) -> Vec<char> {
    format!("{text}{MARQUEE_SEPARATOR}").chars().collect()
}

/// Whether `text` is too long for the marquee and needs to scroll.
pub fn marquee_scrolls(text: &str) -> bool {
    text.chars().count() > MARQUEE_CHARS
}

pub fn draw(canvas: &mut Canvas, skin: &Skin, state: &MainState) {
    use skin::*;
    let pressed = |c| state.pressed == Some(c);
    let pick = |c, up, down| if pressed(c) { down } else { up };
    let dragging = |s| state.dragging == Some(s);
    let stopped = state.play_state == PlayState::Stopped;

    canvas.fill(0);
    canvas.draw(skin, MAIN_BACKGROUND, 0, 0);

    // Title bar and its buttons.
    canvas.draw(skin, if state.focused { TITLE_BAR_SELECTED } else { TITLE_BAR }, 0, 0);
    canvas.draw(skin, pick(Control::Options, OPTIONS_BUTTON, OPTIONS_BUTTON_DEPRESSED), 6, 3);
    canvas.draw(skin, pick(Control::Minimize, MINIMIZE_BUTTON, MINIMIZE_BUTTON_DEPRESSED), 244, 3);
    canvas.draw(skin, pick(Control::Shade, SHADE_BUTTON, SHADE_BUTTON_DEPRESSED), 254, 3);
    canvas.draw(skin, pick(Control::Close, CLOSE_BUTTON, CLOSE_BUTTON_DEPRESSED), 264, 3);
    canvas.draw(skin, CLUTTER_BAR, 10, 22);

    // Play/pause/stop indicator.
    let indicator = match state.play_state {
        PlayState::Playing => PLAYING_INDICATOR,
        PlayState::Paused => PAUSED_INDICATOR,
        PlayState::Stopped => STOPPED_INDICATOR,
    };
    canvas.draw(skin, indicator, 26, 28);
    if state.play_state == PlayState::Playing {
        canvas.draw(skin, WORKING_INDICATOR, 24, 28);
    }

    // The time display is hidden while stopped and blinks while paused.
    if !stopped && state.show_time {
        match (state.time_remaining, state.duration) {
            (true, Some(duration)) => draw_time(canvas, skin, duration.saturating_sub(state.elapsed), true),
            _ => draw_time(canvas, skin, state.elapsed, false),
        }
    }

    draw_marquee(canvas, skin, state);

    // Bitrate and sample rate, hidden while stopped.
    if !stopped {
        if let Some(kbps) = state.kbps {
            // Three character cells; long values are cut, as Winamp does.
            let text: String = format!("{kbps:>3}").chars().take(3).collect();
            draw_text(canvas, skin, &text, 111, 43);
        }
        if let Some(khz) = state.khz {
            let text: String = format!("{khz:>2}").chars().take(2).collect();
            draw_text(canvas, skin, &text, 156, 43);
        }
    }

    // Neither mono nor stereo lights up while stopped.
    let (mono, stereo) = match (state.play_state, state.stereo) {
        (PlayState::Stopped, _) => (MONO, STEREO),
        (_, true) => (MONO, STEREO_SELECTED),
        (_, false) => (MONO_SELECTED, STEREO),
    };
    canvas.draw(skin, mono, 212, 41);
    canvas.draw(skin, stereo, 239, 41);

    // Volume: the background frame changes color with the level.
    let volume = state.volume.clamp(0.0, 1.0);
    let frame = (volume * (VOLUME_FRAMES - 1) as f32).round() as u32;
    canvas.draw(skin, volume_background(frame), 107, 57);
    let thumb = if dragging(Slider::Volume) { VOLUME_THUMB_SELECTED } else { VOLUME_THUMB };
    canvas.draw(skin, thumb, 107 + (volume * (68 - 14) as f32).round() as i32, 58);

    // Balance: the frame shows how far from center the slider is.
    let balance = state.balance.clamp(-1.0, 1.0);
    let frame = (balance.abs() * (VOLUME_FRAMES - 1) as f32).round() as u32;
    canvas.draw(skin, balance_background(frame), 177, 57);
    let thumb = if dragging(Slider::Balance) { BALANCE_THUMB_SELECTED } else { BALANCE_THUMB };
    canvas.draw(skin, thumb, 177 + ((balance + 1.0) / 2.0 * (38 - 14) as f32).round() as i32, 58);

    let eq = match (state.equalizer_open, pressed(Control::Equalizer)) {
        (false, false) => EQ_BUTTON,
        (true, false) => EQ_BUTTON_SELECTED,
        (false, true) => EQ_BUTTON_DEPRESSED,
        (true, true) => EQ_BUTTON_DEPRESSED_SELECTED,
    };
    canvas.draw(skin, eq, 219, 58);
    let pl = match (state.playlist_open, pressed(Control::Playlist)) {
        (false, false) => PLAYLIST_BUTTON,
        (true, false) => PLAYLIST_BUTTON_SELECTED,
        (false, true) => PLAYLIST_BUTTON_DEPRESSED,
        (true, true) => PLAYLIST_BUTTON_DEPRESSED_SELECTED,
    };
    canvas.draw(skin, pl, 242, 58);

    // Seek bar; the thumb only appears while a track with a known length is loaded.
    canvas.draw(skin, POSITION_BACKGROUND, 16, 72);
    if !stopped && state.duration.is_some() {
        let x = 16 + (state.position.clamp(0.0, 1.0) * (248 - 29) as f32).round() as i32;
        let thumb = if dragging(Slider::Position) { POSITION_THUMB_SELECTED } else { POSITION_THUMB };
        canvas.draw(skin, thumb, x, 72);
    }

    // Transport buttons.
    canvas.draw(skin, pick(Control::Previous, PREVIOUS_BUTTON, PREVIOUS_BUTTON_ACTIVE), 16, 88);
    canvas.draw(skin, pick(Control::Play, PLAY_BUTTON, PLAY_BUTTON_ACTIVE), 39, 88);
    canvas.draw(skin, pick(Control::Pause, PAUSE_BUTTON, PAUSE_BUTTON_ACTIVE), 62, 88);
    canvas.draw(skin, pick(Control::Stop, STOP_BUTTON, STOP_BUTTON_ACTIVE), 85, 88);
    canvas.draw(skin, pick(Control::Next, NEXT_BUTTON, NEXT_BUTTON_ACTIVE), 108, 88);
    canvas.draw(skin, pick(Control::Eject, EJECT_BUTTON, EJECT_BUTTON_ACTIVE), 136, 89);

    let shuffle = match (state.shuffle, pressed(Control::Shuffle)) {
        (false, false) => SHUFFLE_BUTTON,
        (false, true) => SHUFFLE_BUTTON_DEPRESSED,
        (true, false) => SHUFFLE_BUTTON_SELECTED,
        (true, true) => SHUFFLE_BUTTON_SELECTED_DEPRESSED,
    };
    canvas.draw(skin, shuffle, 164, 89);
    let repeat = match (state.repeat, pressed(Control::Repeat)) {
        (false, false) => REPEAT_BUTTON,
        (false, true) => REPEAT_BUTTON_DEPRESSED,
        (true, false) => REPEAT_BUTTON_SELECTED,
        (true, true) => REPEAT_BUTTON_SELECTED_DEPRESSED,
    };
    canvas.draw(skin, repeat, 210, 89);
}

/// Draws "mm:ss" with the skin's digit images, with a minus sign in front
/// for time remaining. Skins that include nums_ex.bmp use it in preference
/// to numbers.bmp.
fn draw_time(canvas: &mut Canvas, skin: &Skin, seconds: u32, negative: bool) {
    let sheet = if skin.sheet(Sheet::NumsEx).is_some() { Sheet::NumsEx } else { Sheet::Numbers };
    if negative {
        match sheet {
            Sheet::NumsEx => canvas.draw(skin, skin::MINUS_SIGN_EX, 38, 26),
            _ => canvas.draw(skin, skin::MINUS_SIGN, 38, 32),
        }
    }
    let minutes = (seconds / 60).min(99);
    let seconds = seconds % 60;
    for (n, x) in [(minutes / 10, 48), (minutes % 10, 60), (seconds / 10, 78), (seconds % 10, 90)] {
        canvas.draw(skin, skin::digit(sheet, n), x, 26);
    }
}

/// Draws the marquee, scrolled by `marquee_offset` characters if the text
/// is too long to fit.
fn draw_marquee(canvas: &mut Canvas, skin: &Skin, state: &MainState) {
    let text: String = if marquee_scrolls(&state.marquee) {
        let cycle = marquee_cycle(&state.marquee);
        (0..MARQUEE_CHARS).map(|i| cycle[(state.marquee_offset + i) % cycle.len()]).collect()
    } else {
        state.marquee.clone()
    };
    draw_text(canvas, skin, &text, 111, 27);
}

/// Draws text in the skin's 5x6 bitmap font.
fn draw_text(canvas: &mut Canvas, skin: &Skin, text: &str, x: i32, y: i32) {
    for (i, c) in text.chars().enumerate() {
        canvas.draw(skin, skin::character(c), x + (i as u32 * skin::CHAR_WIDTH) as i32, y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_testing() {
        assert_eq!(hit_test(40, 90), Hit::Control(Control::Play));
        assert_eq!(hit_test(265, 4), Hit::Control(Control::Close));
        assert_eq!(hit_test(120, 60), Hit::Slider(Slider::Volume));
        assert_eq!(hit_test(100, 75), Hit::Slider(Slider::Position));
        assert_eq!(hit_test(60, 30), Hit::Time);
        assert_eq!(hit_test(100, 5), Hit::TitleBar);
        assert_eq!(hit_test(100, 50), Hit::Nothing);
        assert_eq!(hit_test(-1, 5), Hit::Nothing);
    }

    #[test]
    fn slider_values_center_the_thumb_on_the_mouse() {
        assert_eq!(slider_value(Slider::Volume, 107 + 7), 0.0);
        assert_eq!(slider_value(Slider::Volume, 107 + 68 - 7), 1.0);
        assert_eq!(slider_value(Slider::Volume, 0), 0.0);
        assert_eq!(slider_value(Slider::Balance, 177 + 19), 0.0);
        assert_eq!(slider_value(Slider::Balance, 177), -1.0);
        assert_eq!(slider_value(Slider::Position, 300), 1.0);
    }

    #[test]
    fn long_titles_scroll_with_a_separator() {
        let short = "a".repeat(MARQUEE_CHARS);
        assert!(!marquee_scrolls(&short));
        let long = "b".repeat(MARQUEE_CHARS + 1);
        assert!(marquee_scrolls(&long));
        assert_eq!(marquee_cycle("ab"), "ab  ***  ".chars().collect::<Vec<_>>());
    }
}
