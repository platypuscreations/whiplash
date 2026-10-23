//! Opening tracks from the web, and seeking in them, in the background:
//! the server can take a moment to answer, and the windows mustn't freeze
//! while it does.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::Result;

use super::{Source, Track};
use crate::location::Location;

enum Job {
    Open(Source, Location),
    /// Seek to the first time, or if that fails, land at the second.
    Seek(Track, f64, f64),
}

/// A finished job.
pub enum Done {
    Opened(Result<Track>),
    /// `target` is where the seek was asked to go; `landed`, where it did.
    Seeked { track: Track, target: f64, landed: f64 },
}

pub struct Opener {
    jobs: Sender<(u64, Job)>,
    results: Receiver<(u64, Done)>,
}

impl Opener {
    /// Starts the helper thread. `wake` is called after each job is done,
    /// so the app can pick it up.
    pub fn new(wake: impl Fn() + Send + 'static) -> Opener {
        let (jobs, incoming) = mpsc::channel::<(u64, Job)>();
        let (outgoing, results) = mpsc::channel();
        thread::Builder::new()
            .name("opener".into())
            .spawn(move || {
                // Ends when the app drops its `Opener`.
                for (ticket, job) in incoming {
                    let done = match job {
                        Job::Open(source, location) => Done::Opened(Track::open_source(&source, location)),
                        Job::Seek(mut track, target, fallback) => {
                            let landed = track.seek(target).unwrap_or(fallback);
                            Done::Seeked { track, target, landed }
                        }
                    };
                    if outgoing.send((ticket, done)).is_err() {
                        break;
                    }
                    wake();
                }
            })
            .expect("failed to start opener thread");
        Opener { jobs, results }
    }

    /// Opens a track. The result comes back from `results` with the same
    /// `ticket`.
    pub fn open(&self, ticket: u64, source: Source, location: Location) {
        let _ = self.jobs.send((ticket, Job::Open(source, location)));
    }

    /// Seeks to `target` seconds into a track (see `Player::start_seek`),
    /// or if that fails, leaves it at `fallback`.
    pub fn seek(&self, ticket: u64, track: Track, target: f64, fallback: f64) {
        let _ = self.jobs.send((ticket, Job::Seek(track, target, fallback)));
    }

    /// Jobs done since the last call.
    pub fn results(&self) -> impl Iterator<Item = (u64, Done)> + '_ {
        self.results.try_iter()
    }
}
