//! Turning a slice of sound into spectrum-analyzer bar heights.
//!
//! An FFT (fast Fourier transform) splits sound into how loud each
//! frequency is. Bars are spaced logarithmically, like a piano keyboard, so
//! bass and treble each get a fair share of the 19 bars.

use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

/// Samples per analysis: about 46 ms at 44.1 kHz.
pub const FFT_SIZE: usize = 2048;
pub const BARS: usize = 19;
/// Bars stop one pixel short of the top so the peak dot stays visible.
pub const MAX_HEIGHT: f32 = 15.0;

/// Frequency range covered by the bars.
const LOWEST_HZ: f32 = 50.0;
const HIGHEST_HZ: f32 = 16_000.0;
/// Loudness range shown: a full-scale tone fills the bar, and sound this
/// many decibels quieter shows nothing.
const RANGE_DB: f32 = 60.0;
/// Music is much quieter in the treble than the bass. Boosting higher bars
/// by this much per octave above 1 kHz keeps the display lively across
/// the whole range.
const TILT_DB_PER_OCTAVE: f32 = 3.0;

pub struct Spectrum {
    fft: Arc<dyn Fft<f32>>,
    /// A Hann window: fades each slice in and out so its abrupt edges don't
    /// smear energy across all frequencies.
    window: Vec<f32>,
    window_sum: f32,
    buffer: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
}

impl Spectrum {
    pub fn new() -> Spectrum {
        let fft = FftPlanner::new().plan_fft_forward(FFT_SIZE);
        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos())
            .collect();
        let window_sum = window.iter().sum();
        let scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
        Spectrum { fft, window, window_sum, buffer: vec![Complex::default(); FFT_SIZE], scratch }
    }

    /// Bar heights (0 to `MAX_HEIGHT`) for the last `FFT_SIZE` samples.
    pub fn bars(&mut self, samples: &[f32], sample_rate: u32) -> [f32; BARS] {
        let samples = &samples[samples.len().saturating_sub(FFT_SIZE)..];
        self.buffer.fill(Complex::default());
        for ((slot, sample), w) in self.buffer.iter_mut().zip(samples).zip(&self.window) {
            *slot = Complex::new(sample * w, 0.0);
        }
        self.fft.process_with_scratch(&mut self.buffer, &mut self.scratch);

        let bin_hz = sample_rate as f32 / FFT_SIZE as f32;
        let last_bin = FFT_SIZE / 2 - 1;
        let mut heights = [0.0; BARS];
        for (bar, height) in heights.iter_mut().enumerate() {
            let (low, high) = (band_edge(bar), band_edge(bar + 1));
            let first = ((low / bin_hz).round() as usize).clamp(1, last_bin);
            let last = ((high / bin_hz).round() as usize).clamp(first, last_bin);
            let strongest = self.buffer[first..=last].iter().map(|c| c.norm()).fold(0.0, f32::max);

            // Scale so a full-scale sine wave measures 1.0 (0 dB).
            let amplitude = strongest * 2.0 / self.window_sum;
            let center = (low * high).sqrt();
            let db = 20.0 * amplitude.max(1e-9).log10() + TILT_DB_PER_OCTAVE * (center / 1000.0).log2();
            *height = ((db + RANGE_DB) / RANGE_DB * MAX_HEIGHT).clamp(0.0, MAX_HEIGHT);
        }
        heights
    }
}

/// The lower frequency edge of bar `i`; bar `i` ends where bar `i + 1` starts.
fn band_edge(i: usize) -> f32 {
    LOWEST_HZ * (HIGHEST_HZ / LOWEST_HZ).powf(i as f32 / BARS as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, amplitude: f32) -> Vec<f32> {
        (0..FFT_SIZE).map(|i| amplitude * (2.0 * std::f32::consts::PI * hz * i as f32 / 44_100.0).sin()).collect()
    }

    fn bar_for(hz: f32) -> usize {
        (0..BARS).find(|&b| band_edge(b) <= hz && hz < band_edge(b + 1)).unwrap()
    }

    #[test]
    fn silence_shows_nothing() {
        let bars = Spectrum::new().bars(&vec![0.0; FFT_SIZE], 44_100);
        assert!(bars.iter().all(|&h| h == 0.0));
    }

    #[test]
    fn a_tone_lights_its_own_bar_and_not_distant_ones() {
        let mut spectrum = Spectrum::new();
        let bars = spectrum.bars(&sine(1000.0, 0.5), 44_100);
        let bar = bar_for(1000.0);
        // Half amplitude is -6 dB: just under full height.
        assert!(bars[bar] > 12.0, "tone bar was {}", bars[bar]);
        assert!(bars[0] < 1.0 && bars[BARS - 1] < 1.0, "{bars:?}");
    }

    #[test]
    fn quieter_tones_make_shorter_bars() {
        let mut spectrum = Spectrum::new();
        let bar = bar_for(400.0);
        let loud = spectrum.bars(&sine(400.0, 0.5), 44_100)[bar];
        let quiet = spectrum.bars(&sine(400.0, 0.05), 44_100)[bar];
        // 20 dB quieter is a third of the 60 dB range: about 5 pixels.
        assert!((loud - quiet - 5.0).abs() < 0.5, "loud {loud}, quiet {quiet}");
    }
}
