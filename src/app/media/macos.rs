//! Media controls on macOS, through Apple's MediaPlayer framework.
//!
//! macOS sends the media keys, Control Center's buttons and headphone
//! controls to the app it shows as "Now Playing". An app becomes that by
//! handling the remote commands and publishing what it's playing, with
//! its playback state; both happen here.
//!
//! Everything runs on the main thread: macOS calls the command handlers
//! there, and they just pass the command on as a `UserEvent::Media`.

use std::cell::RefCell;
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSDictionary, NSNumber, NSString};
use objc2_media_player::{
    MPChangePlaybackPositionCommandEvent, MPMediaItemPropertyAlbumTitle, MPMediaItemPropertyArtist,
    MPMediaItemPropertyPlaybackDuration, MPMediaItemPropertyTitle, MPNowPlayingInfoCenter,
    MPNowPlayingInfoPropertyElapsedPlaybackTime, MPNowPlayingInfoPropertyPlaybackRate, MPNowPlayingPlaybackState,
    MPRemoteCommand, MPRemoteCommandCenter, MPRemoteCommandEvent, MPRemoteCommandHandlerStatus,
};
use winit::event_loop::EventLoopProxy;

use super::{Command, Track, Update};
use crate::app::UserEvent;
use crate::audio::PlayState;

pub struct Backend {
    /// The commands handled and their handlers, kept for as long as the app runs.
    _targets: Vec<(Retained<MPRemoteCommand>, Retained<AnyObject>)>,
    /// What's published, since macOS takes it all at once.
    now: RefCell<NowPlaying>,
}

#[derive(Default)]
struct NowPlaying {
    track: Option<Track>,
    state: Option<PlayState>,
    position: f64,
}

impl Backend {
    pub fn connect(proxy: EventLoopProxy<UserEvent>) -> Option<Backend> {
        // SAFETY: the shared command center, used on the main thread.
        let center = unsafe { MPRemoteCommandCenter::sharedCommandCenter() };
        let mut targets = Vec::new();
        let mut handle = |command: Retained<MPRemoteCommand>, handler: RcBlock<_>| {
            // SAFETY: a valid handler block; the returned target is kept.
            let target = unsafe {
                command.setEnabled(true);
                command.addTargetWithHandler(&handler)
            };
            targets.push((command, target));
        };
        let on = |command: Command| {
            let proxy = proxy.clone();
            RcBlock::new(move |_: NonNull<MPRemoteCommandEvent>| {
                let _ = proxy.send_event(UserEvent::Media(command));
                MPRemoteCommandHandlerStatus::Success
            })
        };
        // SAFETY: the command center's own commands.
        unsafe {
            handle(center.playCommand(), on(Command::Play));
            handle(center.pauseCommand(), on(Command::Pause));
            handle(center.togglePlayPauseCommand(), on(Command::PlayPause));
            handle(center.stopCommand(), on(Command::Stop));
            handle(center.nextTrackCommand(), on(Command::Next));
            handle(center.previousTrackCommand(), on(Command::Previous));
        }
        // Dragging the position slider in Control Center.
        let seek = RcBlock::new(move |event: NonNull<MPRemoteCommandEvent>| {
            // SAFETY: macOS passes a valid event for the duration of the call.
            let event: &AnyObject = unsafe { event.as_ref() }.as_ref();
            match event.downcast_ref::<MPChangePlaybackPositionCommandEvent>() {
                Some(event) => {
                    // SAFETY: reading the event's position.
                    let at = unsafe { event.positionTime() };
                    let _ = proxy.send_event(UserEvent::Media(Command::SetPosition(at)));
                    MPRemoteCommandHandlerStatus::Success
                }
                None => MPRemoteCommandHandlerStatus::CommandFailed,
            }
        });
        // SAFETY: as above.
        handle(unsafe { Retained::into_super(center.changePlaybackPositionCommand()) }, seek);

        Some(Backend { _targets: targets, now: RefCell::new(NowPlaying::default()) })
    }

    pub fn send(&self, update: Update) {
        let mut now = self.now.borrow_mut();
        match update {
            Update::Track(track) => now.track = track,
            Update::Status(state) => now.state = Some(state),
            Update::Position(at) | Update::Seeked(at) => now.position = at,
            // Control Center has no shuffle or repeat buttons.
            Update::Shuffle(_) | Update::Repeat(_) => return,
        }
        publish(&now);
    }
}

/// Hands macOS the whole now-playing picture.
fn publish(now: &NowPlaying) {
    // SAFETY: the shared info center, used on the main thread.
    let center = unsafe { MPNowPlayingInfoCenter::defaultCenter() };
    let state = match now.state {
        Some(PlayState::Playing) => MPNowPlayingPlaybackState::Playing,
        Some(PlayState::Paused) => MPNowPlayingPlaybackState::Paused,
        _ => MPNowPlayingPlaybackState::Stopped,
    };
    let Some(track) = &now.track else {
        // SAFETY: clearing the info is always allowed.
        unsafe {
            center.setNowPlayingInfo(None);
            center.setPlaybackState(state);
        }
        return;
    };

    let mut keys: Vec<&NSString> = Vec::new();
    let mut values: Vec<Retained<AnyObject>> = Vec::new();
    let mut add = |key: &'static NSString, value: Retained<AnyObject>| {
        keys.push(key);
        values.push(value);
    };
    let text = |s: &str| Retained::into_super(Retained::into_super(NSString::from_str(s)));
    let number = |n: f64| Retained::into_super(Retained::into_super(Retained::into_super(NSNumber::new_f64(n))));
    // SAFETY: the framework's own key constants.
    unsafe {
        add(MPMediaItemPropertyTitle, text(&track.title));
        if let Some(artist) = &track.artist {
            add(MPMediaItemPropertyArtist, text(artist));
        }
        if let Some(album) = &track.album {
            add(MPMediaItemPropertyAlbumTitle, text(album));
        }
        if let Some(duration) = track.duration {
            add(MPMediaItemPropertyPlaybackDuration, number(duration));
        }
        add(MPNowPlayingInfoPropertyElapsedPlaybackTime, number(now.position));
        // macOS moves the clock on by itself at this rate.
        let rate = if now.state == Some(PlayState::Playing) { 1.0 } else { 0.0 };
        add(MPNowPlayingInfoPropertyPlaybackRate, number(rate));
    }
    let values: Vec<&AnyObject> = values.iter().map(|v| &**v).collect();
    let info = NSDictionary::from_slices(&keys, &values);
    // SAFETY: a dictionary of the documented keys and value types.
    unsafe {
        center.setNowPlayingInfo(Some(&info));
        center.setPlaybackState(state);
    }
}
