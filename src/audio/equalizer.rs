//! The 10-band graphic equalizer.
//!
//! Each band is a "peaking" filter: it boosts or cuts a range of
//! frequencies around its center and leaves the rest alone. The filter
//! formulas are the standard ones from Robert Bristow-Johnson's "Audio EQ
//! Cookbook".
//!
//! The UI thread changes settings in `EqShared` (atomics, so no locks); the
//! audio thread notices the version number change and recomputes its
//! filters before the next batch of samples.

use std::f64::consts::PI;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU32};

/// Center frequencies of Winamp's ten bands.
pub const BAND_HZ: [f64; 10] = [60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0, 12000.0, 14000.0, 16000.0];
/// Sliders range from -12 dB to +12 dB.
pub const MAX_DB: f32 = 12.0;
/// How wide each band is. Higher Q means narrower.
const Q: f64 = 1.2;

/// Equalizer settings shared between the UI and audio threads.
pub struct EqShared {
    enabled: AtomicBool,
    /// Gains in decibels, stored as `f32` bit patterns.
    preamp_db: AtomicU32,
    bands_db: [AtomicU32; 10],
    /// Bumped on every change, so the audio thread knows to update.
    version: AtomicU32,
}

impl EqShared {
    pub fn new() -> EqShared {
        EqShared {
            enabled: AtomicBool::new(false),
            preamp_db: AtomicU32::new(0f32.to_bits()),
            bands_db: std::array::from_fn(|_| AtomicU32::new(0f32.to_bits())),
            version: AtomicU32::new(0),
        }
    }

    pub fn set(&self, enabled: bool, preamp_db: f32, bands_db: [f32; 10]) {
        self.enabled.store(enabled, Relaxed);
        self.preamp_db.store(preamp_db.to_bits(), Relaxed);
        for (slot, db) in self.bands_db.iter().zip(bands_db) {
            slot.store(db.to_bits(), Relaxed);
        }
        self.version.fetch_add(1, Relaxed);
    }
}

/// Coefficients of one biquad filter, normalized so a0 = 1.
#[derive(Clone, Copy)]
struct Coefficients {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Coefficients {
    const PASS_THROUGH: Coefficients = Coefficients { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 };

    fn peaking(center_hz: f64, gain_db: f64, sample_rate: f64) -> Coefficients {
        // A filter can't work at or above half the sample rate.
        if gain_db == 0.0 || center_hz >= sample_rate * 0.45 {
            return Coefficients::PASS_THROUGH;
        }
        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * center_hz / sample_rate;
        let alpha = w0.sin() / (2.0 * Q);
        let cos = w0.cos();
        let a0 = 1.0 + alpha / a;
        Coefficients {
            b0: (1.0 + alpha * a) / a0,
            b1: -2.0 * cos / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha / a) / a0,
        }
    }
}

/// The audio thread's side: filter coefficients plus each filter's memory
/// of recent samples (two numbers per filter per channel).
pub struct Equalizer {
    sample_rate: f64,
    seen_version: Option<u32>,
    enabled: bool,
    preamp: f64,
    filters: [Coefficients; 10],
    memory: [[[f64; 2]; 10]; 2],
}

impl Equalizer {
    pub fn new(sample_rate: u32) -> Equalizer {
        Equalizer {
            sample_rate: sample_rate as f64,
            seen_version: None,
            enabled: false,
            preamp: 1.0,
            filters: [Coefficients::PASS_THROUGH; 10],
            memory: [[[0.0; 2]; 10]; 2],
        }
    }

    /// Picks up new settings if they changed. Call before each batch.
    pub fn update(&mut self, shared: &EqShared) {
        let version = shared.version.load(Relaxed);
        if self.seen_version == Some(version) {
            return;
        }
        self.seen_version = Some(version);
        self.enabled = shared.enabled.load(Relaxed);
        self.preamp = 10f64.powf(f32::from_bits(shared.preamp_db.load(Relaxed)) as f64 / 20.0);
        for (filter, (db, hz)) in self.filters.iter_mut().zip(shared.bands_db.iter().zip(BAND_HZ)) {
            let db = f32::from_bits(db.load(Relaxed)) as f64;
            *filter = Coefficients::peaking(hz, db, self.sample_rate);
        }
    }

    /// Filters one stereo frame in place.
    pub fn process(&mut self, left: &mut f32, right: &mut f32) {
        if !self.enabled {
            return;
        }
        for (channel, sample) in [left, right].into_iter().enumerate() {
            let mut x = *sample as f64 * self.preamp;
            for (f, memory) in self.filters.iter().zip(&mut self.memory[channel]) {
                // "Transposed direct form II": the usual way to run a biquad.
                let y = f.b0 * x + memory[0];
                memory[0] = f.b1 * x - f.a1 * y + memory[1];
                memory[1] = f.b2 * x - f.a2 * y;
                x = y;
            }
            *sample = x as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays a sine wave through the equalizer and measures how much
    /// louder (in dB) it comes out.
    fn gain_at(hz: f64, eq: &mut Equalizer) -> f64 {
        let rate = 44_100.0;
        let mut peak_in: f64 = 0.0;
        let mut peak_out: f64 = 0.0;
        for i in 0..44_100 {
            let s = (2.0 * PI * hz * i as f64 / rate).sin() as f32 * 0.1;
            let (mut l, mut r) = (s, s);
            eq.process(&mut l, &mut r);
            if i > 22_050 {
                // ignore the first half second while the filters settle
                peak_in = peak_in.max(s.abs() as f64);
                peak_out = peak_out.max(l.abs() as f64);
            }
        }
        20.0 * (peak_out / peak_in).log10()
    }

    fn equalizer(enabled: bool, preamp: f32, bands: [f32; 10]) -> Equalizer {
        let shared = EqShared::new();
        shared.set(enabled, preamp, bands);
        let mut eq = Equalizer::new(44_100);
        eq.update(&shared);
        eq
    }

    #[test]
    fn flat_settings_change_nothing() {
        let mut eq = equalizer(true, 0.0, [0.0; 10]);
        assert!(gain_at(1000.0, &mut eq).abs() < 0.01);
    }

    #[test]
    fn a_boosted_band_boosts_its_frequency_only() {
        let mut bands = [0.0; 10];
        bands[4] = 12.0; // 1 kHz
        assert!((gain_at(1000.0, &mut equalizer(true, 0.0, bands)) - 12.0).abs() < 0.2);
        assert!(gain_at(100.0, &mut equalizer(true, 0.0, bands)).abs() < 1.0);
        bands[4] = -12.0;
        assert!((gain_at(1000.0, &mut equalizer(true, 0.0, bands)) + 12.0).abs() < 0.2);
    }

    #[test]
    fn preamp_scales_everything() {
        let mut eq = equalizer(true, -6.0, [0.0; 10]);
        assert!((gain_at(440.0, &mut eq) + 6.0).abs() < 0.05);
    }

    #[test]
    fn disabled_equalizer_is_bypassed() {
        let mut eq = equalizer(false, -12.0, [12.0; 10]);
        assert!(gain_at(1000.0, &mut eq).abs() < 0.01);
    }
}
