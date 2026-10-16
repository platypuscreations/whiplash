//! Reading audio files with symphonia and turning them into stereo samples.

use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use symphonia::core::codecs::audio::AudioDecoder;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::StandardTag;
use symphonia::core::units::{Time, TimeBase};

/// File extensions offered in the open-file dialog.
pub const EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "wav", "wave", "aif", "aiff", "aifc", "m4a", "mp4", "aac", "caf", "mka", "webm",
];

/// Facts about a track, for display.
#[derive(Clone, Debug)]
pub struct TrackInfo {
    pub path: PathBuf,
    /// "Artist - Title" from the file's tags, or the file name.
    pub title: String,
    /// Length in seconds, if the file says.
    pub duration: Option<f64>,
    pub sample_rate: u32,
    pub channels: usize,
    /// Average bitrate in kilobits per second.
    pub kbps: Option<u32>,
}

/// An open audio file, ready to be decoded packet by packet.
pub struct Track {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    pub info: TrackInfo,
    /// Frames still to throw away after a seek (see `seek`).
    skip_frames: usize,
    /// Scratch space reused for every packet, to avoid allocating.
    interleaved: Vec<f32>,
    stereo: Vec<f32>,
}

impl Track {
    pub fn open(path: &Path) -> Result<Track> {
        let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let file_size = file.metadata().map(|m| m.len()).ok();
        let source = MediaSourceStream::new(Box::new(file), Default::default());

        // The extension helps symphonia guess the format faster.
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let mut format = symphonia::default::get_probe()
            .probe(&hint, source, Default::default(), Default::default())
            .context("unsupported file format")?;

        let track = format.default_track(TrackType::Audio).context("no audio in file")?;
        let params = track.codec_params.as_ref().and_then(|p| p.audio()).context("unsupported codec")?;
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &Default::default())
            .context("unsupported codec")?;
        let sample_rate = params.sample_rate.context("unknown sample rate")?;
        let channels = params.channels.as_ref().map_or(2, |c| c.count());
        let track_id = track.id;
        let time_base = track.time_base;
        let duration = match (time_base, track.duration) {
            (Some(tb), Some(d)) => tb.calc_duration(d).map(|t| t.as_secs_f64()),
            _ => track.num_frames.map(|n| n as f64 / sample_rate as f64),
        };
        let kbps = match (file_size, duration) {
            (Some(bytes), Some(secs)) if secs > 0.0 => Some((bytes as f64 * 8.0 / secs / 1000.0).round() as u32),
            _ => None,
        };
        let title = read_title(format.as_mut()).unwrap_or_else(|| {
            path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
        });

        Ok(Track {
            format,
            decoder,
            track_id,
            time_base,
            info: TrackInfo { path: path.to_owned(), title, duration, sample_rate, channels, kbps },
            skip_frames: 0,
            interleaved: Vec::new(),
            stereo: Vec::new(),
        })
    }

    /// Decodes the next piece of audio as interleaved stereo samples
    /// (left, right, left, right...) at the file's sample rate. Returns
    /// `None` at the end of the file or on an error we can't skip past.
    pub fn next_chunk(&mut self) -> Option<&[f32]> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) | Err(_) => return None,
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(decoded) => decoded,
                // A damaged packet: skip it and carry on, like other players do.
                Err(Error::DecodeError(_) | Error::IoError(_)) => continue,
                Err(_) => return None,
            };

            let channels = decoded.spec().channels().count().max(1);
            self.interleaved.resize(decoded.samples_interleaved(), 0.0);
            decoded.copy_to_slice_interleaved(&mut self.interleaved);

            // Mono plays on both sides; beyond two channels we keep the
            // front left and right.
            // Drop frames from before a seek target.
            let frames = self.interleaved.len() / channels;
            let skip = self.skip_frames.min(frames);
            self.skip_frames -= skip;
            if skip == frames {
                continue;
            }

            self.stereo.clear();
            for frame in self.interleaved.chunks_exact(channels).skip(skip) {
                let left = frame[0];
                let right = if channels > 1 { frame[1] } else { left };
                self.stereo.extend([left, right]);
            }
            return Some(&self.stereo);
        }
    }

    /// Jumps to `seconds` from the start. Returns where it landed.
    ///
    /// Formats can only jump to the start of a packet, so the reader stops
    /// a little before the target (`actual_ts` is before `required_ts`). We
    /// then decode and discard the frames in between, so playback resumes
    /// exactly at the target.
    pub fn seek(&mut self, seconds: f64) -> Result<f64> {
        let time = Time::try_from_secs_f64(seconds.max(0.0)).context("invalid seek time")?;
        let seeked = self.format.seek(SeekMode::Accurate, SeekTo::Time { time, track_id: Some(self.track_id) })?;
        self.decoder.reset();

        let to_seconds = |ts| self.time_base.and_then(|tb| tb.calc_time(ts)).map(|t| t.as_secs_f64());
        match (to_seconds(seeked.actual_ts), to_seconds(seeked.required_ts)) {
            (Some(actual), Some(required)) => {
                let gap = (required - actual).max(0.0);
                self.skip_frames = (gap * self.info.sample_rate as f64).round() as usize;
                Ok(required)
            }
            (Some(actual), None) => {
                self.skip_frames = 0;
                Ok(actual)
            }
            _ => {
                self.skip_frames = 0;
                Ok(seconds)
            }
        }
    }
}

