//! Public server status. No auth.

use std::time::Duration;

use axum::extract::State;
use axum::{Json, Router};
use serde::Serialize;
use tokio::io::AsyncReadExt;

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/server/status", axum::routing::get(status))
        .route("/health", axum::routing::get(health))
}

/// How long to wait on the status channel. It is a loopback read of one short
/// line, so anything approaching this means the login server is wedged — which
/// is a kind of "down" the caller should hear about promptly rather than wait
/// out. This is also what stops a stuck login server from tying up dashboard
/// workers on a public, unauthenticated endpoint.
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// A cap on the response body. The channel is trusted — loopback, our own
/// process — but trusted is not unbounded: without a limit, anything wedged or
/// misconfigured on that port could stream forever into our allocator.
const MAX_PROBE_BYTES: u64 = 64 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    /// Whether players can actually get in: the login server answered **and**
    /// at least one game server is linked to it.
    pub online: bool,
    /// Accounts in game, summed across linked game servers.
    ///
    /// This is the login server's live view, not `characters.online` — those
    /// rows survive a crash, so the count this endpoint used to return kept
    /// ticking for a server that had died.
    pub players_online: i64,
    /// Per-server detail, so a multi-server setup is not flattened into one
    /// misleading boolean. Empty when the login server is unreachable.
    pub servers: Vec<GameServerStatus>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameServerStatus {
    pub id: i64,
    pub name: String,
    pub up: bool,
    pub players: i64,
    pub max_players: i64,
}

async fn status(State(app): State<AppState>) -> ApiResult<Json<ServerStatus>> {
    let servers = probe(&app.config.status_channel_address).await;
    // `online` requires a *game* server, not merely a reachable login server:
    // with login up and nothing linked, the client shows a server list nobody
    // can enter, and reporting "online" there would be the same lie in a new
    // place.
    let online = servers.iter().any(|s| s.up);
    let players_online = servers.iter().filter(|s| s.up).map(|s| s.players).sum();
    Ok(Json(ServerStatus {
        online,
        players_online,
        servers,
    }))
}

/// Read the login server's one-line status snapshot.
///
/// Every failure — refused, timed out, malformed — collapses to the same
/// answer: no servers, therefore offline. A status probe must never be able to
/// turn a dead game server into a dashboard 500.
async fn probe(address: &str) -> Vec<GameServerStatus> {
    if address.trim().is_empty() {
        return Vec::new();
    }
    let read = async {
        let stream = tokio::net::TcpStream::connect(address).await.ok()?;
        let mut buf = Vec::new();
        stream
            .take(MAX_PROBE_BYTES)
            .read_to_end(&mut buf)
            .await
            .ok()?;
        serde_json::from_slice::<serde_json::Value>(&buf).ok()
    };
    let Ok(Some(body)) = tokio::time::timeout(PROBE_TIMEOUT, read).await else {
        return Vec::new();
    };
    parse_servers(&body)
}

/// Pull the `servers` array out of a snapshot line. Split out so the wire
/// contract is testable without a live login server.
pub(crate) fn parse_servers(body: &serde_json::Value) -> Vec<GameServerStatus> {
    let Some(rows) = body.get("servers").and_then(|s| s.as_array()) else {
        return Vec::new();
    };
    rows.iter()
        .map(|r| GameServerStatus {
            id: r.get("id").and_then(serde_json::Value::as_i64).unwrap_or(0),
            name: r
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            up: r
                .get("up")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            players: r
                .get("players")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0),
            max_players: r
                .get("maxPlayers")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0),
        })
        .collect()
}

/// Liveness for the container orchestrator — does not touch the DB.
async fn health() -> &'static str {
    "ok"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire contract between the login server's status channel and this
    /// route. If either side renames a field, this fails rather than silently
    /// reporting a dead server as up (every getter here defaults, so a rename
    /// degrades to `up: false` / `players: 0` instead of erroring).
    #[test]
    fn a_snapshot_line_parses_into_per_server_status() {
        let line = r#"{"login":"up","servers":[
            {"id":1,"name":"Bartz","up":true,"players":42,"maxPlayers":2000},
            {"id":2,"name":"Sieghardt","up":false,"players":0,"maxPlayers":2000}
        ]}"#;
        let body: serde_json::Value = serde_json::from_str(line).unwrap();
        let servers = parse_servers(&body);
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].name, "Bartz");
        assert!(servers[0].up);
        assert_eq!(servers[0].players, 42);
        assert!(!servers[1].up);

        // The aggregate the endpoint reports: a down server contributes no
        // players, so a crashed server cannot inflate the count.
        let online = servers.iter().any(|s| s.up);
        let total: i64 = servers.iter().filter(|s| s.up).map(|s| s.players).sum();
        assert!(online);
        assert_eq!(total, 42);
    }

    /// Login up but nothing linked is **offline**: the client would show a
    /// server list with nothing enterable behind it.
    #[test]
    fn a_login_server_with_no_game_servers_is_offline() {
        let body: serde_json::Value =
            serde_json::from_str(r#"{"login":"up","servers":[]}"#).unwrap();
        let servers = parse_servers(&body);
        assert!(servers.is_empty());
        assert!(!servers.iter().any(|s| s.up));
    }

    /// A malformed or unexpected body degrades to "offline", never to a panic.
    #[test]
    fn a_malformed_snapshot_reports_offline() {
        for line in [r#"{}"#, r#"{"servers":"nope"}"#, r#"{"servers":[{}]}"#] {
            let body: serde_json::Value = serde_json::from_str(line).unwrap();
            let servers = parse_servers(&body);
            assert!(!servers.iter().any(|s| s.up), "{line} must not read as up");
        }
    }
}
