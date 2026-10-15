//! Parsing `pledit.txt`: the playlist's text colors and font.
//!
//! It's a Windows INI file, usually:
//!
//! ```text
//! [Text]
//! Normal=#00FF00
//! Current=#FFFFFF
//! NormalBG=#000000
//! SelectedBG=#0000C6
//! Font=Arial
//! ```
//!
//! Skins are sloppy with it (missing `#`, odd case, extra text after the
//! color), so anything unreadable falls back to these defaults.

#[derive(Clone, Debug, PartialEq)]
pub struct PlaylistStyle {
    /// Track titles.
    pub normal: u32,
    /// The title of the track that's playing.
    pub current: u32,
    pub normal_bg: u32,
    /// Background of selected tracks.
    pub selected_bg: u32,
    /// Font family name, looked up among the system's fonts.
    pub font: String,
}

impl Default for PlaylistStyle {
    fn default() -> Self {
        PlaylistStyle {
            normal: 0x00FF00,
            current: 0xFFFFFF,
            normal_bg: 0x000000,
            selected_bg: 0x0000C6,
            font: "Arial".to_string(),
        }
    }
}

pub fn parse(text: &str) -> PlaylistStyle {
    let mut style = PlaylistStyle::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        match key.trim().to_ascii_lowercase().as_str() {
            "normal" => set_color(&mut style.normal, value),
            "current" => set_color(&mut style.current, value),
            "normalbg" => set_color(&mut style.normal_bg, value),
            "selectedbg" => set_color(&mut style.selected_bg, value),
            "font" if !value.is_empty() => style.font = value.to_string(),
            _ => {}
        }
    }
    style
}

/// Reads "#RRGGBB" (the "#" is optional); leaves `slot` alone if invalid.
fn set_color(slot: &mut u32, value: &str) {
    let hex: String = value.trim_start_matches('#').chars().take(6).collect();
    if hex.len() == 6
        && let Ok(color) = u32::from_str_radix(&hex, 16)
    {
        *slot = color;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_typical_file() {
        let style = parse("[Text]\r\nNormal=#00FF00\r\nCurrent=#FFFFFF\r\nNormalBG=#000000\r\nSelectedBG=#0000C6\r\nFont=Tahoma\r\n");
        assert_eq!(style.selected_bg, 0x0000C6);
        assert_eq!(style.font, "Tahoma");
    }

    #[test]
    fn tolerates_sloppy_files() {
        let style = parse("[text]\nnormal = 123abc ; comment\nCURRENT=#zzzzzz\nselectedbg=#ff0000ff\n");
        assert_eq!(style.normal, 0x123ABC);
        assert_eq!(style.current, PlaylistStyle::default().current);
        assert_eq!(style.selected_bg, 0xFF0000);
        assert_eq!(style.font, "Arial");
    }
}
