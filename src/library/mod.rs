//! Music servers' libraries: what's in them, as the playlist window's
//! browser shows it, and a helper thread that asks the server.
//!
//! Each server has its own module that speaks its language (`emby`,
//! `squeeze`). The
//! app hands jobs to a `Library`, whose thread does them one at a time and
//! hands back the answers, so the windows never wait on the network.

pub mod emby;
pub mod squeeze;

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::Result;

use crate::config::{EmbyConfig, SqueezeConfig};

/// What a library entry is, which decides what opening it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    /// A way into the library, such as "Artists".
    Folder,
    Artist,
    Album,
    Track,
}

/// One entry in a list from the library.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub kind: ItemKind,
    /// The server's id for it.
    pub id: String,
    pub title: String,
    /// Shown on the right: an album's year, say, or a search result's kind.
    pub detail: String,
    /// A track's length in seconds.
    pub duration: Option<f64>,
}

/// A list to ask the library for.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    Artists,
    /// Every album, or one artist's.
    Albums { artist: Option<String> },
    Tracks { album: String },
    /// Every track by an artist, album by album.
    ArtistTracks { artist: String },
    Search(String),
}

impl Item {
    pub fn folder(title: &str, id: &str) -> Item {
        Item { kind: ItemKind::Folder, id: id.to_string(), title: title.to_string(), detail: String::new(), duration: None }
    }

    /// The list that opening this item shows; none for a track.
    pub fn request(&self) -> Option<Request> {
        match self.kind {
            ItemKind::Folder if self.id == "albums" => Some(Request::Albums { artist: None }),
            ItemKind::Folder => Some(Request::Artists),
            ItemKind::Artist => Some(Request::Albums { artist: Some(self.id.clone()) }),
            ItemKind::Album => Some(Request::Tracks { album: self.id.clone() }),
            ItemKind::Track => None,
        }
    }
}

/// Where browsing starts.
pub fn top_level() -> Vec<Item> {
    vec![Item::folder("Artists", "artists"), Item::folder("Albums", "albums")]
}

/// Which server, and how to reach it.
#[derive(Clone)]
pub enum ServerConfig {
    Emby(EmbyConfig),
    Squeeze(SqueezeConfig),
}

/// What the helper thread is asked to do.
pub enum Job {
    SignIn { user: String, password: String },
    List(Request),
}

/// What it found out.
pub enum Reply {
    /// Signed in: the server's settings to keep from now on.
    SignedIn(ServerConfig),
    Items(Vec<Item>),
}

/// The server said we're not signed in (any more).
#[derive(Debug)]
pub struct SignedOut;

impl std::fmt::Display for SignedOut {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "not signed in")
    }
}

impl std::error::Error for SignedOut {}

/// A music server, as the helper thread talks to it.
trait Server: Send {
    fn sign_in(&mut self, user: &str, password: &str) -> Result<ServerConfig>;
    fn list(&mut self, request: &Request) -> Result<Vec<Item>>;
}

/// The app's side of the helper thread.
pub struct Library {
    jobs: Sender<(u64, Job)>,
    replies: Receiver<(u64, Result<Reply>)>,
    next_ticket: u64,
}

impl Library {
    /// Starts a helper thread for the server. `wake` is called after each
    /// reply, so the app can pick it up.
    pub fn connect(config: ServerConfig, wake: impl Fn() + Send + 'static) -> Library {
        let (jobs, incoming) = mpsc::channel::<(u64, Job)>();
        let (outgoing, replies) = mpsc::channel();
        thread::Builder::new()
            .name("library".into())
            .spawn(move || {
                let mut server: Box<dyn Server> = match config {
                    ServerConfig::Emby(config) => Box::new(emby::Emby::new(config)),
                    ServerConfig::Squeeze(config) => Box::new(squeeze::Squeeze::new(config)),
                };
                // Ends when the app drops its `Library`.
                for (ticket, job) in incoming {
                    let reply = match job {
                        Job::SignIn { user, password } => server.sign_in(&user, &password).map(Reply::SignedIn),
                        Job::List(request) => server.list(&request).map(Reply::Items),
                    };
                    if outgoing.send((ticket, reply)).is_err() {
                        break;
                    }
                    wake();
                }
            })
            .expect("failed to start library thread");
        Library { jobs, replies, next_ticket: 0 }
    }

    /// Hands over a job. Its reply comes back from `replies` with the
    /// ticket returned here.
    pub fn send(&mut self, job: Job) -> u64 {
        self.next_ticket += 1;
        let _ = self.jobs.send((self.next_ticket, job));
        self.next_ticket
    }

    /// Replies that have arrived since the last call.
    pub fn replies(&self) -> impl Iterator<Item = (u64, Result<Reply>)> + '_ {
        self.replies.try_iter()
    }
}

/// An address as typed, tidied: "media/" becomes "http://media:8096"
/// given the server's usual port, 8096. A full address ("https://...") is
/// left as it is, as it may lead through another web server.
pub fn tidy_url(url: &str, port: u16) -> String {
    let url = url.trim().trim_end_matches('/');
    if url.contains("://") {
        return url.to_string();
    }
    let (host, path) = url.split_once('/').map_or((url, String::new()), |(h, p)| (h, format!("/{p}")));
    if host.contains(':') { format!("http://{url}") } else { format!("http://{host}:{port}{path}") }
}

/// Turns a failed request into an error that doesn't show the address
/// (which may hold a token). The server refusing us means we need to sign
/// in again.
fn request_error(err: ureq::Error) -> anyhow::Error {
    match err {
        ureq::Error::Status(401, _) => SignedOut.into(),
        ureq::Error::Status(code, response) => anyhow::anyhow!("the server said {code} {}", response.status_text()),
        ureq::Error::Transport(transport) => {
            let detail = transport.message().map(|m| format!(": {m}")).unwrap_or_default();
            anyhow::anyhow!("couldn't reach the server ({}{detail})", transport.kind())
        }
    }
}

/// A pretend server for tests. `answer` is given each request's first
/// line ("GET /path?query HTTP/1.1") and its headers (lowercased), and
/// returns the status and body to send back. Returns the server's address.
#[cfg(test)]
pub fn mock_server(answer: impl Fn(&str, &[String]) -> (u16, String) + Send + Sync + 'static) -> String {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let answer = std::sync::Arc::new(answer);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let answer = answer.clone();
            thread::spawn(move || {
                let mut reader = BufReader::new(&stream);
                let (mut first, mut headers, mut line) = (String::new(), Vec::new(), String::new());
                reader.read_line(&mut first).unwrap();
                while reader.read_line(&mut line).is_ok_and(|n| n > 0) && line.trim() != "" {
                    headers.push(line.trim().to_lowercase());
                    line.clear();
                }
                let length = headers.iter().find_map(|h| h.strip_prefix("content-length: ")?.parse().ok()).unwrap_or(0);
                reader.take(length).read_to_end(&mut Vec::new()).unwrap();
                let (status, body) = answer(first.trim(), &headers);
                let reply = format!("HTTP/1.1 {status} Whatever\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = (&stream).write_all(reply.as_bytes());
            });
        }
    });
    address
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_tidied() {
        assert_eq!(tidy_url(" media:8096/ ", 8096), "http://media:8096");
        assert_eq!(tidy_url("media", 9000), "http://media:9000");
        assert_eq!(tidy_url("10.0.0.5:9001/", 9000), "http://10.0.0.5:9001");
        assert_eq!(tidy_url("https://media.example/emby/", 8096), "https://media.example/emby");
    }
}
