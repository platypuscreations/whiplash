//! Media controls on Linux, by the MPRIS D-Bus standard.
//!
//! Whiplash registers as `org.mpris.MediaPlayer2.whiplash` on the session
//! bus. The desktop sends it the media keys from there, and reads what's
//! playing to show in its media widget.
//!
//! The D-Bus side runs on a thread of its own, so a slow or missing bus
//! never holds up the app: commands come back as `UserEvent::Media`, and
//! news goes to the thread over a channel that never blocks. If the bus
//! can't be reached, the thread reports it once and ends.

use std::future::Future;
use std::pin::pin;
use std::task::Poll;

use async_channel::{Receiver, Sender};
use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Player, Time, TrackId};
use winit::event_loop::EventLoopProxy;

use super::{Command, Track, Update};
use crate::app::UserEvent;
use crate::audio::PlayState;

pub struct Backend {
    updates: Sender<Update>,
}

impl Backend {
    pub fn connect(proxy: EventLoopProxy<UserEvent>) -> Option<Backend> {
        let (updates, incoming) = async_channel::unbounded();
        let thread = std::thread::Builder::new().name("mpris".into());
        match thread.spawn(move || pollster::block_on(serve(incoming, proxy))) {
            Ok(_) => Some(Backend { updates }),
            Err(err) => {
                eprintln!("media keys unavailable: {err}");
                None
            }
        }
    }

    /// Passes news to the D-Bus thread. Never waits; if the thread has
    /// ended, the news is dropped.
    pub fn send(&self, update: Update) {
        let _ = self.updates.try_send(update);
    }
}

/// Registers on the bus, then answers the desktop and passes on news until
/// the app closes the channel or the bus goes away.
async fn serve(updates: Receiver<Update>, proxy: EventLoopProxy<UserEvent>) {
    let player = match register().await {
        Ok(player) => player,
        Err(err) => {
            eprintln!("media keys unavailable: {err}");
            return;
        }
    };
    let on = |command: Command| {
        let proxy = proxy.clone();
        move |_: &Player| {
            let _ = proxy.send_event(UserEvent::Media(command));
        }
    };
    player.connect_play(on(Command::Play));
    player.connect_pause(on(Command::Pause));
    player.connect_play_pause(on(Command::PlayPause));
    player.connect_stop(on(Command::Stop));
    player.connect_next(on(Command::Next));
    player.connect_previous(on(Command::Previous));
    let shuffle_proxy = proxy.clone();
    player.connect_set_shuffle(move |_, on| {
        let _ = shuffle_proxy.send_event(UserEvent::Media(Command::SetShuffle(on)));
    });
    // Whiplash repeats the whole playlist or nothing. Desktops cycle through
    // off, one track and the playlist (in either order), so asking for one
    // track toggles repeat, which keeps their button working.
    let repeat_proxy = proxy.clone();
    player.connect_set_loop_status(move |player, status| {
        let on = match status {
            LoopStatus::None => false,
            LoopStatus::Playlist => true,
            LoopStatus::Track => player.loop_status() == LoopStatus::None,
        };
        let _ = repeat_proxy.send_event(UserEvent::Media(Command::SetRepeat(on)));
    });
    let seek_proxy = proxy.clone();
    player.connect_seek(move |_, offset| {
        let _ = seek_proxy.send_event(UserEvent::Media(Command::Seek(seconds(offset))));
    });
    player.connect_set_position(move |player, track, position| {
        // The standard says to ignore a request meant for another track.
        if player.metadata().trackid().as_ref() == Some(track) {
            let _ = proxy.send_event(UserEvent::Media(Command::SetPosition(seconds(position))));
        }
    });

    let news = async {
        let mut tracks = 0u64;
        while let Ok(update) = updates.recv().await {
            if let Err(err) = apply(&player, update, &mut tracks).await {
                eprintln!("media controls: {err}");
            }
        }
    };
    first(player.run(), news).await;
}

/// Takes the usual bus name, or one with the process number if another
/// copy of Whiplash already has it, as the standard suggests.
async fn register() -> mpris_server::zbus::Result<Player> {
    let build = |name: &str| {
        Player::builder(name)
            .identity("Whiplash Player")
            .desktop_entry("whiplash")
            .can_play(true)
            .can_pause(true)
            .can_go_next(true)
            .can_go_previous(true)
            .can_seek(true)
            .build()
    };
    match build("whiplash").await {
        Ok(player) => Ok(player),
        Err(_) => build(&format!("whiplash.instance{}", std::process::id())).await,
    }
}

async fn apply(player: &Player, update: Update, tracks: &mut u64) -> mpris_server::zbus::Result<()> {
    match update {
        Update::Track(track) => {
            *tracks += 1;
            player.set_metadata(track.map_or_else(Metadata::new, |t| metadata(&t, *tracks))).await
        }
        Update::Status(state) => player.set_playback_status(status(state)).await,
        Update::Position(at) => {
            player.set_position(time(at));
            Ok(())
        }
        Update::Seeked(at) => {
            player.set_position(time(at));
            player.seeked(time(at)).await
        }
        Update::Shuffle(on) => player.set_shuffle(on).await,
        Update::Repeat(on) => player.set_loop_status(if on { LoopStatus::Playlist } else { LoopStatus::None }).await,
    }
}

fn metadata(track: &Track, number: u64) -> Metadata {
    let mut metadata = Metadata::builder().title(&track.title);
    // Only a file's address is passed on: a server's could hold a password.
    if let Some(path) = track.location.file() {
        metadata = metadata.url(file_url(path));
    }
    // Each new track gets a new id, so the desktop can tell them apart.
    if let Ok(id) = TrackId::try_from(format!("/net/platypuscreations/whiplash/track/{number}")) {
        metadata = metadata.trackid(id);
    }
    if let Some(artist) = &track.artist {
        metadata = metadata.artist([artist]);
    }
    if let Some(album) = &track.album {
        metadata = metadata.album(album);
    }
    if let Some(duration) = track.duration {
        metadata = metadata.length(time(duration));
    }
    metadata.build()
}

fn status(state: PlayState) -> PlaybackStatus {
    match state {
        PlayState::Playing => PlaybackStatus::Playing,
        PlayState::Paused => PlaybackStatus::Paused,
        PlayState::Stopped => PlaybackStatus::Stopped,
    }
}

fn time(seconds: f64) -> Time {
    Time::from_micros((seconds * 1e6) as i64)
}

fn seconds(time: Time) -> f64 {
    time.as_micros() as f64 / 1e6
}

/// Runs two tasks together until either one finishes.
async fn first(a: impl Future<Output = ()>, b: impl Future<Output = ()>) {
    let (mut a, mut b) = (pin!(a), pin!(b));
    std::future::poll_fn(|cx| {
        if a.as_mut().poll(cx).is_ready() || b.as_mut().poll(cx).is_ready() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await
}

/// A file's path as a `file://` address, as the desktop expects.
fn file_url(path: &std::path::Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut url = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            url.push(byte as char);
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls_escape_what_they_must() {
        assert_eq!(file_url(std::path::Path::new("/music/a b/Ünder #1.mp3")), "file:///music/a%20b/%C3%9Cnder%20%231.mp3");
    }
}
