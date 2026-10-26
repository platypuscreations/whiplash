//! Sending samples to the sound card with cpal.
//!
//! cpal calls our callback on a high-priority audio thread whenever the
//! device needs more sound. That callback must never wait, so it only takes
//! samples that the decoder thread has already placed in a ring buffer
//! (a fixed-size, lock-free queue), and plays silence if there are none.

use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use ringbuf::HeapCons;
use ringbuf::traits::{Consumer, Observer};

use super::equalizer::Equalizer;
use super::{PlaybackShared, Settings};

/// Errors this soon after a stream opens are passing hiccups. On Linux,
/// PulseAudio's ALSA layer sometimes answers "I/O error" when asked how
/// much room there is, about 5 ms after a stream opens; measured, no sound
/// is lost, and the stream carries on.
const SETTLING: Duration = Duration::from_millis(250);
/// After that, an error is reported at most this often: one that keeps
/// happening could otherwise come many times a second.
const REPORT_EVERY: Duration = Duration::from_secs(1);

/// An open stream to the sound device. Sound stops when this is dropped.
pub struct Output {
    _stream: cpal::Stream,
    /// The rate the device is actually running at.
    pub sample_rate: u32,
}

/// Opens the default output device, at `preferred_rate` if it supports it.
pub fn open(
    preferred_rate: u32,
    consumer: HeapCons<f32>,
    shared: Arc<PlaybackShared>,
    settings: Arc<Settings>,
) -> Result<Output> {
    let device = cpal::default_host().default_output_device().context("no sound output device")?;

    // Prefer a stereo, floating-point configuration at the track's rate.
    let mut ranges: Vec<_> = device.supported_output_configs()?.collect();
    ranges.sort_by_key(|r| (r.channels() != 2, r.sample_format() != SampleFormat::F32));
    let supported = match ranges.into_iter().find_map(|r| r.try_with_sample_rate(preferred_rate)) {
        Some(config) => config,
        None => device.default_output_config()?,
    };
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let sample_rate = config.sample_rate;

    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, consumer, shared, settings),
        SampleFormat::F64 => build::<f64>(&device, &config, consumer, shared, settings),
        SampleFormat::I16 => build::<i16>(&device, &config, consumer, shared, settings),
        SampleFormat::I32 => build::<i32>(&device, &config, consumer, shared, settings),
        SampleFormat::U16 => build::<u16>(&device, &config, consumer, shared, settings),
        SampleFormat::U8 => build::<u8>(&device, &config, consumer, shared, settings),
        other => bail!("unsupported sample format {other}"),
    }?;
    stream.play()?;
    Ok(Output { _stream: stream, sample_rate })
}

/// Builds a stream for one sample type. `T` is the device's sample format;
/// we work in `f32` and convert at the last moment.
fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut consumer: HeapCons<f32>,
    shared: Arc<PlaybackShared>,
    settings: Arc<Settings>,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    // Allocated once here, so the callback itself never allocates.
    let mut scratch = vec![0.0f32; 16 * 1024];
    let mut equalizer = Equalizer::new(config.sample_rate);

    let callback = move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
        let frames = data.len() / channels;
        let (left_gain, right_gain) = settings.gains();

        // Take whole stereo frames only (an even number of samples), so
        // left and right never swap.
        let mut taken = 0;
        if !settings.paused.load(Relaxed) {
            let wanted = (frames * 2).min(scratch.len());
            let available = consumer.occupied_len() & !1;
            taken = consumer.pop_slice(&mut scratch[..wanted.min(available)]);
            shared.frames_played.fetch_add(taken as u64 / 2, Relaxed);

            equalizer.update(&settings.eq);
            for [left, right] in scratch[..taken].as_chunks_mut::<2>().0 {
                equalizer.process(left, right);
                // Boosting can push past full scale; clip rather than wrap.
                *left = left.clamp(-1.0, 1.0);
                *right = right.clamp(-1.0, 1.0);
            }

            if taken == 0 && shared.decoder_done.load(Relaxed) {
                shared.finished.store(true, Relaxed);
            }
            // Share what we play with the visualizer. `try_lock` never
            // waits: if the UI is reading right now, skip this batch.
            if let Ok(mut history) = settings.history.try_lock() {
                for [left, right] in scratch[..taken].as_chunks::<2>().0 {
                    history.push((left + right) / 2.0);
                }
            }
        }

        for (i, frame) in data.chunks_mut(channels).enumerate() {
            let (left, right) = if 2 * i + 1 < taken {
                (scratch[2 * i] * left_gain, scratch[2 * i + 1] * right_gain)
            } else {
                (0.0, 0.0) // paused, or the decoder hasn't kept up
            };
            match frame {
                [only] => *only = T::from_sample((left + right) / 2.0),
                [l, r, rest @ ..] => {
                    *l = T::from_sample(left);
                    *r = T::from_sample(right);
                    rest.iter_mut().for_each(|s| *s = T::from_sample(0.0f32));
                }
                [] => {}
            }
        }
    };

    let mut errors = ErrorReport::new(Instant::now());
    let on_error = move |err: cpal::Error| errors.error(&err.to_string(), Instant::now());
    Ok(device.build_output_stream(*config,callback, on_error, None)?)
}

