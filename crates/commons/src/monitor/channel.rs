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
//! that instant, oldest first; an empty line returns the whole ring.
//! `clients` returns one [`ClientRecord`] line per open connection, as of now
//! (§10). `kick <id> <connected_ms>` closes that connection and answers one
//! `{"kicked":true|false}` line — `false` when no such connection is open.
//! `logs streams` lists this server's log streams, one
//! [`StreamInfo`](crate::logsearch::StreamInfo) line each, and
//! `logs search <json>` runs one [`SearchRequest`] over this server's own files
//! and answers with one [`Outcome`](crate::logsearch::Outcome) line (§6).
//! Anything else gets one `{"error":…}` line and a close.
//!
//! Same security model as the login server's status channel: it binds to
//! loopback by default and that bind is the control. `InternalMonitorBindAddress`
//! may name a private-network address, for a dashboard on another machine,
//! and nothing wider (`crate::network::internal`): whoever can reach the port
//! sees traffic volumes, process memory, every connected player's address and
//! the server's logs, and can disconnect any player.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tracing::warn;

use super::Ring;
use super::clients::{ClientRecord, ProviderSlot};
use crate::logsearch::{self, SearchRequest, Source};

/// The longest request is `logs search` with its JSON: a pattern of at most
/// `logsearch::MAX_QUERY_LEN` bytes, which JSON escaping can grow several
/// times over, plus a cursor. Anything longer is not one.
const MAX_REQUEST_BYTES: u64 = 8 * 1024;

/// Log searches this server runs at once. Each holds a blocking thread and
/// reads this machine's disk; the dashboard caps its own too
/// (`LogSearchConcurrency`), so this only matters if something else asks.
const SEARCH_CONCURRENCY: usize = 2;

/// A client that connects and says nothing must not hold a task forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// How long a `clients` or `kick` request waits for the server to answer. The
/// game server answers from the game loop, which drains its events every tick;
/// a loop stuck for this long has bigger problems than the Audit page.
#[cfg(not(test))]
const CLIENTS_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(test)]
const CLIENTS_TIMEOUT: Duration = Duration::from_millis(50);

#[derive(Debug, PartialEq, Eq)]
enum Request {
    Since(u64),
    Clients,
    Kick { id: u64, connected_ms: u64 },
    LogStreams,
    LogSearch(Box<SearchRequest>),
}

/// What one server's channel answers from.
pub(crate) struct Channel {
    /// The server's logging name, stamped on every line served.
    pub service: &'static str,
    pub ring: Arc<Ring>,
    pub clients: &'static ProviderSlot,
    /// This server's own log files. `None` answers `logs` requests with an
    /// error line.
    pub logs: Option<Arc<Source>>,
    searches: Arc<Semaphore>,
}

impl Channel {
    pub fn new(
        service: &'static str,
        ring: Arc<Ring>,
        clients: &'static ProviderSlot,
        logs: Option<Source>,
    ) -> Arc<Self> {
        Arc::new(Self {
            service,
            ring,
            clients,
            logs: logs.map(Arc::new),
            searches: Arc::new(Semaphore::new(SEARCH_CONCURRENCY)),
        })
    }
}

