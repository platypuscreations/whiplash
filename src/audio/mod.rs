//! Audio playback.
//!
//! Three threads cooperate while a track plays:
//! - the UI thread owns the `Player` and calls play/pause/seek on it;
//! - a decoder thread reads the file and fills a ring buffer with samples;
//! - cpal's audio thread empties the ring buffer into the sound card.
//!
//! They share a few numbers (volume, paused, frames played) through atomics,
//! which can be read and written from any thread without locking.
//!
//! Each start or seek creates a fresh `Playback` (decoder thread + output
//! stream) and stops the old one. That keeps things simple: no buffer ever
//! has to be flushed while another thread is using it.

mod decode;
mod equalizer;
mod output;
mod resample;

pub use decode::{EXTENSIONS, TrackInfo};
pub use equalizer::{BAND_HZ as EQ_BAND_HZ, MAX_DB as EQ_MAX_DB};

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};
use ringbuf::traits::{Producer, Split};
use ringbuf::{HeapProd, HeapRb};

use decode::Track;
use equalizer::EqShared;
use resample::Resampler;

/// How much decoded audio is kept ready ahead of the sound card.
const BUFFER_SECONDS: f64 = 0.2;

/// Opening the sound device can fail for a moment while the system
/// rearranges its devices (a monitor with speakers was unplugged, say), so
/// it's tried this many times, this far apart, before giving up.
const OPEN_ATTEMPTS: u32 = 3;
const OPEN_RETRY_DELAY: Duration = Duration::from_millis(250);
/// How many of the most recently played samples are kept for visualizers.
const HISTORY_LEN: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayState {
    Stopped,
    Playing,
    Paused,
}

/// Settings that outlive individual tracks. An `f32` is stored in an
/// `AtomicU32` by its bit pattern, since there is no atomic float type.
pub struct Settings {
    volume: AtomicU32,
    balance: AtomicU32,
    paused: AtomicBool,
    /// The sound most recently sent to the sound card, for visualizers.
    history: Mutex<History>,
    eq: EqShared,
}

impl Settings {
    /// Left and right gain from the volume and balance settings.
    fn gains(&self) -> (f32, f32) {
        let volume = f32::from_bits(self.volume.load(Relaxed));
        let balance = f32::from_bits(self.balance.load(Relaxed));
        // Squaring makes the slider feel even: hearing is roughly logarithmic.
        let gain = volume * volume;
        (gain * (1.0 - balance).min(1.0), gain * (1.0 + balance).min(1.0))
    }
}

/// The last `HISTORY_LEN` samples played, mixed down to mono, before
/// volume and balance (so visualizers don't shrink when you turn it down).
pub struct History {
    samples: Vec<f32>,
    /// Where the next sample will be written; the oldest sample is here too.
    next: usize,
    /// How many samples have ever been pushed, so readers can ask for only
    /// the ones they haven't seen.
    total: u64,
}

impl History {
    fn new() -> History {
        History { samples: vec![0.0; HISTORY_LEN], next: 0, total: 0 }
    }

    fn push(&mut self, sample: f32) {
        self.samples[self.next] = sample;
        self.next = (self.next + 1) % self.samples.len();
        self.total += 1;
    }

    /// Appends the samples pushed since `cursor` to `out` (at most the
    /// whole history, if the reader fell far behind) and moves `cursor`
    /// up to date.
    fn since(&self, cursor: &mut u64, out: &mut Vec<f32>) {
        let len = self.samples.len();
        let count = (self.total.saturating_sub(*cursor)).min(len as u64) as usize;
        let start = self.next + len - count;
        out.extend((0..count).map(|i| self.samples[(start + i) % len]));
        *cursor = self.total;
    }

    /// Fills `out` with the most recent samples, oldest first.
    fn latest(&self, out: &mut [f32]) {
        let len = self.samples.len();
        let count = out.len().min(len);
        let start = self.next + len - count;
        for (i, slot) in out.iter_mut().take(count).enumerate() {
            *slot = self.samples[(start + i) % len];
        }
    }
}

