//! Asking servers for things over HTTP: shared by the music servers'
//! libraries and by playing their tracks.

/// Sends `request`, with `body` if there is one. If no connection could be
/// made, it tries once more: looking up a server's name now and then
/// stalls for a few seconds (one whose DNS answer may only be kept for a
/// moment has to be looked up again and again), and the second try goes
/// through.
// Returns ureq's own (large) error, as `call` does, for callers to turn into theirs.
#[allow(clippy::result_large_err)]
pub fn send(request: ureq::Request, body: Option<&str>) -> Result<ureq::Response, ureq::Error> {
    let attempt = |request: ureq::Request| match body {
        Some(body) => request.send_string(body),
        None => request.call(),
    };
    match attempt(request.clone()) {
        Err(ureq::Error::Transport(transport)) if no_connection(transport.kind()) => attempt(request),
        result => result,
    }
}

/// Failures to reach the server at all, which may well not happen again.
fn no_connection(kind: ureq::ErrorKind) -> bool {
    matches!(kind, ureq::ErrorKind::Dns | ureq::ErrorKind::ConnectionFailed | ureq::ErrorKind::Io)
}

/// What went wrong reaching a server, such as "Network Error (timed out)".
/// It never includes the address, which may hold a password or token.
pub fn problem(transport: &ureq::Transport) -> String {
    let mut text = transport.kind().to_string();
    if let Some(message) = transport.message() {
        text += &format!(": {message}");
    }
    if let Some(source) = std::error::Error::source(transport) {
        text += &format!(" ({source})");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn a_second_try_after_failing_to_connect() {
        // A server that drops the first connection without answering.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let connections = Arc::new(AtomicUsize::new(0));
        let counted = connections.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if counted.fetch_add(1, Ordering::Relaxed) == 0 {
                    continue; // dropped: the connection closes unanswered
                }
                let mut line = String::new();
                let mut reader = BufReader::new(&stream);
                while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                    line.clear();
                }
                let _ = (&stream).write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi");
            }
        });
        let response = send(ureq::get(&url), None).unwrap();
        assert_eq!(response.into_string().unwrap(), "hi");
        assert_eq!(connections.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn problems_say_why_without_the_address() {
        let Err(ureq::Error::Transport(transport)) = ureq::get("http://127.0.0.1:1/secret-token").call() else {
            panic!("expected to fail to connect");
        };
        let text = problem(&transport);
        assert!(text.contains('('), "no reason given: {text}");
        assert!(!text.contains("secret-token"), "{text}");
    }
}
