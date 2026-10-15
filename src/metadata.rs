//! Reading track titles and lengths in the background.
//!
//! Opening hundreds of files when a big folder is added would freeze the
//! window, so a helper thread does it and hands back results as they come.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::audio;

/// What was learned about one playlist entry.
pub struct Details {
    pub id: u64,
    pub title: String,
    pub duration: Option<f64>,
}

pub struct MetadataReader {
    requests: Sender<(u64, PathBuf)>,
    results: Receiver<Details>,
}

impl MetadataReader {
    /// Starts the helper thread. `wake` is called after each result so the
    /// app can pick it up.
    pub fn new(wake: impl Fn() + Send + 'static) -> MetadataReader {
        let (requests, incoming) = mpsc::channel::<(u64, PathBuf)>();
        let (outgoing, results) = mpsc::channel();
        thread::Builder::new()
            .name("metadata".into())
            .spawn(move || {
                // Ends when the app drops its `MetadataReader`.
                for (id, path) in incoming {
                    let Ok(info) = audio::read_info(&path) else { continue };
                    if outgoing.send(Details { id, title: info.title, duration: info.duration }).is_err() {
                        break;
                    }
                    wake();
                }
            })
            .expect("failed to start metadata thread");
        MetadataReader { requests, results }
    }

    pub fn request(&self, entries: Vec<(u64, PathBuf)>) {
        for entry in entries {
            let _ = self.requests.send(entry);
        }
    }

    /// Results that have arrived since the last call.
    pub fn results(&self) -> impl Iterator<Item = Details> + '_ {
        self.results.try_iter()
    }
}