/// Serve the channel until the process exits. Errors are logged and dropped:
/// a monitoring endpoint must never be able to take its server down with it.
pub(crate) async fn accept_loop(listener: TcpListener, channel: Arc<Channel>) {
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                warn!("monitor channel: accept failed: {e}");
                continue;
            }
        };
        let channel = channel.clone();
        tokio::spawn(async move {
            let (read, mut write) = stream.into_split();
            let response = match tokio::time::timeout(REQUEST_TIMEOUT, read_request(read)).await {
                Ok(Some(line)) => respond(&line, &channel).await,
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
async fn respond(line: &str, channel: &Channel) -> String {
    let (service, clients) = (channel.service, channel.clients);
    match parse_request(line) {
        Some(Request::Since(since)) => channel
            .ring
            .since(since)
            .iter()
            .map(|s| s.to_json_line(service))
            .collect(),
        Some(Request::Clients) => match clients.list() {
            None => error_line("this server does not list clients"),
            Some(reply) => match tokio::time::timeout(CLIENTS_TIMEOUT, reply).await {
                Ok(Ok(records)) => client_lines(service, records),
                Ok(Err(_)) => error_line("the server dropped the clients request"),
                Err(_) => error_line("timed out building the client list"),
            },
        },
        Some(Request::Kick { id, connected_ms }) => match clients.kick(id, connected_ms) {
            None => error_line("this server does not kick clients"),
            Some(reply) => match tokio::time::timeout(CLIENTS_TIMEOUT, reply).await {
                Ok(Ok(kicked)) => format!("{}\n", serde_json::json!({ "kicked": kicked })),
                Ok(Err(_)) => error_line("the server dropped the kick request"),
                Err(_) => error_line("timed out kicking the client"),
            },
        },
        Some(Request::LogStreams) => match &channel.logs {
            None => error_line("this server does not search its logs"),
            Some(source) => {
                let source = source.clone();
                match tokio::task::spawn_blocking(move || source.streams()).await {
                    Ok(streams) => streams
                        .iter()
                        .filter_map(|s| serde_json::to_string(s).ok())
                        .map(|line| line + "\n")
                        .collect(),
                    Err(_) => error_line("listing the log streams failed"),
                }
            }
        },
        Some(Request::LogSearch(request)) => match &channel.logs {
            None => error_line("this server does not search its logs"),
            Some(source) => search_logs(service, source.clone(), &request, &channel.searches).await,
        },
        None => error_line(
            "expected `since <epoch_ms>`, `clients`, `kick <id> <connected_ms>`, \
             `logs streams` or `logs search <json>`",
        ),
    }
}

/// One `logs search`: validated here too, since the dashboard is not the only
/// thing that could connect, then run on a blocking thread. The answer is
/// stamped with `service`, like every other line served, so an asker can tell
/// it reached the server it meant to.
async fn search_logs(
    service: &str,
    source: Arc<Source>,
    request: &SearchRequest,
    permits: &Arc<Semaphore>,
) -> String {
    let (query, bounds) = match request.validate() {
        Ok(v) => v,
        Err(e) => return error_line(&e),
    };
    // Refuse rather than queue, as the dashboard does: a queued search would
    // hold the asker's connection open past any useful wait.
    let Ok(permit) = permits.clone().try_acquire_owned() else {
        return error_line("too many log searches are running; try again shortly");
    };
    let outcome = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        logsearch::search(&source, &query, &bounds)
    })
    .await;
    match outcome.ok().and_then(|o| serde_json::to_value(o).ok()) {
        Some(serde_json::Value::Object(mut line)) => {
            line.insert("service".into(), service.into());
            serde_json::Value::Object(line).to_string() + "\n"
        }
        _ => error_line("the log search failed"),
    }
}

fn client_lines(service: &str, records: Vec<ClientRecord>) -> String {
    records
        .into_iter()
        .filter_map(|mut r| {
            r.service = service.to_string();
            serde_json::to_string(&r).ok()
        })
        .map(|line| line + "\n")
        .collect()
}

/// `since <ms>` → `Since(ms)`; an empty line → `Since(0)` (everything);
/// `clients` → `Clients`; `kick <id> <connected_ms>` → `Kick`;
/// `logs streams` → `LogStreams`; `logs search <json>` → `LogSearch`;
/// anything else → `None`.
fn parse_request(line: &str) -> Option<Request> {
    let line = line.trim();
    if line.is_empty() {
        return Some(Request::Since(0));
    }
    if line == "clients" {
        return Some(Request::Clients);
    }
    if line == "logs streams" {
        return Some(Request::LogStreams);
    }
    if let Some(json) = line.strip_prefix("logs search ") {
        return serde_json::from_str(json)
            .ok()
            .map(|r| Request::LogSearch(Box::new(r)));
    }
    if let Some(rest) = line.strip_prefix("kick ") {
        let mut parts = rest.split_whitespace();
        let id = parts.next()?.parse().ok()?;
        let connected_ms = parts.next()?.parse().ok()?;
        return parts
            .next()
            .is_none()
            .then_some(Request::Kick { id, connected_ms });
    }
    let rest = line.strip_prefix("since")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    rest.trim().parse().ok().map(Request::Since)
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
            Some(Request::Since(1_759_000_000_000))
        );
        assert_eq!(parse_request("since   42  \r\n"), Some(Request::Since(42)));
        assert_eq!(parse_request("\n"), Some(Request::Since(0)));
        assert_eq!(parse_request(""), Some(Request::Since(0)));
        assert_eq!(parse_request("clients\n"), Some(Request::Clients));
        assert_eq!(parse_request("clients 1"), None);
        assert_eq!(
            parse_request("kick 7 1759000000000\n"),
            Some(Request::Kick {
                id: 7,
                connected_ms: 1_759_000_000_000
            })
        );
        assert_eq!(parse_request("kick 7"), None);
        assert_eq!(parse_request("kick 7 1 2"), None);
        assert_eq!(parse_request("kick -7 1"), None);
        assert_eq!(parse_request("kick7 1"), None);
        assert_eq!(parse_request("since"), None);
        assert_eq!(parse_request("since42"), None);
        assert_eq!(parse_request("since -1"), None);
        assert_eq!(parse_request("since 1 2"), None);
        assert_eq!(parse_request("GET / HTTP/1.1"), None);
        assert_eq!(parse_request("logs streams\n"), Some(Request::LogStreams));
        assert_eq!(parse_request("logs"), None);
        assert_eq!(parse_request("logs search"), None);
        assert_eq!(parse_request("logs search {}"), None, "fields are required");
        let Some(Request::LogSearch(r)) = parse_request(
            "logs search {\"stream\":\"diagnostic\",\"from\":1,\"to\":2,\"limit\":5,\"maxBytes\":9,\"timeoutMs\":3}\n",
        ) else {
            panic!("a search request");
        };
        assert_eq!(
            (r.stream.as_str(), r.limit, r.q.as_str()),
            ("diagnostic", 5, "")
        );
    }

    #[tokio::test]
    async fn an_oversized_request_is_cut_at_the_limit_and_refused() {
        let mut huge = b"since ".to_vec();
        huge.extend([b'9'; 3 * MAX_REQUEST_BYTES as usize]);
        let line = read_request(huge.as_slice()).await.unwrap();
        assert_eq!(line.len() as u64, MAX_REQUEST_BYTES);
        assert_eq!(parse_request(&line), None);
    }

    fn no_provider() -> &'static ProviderSlot {
        Box::leak(Box::new(ProviderSlot::new()))
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
        tokio::spawn(accept_loop(
            listener,
            Channel::new("game_server", ring, no_provider(), None),
        ));

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
        tokio::spawn(accept_loop(
            listener,
            Channel::new("login_server", ring, no_provider(), None),
        ));

        let bad = ask(addr, b"hello\n").await;
        assert!(bad.contains("\"error\""), "got {bad:?}");
        // The listener survived it.
        assert_eq!(ask(addr, b"since 0\n").await.lines().count(), 1);
    }

    #[tokio::test]
    async fn clients_are_served_stamped_with_the_service() {
        use super::super::clients::{ClientsReply, Provider, Traffic, ready};
        let slot = no_provider();
        let provider = || -> Option<ClientsReply> {
            ready(vec![ClientRecord {
                service: "whatever the provider says".into(),
                id: 3,
                ip: "10.0.0.2".into(),
                port: 50000,
                connected_ms: 1,
                stage: "lobby".into(),
                account: Some("acc".into()),
                character: None,
                hwid: None,
                traffic: Traffic::default(),
                details: serde_json::Map::new(),
            }])
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(accept_loop(
            listener,
            Channel::new("game_server", Arc::new(Ring::new(1)), slot, None),
        ));

        // No provider yet: an error line, not a hang.
        assert!(ask(addr, b"clients\n").await.contains("\"error\""));

        assert!(slot.set_for_test(Provider {
            list: Box::new(provider),
            kick: Box::new(|id, connected_ms| ready(id == 3 && connected_ms == 1)),
        }));
        let out = ask(addr, b"clients\n").await;
        let record: ClientRecord = serde_json::from_str(out.trim()).unwrap();
        assert_eq!((record.service.as_str(), record.id), ("game_server", 3));

        assert_eq!(ask(addr, b"kick 3 1\n").await, "{\"kicked\":true}\n");
        // Same id, another connection: refused, not kicked.
        assert_eq!(ask(addr, b"kick 3 2\n").await, "{\"kicked\":false}\n");
    }

    #[tokio::test]
    async fn a_provider_that_never_answers_times_out_with_an_error() {
        let slot = no_provider();
        // The sender is leaked, so the reply never resolves and never errors.
        assert!(slot.set_for_test(super::super::clients::Provider {
            list: Box::new(|| {
                let (tx, rx) = tokio::sync::oneshot::channel();
                std::mem::forget(tx);
                Some(rx)
            }),
            kick: Box::new(|_, _| None),
        }));
        let channel = Channel::new("game_server", Arc::new(Ring::new(1)), slot, None);
        let out = respond("clients", &channel).await;
        assert!(out.contains("timed out"), "got {out:?}");
    }

    #[tokio::test]
    async fn logs_are_listed_and_searched_on_the_server_that_wrote_them() {
        let dir = std::env::temp_dir().join(format!(
            "commons-channel-logs-{}-{}",
            std::process::id(),
            super::super::epoch_ms()
        ));
        std::fs::create_dir_all(dir.join("log/audit")).unwrap();
        std::fs::write(
            dir.join("log/game_server.2026-08-14.json"),
            "{\"timestamp\":\"2026-08-14T01:00:00Z\",\"level\":\"INFO\",\"message\":\"one\"}\n\
             {\"timestamp\":\"2026-08-14T02:00:00Z\",\"level\":\"WARN\",\"message\":\"two\"}\n",
        )
        .unwrap();
        let source = Source {
            service: "game_server".into(),
            log_dir: dir.join("log"),
            audit_dir: dir.join("log/audit"),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(accept_loop(
            listener,
            Channel::new(
                "game_server",
                Arc::new(Ring::new(1)),
                no_provider(),
                Some(source),
            ),
        ));

        let streams = ask(addr, b"logs streams\n").await;
        let info: logsearch::StreamInfo = serde_json::from_str(streams.trim()).unwrap();
        assert_eq!(
            (info.service.as_str(), info.stream.as_str()),
            ("game_server", "diagnostic")
        );

        // 2026-08-14 UTC, as epoch ms.
        let request = |extra: &str| {
            format!(
                "logs search {{\"stream\":\"diagnostic\",\"from\":1786665600000,\"to\":1786752000000,\
                 \"limit\":10,\"maxBytes\":1048576,\"timeoutMs\":2000{extra}}}\n"
            )
        };
        let out = ask(addr, request("").as_bytes()).await;
        let outcome: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        let messages: Vec<&str> = outcome["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["line"]["message"].as_str().unwrap())
            .collect();
        assert_eq!(messages, vec!["two", "one"]);
        assert_eq!(outcome["service"], "game_server");

        let warn = ask(addr, request(",\"level\":\"warn\"").as_bytes()).await;
        assert!(
            warn.contains("\"two\"") && !warn.contains("\"one\""),
            "got {warn:?}"
        );

        let bad = ask(addr, request(",\"q\":\"(\",\"regex\":true").as_bytes()).await;
        assert!(bad.contains("invalid regex"), "got {bad:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_server_without_log_access_says_so() {
        let channel = Channel::new("login_server", Arc::new(Ring::new(1)), no_provider(), None);
        assert!(
            respond("logs streams", &channel)
                .await
                .contains("does not search")
        );
    }
}