/// State for one playback, shared between its three threads.
#[derive(Default)]
pub struct PlaybackShared {
    /// Frames the sound card has been given, to compute the position.
    frames_played: AtomicU64,
    /// Set by the decoder thread at the end of the file.
    decoder_done: AtomicBool,
    /// Set by the output once the decoder is done and the buffer is empty.
    finished: AtomicBool,
    /// Set by the UI thread to make the decoder thread exit.
    stop: AtomicBool,
}

/// One track playing from a given position.
struct Playback {
    shared: Arc<PlaybackShared>,
    output: Option<output::Output>,
    decoder: Option<JoinHandle<Track>>,
    /// Where in the track this playback started, in seconds.
    start: f64,
}

impl Playback {
    fn start(track: Track, start: f64, settings: &Arc<Settings>) -> Result<Playback> {
        let shared = Arc::new(PlaybackShared::default());
        let rate = track.info.sample_rate;
        let capacity = ((rate as f64 * BUFFER_SECONDS) as usize * 2).max(4096);
        let mut attempt = 1;
        let (output, producer) = loop {
            let (producer, consumer) = HeapRb::<f32>::new(capacity).split();
            match output::open(rate, consumer, shared.clone(), settings.clone()) {
                Ok(output) => break (output, producer),
                Err(err) if attempt < OPEN_ATTEMPTS => {
                    eprintln!("opening the sound device, attempt {attempt}: {err:#}");
                    attempt += 1;
                    thread::sleep(OPEN_RETRY_DELAY);
                }
                Err(err) => return Err(err).context("couldn't open the sound device"),
            }
        };
        let output_rate = output.sample_rate;
        let thread_shared = shared.clone();
        let decoder = thread::Builder::new()
            .name("decoder".into())
            .spawn(move || decode_loop(track, producer, thread_shared, output_rate))?;

        Ok(Playback { shared, output: Some(output), decoder: Some(decoder), start })
    }

    fn position(&self) -> f64 {
        let rate = self.output.as_ref().map_or(44_100, |o| o.sample_rate);
        self.start + self.shared.frames_played.load(Relaxed) as f64 / rate as f64
    }

    /// Stops playing and hands back the track, so it can be reused (to seek).
    fn stop(mut self) -> Option<Track> {
        self.shared.stop.store(true, Relaxed);
        self.output = None; // closes the sound device
        self.decoder.take()?.join().ok()
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.shared.stop.store(true, Relaxed);
        if let Some(decoder) = self.decoder.take() {
            let _ = decoder.join();
        }
    }
}

/// Runs on the decoder thread: decode, resample if needed, and push into the
/// ring buffer, waiting whenever it is full. Returns the track when stopped.
fn decode_loop(mut track: Track, mut producer: HeapProd<f32>, shared: Arc<PlaybackShared>, output_rate: u32) -> Track {
    let mut resampler = (track.info.sample_rate != output_rate).then(|| Resampler::new(track.info.sample_rate, output_rate));
    let mut resampled = Vec::new();

    'decoding: while !shared.stop.load(Relaxed) {
        let Some(chunk) = track.next_chunk() else {
            shared.decoder_done.store(true, Relaxed);
            break;
        };
        let mut samples = match &mut resampler {
            Some(resampler) => {
                resampled.clear();
                resampler.process(chunk, &mut resampled);
                &resampled[..]
            }
            None => chunk,
        };
        while !samples.is_empty() {
            if shared.stop.load(Relaxed) {
                break 'decoding;
            }
            let pushed = producer.push_slice(samples);
            samples = &samples[pushed..];
            if pushed == 0 {
                thread::sleep(Duration::from_millis(5)); // buffer full; let the sound card catch up
            }
        }
    }
    track
}

/// Reads a file's title, length and format without playing it.
pub fn read_info(path: &Path) -> Result<TrackInfo> {
    Ok(Track::open(path)?.info)
}

/// The music player, as seen by the UI.
pub struct Player {
    settings: Arc<Settings>,
    playback: Option<Playback>,
    info: Option<TrackInfo>,
}

