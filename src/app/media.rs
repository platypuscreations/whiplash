//! The system's media controls: the keyboard's play/pause, next and
//! previous keys, and the desktop's "now playing" display.
//!
//! The keys don't reach the app as ordinary key presses. The system
//! catches them and passes them to the music player it thinks is current:
//! on macOS the "Now Playing" app (see `macos.rs`), on Linux over D-Bus,
//! by the MPRIS standard (see `mpris.rs`), and on Windows through its
//! System Media Transport Controls (see `smtc.rs`). The same connection
//! tells the system what's playing, which shows in Control Center, the
//! Plasma media widget, GNOME's notifications, `playerctl` and Windows'
//! media flyout.
//!
//! None of this is needed for playing music, so it never gets in the way:
//! if the desktop can't be reached, the controls just don't work, and the
//! app never waits on them.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(all(unix, not(target_os = "macos")))]
mod mpris;
#[cfg(windows)]
mod smtc;

use std::time::{Duration, Instant};

use winit::event_loop::EventLoopProxy;

use super::UserEvent;
use crate::audio::{PlayState, Player};
use crate::location::Location;

/// What the desktop asked the player to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    /// Moves this many seconds forward (or back, if negative).
    #[cfg_attr(any(target_os = "macos", windows), allow(dead_code))] // only Linux's MPRIS asks for this
    Seek(f64),
    /// Jumps to this many seconds into the track.
    SetPosition(f64),
    #[cfg_attr(target_os = "macos", allow(dead_code))] // Control Center has no shuffle button
    SetShuffle(bool),
    #[cfg_attr(target_os = "macos", allow(dead_code))] // ...or repeat button
    SetRepeat(bool),
}

/// What the desktop is told about the current track.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub location: Location,
    /// The tagged title, or the file name.
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
}

/// News for the desktop, sent as things change.
#[derive(Clone, Debug, PartialEq)]
enum Update {
    /// A new track, or none.
    Track(Option<Track>),
    Status(PlayState),
    /// Where playback is, sent now and then so the desktop's clock stays right.
    Position(f64),
    /// Playback jumped to a new place.
    Seeked(f64),
    Shuffle(bool),
    Repeat(bool),
}

/// How often the position is passed on while playing. The desktop counts
/// time on by itself between updates.
const POSITION_EVERY: Duration = Duration::from_secs(1);

/// A jump bigger than this between updates counts as a seek.
const SEEK_THRESHOLD: f64 = 1.5;

/// The app's side of the media controls.
pub struct MediaControls {
    backend: Option<Backend>,
    /// What the desktop was last told, to send only what changed.
    track: Option<Location>,
    status: PlayState,
    position: f64,
    position_sent: Option<Instant>,
    shuffle: Option<bool>,
    repeat: Option<bool>,
}

impl MediaControls {
    /// Starts talking to the desktop. Commands arrive as `UserEvent::Media`.
    pub fn connect(proxy: EventLoopProxy<UserEvent>) -> MediaControls {
        MediaControls {
            backend: Backend::connect(proxy),
            track: None,
            status: PlayState::Stopped,
            position: 0.0,
            position_sent: None,
            shuffle: None,
            repeat: None,
        }
    }

    /// Windows ties its media controls to a window, so they start working
    /// once the player window exists. Elsewhere this does nothing.
    pub fn window_created(&mut self, window: &winit::window::Window) {
        #[cfg(windows)]
        if let Some(backend) = &mut self.backend {
            backend.attach(window);
            // Tell the new controls everything.
            self.track = None;
            self.position_sent = None;
            self.shuffle = None;
            self.repeat = None;
        }
        #[cfg(not(windows))]
        let _ = window;
    }

    /// Whether media keys pressed in Whiplash's own windows reach it through
    /// the system too, and so must not be handled a second time.
    pub fn takes_media_keys(&self) -> bool {
        cfg!(windows) && self.backend.is_some()
    }

    /// Tells the desktop what changed since last time. Cheap enough to call
    /// on every pass of the event loop.
    pub fn sync(&mut self, player: &Player, shuffle: bool, repeat: bool) {
        let Some(backend) = &self.backend else { return };
        let info = player.info();
        let status = if info.is_some() { player.state() } else { PlayState::Stopped };
        let mut updates = Vec::new();

        if self.shuffle != Some(shuffle) {
            self.shuffle = Some(shuffle);
            updates.push(Update::Shuffle(shuffle));
        }
        if self.repeat != Some(repeat) {
            self.repeat = Some(repeat);
            updates.push(Update::Repeat(repeat));
        }

        let location = info.map(|i| &i.location);
        if location != self.track.as_ref() {
            self.track = location.cloned();
            updates.push(Update::Track(info.map(|info| Track {
                location: info.location.clone(),
                title: info.tags.title.clone().unwrap_or_else(|| info.location.fallback_title()),
                artist: info.tags.artist.clone(),
                album: info.tags.album.clone(),
                duration: info.duration,
            })));
            self.position_sent = None;
        }
        if status != self.status {
            self.status = status;
            self.position_sent = None;
            updates.push(Update::Status(status));
        }

        let position = if status == PlayState::Stopped { 0.0 } else { player.position() };
        let now = Instant::now();
        match self.position_sent {
            None => updates.push(Update::Position(position)),
            Some(sent) => {
                let expected = match status {
                    PlayState::Playing => self.position + (now - sent).as_secs_f64(),
                    _ => self.position,
                };
                if (position - expected).abs() > SEEK_THRESHOLD {
                    updates.push(Update::Seeked(position));
                } else if status == PlayState::Playing && now - sent >= POSITION_EVERY {
                    updates.push(Update::Position(position));
                } else {
                    return send(backend, updates);
                }
            }
        }
        self.position = position;
        self.position_sent = Some(now);
        send(backend, updates);
    }
}

fn send(backend: &Backend, updates: Vec<Update>) {
    for update in updates {
        backend.send(update);
    }
}

#[cfg(target_os = "macos")]
use macos::Backend;
#[cfg(all(unix, not(target_os = "macos")))]
use mpris::Backend;
#[cfg(windows)]
use smtc::Backend;
