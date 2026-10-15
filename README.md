# skinamp

A native music player for macOS and Linux that uses classic Winamp 2.x skins (`.wsz`).

Status: phase 6 (window behavior). The main window renders from any classic skin, plays MP3, FLAC, Ogg Vorbis, WAV, AIFF and AAC/ALAC (M4A), and shows the classic spectrum analyzer or oscilloscope in the skin's colors. The equalizer window has a 10-band EQ with preamp and loads and saves Winamp `.eqf` presets. The playlist window is resizable, uses the skin's colors and font, and reads and writes M3U and PLS playlists. Windows snap and dock together, have shade modes, and everything is remembered between runs.

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
| Clutter bar O / A / I / D / V | Menu, always on top, file info, double size, visualizer |
| Drag a title bar | Move the window; windows snap together, and docked windows follow the main window |
| Double-click a title bar, shade button | Shade mode (shrink to a strip) |

## Tests

```sh
cargo test
# Also play real files through the sound card (needs an audio device):
SKINAMP_TEST_AUDIO=path/to/folder cargo test -- --ignored --nocapture
```

## Settings and skins

Settings, window positions and the playlist are saved when skinamp quits (including with Ctrl+C in a terminal), in `~/Library/Application Support/skinamp` on macOS or `~/.config/skinamp` on Linux. Set `SKINAMP_CONFIG_DIR` to use a different folder.

Skins can be found at <https://skins.webamp.org>. Put `.wsz` files in the `Skins` folder inside the settings folder (the skin browser's "Open skins folder" opens it); skins next to the current skin are listed too. Skins are other people's artwork: the `skins/` folder in this repository is ignored by git.

Not supported yet: non-rectangular windows from a skin's `region.txt` (the drawing library can't make parts of a window see-through on macOS), internet streams, and MilkDrop visualizations (phase 7).

## Credits

Sprite coordinates, window layout and the visualizer's look are taken from [Webamp](https://github.com/captbaritone/webamp) (MIT license) by Jordan Eldredge and contributors.

This project contains no code from Winamp. It is not affiliated with or endorsed by Winamp or Llama Group.