impl Player {
    pub fn new() -> Player {
        Player {
            settings: Arc::new(Settings {
                volume: AtomicU32::new(1.0f32.to_bits()),
                balance: AtomicU32::new(0.0f32.to_bits()),
                paused: AtomicBool::new(false),
                history: Mutex::new(History::new()),
                eq: EqShared::new(),
            }),
            playback: None,
            info: None,
        }
    }

    /// Opens a file and starts playing it from the beginning.
    pub fn open(&mut self, path: &Path) -> Result<()> {
        self.stop();
        let track = Track::open(path)?;
        self.info = Some(track.info.clone());
        self.playback = Some(Playback::start(track, 0.0, &self.settings)?);
        Ok(())
    }

    /// Resumes if paused, otherwise (re)starts the current track.
    pub fn play(&mut self) -> Result<()> {
        if self.state() == PlayState::Paused {
            self.settings.paused.store(false, Relaxed);
            return Ok(());
        }
        match self.info.as_ref().and_then(|info| info.location.file()).map(Path::to_path_buf) {
            Some(path) => self.open(&path),
            None => Ok(()),
        }
    }

    /// Toggles pause. Does nothing while stopped.
    pub fn pause(&mut self) {
        if self.playback.is_some() {
            self.settings.paused.fetch_xor(true, Relaxed);
        }
    }

    pub fn stop(&mut self) {
        if let Some(playback) = self.playback.take() {
            playback.stop();
        }
        self.settings.paused.store(false, Relaxed);
    }

    /// Stops and forgets the current track, as if none had been opened.
    pub fn clear(&mut self) {
        self.stop();
        self.info = None;
    }

    /// Jumps to `seconds` into the current track, keeping paused/playing.
    pub fn seek(&mut self, seconds: f64) -> Result<()> {
        let Some(playback) = self.playback.take() else { return Ok(()) };
        let current = playback.position();
        let Some(mut track) = playback.stop() else { return Ok(()) };
        let landed = track.seek(seconds).unwrap_or(current);
        self.playback = Some(Playback::start(track, landed, &self.settings)?);
        Ok(())
    }

    pub fn state(&self) -> PlayState {
        match &self.playback {
            None => PlayState::Stopped,
            Some(_) if self.settings.paused.load(Relaxed) => PlayState::Paused,
            Some(_) => PlayState::Playing,
        }
    }

    /// Seconds into the current track.
    pub fn position(&self) -> f64 {
        self.playback.as_ref().map_or(0.0, Playback::position)
    }

    /// Copies the most recently played sound into `out` (mono, oldest
    /// first) and returns its sample rate, or `None` while stopped.
    pub fn recent_samples(&self, out: &mut [f32]) -> Option<u32> {
        let rate = self.playback.as_ref()?.output.as_ref()?.sample_rate;
        self.settings.history.lock().ok()?.latest(out);
        Some(rate)
    }

    /// Appends the sound played since `cursor` (mono) to `out`, for
    /// visualizers that need every sample once, like MilkDrop.
    #[cfg_attr(not(feature = "milkdrop"), allow(dead_code))]
    pub fn samples_since(&self, cursor: &mut u64, out: &mut Vec<f32>) {
        if let Ok(history) = self.settings.history.lock() {
            history.since(cursor, out);
        }
    }

    /// True once the current track has played to its end.
    pub fn finished(&self) -> bool {
        self.playback.as_ref().is_some_and(|p| p.shared.finished.load(Relaxed))
    }

    /// The most recently opened track, kept after stopping.
    pub fn info(&self) -> Option<&TrackInfo> {
        self.info.as_ref()
    }

    /// 0.0 to 1.0.
    pub fn set_volume(&self, volume: f32) {
        self.settings.volume.store(volume.clamp(0.0, 1.0).to_bits(), Relaxed);
    }

    /// Equalizer settings: preamp and the ten bands in decibels
    /// (-12 to +12). Takes effect within a few milliseconds.
    pub fn set_equalizer(&self, enabled: bool, preamp_db: f32, bands_db: [f32; 10]) {
        self.settings.eq.set(enabled, preamp_db, bands_db);
    }

