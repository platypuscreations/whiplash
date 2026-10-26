//! Reading a track from a web address, as a music server sends it.
//!
//! symphonia reads a track as a stream of bytes it can jump around in.
//! Over the web, jumping means asking the server for the file from a
//! given byte on (an HTTP "Range" request). symphonia jumps often while it
//! works out a file's format, mostly short hops forward, so a jump here
//! only notes the new place: the next read skips ahead in the download
//! already open, or makes a new request if the place is far away.

use std::io::{self, Read, Seek, SeekFrom};
use std::time::Duration;

use symphonia::core::io::MediaSource;

/// A jump forward of up to this many bytes reads on through the open
/// download rather than starting another.
const SKIP_LIMIT: u64 = 256 * 1024;

/// What a read must do first to be at the right place.
#[derive(Debug, PartialEq, Eq)]
enum Plan {
    /// The open download is there already.
    Read,
    /// Throw away this many bytes of the open download first.
    Skip(u64),
    /// Ask the server again, from the new place.
    Reconnect,
}

/// `body` is how far the open download has got, if one is open.
fn plan(position: u64, body: Option<u64>, seekable: bool) -> Plan {
    match body {
        Some(at) if at == position => Plan::Read,
        // A server that can't send part of a file can only be read on.
        Some(at) if at < position && (position - at <= SKIP_LIMIT || !seekable) => Plan::Skip(position - at),
        _ => Plan::Reconnect,
    }
}

/// The file's total size from a `Content-Range` header ("bytes 0-99/1000").
fn total_from_content_range(header: &str) -> Option<u64> {
    header.strip_prefix("bytes ")?.split_once('/')?.1.trim().parse().ok()
}

pub struct HttpSource {
    agent: ureq::Agent,
    url: String,
    /// The `Authorization` header, for a server that wants a password.
    authorization: Option<String>,
    /// The file's size, if the server said.
    len: Option<u64>,
    /// The server can send part of the file, so jumps backward work.
    seekable: bool,
    /// The file type the server gave, such as "audio/flac".
    content_type: Option<String>,
    /// Where the next read starts.
    position: u64,
    /// The download in progress, and how far it has got.
    body: Option<(u64, Box<dyn Read + Send + Sync>)>,
}

impl HttpSource {
    /// Starts downloading from `url`, sending `authorization` as the
    /// `Authorization` header if given. Fails if the server can't be
    /// reached or won't send the file.
    pub fn open(url: &str, authorization: Option<String>) -> io::Result<HttpSource> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(15))
            .build();
        let response = request(&agent, url, authorization.as_deref(), 0)?;
        let length = response.header("Content-Length").and_then(|l| l.trim().parse().ok());
        let total = response.header("Content-Range").and_then(total_from_content_range);
        let seekable = response.status() == 206 && total.is_some();
        let content_type = response.header("Content-Type").map(|t| t.split(';').next().unwrap_or(t).trim().to_string());
        Ok(HttpSource {
            agent,
            url: url.to_string(),
            authorization,
            len: if seekable { total } else { length },
            seekable,
            content_type,
            position: 0,
            body: Some((0, response.into_reader())),
        })
    }

    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Gets the open download to `self.position`.
    fn get_there(&mut self) -> io::Result<()> {
        match plan(self.position, self.body.as_ref().map(|(at, _)| *at), self.seekable) {
            Plan::Read => Ok(()),
            Plan::Skip(bytes) => {
                let (at, body) = self.body.as_mut().expect("planned a skip without a download");
                let skipped = io::copy(&mut body.take(bytes), &mut io::sink())?;
                *at += skipped;
                if skipped < bytes {
                    // The download ended early; the read finds nothing more.
                    self.position = *at;
                }
                Ok(())
            }
            Plan::Reconnect if !self.seekable => {
                Err(io::Error::new(io::ErrorKind::Unsupported, "the server can't jump back in this file"))
            }
            Plan::Reconnect => {
                self.body = None;
                let response = request(&self.agent, &self.url, self.authorization.as_deref(), self.position)?;
                self.body = Some((self.position, response.into_reader()));
                Ok(())
            }
        }
    }

    /// Reads from the open download, counting how far it has got.
    fn read_body(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let Some((at, body)) = self.body.as_mut() else { return Ok(0) };
        let read = body.read(buf)?;
        *at += read as u64;
        Ok(read)
    }
}

