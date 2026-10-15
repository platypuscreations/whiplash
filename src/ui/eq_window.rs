//! The equalizer window: layout, hit-testing and drawing.
//!
//! On-screen positions come from Webamp's `equalizer-window.css` and
//! components (MIT license, https://github.com/captbaritone/webamp).

use super::canvas::Canvas;
use crate::skin::{self, Skin};

pub const WIDTH: u32 = 275;
pub const HEIGHT: u32 = 116;
pub const SHADE_HEIGHT: u32 = 14;

const TITLE_BAR_HEIGHT: i32 = 14;

/// Slider positions run from 0 (-12 dB) to 100 (+12 dB); 50 is flat.
pub const SLIDER_MAX: f32 = 100.0;
pub const SLIDER_FLAT: f32 = 50.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EqControl {
    Close,
    Shade,
    On,
    Auto,
    Presets,
    /// The "+12 dB", "0 dB" and "-12 dB" labels set every band at once.
    AllMax,
    AllFlat,
    AllMin,
}

/// (control, x, y, w, h)
const CONTROLS: [(EqControl, i32, i32, i32, i32); 8] = [
    (EqControl::Close, 264, 3, 9, 9),
    (EqControl::Shade, 254, 3, 9, 9),
    (EqControl::On, 14, 18, 26, 12),
    (EqControl::Auto, 40, 18, 32, 12),
    (EqControl::Presets, 217, 18, 44, 12),
    (EqControl::AllMax, 45, 36, 22, 8),
    (EqControl::AllFlat, 45, 64, 22, 8),
    (EqControl::AllMin, 45, 95, 22, 8),
];

/// Left edge of each slider: the preamp, then the ten bands.
const SLIDER_X: [i32; 11] = [21, 78, 96, 114, 132, 150, 168, 186, 204, 222, 240];
const SLIDER_Y: i32 = 38;
const SLIDER_WIDTH: i32 = 14;
const SLIDER_HEIGHT: i32 = 63;
/// The thumb travels this far: the 62 px track minus the 11 px thumb.
const THUMB_TRAVEL: f32 = 51.0;

const GRAPH_X: i32 = 86;
const GRAPH_Y: i32 = 17;
const GRAPH_HEIGHT: i32 = 19;
/// The curve passes through one point per band, 12 px apart.
const GRAPH_BAND_SPACING: f32 = 12.0;

/// Everything the equalizer window shows.
pub struct EqState {
    pub focused: bool,
    /// Shrunk to a strip ("windowshade" mode).
    pub shaded: bool,
    pub pressed: Option<EqControl>,
    /// The slider being dragged: 0 is the preamp, 1-10 are the bands.
    pub dragging: Option<usize>,
    pub enabled: bool,
    pub auto: bool,
    pub preamp: f32,
    pub bands: [f32; 10],
}

impl Default for EqState {
    fn default() -> Self {
        EqState {
            focused: false,
            shaded: false,
            pressed: None,
            dragging: None,
            enabled: true,
            auto: false,
            preamp: SLIDER_FLAT,
            bands: [SLIDER_FLAT; 10],
        }
    }
}

impl EqState {
    /// Slider `index`: 0 is the preamp, 1-10 are the bands.
    pub fn slider(&self, index: usize) -> f32 {
        if index == 0 { self.preamp } else { self.bands[index - 1] }
    }

    pub fn set_slider(&mut self, index: usize, value: f32) {
        let value = value.clamp(0.0, SLIDER_MAX);
        if index == 0 {
            self.preamp = value;
        } else {
            self.bands[index - 1] = value;
        }
    }
}

/// Converts a slider position to decibels.
pub fn to_db(value: f32) -> f32 {
    (value - SLIDER_FLAT) / SLIDER_FLAT * crate::audio::EQ_MAX_DB
}

/// Converts decibels to a slider position.
pub fn from_db(db: f32) -> f32 {
    (SLIDER_FLAT + db / crate::audio::EQ_MAX_DB * SLIDER_FLAT).clamp(0.0, SLIDER_MAX)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Control(EqControl),
    Slider(usize),
    /// The tiny volume and balance sliders in shade mode.
    ShadeVolume,
    ShadeBalance,
    TitleBar,
    Nothing,
}

