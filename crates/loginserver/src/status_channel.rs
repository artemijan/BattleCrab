//! The internal status channel — a loopback TCP port that answers "is the
//! server up, and who is on it?" for the operator dashboard.
//!
//! # Why this exists
//!
//! The dashboard used to report `online: true` unconditionally, which only ever
//! meant "the database answered". A crashed game server leaves its
//! `characters.online` flags set, so the player count kept ticking over for a
//! server that was gone. Liveness has to come from the process, not from rows
//! it wrote before it died.
//!
//! # Why only the login server carries it
//!
//! The login server already knows, for every registered game server, whether it
//! is authenticated and holding a live link, and how many accounts are in game —
//! it has to, because the client's server-select screen displays exactly that.
//! When a game server dies its link drops and `up` goes false on its own. So one
//! channel here answers for the whole cluster, and it keeps working when there
//! is more than one game server, which a per-server endpoint would not.
//!
//! The tradeoff: if the *login* server is down the dashboard learns nothing and
//! reports offline. That is the honest answer — with login down nobody can get
//! in regardless — but a game-up/login-down window reads pessimistically.
//!
//! # Why there is no authentication
//!
//! The listener binds to **loopback by default**, which the kernel enforces:
//! nothing off-host can reach it whatever the firewall says. That is the actual
//! security control. `InternalStatusBindAddress` can widen it, and the config
//! comment says plainly that doing so publishes account counts and server
//! topology to anyone who can reach the port — at which point the operator owns
//! putting a control in front of it.
//!
//! # The protocol
//!
//! There isn't one, deliberately. Connect, read one line of JSON, done — no
//! request, no framing to agree on, no keep-alive:
//!
//! ```text
//! $ nc 127.0.0.1 7778
//! {"login":"up","servers":[{"id":1,"name":"Bartz","up":true,"players":42,"maxPlayers":2000}]}
//! ```
//!
//! Reaching it at all proves the login server is up, which is why `login` is a
//! constant: a dead process refuses the connection instead.

use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::context::LoginContext;

/// Serve the status channel until the process exits.
///
/// One connection is one line and one close. Errors are logged and dropped —
/// a monitoring endpoint must never be able to take the login server with it.
pub async fn accept_loop(listener: TcpListener, ctx: Arc<LoginContext>) {
    loop {
        let (mut stream, peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                warn!("status channel: accept failed: {e}");
                continue;
            }
        };
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let line = snapshot_line(&ctx).await;
            if let Err(e) = stream.write_all(line.as_bytes()).await {
                // A dashboard that hangs up mid-poll is routine, not an error
                // worth alarming on.
                warn!("status channel: write to {peer} failed: {e}");
            }
            let _ = stream.shutdown().await;
        });
    }
}

/// The single JSON line a connection receives, newline-terminated.
pub(crate) async fn snapshot_line(ctx: &LoginContext) -> String {
    let servers: Vec<serde_json::Value> = ctx
        .controller
        .status_snapshot()
        .await
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "name": s.name,
                "up": s.up,
                "players": s.players,
                "maxPlayers": s.max_players,
            })
        })
        .collect();
    // `login` is unconditional on purpose: if this line is being written, the
    // login server is up. The dashboard's "offline" signal is the connection
    // being refused, not a field in here.
    let body = serde_json::json!({ "login": "up", "servers": servers });
    format!("{body}\n")
}

/// Bind the status channel, if the operator left it enabled.
///
/// A bind failure is logged and swallowed rather than aborting startup: the
/// game must not fail to boot because a monitoring port is already in use.
pub async fn spawn(ctx: Arc<LoginContext>) {
    let cfg = &ctx.config;
    if cfg.internal_status_port == 0 {
        info!("Status channel: disabled (InternalStatusPort = 0).");
        return;
    }
    let bind = format!(
        "{}:{}",
        cfg.internal_status_bind_address, cfg.internal_status_port
    );
    match TcpListener::bind(&bind).await {
        Ok(listener) => {
            info!("Status channel: listening on {bind}.");
            tokio::spawn(accept_loop(listener, ctx));
        }
        Err(e) => warn!("Status channel: could not bind {bind}: {e} — status unavailable."),
    }
}
