//! Logitech Media Server (now Lyrion Music Server), which Squeezebox
//! players play from: browsing its music library through its JSON-RPC
//! interface, and the address a track's file downloads from.
//!
//! Every question is a command line, as the server's own command-line
//! interface takes it (`["titles", 0, 100, "album_id:12", "tags:ad"]`),
//! sent to `/jsonrpc.js`. Answers hold lists named after the command
//! (`titles_loop`). The server isn't always consistent about whether a
//! number comes as a number or as text, so answers are read loosely.

use std::time::Duration;

use anyhow::{Context, Result};
use base64::Engine;
use serde_json::{Value, json};

use super::{Item, ItemKind, Request, Server, ServerConfig, request_error};
use crate::config::SqueezeConfig;

/// How many entries to ask for: everything, for any library short of
/// enormous.
const ALL: u32 = 100_000;
/// At most this many search results of each kind.
const SEARCH_LIMIT: u32 = 100;

pub struct Squeeze {
    config: SqueezeConfig,
    agent: ureq::Agent,
}

/// The address a track's file downloads from.
pub fn stream_url(config: &SqueezeConfig, id: &str) -> String {
    format!("{}/music/{id}/download", config.url)
}

/// The `Authorization` header for a server with a password, if it has one.
pub fn authorization(config: &SqueezeConfig) -> Option<String> {
    if config.user.is_empty() && config.password.is_empty() {
        return None;
    }
    let pair = format!("{}:{}", config.user, config.password);
    Some(format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(pair)))
}

impl Squeeze {
    pub fn new(config: SqueezeConfig) -> Squeeze {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(30))
            .build();
        Squeeze { config, agent }
    }

    /// Sends a command and returns the answer's `result`.
    fn command(&self, command: Value) -> Result<Value> {
        let body = json!({ "id": 1, "method": "slim.request", "params": ["", command] }).to_string();
        let mut request =
            self.agent.post(&format!("{}/jsonrpc.js", self.config.url)).set("Content-Type", "application/json");
        if let Some(authorization) = authorization(&self.config) {
            request = request.set("Authorization", &authorization);
        }
        let response = request.send_string(&body).map_err(|err| match err {
            ureq::Error::Status(401, _) => anyhow::anyhow!("wrong user name or password"),
            err => request_error(err),
        })?;
        let answer: Value =
            serde_json::from_reader(response.into_reader()).context("that doesn't look like a Logitech Media Server")?;
        answer.get("result").cloned().context("the server didn't understand the question")
    }

    /// The entries of a list command, such as "albums".
    fn list_of(&self, command: &str, filters: &[String]) -> Result<Vec<Value>> {
        let mut line = vec![json!(command), json!(0), json!(ALL)];
        line.extend(filters.iter().map(|f| json!(f)));
        Ok(loop_of(&self.command(Value::Array(line))?, command))
    }
}

impl Server for Squeeze {
    /// There's no signing in: this checks that the server answers (and
    /// takes the password, if it has one).
    fn sign_in(&mut self, user: &str, password: &str) -> Result<ServerConfig> {
        self.config.user = user.to_string();
        self.config.password = password.to_string();
        let status = self.command(json!(["serverstatus", 0, 0]))?;
        if status.get("version").is_none() {
            anyhow::bail!("that doesn't look like a Logitech Media Server");
        }
        Ok(ServerConfig::Squeeze(self.config.clone()))
    }

    fn list(&mut self, request: &Request) -> Result<Vec<Item>> {
        let f = |filter: &str| filter.to_string();
        Ok(match request {
            Request::Artists => self.list_of("artists", &[])?.iter().map(|a| artist(a, "")).collect(),
            Request::Albums { artist: None } => {
                let found = self.list_of("albums", &[f("tags:lay"), f("sort:album")])?;
                found.iter().map(|a| album(a, text(a, "artist"))).collect()
            }
            Request::Albums { artist: Some(id) } => {
                let found = self.list_of("albums", &[format!("artist_id:{id}"), f("tags:lay"), f("sort:yearalbum")])?;
                found.iter().map(|a| album(a, text(a, "year").filter(|y| y != "0"))).collect()
            }
            Request::Tracks { album } => {
                let found = self.list_of("titles", &[format!("album_id:{album}"), f("tags:ad"), f("sort:tracknum")])?;
                found.iter().map(track).collect()
            }
            Request::ArtistTracks { artist } => {
                let found = self.list_of("titles", &[format!("artist_id:{artist}"), f("tags:ad"), f("sort:albumtrack")])?;
                found.iter().map(track).collect()
            }
            Request::Genres => self.list_of("genres", &[])?.iter().map(genre).collect(),
            Request::GenreAlbums { genre } => {
                let found = self.list_of("albums", &[format!("genre_id:{genre}"), f("tags:lay"), f("sort:artflow")])?;
                found.iter().map(|a| album(a, text(a, "artist"))).collect()
            }
            Request::GenreTracks { genre } => {
                let found = self.list_of("titles", &[format!("genre_id:{genre}"), f("tags:ad"), f("sort:albumtrack")])?;
                found.iter().map(track).collect()
            }
            Request::Search(text) => {
                let search = format!("search:{text}");
                let limit = |mut found: Vec<Value>| {
                    found.truncate(SEARCH_LIMIT as usize);
                    found
                };
                let artists = limit(self.list_of("artists", std::slice::from_ref(&search))?);
                let albums = limit(self.list_of("albums", &[search.clone(), f("tags:la")])?);
                let tracks = limit(self.list_of("titles", &[search, f("tags:ad")])?);
                let mut items: Vec<Item> = artists.iter().map(|a| artist(a, "artist")).collect();
                items.extend(albums.iter().map(|a| album(a, Some("album".to_string()))));
                items.extend(tracks.iter().map(track));
                items
            }
        })
    }
}