/// Shade mode's tiny sliders: (x, y, width, height); thumbs are 3 wide.
const SHADE_VOLUME: (i32, i32, i32, i32) = (61, 4, 97, 6);
const SHADE_BALANCE: (i32, i32, i32, i32) = (164, 4, 43, 6);

pub fn hit_test(x: i32, y: i32, shaded: bool) -> Hit {
    let inside = |rx: i32, ry: i32, w: i32, h: i32| x >= rx && x < rx + w && y >= ry && y < ry + h;
    if shaded {
        let (vx, vy, vw, vh) = SHADE_VOLUME;
        let (bx, by, bw, bh) = SHADE_BALANCE;
        return if inside(264, 3, 9, 9) {
            Hit::Control(EqControl::Close)
        } else if inside(254, 3, 9, 9) {
            Hit::Control(EqControl::Shade)
        } else if inside(vx, vy, vw, vh) {
            Hit::ShadeVolume
        } else if inside(bx, by, bw, bh) {
            Hit::ShadeBalance
        } else if inside(0, 0, WIDTH as i32, SHADE_HEIGHT as i32) {
            Hit::TitleBar
        } else {
            Hit::Nothing
        };
    }
    for (control, cx, cy, w, h) in CONTROLS {
        if inside(cx, cy, w, h) {
            return Hit::Control(control);
        }
    }
    for (index, sx) in SLIDER_X.iter().enumerate() {
        if inside(*sx, SLIDER_Y, SLIDER_WIDTH, SLIDER_HEIGHT) {
            return Hit::Slider(index);
        }
    }
    if inside(0, 0, WIDTH as i32, TITLE_BAR_HEIGHT) { Hit::TitleBar } else { Hit::Nothing }
}

/// The slider value when the mouse is at window y-coordinate `y`, with the
/// thumb centered on the mouse. Up is louder.
pub fn slider_value(y: i32) -> f32 {
    let from_top = (y - SLIDER_Y) as f32 - 5.5;
    ((1.0 - from_top / THUMB_TRAVEL) * SLIDER_MAX).clamp(0.0, SLIDER_MAX)
}

/// Volume (0 to 1) for the mouse at `x` on shade mode's volume slider.
pub fn shade_volume_value(x: i32) -> f32 {
    let (sx, _, width, _) = SHADE_VOLUME;
    ((x - sx - 1) as f32 / (width - 3) as f32).clamp(0.0, 1.0)
}

/// Balance (-1 to 1) for the mouse at `x` on shade mode's balance slider.
pub fn shade_balance_value(x: i32) -> f32 {
    let (sx, _, width, _) = SHADE_BALANCE;
    ((x - sx - 1) as f32 / (width - 3) as f32).clamp(0.0, 1.0) * 2.0 - 1.0
}

