//! Winamp's `.eqf` equalizer preset files (and `.q1` preset libraries, which
//! use the same format with several presets).
//!
//! Format (as documented by Webamp's `winamp-eqf` package):
//! - the header `Winamp EQ library file v1.1`, then the bytes `0x1A ! - -`;
//! - then for each preset: a 257-byte name padded with zero bytes, followed
//!   by 11 bytes for the ten bands and the preamp. Each byte is 64 minus a
//!   level from 1 (-12 dB) to 64 (+12 dB).

use anyhow::{Result, bail};

const HEADER: &[u8] = b"Winamp EQ library file v1.1\x1a!--";
const NAME_LEN: usize = 257;
const PRESET_LEN: usize = NAME_LEN + 11;

/// One preset. Values are slider positions from 0 (-12 dB) to 100 (+12 dB).
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    pub preamp: f32,
    pub bands: [f32; 10],
}

pub fn parse(bytes: &[u8]) -> Result<Vec<Preset>> {
    let Some(mut rest) = bytes.strip_prefix(HEADER) else { bail!("not a Winamp EQ preset file") };
    let mut presets = Vec::new();
    while rest.len() >= PRESET_LEN {
        let (preset, tail) = rest.split_at(PRESET_LEN);
        rest = tail;

        let name_bytes = &preset[..NAME_LEN];
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
        // Names are in Windows' old single-byte encoding; for the ASCII
        // range that covers almost every name, bytes map directly to chars.
        let name = name_bytes[..name_end].iter().map(|&b| b as char).collect();

        let values: Vec<f32> = preset[NAME_LEN..].iter().map(|&b| byte_to_slider(b)).collect();
        presets.push(Preset { name, bands: values[..10].try_into().unwrap(), preamp: values[10] });
    }
    if presets.is_empty() {
        bail!("the file contains no presets");
    }
    Ok(presets)
}

pub fn write(presets: &[Preset]) -> Vec<u8> {
    let mut bytes = HEADER.to_vec();
    for preset in presets {
        let mut name: Vec<u8> = preset.name.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
        name.resize(NAME_LEN, 0);
        name[NAME_LEN - 1] = 0; // keep it zero-terminated even if the name was too long
        bytes.extend(name);
        bytes.extend(preset.bands.iter().chain([&preset.preamp]).map(|&v| slider_to_byte(v)));
    }
    bytes
}

fn byte_to_slider(byte: u8) -> f32 {
    let level = (64 - byte.min(63) as i32) as f32; // 1..=64
    (level - 1.0) / 63.0 * 100.0
}

fn slider_to_byte(value: f32) -> u8 {
    let level = (value.clamp(0.0, 100.0) / 100.0 * 63.0).round() as u8 + 1;
    64 - level
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let presets = vec![
            Preset { name: "Rock".into(), preamp: 50.0, bands: [100.0, 80.0, 50.0, 30.0, 0.0, 10.0, 20.0, 60.0, 70.0, 90.0] },
            Preset { name: "Flat".into(), preamp: 50.0, bands: [50.0; 10] },
        ];
        let bytes = write(&presets);
        assert_eq!(bytes.len(), HEADER.len() + 2 * PRESET_LEN);
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "Rock");
        // Files store 64 steps, so values come back within one step.
        for (a, b) in parsed[0].bands.iter().zip(presets[0].bands) {
            assert!((a - b).abs() <= 100.0 / 63.0, "{a} vs {b}");
        }
        assert_eq!(parsed[0].bands[0], 100.0);
        assert_eq!(parsed[0].bands[4], 0.0);
    }

    #[test]
    fn extremes_match_the_format() {
        assert_eq!(slider_to_byte(100.0), 0); // +12 dB is stored as 0
        assert_eq!(slider_to_byte(0.0), 63); // -12 dB is stored as 63
    }

    #[test]
    fn rejects_other_files() {
        assert!(parse(b"hello").is_err());
        assert!(parse(HEADER).is_err());
    }
}
