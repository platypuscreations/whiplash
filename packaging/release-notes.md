Whiplash Player plays music with classic Winamp 2.x skins, on macOS, Linux and Windows.

**Download** the file for your computer from **Assets** below:

- **macOS** (Apple silicon, macOS 11 or later): the `.dmg`
- **Debian, Ubuntu and similar** (64-bit PC): the `.deb`
- **Other Linux** (64-bit PC): the `.AppImage`
- **Windows 10 or 11** (64-bit PC): the `.msi`

Whiplash comes with its own skin and plays classic Winamp skins (`.wsz`) too. macOS asks you to approve it the first time it opens. The Windows installer isn't signed, so Windows may say it protected your PC: click **More info**, then **Run anyway**. The [README](https://github.com/platypuscreations/whiplash#install) walks through installing it and getting more skins, such as the classic base 2.91.

**New in 0.5.0: play from your music server.** Right-click the player to choose where the music comes from: **Local files** as before, an **Emby** server, or **Squeeze**, a Logitech Media Server (now Lyrion Music Server). In a server mode, the playlist window gets a tab for each of the server's music libraries, such as Music and Audio books, to browse by artist, album or genre or to search, and a **Queue** tab for the playlist. Tracks play through Whiplash itself, so the equalizer, visualizers and MilkDrop work just as with files. The [README](https://github.com/platypuscreations/whiplash#music-servers-emby-and-squeeze) explains how to set it up.

Also new:

- The playlist window's mini visualizer (shown when the window is wide) now works, and its running time shows how far through the playlist you are.
- Control-click opens the menu on a Mac.
- Errors from the sound device now go to `whiplash.log` in the settings folder too.

**Known issues** we're still tracking down. If you run into either, please [open an issue](https://github.com/platypuscreations/whiplash/issues):

- **Windows:** after unplugging a monitor with speakers while music played, pressing Play once failed with an error about the audio client. We couldn't make it happen again. Whiplash now tries the sound device a few more times before giving up, and writes any error to `whiplash.log` in its settings folder (`%APPDATA%\whiplash\config`); please attach that file.
- **Windows:** MilkDrop froze a few times during testing. A fix for a related problem is in this release, and we haven't seen it since. If it happens, please say whether only MilkDrop's picture stopped or the player too, and which preset was showing.
