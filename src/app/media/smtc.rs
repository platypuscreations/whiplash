//! Media controls on Windows, through the System Media Transport Controls.
//!
//! Windows sends the media keys and the buttons of its media flyout (the
//! one that appears with the volume, and on the lock screen) to the app
//! whose controls are current, and shows what that app says is playing.
//! The controls belong to a window, so they're set up once the player
//! window exists (`attach`).
//!
//! Windows calls the handlers on a thread of its own; they just pass the
//! command on as a `UserEvent::Media`.

use std::cell::Cell;

use windows::Foundation::{TimeSpan, TypedEventHandler};
use windows::Media::{
    AutoRepeatModeChangeRequestedEventArgs, MediaPlaybackAutoRepeatMode, MediaPlaybackStatus, MediaPlaybackType,
    PlaybackPositionChangeRequestedEventArgs, ShuffleEnabledChangeRequestedEventArgs, SystemMediaTransportControls,
    SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
    SystemMediaTransportControlsTimelineProperties,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
use windows::core::{HSTRING, Ref, Result};
use winit::event_loop::EventLoopProxy;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use super::{Command, Track, Update};
use crate::app::UserEvent;
use crate::audio::PlayState;

pub struct Backend {
    proxy: EventLoopProxy<UserEvent>,
    /// None until the player window exists, or if Windows refused.
    controls: Option<SystemMediaTransportControls>,
    /// The track's length in seconds, for the position bar.
    duration: Cell<f64>,
}

impl Backend {
    pub fn connect(proxy: EventLoopProxy<UserEvent>) -> Option<Backend> {
        Some(Backend { proxy, controls: None, duration: Cell::new(0.0) })
    }

    /// Sets up the controls for the player window.
    pub fn attach(&mut self, window: &Window) {
        let Ok(RawWindowHandle::Win32(handle)) = window.window_handle().map(|h| h.as_raw()) else { return };
        match controls_for(HWND(handle.hwnd.get() as *mut _), &self.proxy) {
            Ok(controls) => self.controls = Some(controls),
            Err(err) => eprintln!("media controls unavailable: {err}"),
        }
    }

    pub fn send(&self, update: Update) {
        let Some(controls) = &self.controls else { return };
        if let Err(err) = self.apply(controls, update) {
            eprintln!("updating media controls: {err}");
        }
    }

    fn apply(&self, controls: &SystemMediaTransportControls, update: Update) -> Result<()> {
        match update {
            Update::Track(track) => {
                let display = controls.DisplayUpdater()?;
                match track {
                    Some(Track { title, artist, album, duration, .. }) => {
                        display.SetType(MediaPlaybackType::Music)?;
                        let music = display.MusicProperties()?;
                        music.SetTitle(&HSTRING::from(title))?;
                        music.SetArtist(&HSTRING::from(artist.unwrap_or_default()))?;
                        music.SetAlbumTitle(&HSTRING::from(album.unwrap_or_default()))?;
                        self.duration.set(duration.unwrap_or(0.0));
                    }
                    None => {
                        display.ClearAll()?;
                        self.duration.set(0.0);
                    }
                }
                display.Update()
            }
            Update::Status(state) => controls.SetPlaybackStatus(match state {
                PlayState::Playing => MediaPlaybackStatus::Playing,
                PlayState::Paused => MediaPlaybackStatus::Paused,
                PlayState::Stopped => MediaPlaybackStatus::Stopped,
            }),
            Update::Position(at) | Update::Seeked(at) => {
                let duration = seconds(self.duration.get());
                let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
                timeline.SetStartTime(seconds(0.0))?;
                timeline.SetMinSeekTime(seconds(0.0))?;
                timeline.SetEndTime(duration)?;
                timeline.SetMaxSeekTime(duration)?;
                timeline.SetPosition(seconds(at))?;
                controls.UpdateTimelineProperties(&timeline)
            }
            Update::Shuffle(on) => controls.SetShuffleEnabled(on),
            Update::Repeat(on) => controls.SetAutoRepeatMode(if on {
                MediaPlaybackAutoRepeatMode::List
            } else {
                MediaPlaybackAutoRepeatMode::None
            }),
        }
    }
}

/// Gets the window's controls, turns on the buttons Whiplash answers and
/// starts listening to them.
fn controls_for(hwnd: HWND, proxy: &EventLoopProxy<UserEvent>) -> Result<SystemMediaTransportControls> {
    let interop =
        windows::core::factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
    // Safety: the handle is the live player window's.
    let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(hwnd) }?;
    controls.SetIsEnabled(true)?;
    controls.SetIsPlayEnabled(true)?;
    controls.SetIsPauseEnabled(true)?;
    controls.SetIsStopEnabled(true)?;
    controls.SetIsNextEnabled(true)?;
    controls.SetIsPreviousEnabled(true)?;
    controls.SetPlaybackStatus(MediaPlaybackStatus::Stopped)?;

    let send = |proxy: &EventLoopProxy<UserEvent>, command| {
        let _ = proxy.send_event(UserEvent::Media(command));
    };
    let p = proxy.clone();
    controls.ButtonPressed(&TypedEventHandler::new(move |_, args: Ref<SystemMediaTransportControlsButtonPressedEventArgs>| {
        let command = match args.ok()?.Button()? {
            SystemMediaTransportControlsButton::Play => Command::Play,
            SystemMediaTransportControlsButton::Pause => Command::Pause,
            SystemMediaTransportControlsButton::Stop => Command::Stop,
            SystemMediaTransportControlsButton::Next => Command::Next,
            SystemMediaTransportControlsButton::Previous => Command::Previous,
            _ => return Ok(()),
        };
        send(&p, command);
        Ok(())
    }))?;
    let p = proxy.clone();
    controls.PlaybackPositionChangeRequested(&TypedEventHandler::new(move |_, args: Ref<PlaybackPositionChangeRequestedEventArgs>| {
        let at = args.ok()?.RequestedPlaybackPosition()?;
        send(&p, Command::SetPosition(at.Duration as f64 / TICKS_PER_SECOND));
        Ok(())
    }))?;
    let p = proxy.clone();
    controls.ShuffleEnabledChangeRequested(&TypedEventHandler::new(move |_, args: Ref<ShuffleEnabledChangeRequestedEventArgs>| {
        send(&p, Command::SetShuffle(args.ok()?.RequestedShuffleEnabled()?));
        Ok(())
    }))?;
    let p = proxy.clone();
    controls.AutoRepeatModeChangeRequested(&TypedEventHandler::new(move |_, args: Ref<AutoRepeatModeChangeRequestedEventArgs>| {
        // Whiplash has no repeat-one mode, so that turns repeat on too.
        let mode = args.ok()?.RequestedAutoRepeatMode()?;
        send(&p, Command::SetRepeat(mode != MediaPlaybackAutoRepeatMode::None));
        Ok(())
    }))?;
    Ok(controls)
}

/// Windows counts time in ticks of 100 nanoseconds.
const TICKS_PER_SECOND: f64 = 10_000_000.0;

fn seconds(seconds: f64) -> TimeSpan {
    TimeSpan { Duration: (seconds.max(0.0) * TICKS_PER_SECOND) as i64 }
}