/// How one stream reports the errors cpal passes on: to the console and
/// to `whiplash.log`.
struct ErrorReport {
    opened: Instant,
    /// A hiccup as the stream opened has been noted.
    noted_settling: bool,
    reported: Option<Instant>,
    /// Errors since the last report, not reported yet.
    held_back: u32,
}

/// What to do with an error.
#[derive(Debug, PartialEq)]
enum Report {
    /// Only note it in the log: a hiccup as the stream opened.
    Note(String),
    /// Print it and log it.
    Error(String),
    Nothing,
}

impl ErrorReport {
    fn new(opened: Instant) -> ErrorReport {
        ErrorReport { opened, noted_settling: false, reported: None, held_back: 0 }
    }

    fn error(&mut self, err: &str, now: Instant) {
        match self.judge(err, now) {
            Report::Note(text) => crate::log::write(&text),
            Report::Error(text) => {
                eprintln!("{text}");
                crate::log::write(&text);
            }
            Report::Nothing => {}
        }
    }

    fn judge(&mut self, err: &str, now: Instant) -> Report {
        if now.duration_since(self.opened) < SETTLING {
            return match std::mem::replace(&mut self.noted_settling, true) {
                false => Report::Note(format!("sound device hiccup as playback started (it carries on): {err}")),
                true => Report::Nothing,
            };
        }
        if self.reported.is_some_and(|at| now.duration_since(at) < REPORT_EVERY) {
            self.held_back += 1;
            return Report::Nothing;
        }
        self.reported = Some(now);
        let more = match std::mem::take(&mut self.held_back) {
            0 => String::new(),
            n => format!(" ({n} more since the last report)"),
        };
        Report::Error(format!("audio output error: {err}{more}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hiccups_as_a_stream_opens_are_only_noted() {
        let opened = Instant::now();
        let at = |ms: u64| opened + Duration::from_millis(ms);
        let mut errors = ErrorReport::new(opened);
        assert!(matches!(errors.judge("I/O error", at(5)), Report::Note(_)));
        assert_eq!(errors.judge("I/O error", at(6)), Report::Nothing);
    }

    #[test]
    fn later_errors_are_reported_but_not_too_often() {
        let opened = Instant::now();
        let at = |ms: u64| opened + Duration::from_millis(ms);
        let mut errors = ErrorReport::new(opened);
        assert_eq!(errors.judge("stuck", at(2000)), Report::Error("audio output error: stuck".into()));
        assert_eq!(errors.judge("stuck", at(2100)), Report::Nothing);
        assert_eq!(errors.judge("stuck", at(2200)), Report::Nothing);
        let later = errors.judge("stuck", at(3100));
        assert_eq!(later, Report::Error("audio output error: stuck (2 more since the last report)".into()));
    }
}