/// Builds "Artist - Title" from the file's tags, if it has a title.
fn read_title(format: &mut dyn FormatReader) -> Option<String> {
    let mut metadata = format.metadata();
    let revision = metadata.skip_to_latest()?;
    let tags = revision.media.tags.iter().chain(revision.per_track.iter().flat_map(|t| t.metadata.tags.iter()));

    let (mut artist, mut title) = (None, None);
    for tag in tags {
        match &tag.std {
            Some(StandardTag::Artist(a)) if artist.is_none() => artist = Some(a.trim().to_string()),
            Some(StandardTag::TrackTitle(t)) if title.is_none() => title = Some(t.trim().to_string()),
            _ => {}
        }
    }
    match (artist, title) {
        (Some(artist), Some(title)) if !artist.is_empty() && !title.is_empty() => Some(format!("{artist} - {title}")),
        (_, Some(title)) if !title.is_empty() => Some(title),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes a 16-bit PCM WAV file with `frames` frames of a rising ramp.
    fn write_wav(path: &Path, rate: u32, channels: u16, frames: u32) {
        let data_len = frames * channels as u32 * 2;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((36 + data_len).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes()); // PCM
        bytes.extend(channels.to_le_bytes());
        bytes.extend(rate.to_le_bytes());
        bytes.extend((rate * channels as u32 * 2).to_le_bytes());
        bytes.extend((channels * 2).to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(data_len.to_le_bytes());
        for i in 0..frames {
            for _ in 0..channels {
                bytes.extend(((i % 1000) as i16 * 16).to_le_bytes());
            }
        }
        std::fs::write(path, bytes).unwrap();
    }

    fn temp_wav(name: &str, rate: u32, channels: u16, frames: u32) -> PathBuf {
        let path = std::env::temp_dir().join(format!("whiplash-test-{}-{name}.wav", std::process::id()));
        write_wav(&path, rate, channels, frames);
        path
    }

    #[test]
    fn decodes_every_frame_as_stereo() {
        let path = temp_wav("mono", 8000, 1, 8000 * 2);
        let mut track = Track::open(&path).unwrap();
        assert_eq!(track.info.sample_rate, 8000);
        assert_eq!(track.info.channels, 1);
        assert_eq!(track.info.duration, Some(2.0));
        assert_eq!(track.info.title, path.file_stem().unwrap().to_string_lossy());

        let mut samples = 0;
        while let Some(chunk) = track.next_chunk() {
            // Mono is copied to both sides.
            assert!(chunk.chunks(2).all(|f| f[0] == f[1]));
            samples += chunk.len();
        }
        assert_eq!(samples, 8000 * 2 * 2);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn seeking_lands_near_the_target() {
        let path = temp_wav("seek", 8000, 2, 8000 * 3);
        let mut track = Track::open(&path).unwrap();
        let landed = track.seek(1.5).unwrap();
        assert!((landed - 1.5).abs() < 0.001, "landed at {landed}");

        let mut frames = 0;
        while let Some(chunk) = track.next_chunk() {
            frames += chunk.len() / 2;
        }
        assert!((frames as i64 - 12_000).abs() <= 1, "{frames} frames left");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn non_audio_file_is_an_error() {
        let path = std::env::temp_dir().join(format!("whiplash-test-{}.txt", std::process::id()));
        std::fs::write(&path, "hello").unwrap();
        assert!(Track::open(&path).is_err());
        std::fs::remove_file(path).ok();
    }
}
