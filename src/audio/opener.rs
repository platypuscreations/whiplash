//! Opening tracks from the web in the background: the server can take a
//! moment to answer, and the windows mustn't freeze while it does.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::Result;

use super::{Source, Track};
use crate::location::Location;

pub struct Opener {
    requests: Sender<(u64, Source, Location)>,
    results: Receiver<(u64, Result<Track>)>,
}

impl Opener {
    /// Starts the helper thread. `wake` is called after each track opens
    /// (or fails to), so the app can pick it up.
    pub fn new(wake: impl Fn() + Send + 'static) -> Opener {
        let (requests, incoming) = mpsc::channel::<(u64, Source, Location)>();
        let (outgoing, results) = mpsc::channel();
        thread::Builder::new()
            .name("opener".into())
            .spawn(move || {
                // Ends when the app drops its `Opener`.
                for (ticket, source, location) in incoming {
                    if outgoing.send((ticket, Track::open_source(&source, location))).is_err() {
                        break;
                    }
                    wake();
                }
            })
            .expect("failed to start opener thread");
        Opener { requests, results }
    }

    /// Opens a track. The result comes back from `results` with the same
    /// `ticket`.
    pub fn open(&self, ticket: u64, source: Source, location: Location) {
        let _ = self.requests.send((ticket, source, location));
    }

    /// Tracks opened since the last call.
    pub fn results(&self) -> impl Iterator<Item = (u64, Result<Track>)> + '_ {
        self.results.try_iter()
    }
}