    /// -1.0 (left) to 1.0 (right).
    pub fn set_balance(&self, balance: f32) {
        self.settings.balance.store(balance.clamp(-1.0, 1.0).to_bits(), Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_returns_the_latest_samples_in_order() {
        let mut history = History::new();
        for i in 0..HISTORY_LEN + 10 {
            history.push(i as f32);
        }
        let mut out = [0.0; 3];
        history.latest(&mut out);
        let last = (HISTORY_LEN + 9) as f32;
        assert_eq!(out, [last - 2.0, last - 1.0, last]);
    }

    #[test]
    fn readers_get_each_new_sample_once() {
        let mut history = History::new();
        let mut cursor = 0;
        let mut out = Vec::new();
        for i in 0..5 {
            history.push(i as f32);
        }
        history.since(&mut cursor, &mut out);
        assert_eq!(out, [0.0, 1.0, 2.0, 3.0, 4.0]);
        out.clear();
        history.since(&mut cursor, &mut out);
        assert!(out.is_empty());
        history.push(5.0);
        history.since(&mut cursor, &mut out);
        assert_eq!(out, [5.0]);

        // A reader that falls far behind gets the most recent history only.
        out.clear();
        for i in 0..HISTORY_LEN * 2 {
            history.push(i as f32);
        }
        history.since(&mut cursor, &mut out);
        assert_eq!(out.len(), HISTORY_LEN);
        assert_eq!(*out.last().unwrap(), (HISTORY_LEN * 2 - 1) as f32);
    }

    #[test]
    fn balance_turns_down_the_opposite_side() {
        let player = Player::new();
        player.set_volume(1.0);
        player.set_balance(-0.5);
        assert_eq!(player.settings.gains(), (1.0, 0.5));
        player.set_balance(1.0);
        assert_eq!(player.settings.gains(), (0.0, 1.0));
        player.set_volume(0.5);
        player.set_balance(0.0);
        assert_eq!(player.settings.gains(), (0.25, 0.25));
    }
}

/// Plays real files through the sound card. Needs an audio device, so it
/// only runs on request:
///
/// ```sh
/// WHIPLASH_TEST_AUDIO=path/to/folder cargo test -- --ignored --nocapture
/// ```
#[cfg(test)]
mod device_tests {
    use super::*;

    #[test]
    #[ignore]
    fn plays_pauses_and_seeks_real_files() {
        let dir = std::env::var("WHIPLASH_TEST_AUDIO").expect("set WHIPLASH_TEST_AUDIO to a folder of audio files");
        let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
        files.sort();
        let mut player = Player::new();
        player.set_volume(0.3);

        for file in files {
            player.open(&file).unwrap_or_else(|e| panic!("{}: {e:#}", file.display()));
            let info = player.info().unwrap().clone();
            println!(
                "{}: \"{}\" {:?}s {} Hz {} ch {:?} kbps",
                file.file_name().unwrap().to_string_lossy(),
                info.title,
                info.duration.map(|d| (d * 100.0).round() / 100.0),
                info.sample_rate,
                info.channels,
                info.kbps
            );

            thread::sleep(Duration::from_millis(600));
            let played = player.position();
            assert!((0.4..0.8).contains(&played), "position after 0.6 s was {played}");

            player.pause();
            assert_eq!(player.state(), PlayState::Paused);
            let paused_at = player.position();
            thread::sleep(Duration::from_millis(300));
            assert!((player.position() - paused_at).abs() < 0.02, "position moved while paused");
            player.pause();

            let duration = info.duration.unwrap();
            player.seek(duration - 0.5).unwrap();
            assert!((player.position() - (duration - 0.5)).abs() < 0.1, "seek landed at {}", player.position());
            thread::sleep(Duration::from_millis(1000));
            assert!(player.finished(), "track should have finished");
            println!("  ok: played, paused, seeked and finished");
        }
        player.stop();
        assert_eq!(player.state(), PlayState::Stopped);
    }
}
