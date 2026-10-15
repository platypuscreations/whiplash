//! The classic visualizer in the main window: a spectrum analyzer or an
//! oscilloscope, drawn in the skin's `viscolor.txt` colors. Clicking it
//! cycles analyzer → oscilloscope → off.
//!
//! The look and motion follow Webamp's recreation of Winamp's visualizer
//! (MIT license, https://github.com/captbaritone/webamp).

mod spectrum;

use std::time::{Duration, Instant};

use crate::skin::Skin;
use crate::ui::canvas::Canvas;
use spectrum::{BARS, FFT_SIZE, Spectrum};

pub const WIDTH: i32 = 76;
pub const HEIGHT: i32 = 16;

/// Bars and peaks move in fixed steps of 1/60 s, however often we draw.
const STEP: Duration = Duration::from_micros(16_667);
/// Pixels a bar drops per step when the sound gets quieter.
const BAR_FALL: f32 = 0.75;
/// Peaks hang in the air, then fall faster and faster: they start at this
/// speed (pixels per step) and speed up by `PEAK_ACCELERATION` each step.
const PEAK_START_SPEED: f32 = 3.0 / 256.0;
const PEAK_ACCELERATION: f32 = 1.1;
/// The oscilloscope shows this many of the latest samples (13 ms at 44.1 kHz).
const SCOPE_SAMPLES: usize = 576;

/// Indexes into the skin's 24 visualizer colors.
const COLOR_BACKGROUND: usize = 0;
const COLOR_DOTS: usize = 1;
const COLOR_BAR_TOP: usize = 2; // 2..=17 run from the top row to the bottom row
const COLOR_SCOPE: usize = 18; // 18..=22 run from the center line outwards
const COLOR_PEAK: usize = 23;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Analyzer,
    Oscilloscope,
    Off,
}

pub struct Visualizer {
    pub mode: Mode,
    spectrum: Spectrum,
    /// The latest sound, oldest first.
    samples: Vec<f32>,
    bars: [f32; BARS],
    peaks: [f32; BARS],
    peak_speeds: [f32; BARS],
    /// When bars and peaks last moved.
    last_step: Option<Instant>,
}

