//! Emby's web API: signing in, browsing the music library, and the address
//! a track plays from (see https://dev.emby.media).

use std::io::Read;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{Item, ItemKind, Request, Section, Server, ServerConfig, SignedOut, request_error};
use crate::config::EmbyConfig;

/// At most this many search results of each kind.
const SEARCH_LIMIT: &str = "100";

pub struct Emby {
    config: EmbyConfig,
    agent: ureq::Agent,
    /// The library being asked about (see `list`), if just one.
    section: Option<String>,
}

/// An entry as Emby describes it; only what we use.
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct EmbyItem {
    id: String,
    name: String,
    #[serde(rename = "Type")]
    kind: String,
    production_year: Option<u32>,
    album_artist: Option<String>,
    artists: Vec<String>,
    /// Length in ticks of 100 nanoseconds.
    run_time_ticks: Option<u64>,
    /// What a library holds, such as "music" or "movies".
    collection_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ItemList {
    items: Vec<EmbyItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SignInAnswer {
    access_token: String,
    user: User,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct User {
    id: String,
}

/// The address a track plays from. It holds the token, so it must never
/// be shown or saved.
pub fn stream_url(config: &EmbyConfig, id: &str) -> String {
    format!("{}/Audio/{id}/stream?static=true&api_key={}", config.url, config.token)
}

impl Emby {
    pub fn new(config: EmbyConfig) -> Emby {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(30))
            .build();
        Emby { config, agent, section: None }
    }

    /// Who's asking, as Emby wants every app to say.
    fn authorization(&self) -> String {
        format!(
            "Emby Client=\"Whiplash\", Device=\"Whiplash Player\", DeviceId=\"{}\", Version=\"{}\"",
            self.config.device_id,
            env!("CARGO_PKG_VERSION")
        )
    }

    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Vec<EmbyItem>> {
        if self.config.token.is_empty() {
            return Err(SignedOut.into());
        }
        let mut request = self
            .agent
            .get(&format!("{}{path}", self.config.url))
            .set("X-Emby-Authorization", &self.authorization())
            .set("X-Emby-Token", &self.config.token);
        let common = [("Recursive", "true"), ("EnableImages", "false"), ("EnableUserData", "false")];
        for (name, value) in query.iter().chain(&common) {
            request = request.query(name, value);
        }
        // Only from the one library, unless the question is already about
        // something inside it (an album's tracks).
        if let Some(section) = &self.section
            && !query.iter().any(|(name, _)| *name == "ParentId")
        {
            request = request.query("ParentId", section);
        }
        let response = request.call().map_err(request_error)?;
        parse_items(response.into_reader())
    }

    /// The user's library entries that match `query`.
    fn items(&self, query: &[(&str, &str)]) -> Result<Vec<EmbyItem>> {
        self.get(&format!("/Users/{}/Items", self.config.user_id), query)
    }
}

impl Server for Emby {
    fn sign_in(&mut self, user: &str, password: &str) -> Result<ServerConfig> {
        let body = serde_json::json!({ "Username": user, "Pw": password }).to_string();
        let response = self
            .agent
            .post(&format!("{}/Users/AuthenticateByName", self.config.url))
            .set("X-Emby-Authorization", &self.authorization())
            .set("Content-Type", "application/json")
            .send_string(&body)
            .map_err(|err| match err {
                ureq::Error::Status(401, _) => anyhow::anyhow!("wrong user name or password"),
                err => request_error(err),
            })?;
        let answer: SignInAnswer =
            serde_json::from_reader(response.into_reader()).context("that doesn't look like an Emby server")?;
        self.config.user = user.to_string();
        self.config.user_id = answer.user.id;
        self.config.token = answer.access_token;
        Ok(ServerConfig::Emby(self.config.clone()))
    }

    /// The user's music and audiobook libraries.
    fn sections(&mut self) -> Result<Vec<Section>> {
        self.section = None;
        let views = self.get(&format!("/Users/{}/Views", self.config.user_id), &[])?;
        Ok(audio_sections(views))
    }

    fn list(&mut self, section: Option<&str>, request: &Request) -> Result<Vec<Item>> {
        self.section = section.map(str::to_string);
        let user_id = self.config.user_id.clone();
        Ok(match request {
            Request::Artists => {
                let found = self.get("/Artists/AlbumArtists", &[("UserId", &user_id), ("SortBy", "SortName")])?;
                found.into_iter().map(|item| artist(item, "")).collect()
            }
            Request::Albums { artist: None } => {
                let found = self.items(&[("IncludeItemTypes", "MusicAlbum"), ("SortBy", "SortName")])?;
                found.into_iter().map(|item| album(item, |i| i.album_artist.clone().unwrap_or_default())).collect()
            }
            Request::Albums { artist: Some(id) } => {
                let query = [("IncludeItemTypes", "MusicAlbum"), ("AlbumArtistIds", id), ("SortBy", "ProductionYear,SortName")];
                let found = self.items(&query)?;
                found.into_iter().map(|item| album(item, |i| i.production_year.map(|y| y.to_string()).unwrap_or_default())).collect()
            }
            Request::Tracks { album } => {
                let query = [("ParentId", album.as_str()), ("IncludeItemTypes", "Audio"), ("SortBy", "ParentIndexNumber,IndexNumber,SortName")];
                self.items(&query)?.into_iter().map(track).collect()
            }
            Request::ArtistTracks { artist } => {
                let query = [
                    ("ArtistIds", artist.as_str()),
                    ("IncludeItemTypes", "Audio"),
                    ("SortBy", "ProductionYear,Album,ParentIndexNumber,IndexNumber"),
                ];
                self.items(&query)?.into_iter().map(track).collect()
            }
            Request::Genres => {
                let found = self.get("/MusicGenres", &[("UserId", &user_id), ("SortBy", "SortName")])?;
                found.into_iter().map(genre).collect()
            }
            Request::GenreAlbums { genre } => {
                let query = [("IncludeItemTypes", "MusicAlbum"), ("GenreIds", genre.as_str()), ("SortBy", "AlbumArtist,SortName")];
                let found = self.items(&query)?;
                found.into_iter().map(|item| album(item, |i| i.album_artist.clone().unwrap_or_default())).collect()
            }
            Request::GenreTracks { genre } => {
                let query = [
                    ("GenreIds", genre.as_str()),
                    ("IncludeItemTypes", "Audio"),
                    ("SortBy", "AlbumArtist,Album,ParentIndexNumber,IndexNumber"),
                ];
                self.items(&query)?.into_iter().map(track).collect()
            }
            Request::Search(text) => {
                let query = [("SearchTerm", text.as_str()), ("IncludeItemTypes", "MusicArtist,MusicAlbum,Audio"), ("Limit", SEARCH_LIMIT)];
                search_results(self.items(&query)?)
            }
        })
    }
}

fn parse_items(reader: impl Read) -> Result<Vec<EmbyItem>> {
    let list: ItemList = serde_json::from_reader(reader).context("the server's answer didn't make sense")?;
    Ok(list.items)
}

/// The libraries that hold music or audiobooks, of all a user's.
fn audio_sections(views: Vec<EmbyItem>) -> Vec<Section> {
    views
        .into_iter()
        .filter(|view| matches!(view.collection_type.as_deref(), Some("music" | "audiobooks")))
        .map(|view| Section { id: view.id, name: view.name })
        .collect()
}

fn genre(item: EmbyItem) -> Item {
    Item { kind: ItemKind::Genre, id: item.id, title: item.name, detail: String::new(), duration: None }
}

fn artist(item: EmbyItem, detail: &str) -> Item {
    Item { kind: ItemKind::Artist, id: item.id, title: item.name, detail: detail.to_string(), duration: None }
}

fn album(item: EmbyItem, detail: impl Fn(&EmbyItem) -> String) -> Item {
    let detail = detail(&item);
    Item { kind: ItemKind::Album, id: item.id, title: item.name, detail, duration: None }
}

/// A track, titled "Artist - Title" as in the playlist.
fn track(item: EmbyItem) -> Item {
    let artist = if item.artists.is_empty() { item.album_artist.unwrap_or_default() } else { item.artists.join(", ") };
    let title = if artist.is_empty() { item.name } else { format!("{artist} - {}", item.name) };
    let duration = item.run_time_ticks.map(|ticks| ticks as f64 / 10_000_000.0);
    Item { kind: ItemKind::Track, id: item.id, title, detail: String::new(), duration }
}

/// Artists first, then albums, then tracks, each in the server's order.
fn search_results(found: Vec<EmbyItem>) -> Vec<Item> {
    let mut items: Vec<Item> = found
        .into_iter()
        .filter_map(|item| match item.kind.as_str() {
            "MusicArtist" => Some(artist(item, "artist")),
            "MusicAlbum" => Some(album(item, |_| "album".to_string())),
            "Audio" => Some(track(item)),
            _ => None,
        })
        .collect();
    items.sort_by_key(|item| item.kind as u8);
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(items: &[Item]) -> Vec<(&str, &str)> {
        items.iter().map(|i| (i.title.as_str(), i.detail.as_str())).collect()
    }

    #[test]
    fn albums_and_tracks() {
        let albums = r#"{"Items":[
            {"Name":"OK Computer","Id":"12","Type":"MusicAlbum","ProductionYear":1997,"AlbumArtist":"Radiohead","IsFolder":true},
            {"Name":"Kid A","Id":"13","Type":"MusicAlbum","AlbumArtist":"Radiohead"}],"TotalRecordCount":2}"#;
        let found = parse_items(albums.as_bytes()).unwrap();
        let items: Vec<Item> = found.into_iter().map(|i| album(i, |i| i.production_year.map(|y| y.to_string()).unwrap_or_default())).collect();
        assert_eq!(titles(&items), [("OK Computer", "1997"), ("Kid A", "")]);
        assert_eq!(items[0].request(), Some(Request::Tracks { album: "12".into() }));

        let tracks = r#"{"Items":[
            {"Name":"Airbag","Id":"2001","Type":"Audio","RunTimeTicks":2843000000,"Artists":["Radiohead"],"AlbumArtist":"Radiohead","Container":"flac"},
            {"Name":"Intro","Id":"2002","Type":"Audio","AlbumArtist":"Various"},
            {"Name":"Silence","Id":"2003","Type":"Audio"}]}"#;
        let items: Vec<Item> = parse_items(tracks.as_bytes()).unwrap().into_iter().map(track).collect();
        assert_eq!(items[0].title, "Radiohead - Airbag");
        assert_eq!(items[0].duration, Some(284.3));
        assert_eq!(items[1].title, "Various - Intro");
        assert_eq!(items[2].title, "Silence");
        assert_eq!(items[2].request(), None);
    }

    #[test]
    fn genres() {
        let found = r#"{"Items":[{"Name":"Alt. Rock","Id":"30661","Type":"MusicGenre"},{"Name":"Rock","Id":"29872","Type":"MusicGenre"}]}"#;
        let items: Vec<Item> = parse_items(found.as_bytes()).unwrap().into_iter().map(genre).collect();
        assert_eq!(titles(&items), [("Alt. Rock", ""), ("Rock", "")]);
        assert_eq!(items[1].request(), Some(Request::GenreAlbums { genre: "29872".into() }));
    }

    #[test]
    fn search_results_by_kind() {
        let found = r#"{"Items":[
            {"Name":"Airbag","Id":"1","Type":"Audio","Artists":["Radiohead"]},
            {"Name":"OK Computer","Id":"2","Type":"MusicAlbum"},
            {"Name":"Radiohead","Id":"3","Type":"MusicArtist"},
            {"Name":"A video","Id":"4","Type":"Video"}]}"#;
        let items = search_results(parse_items(found.as_bytes()).unwrap());
        assert_eq!(titles(&items), [("Radiohead", "artist"), ("OK Computer", "album"), ("Radiohead - Airbag", "")]);
    }

    #[test]
    fn talks_to_a_server() {
        let url = super::super::mock_server(|line, headers| {
            let has = |header: &str| headers.iter().any(|h| h.starts_with(header));
            if !has("x-emby-authorization: emby client=\"whiplash\"") {
                return (400, String::new());
            }
            let signed_in = has("x-emby-token: t1");
            match line.split(' ').nth(1).unwrap_or("") {
                "/Users/AuthenticateByName" if line.starts_with("POST") => {
                    (200, r#"{"User":{"Id":"u1"},"AccessToken":"t1"}"#.to_string())
                }
                path if path.starts_with("/Users/u1/Views?") && signed_in => (200, VIEWS.to_string()),
                // Asked within the music library: only its own albums, and
                // an album's tracks by the album alone.
                path if path.starts_with("/Users/u1/Items?") && signed_in && path.contains("SearchTerm=kid") => {
                    match path.contains("ParentId=m1") {
                        true => (200, r#"{"Items":[{"Name":"Kid A","Id":"13","Type":"MusicAlbum"}]}"#.to_string()),
                        false => (404, String::new()),
                    }
                }
                path if path.starts_with("/Users/u1/Items?") && signed_in && path.contains("ParentId=m1") => (404, String::new()),
                path if path.starts_with("/Users/u1/Items?") && signed_in => {
                    let tracks = path.contains("ParentId=12") && path.contains("IncludeItemTypes=Audio");
                    let found = path.contains("SearchTerm=ok+computer") || path.contains("SearchTerm=ok%20computer");
                    match (tracks, found) {
                        (true, _) => (200, r#"{"Items":[{"Name":"Airbag","Id":"2001","Type":"Audio","Artists":["Radiohead"]}]}"#.to_string()),
                        (_, true) => (200, r#"{"Items":[{"Name":"OK Computer","Id":"12","Type":"MusicAlbum"}]}"#.to_string()),
                        _ => (404, String::new()),
                    }
                }
                _ => (401, String::new()),
            }
        });
        let mut emby = Emby::new(EmbyConfig { url, device_id: "d1".into(), ..EmbyConfig::default() });
        assert!(emby.list(None, &Request::Artists).unwrap_err().is::<SignedOut>());
        let Ok(ServerConfig::Emby(config)) = emby.sign_in("luke", "secret") else { panic!("didn't sign in") };
        assert_eq!((config.user.as_str(), config.user_id.as_str(), config.token.as_str()), ("luke", "u1", "t1"));
        let tracks = emby.list(None, &Request::Tracks { album: "12".into() }).unwrap();
        assert_eq!(titles(&tracks), [("Radiohead - Airbag", "")]);
        let found = emby.list(None, &Request::Search("ok computer".into())).unwrap();
        assert_eq!(titles(&found), [("OK Computer", "album")]);

        let sections = emby.sections().unwrap();
        assert_eq!(sections, [Section { id: "m1".into(), name: "Music".into() }, Section { id: "b1".into(), name: "Audio books".into() }]);
        let found = emby.list(Some("m1"), &Request::Search("kid".into())).unwrap();
        assert_eq!(titles(&found), [("Kid A", "album")]);
        let tracks = emby.list(Some("m1"), &Request::Tracks { album: "12".into() }).unwrap();
        assert_eq!(titles(&tracks), [("Radiohead - Airbag", "")]);

        // A server that no longer takes the token means signing in again.
        let mut stale = Emby::new(EmbyConfig { token: "old".into(), ..config });
        assert!(stale.list(None, &Request::Tracks { album: "12".into() }).unwrap_err().is::<SignedOut>());
    }

    /// A user's libraries: music, audiobooks, and others that aren't audio.
    const VIEWS: &str = r#"{"Items":[
        {"Name":"Music","Id":"m1","CollectionType":"music"},
        {"Name":"Movies","Id":"v1","CollectionType":"movies"},
        {"Name":"Audio books","Id":"b1","CollectionType":"audiobooks"},
        {"Name":"Mixed content","Id":"x1"}]}"#;

    #[test]
    fn only_audio_libraries() {
        let sections = audio_sections(parse_items(VIEWS.as_bytes()).unwrap());
        let names: Vec<&str> = sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Music", "Audio books"]);
    }

    #[test]
    fn signing_in_answer() {
        let answer = r#"{"User":{"Name":"luke","Id":"e88"},"SessionInfo":{},"AccessToken":"4b19","ServerId":"x"}"#;
        let answer: SignInAnswer = serde_json::from_str(answer).unwrap();
        assert_eq!((answer.user.id.as_str(), answer.access_token.as_str()), ("e88", "4b19"));
        let config = EmbyConfig { url: "http://media:8096".into(), token: "4b19".into(), ..EmbyConfig::default() };
        assert_eq!(stream_url(&config, "2001"), "http://media:8096/Audio/2001/stream?static=true&api_key=4b19");
    }
}

/// Signs in to a real Emby server, browses to a track and decodes the
/// start of it. Needs a server, so it only runs on request:
///
/// ```sh
/// WHIPLASH_TEST_EMBY="http://media:8096 user password" cargo test emby -- --ignored --nocapture
/// ```
#[cfg(test)]
mod server_tests {
    use super::*;
    use crate::audio::{Source, Track};
    use crate::location::Location;

    #[test]
    #[ignore]
    fn browses_and_plays_from_a_real_server() {
        let account = std::env::var("WHIPLASH_TEST_EMBY").expect("set WHIPLASH_TEST_EMBY to \"url user password\"");
        let [url, user, password] = <[&str; 3]>::try_from(account.split_whitespace().collect::<Vec<_>>()).expect("url user password");
        let config = EmbyConfig { url: super::super::tidy_url(url, 8096), device_id: "whiplash-test".into(), ..EmbyConfig::default() };
        let mut emby = Emby::new(config);
        let Ok(ServerConfig::Emby(config)) = emby.sign_in(user, password) else { panic!("didn't sign in") };

        let sections = emby.sections().unwrap();
        println!("libraries: {:?}", sections.iter().map(|s| &s.name).collect::<Vec<_>>());
        for section in &sections {
            let artists = emby.list(Some(&section.id), &Request::Artists).unwrap();
            let albums = emby.list(Some(&section.id), &Request::Albums { artist: None }).unwrap();
            println!("  {}: {} artists, {} albums", section.name, artists.len(), albums.len());
        }
        let artists = emby.list(None, &Request::Artists).unwrap();
        println!("{} artists, starting with {:?}", artists.len(), artists.first().map(|a| &a.title));
        let albums = emby.list(None, &artists[0].request().unwrap()).unwrap();
        println!("{}: {} albums", artists[0].title, albums.len());
        let tracks = emby.list(None, &albums[0].request().unwrap()).unwrap();
        println!("{}: {} tracks", albums[0].title, tracks.len());
        let found = emby.list(None, &Request::Search(artists[0].title.clone())).unwrap();
        println!("searching for {:?}: {} results", artists[0].title, found.len());
        let genres = emby.list(None, &Request::Genres).unwrap();
        let in_genre = emby.list(None, &genres[0].request().unwrap()).unwrap();
        let genre_tracks = emby.list(None, &Request::GenreTracks { genre: genres[0].id.clone() }).unwrap();
        println!("{} genres; {}: {} albums, {} tracks", genres.len(), genres[0].title, in_genre.len(), genre_tracks.len());
        assert!(!in_genre.is_empty() && !genre_tracks.is_empty());

        let track = &tracks[0];
        let source = Source::Http { url: stream_url(&config, &track.id), authorization: None, title: track.title.clone() };
        let mut opened = Track::open_source(&source, Location::Emby(track.id.clone())).unwrap();
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