/// The entries of a list in an answer: `albums_loop` for "albums".
fn loop_of(result: &Value, command: &str) -> Vec<Value> {
    let name = if command == "titles" { "titles_loop".to_string() } else { format!("{command}_loop") };
    result.get(&name).and_then(Value::as_array).cloned().unwrap_or_default()
}

/// A field as text, whether the server sent text or a number.
fn text(entry: &Value, field: &str) -> Option<String> {
    match entry.get(field)? {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// A field as a number, whether the server sent a number or text.
fn number(entry: &Value, field: &str) -> Option<f64> {
    match entry.get(field)? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn genre(entry: &Value) -> Item {
    Item {
        kind: ItemKind::Genre,
        id: text(entry, "id").unwrap_or_default(),
        title: text(entry, "genre").unwrap_or_default(),
        detail: String::new(),
        duration: None,
    }
}

fn artist(entry: &Value, detail: &str) -> Item {
    Item {
        kind: ItemKind::Artist,
        id: text(entry, "id").unwrap_or_default(),
        title: text(entry, "artist").unwrap_or_default(),
        detail: detail.to_string(),
        duration: None,
    }
}

fn album(entry: &Value, detail: Option<String>) -> Item {
    Item {
        kind: ItemKind::Album,
        id: text(entry, "id").unwrap_or_default(),
        title: text(entry, "album").unwrap_or_default(),
        detail: detail.unwrap_or_default(),
        duration: None,
    }
}

/// A track, titled "Artist - Title" as in the playlist.
fn track(entry: &Value) -> Item {
    let name = text(entry, "title").unwrap_or_default();
    let title = match text(entry, "artist") {
        Some(artist) => format!("{artist} - {name}"),
        None => name,
    };
    Item {
        kind: ItemKind::Track,
        id: text(entry, "id").unwrap_or_default(),
        title,
        detail: String::new(),
        duration: number(entry, "duration").filter(|&d| d > 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(items: &[Item]) -> Vec<(&str, &str)> {
        items.iter().map(|i| (i.title.as_str(), i.detail.as_str())).collect()
    }

    #[test]
    fn reads_answers_loosely() {
        let result = json!({
            "count": 2,
            "titles_loop": [
                {"id": 2001, "title": "Airbag", "artist": "Radiohead", "duration": 284.3},
                {"id": "2002", "title": "Untitled", "duration": "61"}
            ]
        });
        let items: Vec<Item> = loop_of(&result, "titles").iter().map(track).collect();
        assert_eq!(titles(&items), [("Radiohead - Airbag", ""), ("Untitled", "")]);
        assert_eq!((items[0].id.as_str(), items[0].duration), ("2001", Some(284.3)));
        assert_eq!((items[1].id.as_str(), items[1].duration), ("2002", Some(61.0)));
        assert!(loop_of(&result, "albums").is_empty());

        let result = json!({"albums_loop": [{"id": 12, "album": "OK Computer", "year": 1997}, {"id": 13, "album": "Kid A", "year": 0}]});
        let items: Vec<Item> = loop_of(&result, "albums").iter().map(|a| album(a, text(a, "year").filter(|y| y != "0"))).collect();
        assert_eq!(titles(&items), [("OK Computer", "1997"), ("Kid A", "")]);
        assert_eq!(items[0].request(), Some(Request::Tracks { album: "12".into() }));
    }

    #[test]
    fn genres() {
        let result = json!({"count": 2, "genres_loop": [{"genre": "Rock", "id": 127}, {"id": "128", "genre": "Jazz"}]});
        let items: Vec<Item> = loop_of(&result, "genres").iter().map(genre).collect();
        assert_eq!(titles(&items), [("Rock", ""), ("Jazz", "")]);
        assert_eq!(items[0].request(), Some(Request::GenreAlbums { genre: "127".into() }));
    }

    #[test]
    fn passwords_go_in_a_header() {
        let mut config = SqueezeConfig { url: "http://music:9000".into(), ..SqueezeConfig::default() };
        assert_eq!(authorization(&config), None);
        config.user = "luke".into();
        config.password = "secret".into();
        assert_eq!(authorization(&config).as_deref(), Some("Basic bHVrZTpzZWNyZXQ="));
        assert_eq!(stream_url(&config, "2001"), "http://music:9000/music/2001/download");
    }

    #[test]
    fn talks_to_a_server() {
        let url = super::super::mock_server(|line, headers| {
            if !line.starts_with("POST /jsonrpc.js ") || !headers.iter().any(|h| h == "authorization: basic bhvrztpzzwnyzxq=") {
                return (401, String::new());
            }
            (200, json!({"result": {"version": "8.5.0", "artists_loop": [{"id": 3, "artist": "Radiohead"}]}}).to_string())
        });
        let mut squeeze = Squeeze::new(SqueezeConfig { url, ..SqueezeConfig::default() });
        assert_eq!(squeeze.sign_in("luke", "wrong").err().map(|e| e.to_string()).as_deref(), Some("wrong user name or password"));
        let Ok(ServerConfig::Squeeze(config)) = squeeze.sign_in("luke", "secret") else { panic!("didn't sign in") };
        assert_eq!(config.password, "secret");
        let artists = squeeze.list(&Request::Artists).unwrap();
        assert_eq!(titles(&artists), [("Radiohead", "")]);
        assert_eq!(artists[0].request(), Some(Request::Albums { artist: Some("3".into()) }));
    }
}

/// Browses a real Logitech Media Server to a track and decodes the start
/// of it. Needs a server, so it only runs on request:
///
/// ```sh
/// WHIPLASH_TEST_SQUEEZE="http://music:9000 [user password]" cargo test squeeze -- --ignored --nocapture
/// ```
#[cfg(test)]
mod server_tests {
    use super::*;
    use crate::audio::{Source, Track};
    use crate::location::Location;

    #[test]
    #[ignore]
    fn browses_and_plays_from_a_real_server() {
        let account = std::env::var("WHIPLASH_TEST_SQUEEZE").expect("set WHIPLASH_TEST_SQUEEZE to \"url [user password]\"");
        let mut words = account.split_whitespace();
        let url = super::super::tidy_url(words.next().expect("url"), 9000);
        let (user, password) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
        let mut squeeze = Squeeze::new(SqueezeConfig { url, ..SqueezeConfig::default() });
        let Ok(ServerConfig::Squeeze(config)) = squeeze.sign_in(user, password) else { panic!("didn't connect") };

        let artists = squeeze.list(&Request::Artists).unwrap();
        println!("{} artists, starting with {:?}", artists.len(), artists.first().map(|a| &a.title));
        let albums = squeeze.list(&artists[0].request().unwrap()).unwrap();
        println!("{}: {} albums", artists[0].title, albums.len());
        let tracks = squeeze.list(&albums[0].request().unwrap()).unwrap();
        println!("{}: {} tracks", albums[0].title, tracks.len());
        let found = squeeze.list(&Request::Search(artists[0].title.clone())).unwrap();
        println!("searching for {:?}: {} results", artists[0].title, found.len());
        let genres = squeeze.list(&Request::Genres).unwrap();
        let in_genre = squeeze.list(&genres[0].request().unwrap()).unwrap();
        let genre_tracks = squeeze.list(&Request::GenreTracks { genre: genres[0].id.clone() }).unwrap();
        println!("{} genres; {}: {} albums, {} tracks", genres.len(), genres[0].title, in_genre.len(), genre_tracks.len());
        assert!(!in_genre.is_empty() && !genre_tracks.is_empty());

        let track = &tracks[0];
        let source =
            Source::Http { url: stream_url(&config, &track.id), authorization: authorization(&config), title: track.title.clone() };
        let mut opened = Track::open_source(&source, Location::Squeeze(track.id.clone())).unwrap();
        let info = opened.info.clone();
        println!("{}: {:?} s, {} Hz, {} channels, {:?} kbps", info.title, info.duration, info.sample_rate, info.channels, info.kbps);
        let mut samples = 0;
        while samples < 44_100 * 2 * 5 {
            samples += opened.next_chunk().expect("the track ended early").len();
        }
        if let Some(duration) = info.duration.filter(|&d| d > 30.0) {
            let landed = opened.seek(duration / 2.0).unwrap();
            println!("seeked to {landed:.1} s");
            assert!(opened.next_chunk().is_some());
        }
    }
}