impl Visualizer {
    pub fn new() -> Visualizer {
        Visualizer {
            mode: Mode::Analyzer,
            spectrum: Spectrum::new(),
            samples: vec![0.0; FFT_SIZE],
            bars: [0.0; BARS],
            peaks: [0.0; BARS],
            peak_speeds: [0.0; BARS],
            last_step: None,
        }
    }

    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            Mode::Analyzer => Mode::Oscilloscope,
            Mode::Oscilloscope => Mode::Off,
            Mode::Off => Mode::Analyzer,
        };
    }

    /// Clears everything, so the next song starts from a blank display.
    pub fn reset(&mut self) {
        self.samples.fill(0.0);
        self.bars = [0.0; BARS];
        self.peaks = [0.0; BARS];
        self.last_step = None;
    }

    /// Takes in the latest sound and moves bars and peaks. `read` fills the
    /// buffer with recent samples and returns their sample rate.
    pub fn update(&mut self, now: Instant, read: impl FnOnce(&mut [f32]) -> Option<u32>) {
        let Some(sample_rate) = read(&mut self.samples) else { return };
        if self.mode != Mode::Analyzer {
            return;
        }
        let targets = self.spectrum.bars(&self.samples, sample_rate);

        // Catch up on however many 1/60 s steps have passed (at most a few,
        // so a stall doesn't make everything drop at once).
        let last = *self.last_step.get_or_insert(now);
        let steps = ((now - last).as_secs_f64() / STEP.as_secs_f64()) as u32;
        if steps == 0 {
            return;
        }
        self.last_step = Some(last + STEP * steps);
        for _ in 0..steps.min(8) {
            self.step(&targets);
        }
    }

    fn step(&mut self, targets: &[f32; BARS]) {
        let bars = self.bars.iter_mut().zip(&mut self.peaks).zip(&mut self.peak_speeds).zip(targets);
        for (((bar, peak), peak_speed), &target) in bars {
            // Bars jump up instantly but fall gradually.
            *bar = (*bar - BAR_FALL).max(target);
            if *bar >= *peak {
                *peak = *bar;
                *peak_speed = PEAK_START_SPEED;
            } else {
                *peak = (*peak - *peak_speed).max(0.0);
                *peak_speed *= PEAK_ACCELERATION;
            }
        }
    }

    /// Draws the visualizer with its top-left corner at (x, y). When off,
    /// nothing is drawn and the skin's own background shows through.
    pub fn draw(&self, canvas: &mut Canvas, skin: &Skin, x: i32, y: i32) {
        if self.mode == Mode::Off {
            return;
        }
        let colors = &skin.viscolor;

        // Background with a grid of dots on every other pixel.
        canvas.fill_rect(x, y, WIDTH as u32, HEIGHT as u32, colors[COLOR_BACKGROUND]);
        for dy in (1..HEIGHT).step_by(2) {
            for dx in (0..WIDTH).step_by(2) {
                canvas.set(x + dx, y + dy, colors[COLOR_DOTS]);
            }
        }

        match self.mode {
            Mode::Analyzer => self.draw_analyzer(canvas, colors, x, y),
            Mode::Oscilloscope => self.draw_oscilloscope(canvas, colors, x, y),
            Mode::Off => {}
        }
    }

    /// 19 bars, each 3 pixels wide with a 1 pixel gap. Each row has its own
    /// color, so tall bars show the whole gradient.
    fn draw_analyzer(&self, canvas: &mut Canvas, colors: &[u32; 24], x: i32, y: i32) {
        for (i, (&bar, &peak)) in self.bars.iter().zip(&self.peaks).enumerate() {
            let left = x + i as i32 * 4;
            let height = bar.round() as i32;
            for row in HEIGHT - height..HEIGHT {
                canvas.fill_rect(left, y + row, 3, 1, colors[COLOR_BAR_TOP + row as usize]);
            }
            let peak = peak.round() as i32;
            if peak >= 1 {
                let row = (HEIGHT - 1 - peak).max(0);
                canvas.fill_rect(left, y + row, 3, 1, colors[COLOR_PEAK]);
            }
        }
    }

    /// The waveform, one sample per column, joined by vertical lines.
    /// Colors fade from the center line outwards.
    fn draw_oscilloscope(&self, canvas: &mut Canvas, colors: &[u32; 24], x: i32, y: i32) {
        let recent = &self.samples[self.samples.len() - SCOPE_SAMPLES..];
        let spacing = SCOPE_SAMPLES / (WIDTH as usize - 1);
        let center = (HEIGHT - 1) as f32 / 2.0;
        let mut previous = None;

        for column in 0..WIDTH - 1 {
            let sample = recent[column as usize * spacing];
            // Full volume (±1.0) is twice the height: quiet music still shows.
            let row = (center - sample * HEIGHT as f32).round().clamp(0.0, (HEIGHT - 1) as f32) as i32;
            let distance = (row as f32 - center).abs().round() as usize;
            let color = colors[COLOR_SCOPE + (distance / 2).min(4)];

            // Join to the previous column so the line has no gaps.
            let (top, bottom) = match previous {
                Some(p) if p < row => (p + 1, row),
                Some(p) if p > row => (row, p - 1),
                _ => (row, row),
            };
            canvas.fill_rect(x + column, y + top, 1, (bottom - top + 1) as u32, color);
            previous = Some(row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_rise_instantly_and_fall_gradually() {
        let mut vis = Visualizer::new();
        let mut targets = [0.0; BARS];
        targets[3] = 10.0;
        vis.step(&targets);
        assert_eq!(vis.bars[3], 10.0);
        assert_eq!(vis.peaks[3], 10.0);

        targets[3] = 0.0;
        vis.step(&targets);
        assert_eq!(vis.bars[3], 10.0 - BAR_FALL);
        // The peak barely moves at first...
        assert!(vis.peaks[3] > 9.9);
        // ...then accelerates and falls faster than the bar.
        for _ in 0..60 {
            vis.step(&targets);
        }
        assert_eq!(vis.bars[3], 0.0);
        assert_eq!(vis.peaks[3], 0.0);
    }

    #[test]
    fn clicking_cycles_modes() {
        let mut vis = Visualizer::new();
        assert_eq!(vis.mode, Mode::Analyzer);
        vis.cycle_mode();
        assert_eq!(vis.mode, Mode::Oscilloscope);
        vis.cycle_mode();
        assert_eq!(vis.mode, Mode::Off);
        vis.cycle_mode();
        assert_eq!(vis.mode, Mode::Analyzer);
    }

    #[test]
    fn steps_follow_the_clock() {
        let mut vis = Visualizer::new();
        let start = Instant::now();
        let loud = |buf: &mut [f32]| {
            for (i, s) in buf.iter_mut().enumerate() {
                *s = (i as f32 * 0.15).sin() * 0.5;
            }
            Some(44_100)
        };
        vis.update(start, loud);
        assert!(vis.bars.iter().all(|&b| b == 0.0), "the first update only starts the clock");
        vis.update(start + STEP * 2, loud);
        assert!(vis.bars.iter().any(|&b| b > 5.0));
    }
}
