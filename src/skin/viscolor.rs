//! Parsing `viscolor.txt`: the 24 colors used by the built-in visualizer.
//!
//! Each line starts with three numbers (red, green, blue), usually followed by
//! a comment, for example `239,49,16, // top of spectrum`. Index 0 is the
//! background, 1 the grid dots, 2-17 the spectrum bars from top to bottom,
//! 18-22 the oscilloscope and 23 the analyzer peak dots.

/// Colors used when a skin has no `viscolor.txt` or the file is short.
pub const DEFAULT: [u32; 24] = [
    0x000000, 0x182129, 0xEF3110, 0xCE2910, 0xD65A00, 0xD66600, 0xD67300, 0xC67B08, 0xDEA518,
    0xD6B521, 0xBDDE29, 0x94DE21, 0x29CE10, 0x32BE10, 0x39B510, 0x319C08, 0x299400, 0x188408,
    0xFFFFFF, 0xD6D6DE, 0xB5BDBD, 0xA0AAAF, 0x949CA5, 0x969696,
];

pub fn parse(text: &str) -> [u32; 24] {
    let mut colors = DEFAULT;
    let parsed = text.lines().filter_map(parse_line);
    for (slot, color) in colors.iter_mut().zip(parsed) {
        *slot = color;
    }
    colors
}

/// Reads the leading "r,g,b" of a line. Separators may be commas and/or
/// spaces. Lines that don't start with three numbers are skipped.
fn parse_line(line: &str) -> Option<u32> {
    let code = line.split("//").next().unwrap_or_default();
    let mut numbers = code
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<u32>().ok().map(|n| n.min(255)));

    let r = numbers.next()??;
    let g = numbers.next()??;
    let b = numbers.next()??;
    Some(r << 16 | g << 8 | b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comments_spaces_and_short_files() {
        let colors = parse("0,0,0, // black\n  24 , 33,41,// grey\n\n150, 150, 150 , // peak\n");
        assert_eq!(colors[0], 0x000000);
        assert_eq!(colors[1], 0x182129);
        // The blank line is skipped, so the third valid line fills slot 2.
        assert_eq!(colors[2], 0x969696);
        // The rest keep their defaults.
        assert_eq!(colors[3], DEFAULT[3]);
    }

    #[test]
    fn out_of_range_values_are_clamped_and_junk_is_skipped() {
        let colors = parse("300,0,0\nnot a color\n1,2\n4,5,6\n");
        assert_eq!(colors[0], 0xFF0000);
        assert_eq!(colors[1], 0x040506);
    }
}
