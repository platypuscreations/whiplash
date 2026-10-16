//! The built-in skin's colors: graphite metal with an amber display.
//! Change the look of the whole skin here.

/// Colors as 0xRRGGBB.
pub struct Palette {
    /// The brushed metal of the window faces, top to bottom.
    pub face_top: u32,
    pub face_bottom: u32,
    /// Bevel edges: the lit top-left and the shadowed bottom-right.
    pub edge_light: u32,
    pub edge_dark: u32,
    /// The outline around each window.
    pub outline: u32,
    /// Sunken grooves for sliders.
    pub groove: u32,
    /// Engraved text and marks on the metal.
    pub engraved: u32,
    /// Title bar text and accents: active window, inactive window.
    pub title_active: u32,
    pub title_inactive: u32,
    /// The display panels.
    pub lcd_background: u32,
    pub lcd_grid: u32,
    pub lcd_text: u32,
    /// Unlit indicators and labels on the display.
    pub lcd_dim: u32,
    /// The faint unlit segments behind the time digits.
    pub lcd_ghost: u32,
    /// Push buttons, top to bottom, and the symbols on them.
    pub button_top: u32,
    pub button_bottom: u32,
    pub button_light: u32,
    pub button_dark: u32,
    pub button_symbol: u32,
    /// An equalizer band turned down (turned up uses `led_on`).
    pub cut: u32,
    /// Lit indicator lights on buttons, and unlit ones.
    pub led_on: u32,
    pub led_off: u32,
    /// The highlight behind selected playlist entries.
    pub selection: u32,
    /// The current track in the playlist.
    pub current_track: u32,
    /// The visualizer's colors from loud to quiet, after the app icon's
    /// spectrum bars.
    pub spectrum: [u32; 4],
    /// The visualizer's peak marks.
    pub peak: u32,
}

pub const PALETTE: Palette = Palette {
    face_top: 0x3C3C42,
    face_bottom: 0x232327,
    edge_light: 0x6A6A74,
    edge_dark: 0x111113,
    outline: 0x08080A,
    groove: 0x141416,
    engraved: 0xA8A8B0,
    title_active: 0xFFB020,
    title_inactive: 0x8A8A92,
    lcd_background: 0x0A0907,
    lcd_grid: 0x2A2214,
    lcd_text: 0xFFB020,
    lcd_dim: 0x3A2C10,
    lcd_ghost: 0x1A140A,
    button_top: 0xC4C4CA,
    button_bottom: 0x8C8C94,
    button_light: 0xEEEEF2,
    button_dark: 0x4A4A52,
    button_symbol: 0x1A1A1E,
    cut: 0xD07A34,
    led_on: 0xFFB020,
    led_off: 0x4A3A18,
    selection: 0x5C3A00,
    current_track: 0xFFFFFF,
    spectrum: [0xEF3110, 0xE0B020, 0x9CDE21, 0x18A808],
    peak: 0xC8C8C8,
};
