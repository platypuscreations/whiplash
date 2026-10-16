//! `--render-to`: draws a skin's windows in a few typical states onto one
//! image, without opening any windows. Handy for checking how a skin looks.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::playlist::Playlist;
use crate::skin::Skin;
use crate::ui::canvas::Canvas;
use crate::ui::eq_window::{self, EqState};
use crate::ui::main_window::{self, MainState, PlayState};
use crate::ui::playlist_window::{self, MenuButton, PlaylistView};
use crate::ui::text::Font;
use crate::vis::{Mode, Visualizer};

/// Space around and between the windows, in skin pixels.
const GAP: u32 = 8;
const BACKDROP: u32 = 0x5A5F66;

/// Saves the picture as a BMP, with every skin pixel `scale` x `scale`.
pub fn render(skin: &Skin, out: &Path, scale: u32) -> Result<()> {
    let scale = scale.max(1);
    let playing = MainState {
        play_state: PlayState::Playing,
        elapsed: 83,
        duration: Some(245),
        position: 0.34,
        volume: 0.7,
        balance: -0.3,
        kbps: Some(192),
        khz: Some(44),
        playlist_open: true,
        equalizer_open: true,
        marquee: "1. WHIPLASH PLAYER - BUILT-IN SKIN (4:05)".into(),
        ..MainState::default()
    };
    let paused = MainState { focused: false, play_state: PlayState::Paused, shuffle: true, repeat: true, ..playing.clone() };

    let mut eq = EqState { focused: false, ..EqState::default() };
    for (band, value) in eq.bands.iter_mut().zip([72.0, 66.0, 58.0, 50.0, 44.0, 40.0, 46.0, 56.0, 64.0, 70.0]) {
        *band = value;
    }
    let mut playlist = Playlist::default();
    let names = ["Opening Theme", "Night Drive", "Neon Rain", "Static", "Afterglow", "Long Way Home", "Coda", "Hidden Track"];
    let added = playlist.append(names.iter().map(|n| PathBuf::from(format!("{n}.mp3"))).collect());
    for (i, ((id, _), name)) in added.into_iter().zip(names).enumerate() {
        playlist.set_details(id, format!("Whiplash - {name}"), Some(150.0 + i as f64 * 37.0));
    }
    playlist.set_current(1);
    playlist.click(3, crate::playlist::Select::Only);

    let mut vis = Visualizer::new();
    feed(&mut vis);

    // Left column: the windows in use. Right column: the same windows
    // unfocused, paused, in shade mode, and with a menu open.
    let mut pieces: Vec<(Canvas, u32, u32)> = Vec::new();
    let left = GAP;
    let right = GAP * 2 + main_window::WIDTH;
    let mut y = GAP;

    let mut main = window(main_window::WIDTH, main_window::HEIGHT, |c| {
        main_window::draw(c, skin, &playing);
        vis.draw(c, skin, main_window::VIS_POSITION.0, main_window::VIS_POSITION.1);
    });
    pieces.push((scaled(&main, scale), left, y));
    main = window(main_window::WIDTH, main_window::HEIGHT, |c| main_window::draw(c, skin, &paused));
    pieces.push((scaled(&main, scale), right, y));
    y += main_window::HEIGHT + GAP;

    let focused_eq = EqState { focused: true, ..eq.clone() };
    let canvas = window(eq_window::WIDTH, eq_window::HEIGHT, |c| eq_window::draw(c, skin, &focused_eq, 0.7, -0.3));
    pieces.push((scaled(&canvas, scale), left, y));
    let canvas = window(eq_window::WIDTH, eq_window::HEIGHT, |c| eq_window::draw(c, skin, &eq, 0.7, -0.3));
    pieces.push((scaled(&canvas, scale), right, y));
    y += eq_window::HEIGHT + GAP;

    let view = PlaylistView { focused: true, ..PlaylistView::default() };
    pieces.push((playlist_window(skin, &view, &playlist, &playing, scale), left, y));
    let menu = PlaylistView { open_menu: Some(MenuButton::Add), ..PlaylistView::default() };
    pieces.push((playlist_window(skin, &menu, &playlist, &paused, scale), right, y));
    y += view.height + GAP;

    // Shade mode strips, focused and not.
    for (i, focused) in [true, false].into_iter().enumerate() {
        let x = if i == 0 { left } else { right };
        let mut strip_y = y;
        let state = MainState { shaded: true, focused, ..playing.clone() };
        let canvas = window(main_window::WIDTH, main_window::SHADE_HEIGHT, |c| {
            main_window::draw(c, skin, &state);
            let (vx, vy) = main_window::SHADE_VIS_POSITION;
            vis.draw_small(c, skin, vx, vy);
        });
        pieces.push((scaled(&canvas, scale), x, strip_y));
        strip_y += main_window::SHADE_HEIGHT + GAP / 2;
        let state = EqState { shaded: true, focused, ..eq.clone() };
        let canvas = window(eq_window::WIDTH, eq_window::SHADE_HEIGHT, |c| eq_window::draw(c, skin, &state, 0.7, -0.3));
        pieces.push((scaled(&canvas, scale), x, strip_y));
        strip_y += eq_window::SHADE_HEIGHT + GAP / 2;
        let view = PlaylistView { shaded: true, focused, ..PlaylistView::default() };
        pieces.push((playlist_window(skin, &view, &playlist, &playing, scale), x, strip_y));
    }
    y += 3 * (playlist_window::SHADE_HEIGHT + GAP / 2) + GAP / 2;

    let (width, height) = (right + main_window::WIDTH + GAP, y);
    let mut sheet = Canvas::new(width * scale, height * scale);
    sheet.fill(BACKDROP);
    for (piece, x, y) in &pieces {
        paste(&mut sheet, piece, x * scale, y * scale);
    }
    let rgb: Vec<u8> = sheet.pixels.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
    image::save_buffer_with_format(out, &rgb, sheet.width, sheet.height, image::ColorType::Rgb8, image::ImageFormat::Bmp)?;
    Ok(())
}

