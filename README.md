# Whiplash Player

A native music player for macOS and Linux that uses classic Winamp 2.x skins (`.wsz`). Whiplash (`whiplash` for short) lives at <https://github.com/platypuscreations/whiplash>.

Status: phase 8 (packaging). The main window renders from any classic skin, plays MP3, FLAC, Ogg Vorbis, WAV, AIFF and AAC/ALAC (M4A), and shows the classic spectrum analyzer or oscilloscope in the skin's colors. The equalizer window has a 10-band EQ with preamp and loads and saves Winamp `.eqf` presets. The playlist window is resizable, uses the skin's colors and font, and reads and writes M3U and PLS playlists. Windows snap and dock together, have shade modes, and everything is remembered between runs. MilkDrop presets play in their own window through projectM.

## Building

Needs Rust, plus CMake and a C++ compiler for projectM (MilkDrop), which is compiled from source as part of the build (the first build takes a minute or so). On macOS, `xcode-select --install` and `brew install cmake` cover it. To build without MilkDrop: `cargo build --no-default-features`.

## Packaging

- **macOS:** `packaging/macos/build-app.sh --dmg` makes `Whiplash Player.app` and a disk image in `target/release/bundle` (needs `brew install cmake librsvg`). The app is signed ad hoc, so on another Mac it opens with a right-click → Open the first time.
- **Linux:** `packaging/linux/build-packages.sh` makes a `.deb` and an AppImage (the script lists what to install first). Or build them in Docker from any computer:
  ```sh
  docker build -t whiplash-linux packaging/linux
  docker run --rm -v "$PWD":/src:ro -v "$PWD/dist":/out whiplash-linux
  ```
- **GitHub Actions** (`.github/workflows/build.yml`) tests on macOS and Ubuntu and builds all three packages on every push.

## Running

```sh
cargo run -- path/to/skin.wsz song.mp3 ~/Music/Album list.m3u   # a skin, music, folders, playlists
cargo run                                          # then drop skins or music onto the window
cargo run -- --render-to out.bmp path/to/skin.wsz  # save a screenshot without opening a window
```

| Input | Action |
|---|---|
| Eject button, L | Open files (replaces the playlist) |
| Z / X / C / V / B | Previous / Play / Pause / Stop / Next |
| ← / → | Seek back / forward 5 seconds |
| ↑ / ↓, mouse wheel | Volume |
| Click the time | Switch between elapsed and remaining |
| Click the visualizer | Analyzer → oscilloscope → off |
| EQ button, Alt+G | Show or hide the equalizer |
| Presets button | Built-in presets, load or save `.eqf` files, and per-song presets for the Auto button |
| PL button, Alt+E | Show or hide the playlist |
| Playlist: click / Shift+click / Ctrl+click (Cmd+click) | Select tracks |
| Playlist: double-click, Enter | Play a track |
| Playlist: drag selected tracks | Reorder them |
| Playlist: Delete, Ctrl+A (Cmd+A), ↑ / ↓ | Remove selected, select all, move selection |
| Playlist: drag the bottom-right corner | Resize |
| Playlist: ADD / REM / SEL / MISC / LIST OPTS | Add files or folders, remove, select, sort, file info, new/save/load list |
| +12 / 0 / −12 dB labels | Set every band at once |
| Ctrl+D (Cmd+D) | Double size |
| Ctrl+T (Cmd+T) | Always on top |
| Rest the mouse on a control | Tooltip naming it, with its current setting |
| Right-click, O button | Main menu |
| Alt+S | Skin browser |
| Ctrl+K (Cmd+K) | MilkDrop window |
| MilkDrop: N / P / R / L, Space / Backspace | Next, previous, random preset; lock the current one |
| MilkDrop: F, Enter, double-click / Esc | Full screen / leave full screen or close |
| Clutter bar O / A / I / D / V | Menu, always on top, file info, double size, visualizer |
| Drag a title bar | Move the window; windows snap together, and docked windows follow the main window |
| Double-click a title bar, shade button | Shade mode (shrink to a strip) |

## Tests

```sh
cargo test
# Also play real files through the sound card (needs an audio device):
WHIPLASH_TEST_AUDIO=path/to/folder cargo test -- --ignored --nocapture
```

## Settings and skins

Settings, window positions and the playlist are saved when Whiplash quits (including with Ctrl+C in a terminal), in `~/Library/Application Support/whiplash` on macOS or `~/.config/whiplash` on Linux. Set `WHIPLASH_CONFIG_DIR` to use a different folder.

Skins can be found at <https://skins.webamp.org>. Put `.wsz` files in the `Skins` folder inside the settings folder (the skin browser's "Open skins folder" opens it); skins next to the current skin are listed too. Skins are other people's artwork: the `skins/` folder in this repository is ignored by git.

MilkDrop presets (`.milk` files) aren't included. Put them in the `Presets` folder inside the settings folder, or choose any folder from the main menu ("MilkDrop presets folder..."). projectM's curated pack is a good start: <https://github.com/projectM-visualizer/presets-cream-of-the-crop> (about 9,800 presets); its companion texture pack <https://github.com/projectM-visualizer/presets-milkdrop-texture-pack> can go in a `textures` folder inside or next to the presets folder.

Not supported yet: non-rectangular windows from a skin's `region.txt` (the drawing library can't make parts of a window see-through on macOS) and internet streams.

## License

Copyright © 2026 Luke Hindman.

Whiplash is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version. It is distributed in the hope that it will be useful, but without any warranty. See [LICENSE](LICENSE).

It includes projectM (LGPL) and builds on Webamp's work (MIT) and many Rust crates: see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md). After changing dependencies, run `packaging/generate-notices.sh` to update that file.

## Credits

Sprite coordinates, window layout and the visualizer's look are taken from [Webamp](https://github.com/captbaritone/webamp) (MIT license) by Jordan Eldredge and contributors.

MilkDrop visualizations are rendered by [projectM](https://github.com/projectM-visualizer/projectm) (LGPL 2.1), compiled into Whiplash.

This project contains no code from Winamp. It is not affiliated with or endorsed by Winamp or Llama Group.
