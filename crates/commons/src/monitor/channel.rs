//! The internal monitor channel — `docs/MONITORING.md` §4.
//!
//! One line in, NDJSON out, close:
//!
//! ```text
//! $ printf 'since 1759000000000\n' | nc 127.0.0.1 7779
//! {"service":"game_server","ts":1759000005000,"started":1758990000000,"interval_ms":5000,…}
//! ```
//!
//! `since <epoch_ms>` returns every buffered sample stamped strictly after
//! that instant, oldest first; an empty line returns the whole ring. Anything
//! else gets one `{"error":…}` line and a close.
//!
//! Same security model as the login server's status channel: it binds to
//! loopback by default and that bind is the control. Widening
//! `InternalMonitorBindAddress` publishes traffic volumes and process memory
//! to anyone who can reach the port.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tracing::warn;

use super::Ring;

/// A request is `since ` plus at most 20 digits; anything longer is not one.
const MAX_REQUEST_BYTES: u64 = 64;

/// A client that connects and says nothing must not hold a task forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Serve the channel until the process exits. Errors are logged and dropped:
/// a monitoring endpoint must never be able to take its server down with it.
pub(crate) async fn accept_loop(listener: TcpListener, service: &'static str, ring: Arc<Ring>) {
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                warn!("monitor channel: accept failed: {e}");
                continue;
            }
        };
        let ring = ring.clone();
        tokio::spawn(async move {
            let (read, mut write) = stream.into_split();
            let response = match tokio::time::timeout(REQUEST_TIMEOUT, read_request(read)).await {
                Ok(Some(line)) => respond(&line, service, &ring),
                Ok(None) => error_line("unreadable request"),
                Err(_) => error_line("timed out waiting for a request line"),
            };
            if let Err(e) = write.write_all(response.as_bytes()).await {
                // A poller that hangs up mid-response is routine.
                warn!("monitor channel: write to {peer} failed: {e}");
            }
            let _ = write.shutdown().await;
        });
    }
}

/// Reads at most [`MAX_REQUEST_BYTES`]: a longer "line" is cut there and
/// fails to parse, rather than growing a buffer for whoever is sending it.
async fn read_request<R: tokio::io::AsyncRead + Unpin>(read: R) -> Option<String> {
    let mut line = String::new();
    BufReader::new(read.take(MAX_REQUEST_BYTES))
        .read_line(&mut line)
        .await
        .ok()?;
    Some(line)
}

/// What the channel writes back for one request line.
fn respond(line: &str, service: &str, ring: &Ring) -> String {
    match parse_request(line) {
        Some(since) => ring
            .since(since)
            .iter()
            .map(|s| s.to_json_line(service))
            .collect(),
        None => error_line("expected `since <epoch_ms>`"),
    }
}

/// `since <ms>` → `Some(ms)`; an empty line → `Some(0)` (everything); anything
/// else → `None`.
fn parse_request(line: &str) -> Option<u64> {
    let line = line.trim();
    if line.is_empty() {
        return Some(0);
    }
    let rest = line.strip_prefix("since")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    rest.trim().parse().ok()
}

fn error_line(msg: &str) -> String {
    format!("{}\n", serde_json::json!({ "error": msg }))
}

#[cfg(test)]
mod tests {
    use super::super::Sample;
    use super::*;
    use std::collections::BTreeMap;
    use tokio::net::TcpStream;

    fn sample(ts_ms: u64) -> Sample {
        Sample {
            ts_ms,
            started_ms: 1,
            interval_ms: 5000,
            cpu_micros: 1,
            rss_bytes: None,
            heap_bytes: None,
            metrics: BTreeMap::new(),
        }
    }

    #[test]
    fn request_grammar() {
        assert_eq!(
            parse_request("since 1759000000000\n"),
            Some(1_759_000_000_000)
        );
        assert_eq!(parse_request("since   42  \r\n"), Some(42));
        assert_eq!(parse_request("\n"), Some(0));
        assert_eq!(parse_request(""), Some(0));
        assert_eq!(parse_request("since"), None);
        assert_eq!(parse_request("since42"), None);
        assert_eq!(parse_request("since -1"), None);
        assert_eq!(parse_request("since 1 2"), None);
        assert_eq!(parse_request("GET / HTTP/1.1"), None);
    }

    #[tokio::test]
    async fn an_oversized_request_is_cut_at_the_limit_and_refused() {
        let mut huge = b"since ".to_vec();
        huge.extend([b'9'; 4096]);
        let line = read_request(huge.as_slice()).await.unwrap();
        assert_eq!(line.len() as u64, MAX_REQUEST_BYTES);
        assert_eq!(parse_request(&line), None);
    }

    async fn ask(listener_addr: std::net::SocketAddr, request: &[u8]) -> String {
        let mut stream = TcpStream::connect(listener_addr).await.unwrap();
        stream.write_all(request).await.unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).await.unwrap();
        out
    }

    #[tokio::test]
    async fn serves_only_samples_newer_than_since_then_closes() {
        let ring = Arc::new(Ring::new(10));
        for ts in [5000, 10000, 15000] {
            ring.push(sample(ts));
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(accept_loop(listener, "game_server", ring));

        let out = ask(addr, b"since 5000\n").await;
        let ts: Vec<u64> = out
            .lines()
            .map(|l| {
                serde_json::from_str::<serde_json::Value>(l).unwrap()["ts"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        assert_eq!(ts, vec![10000, 15000]);

        assert_eq!(ask(addr, b"\n").await.lines().count(), 3);
        assert_eq!(ask(addr, b"since 15000\n").await, "");
    }

    #[tokio::test]
    async fn a_malformed_request_is_refused_not_fatal() {
        let ring = Arc::new(Ring::new(10));
        ring.push(sample(5000));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(accept_loop(listener, "login_server", ring));

        let bad = ask(addr, b"hello\n").await;
        assert!(bad.contains("\"error\""), "got {bad:?}");
        // The listener survived it.
        assert_eq!(ask(addr, b"since 0\n").await.lines().count(), 1);
    }
}