/// A window drawn at skin size.
fn window(width: u32, height: u32, draw: impl FnOnce(&mut Canvas)) -> Canvas {
    let mut canvas = Canvas::new(width, height);
    draw(&mut canvas);
    canvas
}

/// The playlist window enlarged, with its track list drawn at full size
/// as the app does.
fn playlist_window(skin: &Skin, view: &PlaylistView, playlist: &Playlist, main: &MainState, scale: u32) -> Canvas {
    let canvas = window(view.width, view.window_height(), |c| playlist_window::draw(c, skin, view, playlist, main));
    let mut big = scaled(&canvas, scale);
    if let Some(mut font) = Font::load(&skin.playlist.font, playlist_window::FONT_PX * scale as f32) {
        playlist_window::draw_track_text(&mut big, skin, &mut font, view, playlist, scale);
    }
    big
}

fn scaled(canvas: &Canvas, scale: u32) -> Canvas {
    let mut big = Canvas::new(canvas.width * scale, canvas.height * scale);
    canvas.scale_into(&mut big.pixels, big.width, big.height, scale);
    big
}

fn paste(onto: &mut Canvas, piece: &Canvas, x: u32, y: u32) {
    for row in 0..piece.height {
        let from = (row * piece.width) as usize;
        let to = ((y + row) * onto.width + x) as usize;
        onto.pixels[to..to + piece.width as usize].copy_from_slice(&piece.pixels[from..from + piece.width as usize]);
    }
}

/// Plays a made-up chord into the visualizer so it has something to show.
fn feed(vis: &mut Visualizer) {
    vis.mode = Mode::Analyzer;
    let chord = |samples: &mut [f32]| {
        let rate = 44_100.0;
        for (i, s) in samples.iter_mut().enumerate() {
            let t = i as f32 / rate;
            *s = [(110.0, 0.5), (220.0, 0.3), (660.0, 0.2), (1760.0, 0.12), (5000.0, 0.05)]
                .iter()
                .map(|(hz, amp)| amp * (t * hz * std::f32::consts::TAU).sin())
                .sum();
        }
        Some(44_100)
    };
    let start = Instant::now();
    vis.update(start, chord);
    vis.update(start + Duration::from_millis(100), chord);
}