/// Asks for the file from byte `from` on. The error never includes the
/// address, which may hold a password or token.
fn request(agent: &ureq::Agent, url: &str, authorization: Option<&str>, from: u64) -> io::Result<ureq::Response> {
    let mut request = agent
        .get(url)
        .set("Range", &format!("bytes={from}-"))
        // Compressed downloads would throw the byte counts off.
        .set("Accept-Encoding", "identity");
    if let Some(authorization) = authorization {
        request = request.set("Authorization", authorization);
    }
    crate::net::send(request, None).map_err(|err| match err {
        ureq::Error::Status(code, response) => {
            io::Error::other(format!("the server said {code} {}", response.status_text()))
        }
        ureq::Error::Transport(transport) => io::Error::other(crate::net::problem(&transport)),
    })
}

impl Read for HttpSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.len.is_some_and(|len| self.position >= len) {
            return Ok(0);
        }
        self.get_there()?;
        let read = match self.read_body(buf) {
            // A dropped connection: try once more from here.
            Err(_) if self.seekable => {
                self.body = None;
                self.get_there()?;
                self.read_body(buf)?
            }
            result => result?,
        };
        self.position += read as u64;
        Ok(read)
    }
}

impl Seek for HttpSource {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let target = match from {
            SeekFrom::Start(offset) => Some(offset as i128),
            SeekFrom::Current(delta) => Some(self.position as i128 + delta as i128),
            SeekFrom::End(delta) => self.len.map(|len| len as i128 + delta as i128),
        };
        match target {
            Some(target) if target >= 0 => {
                self.position = target as u64;
                Ok(self.position)
            }
            Some(_) => Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before the start of the file")),
            None => Err(io::Error::new(io::ErrorKind::Unsupported, "the file's size is unknown")),
        }
    }
}

impl MediaSource for HttpSource {
    fn is_seekable(&self) -> bool {
        self.seekable
    }

    fn byte_len(&self) -> Option<u64> {
        self.len
    }
}

/// A tiny web server for tests, sending `bytes` to every request and
/// honoring Range requests. Returns its address.
#[cfg(test)]
pub fn serve(bytes: Vec<u8>) -> String {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/track", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let bytes = bytes.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(&stream);
                let mut from = 0;
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 0) && line.trim() != "" {
                    if let Some(range) = line.to_lowercase().strip_prefix("range: bytes=") {
                        from = range.trim().trim_end_matches('-').parse::<usize>().unwrap_or(0).min(bytes.len());
                    }
                    line.clear();
                }
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nContent-Range: bytes {from}-{}/{}\r\n\r\n",
                    bytes.len() - from,
                    bytes.len().saturating_sub(1),
                    bytes.len()
                );
                let mut stream = &stream;
                let _ = stream.write_all(head.as_bytes()).and_then(|_| stream.write_all(&bytes[from..]));
            });
        }
    });
    address
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_hops_forward_read_on() {
        assert_eq!(plan(100, Some(100), true), Plan::Read);
        assert_eq!(plan(150, Some(100), true), Plan::Skip(50));
        assert_eq!(plan(100 + SKIP_LIMIT + 1, Some(100), true), Plan::Reconnect);
        assert_eq!(plan(50, Some(100), true), Plan::Reconnect);
        assert_eq!(plan(0, None, true), Plan::Reconnect);
        // Without Range requests, only reading on works.
        assert_eq!(plan(100 + SKIP_LIMIT + 1, Some(100), false), Plan::Skip(SKIP_LIMIT + 1));
    }

    #[test]
    fn content_range_gives_the_size() {
        assert_eq!(total_from_content_range("bytes 0-99/1000"), Some(1000));
        assert_eq!(total_from_content_range("bytes 0-99/*"), None);
        assert_eq!(total_from_content_range("items 0-1/2"), None);
    }

    #[test]
    fn reads_and_jumps_around_a_download() {
        let bytes: Vec<u8> = (0..1_000_000u32).map(|i| (i % 251) as u8).collect();
        let mut source = HttpSource::open(&serve(bytes.clone()), None).unwrap();
        assert!(source.is_seekable());
        assert_eq!(source.byte_len(), Some(1_000_000));
        assert_eq!(source.content_type(), Some("audio/wav"));
        let mut buf = [0; 10];
        let mut check = |source: &mut HttpSource, at: usize| {
            source.read_exact(&mut buf).unwrap();
            assert_eq!(buf[..], bytes[at..at + 10]);
        };
        check(&mut source, 0);
        source.seek(SeekFrom::Current(1000)).unwrap(); // skips ahead
        check(&mut source, 1010);
        source.seek(SeekFrom::Start(900_000)).unwrap(); // asks again
        check(&mut source, 900_000);
        source.seek(SeekFrom::Start(5)).unwrap(); // and back
        check(&mut source, 5);
        source.seek(SeekFrom::End(-10)).unwrap();
        check(&mut source, 999_990);
        assert_eq!(source.read(&mut buf).unwrap(), 0);
    }
}