/// Draws the window. Shade mode also shows the player's `volume` (0 to 1)
/// and `balance` (-1 to 1).
pub fn draw(canvas: &mut Canvas, skin: &Skin, state: &EqState, volume: f32, balance: f32) {
    use skin::*;
    let pressed = |c| state.pressed == Some(c);
    if state.shaded {
        canvas.fill(0);
        canvas.draw(skin, if state.focused { EQ_SHADE_BACKGROUND_SELECTED } else { EQ_SHADE_BACKGROUND }, 0, 0);
        if pressed(EqControl::Shade) {
            canvas.draw(skin, EQ_MAXIMIZE_BUTTON_ACTIVE, 254, 3);
        }
        let close = if pressed(EqControl::Close) { EQ_SHADE_CLOSE_BUTTON_ACTIVE } else { EQ_SHADE_CLOSE_BUTTON };
        canvas.draw(skin, close, 264, 3);
        // The thumbs change look across the left, middle and right thirds.
        let third = |fraction: f32| ((fraction * 3.0) as usize).min(2);
        let (vx, vy, vw, _) = SHADE_VOLUME;
        let volume = volume.clamp(0.0, 1.0);
        canvas.draw(skin, EQ_SHADE_VOLUME_THUMBS[third(volume)], vx + (volume * (vw - 3) as f32).round() as i32, vy);
        let (bx, by, bw, _) = SHADE_BALANCE;
        let balance = (balance.clamp(-1.0, 1.0) + 1.0) / 2.0;
        canvas.draw(skin, EQ_SHADE_BALANCE_THUMBS[third(balance)], bx + (balance * (bw - 3) as f32).round() as i32, by);
        return;
    }

    canvas.fill(0);
    canvas.draw(skin, EQ_BACKGROUND, 0, 0);
    canvas.draw(skin, if state.focused { EQ_TITLE_BAR_SELECTED } else { EQ_TITLE_BAR }, 0, 0);
    canvas.draw(skin, if pressed(EqControl::Close) { EQ_CLOSE_BUTTON_ACTIVE } else { EQ_CLOSE_BUTTON }, 264, 3);

    let on = match (state.enabled, pressed(EqControl::On)) {
        (false, false) => EQ_ON_BUTTON,
        (false, true) => EQ_ON_BUTTON_DEPRESSED,
        (true, false) => EQ_ON_BUTTON_SELECTED,
        (true, true) => EQ_ON_BUTTON_SELECTED_DEPRESSED,
    };
    canvas.draw(skin, on, 14, 18);
    let auto = match (state.auto, pressed(EqControl::Auto)) {
        (false, false) => EQ_AUTO_BUTTON,
        (false, true) => EQ_AUTO_BUTTON_DEPRESSED,
        (true, false) => EQ_AUTO_BUTTON_SELECTED,
        (true, true) => EQ_AUTO_BUTTON_SELECTED_DEPRESSED,
    };
    canvas.draw(skin, auto, 40, 18);
    let presets = if pressed(EqControl::Presets) { EQ_PRESETS_BUTTON_SELECTED } else { EQ_PRESETS_BUTTON };
    canvas.draw(skin, presets, 217, 18);

    draw_graph(canvas, skin, state);

    for (index, &x) in SLIDER_X.iter().enumerate() {
        let value = state.slider(index) / SLIDER_MAX;
        let frame = (value * (EQ_SLIDER_FRAMES - 1) as f32).round() as u32;
        canvas.draw(skin, eq_slider_background(frame), x, SLIDER_Y);
        let thumb = if state.dragging == Some(index) { EQ_SLIDER_THUMB_SELECTED } else { EQ_SLIDER_THUMB };
        let thumb_y = SLIDER_Y + ((1.0 - value) * THUMB_TRAVEL).round() as i32;
        canvas.draw(skin, thumb, x + 1, thumb_y);
    }
}

/// The little graph of the EQ curve: a smooth line through the ten band
/// settings, plus a flat line for the preamp.
fn draw_graph(canvas: &mut Canvas, skin: &Skin, state: &EqState) {
    canvas.draw(skin, skin::EQ_GRAPH_BACKGROUND, GRAPH_X, GRAPH_Y);
    let max_row = (GRAPH_HEIGHT - 1) as f32;
    let row_for = |value: f32| ((1.0 - value / SLIDER_MAX) * max_row).round().clamp(0.0, max_row) as i32;

    canvas.draw(skin, skin::EQ_PREAMP_LINE, GRAPH_X, GRAPH_Y + row_for(state.preamp));

    // The line's color at each height comes from a 1-pixel column in the skin.
    let colors = skin::EQ_GRAPH_LINE_COLORS;
    let color_at = |row: i32| {
        skin.sheet(colors.sheet).and_then(|sheet| sheet.get(colors.x, colors.y + row as u32)).unwrap_or(0xFFFFFF)
    };

    let curve = spline(&state.bands, GRAPH_BAND_SPACING);
    let mut previous = row_for(state.bands[0]);
    for (x, value) in curve.iter().enumerate() {
        let row = row_for(*value);
        // Fill between this column and the last so steep slopes stay joined.
        for r in row.min(previous)..=row.max(previous) {
            canvas.set(GRAPH_X + 2 + x as i32, GRAPH_Y + r, color_at(r));
        }
        previous = row;
    }
}

