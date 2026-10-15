# skinamp

A native music player for macOS and Linux that uses classic Winamp 2.x skins (`.wsz`).

Status: phase 4 (equalizer). The main window renders from any classic skin, plays MP3, FLAC, Ogg Vorbis, WAV, AIFF and AAC/ALAC (M4A), and shows the classic spectrum analyzer or oscilloscope in the skin's colors. The equalizer window has a 10-band EQ with preamp and loads and saves Winamp `.eqf` presets. The playlist window comes next.

## Running

```sh
cargo run -- path/to/skin.wsz song.mp3 more.flac   # a skin and some music
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
| Presets button | Load or save a Winamp `.eqf` preset |
| +12 / 0 / −12 dB labels | Set every band at once |
| Ctrl+D (Cmd+D) | Double size |
| Drag the title bar | Move the window |

## Tests

```sh
cargo test
# Also play real files through the sound card (needs an audio device):
SKINAMP_TEST_AUDIO=path/to/folder cargo test -- --ignored --nocapture
```

Skins can be found at <https://skins.webamp.org>. Put any you download in `skins/`, which git ignores; skins are other people's artwork and are not part of this repository.

## Credits

Sprite coordinates, window layout and the visualizer's look are taken from [Webamp](https://github.com/captbaritone/webamp) (MIT license) by Jordan Eldredge and contributors.

This project contains no code from Winamp. It is not affiliated with or endorsed by Winamp or Llama Group.