/// A "natural cubic spline": the smoothest curve through the points
/// (i * spacing, values[i]), sampled at every whole x from the first point
/// to the last.
fn spline(values: &[f32], spacing: f32) -> Vec<f32> {
    let n = values.len();
    let h = spacing;
    // Solve for the curve's second derivative at each point (zero at both
    // ends) with the tridiagonal "Thomas algorithm".
    let mut second = vec![0.0f32; n];
    if n > 2 {
        let mut diagonal = vec![4.0f32; n];
        let mut rhs: Vec<f32> = (0..n)
            .map(|i| if i == 0 || i == n - 1 { 0.0 } else { 6.0 * (values[i + 1] - 2.0 * values[i] + values[i - 1]) / (h * h) })
            .collect();
        for i in 2..n - 1 {
            let factor = 1.0 / diagonal[i - 1];
            diagonal[i] -= factor;
            rhs[i] -= factor * rhs[i - 1];
        }
        for i in (1..n - 1).rev() {
            second[i] = (rhs[i] - second[i + 1]) / diagonal[i];
        }
    }

    let last_x = ((n - 1) as f32 * h) as usize;
    (0..=last_x)
        .map(|x| {
            let x = x as f32;
            let i = ((x / h) as usize).min(n - 2);
            let (left, right) = (i as f32 * h, (i + 1) as f32 * h);
            let (a, b) = (right - x, x - left);
            second[i] * a.powi(3) / (6.0 * h)
                + second[i + 1] * b.powi(3) / (6.0 * h)
                + (values[i] - second[i] * h * h / 6.0) * a / h
                + (values[i + 1] - second[i + 1] * h * h / 6.0) * b / h
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_testing() {
        assert_eq!(hit_test(20, 20, false), Hit::Control(EqControl::On));
        assert_eq!(hit_test(230, 22, false), Hit::Control(EqControl::Presets));
        assert_eq!(hit_test(25, 60, false), Hit::Slider(0));
        assert_eq!(hit_test(245, 60, false), Hit::Slider(10));
        assert_eq!(hit_test(50, 66, false), Hit::Control(EqControl::AllFlat));
        assert_eq!(hit_test(100, 5, false), Hit::TitleBar);
        assert_eq!(hit_test(100, 110, false), Hit::Nothing);
        assert_eq!(hit_test(258, 5, false), Hit::Control(EqControl::Shade));
        assert_eq!(hit_test(100, 6, true), Hit::ShadeVolume);
        assert_eq!(hit_test(180, 6, true), Hit::ShadeBalance);
        assert_eq!(hit_test(30, 6, true), Hit::TitleBar);
        assert_eq!(shade_balance_value(164 + 21), 0.0);
    }

    #[test]
    fn slider_follows_the_mouse() {
        assert_eq!(slider_value(SLIDER_Y), SLIDER_MAX);
        assert_eq!(slider_value(SLIDER_Y + 200), 0.0);
        let middle = slider_value(SLIDER_Y + 5 + 26);
        assert!((middle - SLIDER_FLAT).abs() < 2.0, "{middle}");
    }

    #[test]
    fn decibels() {
        assert_eq!(to_db(SLIDER_MAX), 12.0);
        assert_eq!(to_db(SLIDER_FLAT), 0.0);
        assert_eq!(to_db(0.0), -12.0);
        assert_eq!(from_db(6.0), 75.0);
        assert_eq!(from_db(-20.0), 0.0);
    }

    #[test]
    fn spline_passes_through_every_point() {
        let values = [50.0, 80.0, 20.0, 50.0, 100.0, 0.0, 50.0, 50.0, 60.0, 40.0];
        let curve = spline(&values, 12.0);
        assert_eq!(curve.len(), 109);
        for (i, v) in values.iter().enumerate() {
            assert!((curve[i * 12] - v).abs() < 1e-3, "point {i}: {} vs {v}", curve[i * 12]);
        }
        // A flat line stays flat.
        assert!(spline(&[50.0; 10], 12.0).iter().all(|&v| (v - 50.0).abs() < 1e-4));
    }
}
