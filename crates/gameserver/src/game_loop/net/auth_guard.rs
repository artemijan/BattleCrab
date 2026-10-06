//! The authentication deadline (`Security.ini`, `docs/SECURITY.md`): a game
//! connection has `UnauthenticatedTimeout` to authenticate, or it is dropped.
//!
//! A real client sends `AuthLogin` right after the handshake, and the login
//! server confirms it in a round trip, so a few seconds is generous. What
//! misses it is a port scanner, a stalled bot, or something holding sockets
//! open. Such a drop is a **strike** against the address, and
//! `UnauthenticatedStrikesBeforeBan` strikes within
//! `UnauthenticatedStrikeWindow` put the address on the IP ban list
//! (`ip_bans`), which both servers refuse on connect.
//!
//! A client that did send `AuthLogin` and is still waiting on the login server
//! is dropped without a strike: a slow or unreachable login server is not the
//! client's fault, and striking for it would ban real players during an
//! outage.

use std::net::IpAddr;

use tracing::{info, warn};

use crate::db::DbCommand;
use crate::scheduler::{ScheduledTask, ms_to_ticks};
use crate::session::ClientSession;
use crate::world::World;

/// Above this many tracked addresses, a strike also sweeps out addresses whose
/// strikes have all aged out, so a scan from many addresses can't grow the map
/// without bound.
const SWEEP_ABOVE: usize = 1024;

/// A new connection: start its deadline.
pub(crate) fn arm(world: &mut World, client_id: u32) {
    let timeout = world.auth_guard.cfg.unauthenticated_timeout_ms;
    if timeout == 0 {
        return;
    }
    let at = world.tick + ms_to_ticks(timeout);
    world
        .scheduler
        .schedule(at, ScheduledTask::AuthDeadline { client_id });
}

/// The deadline passed. Client ids are never reused within a process, so a
/// client that is gone or past `Connecting` simply has nothing to answer for.
pub(crate) fn on_deadline(world: &mut World, client_id: u32) {
    let Some(ClientSession::Connecting(session)) = world.clients.get(&client_id) else {
        return;
    };
    let ip = session.addr.ip();
    let sent_credentials = world
        .login
        .waiting
        .values()
        .any(|w| w.client_id == client_id);
    crate::game_loop::helpers::kick_client(world, client_id);
    if sent_credentials {
        info!(
            "GameServer: client {client_id} ({ip}) still unconfirmed by the login server — dropped."
        );
        return;
    }
    info!("GameServer: client {client_id} ({ip}) did not authenticate in time — dropped.");
    strike(world, ip, commons::util::now_millis());
}

/// One strike against `ip` at `now_ms`; bans it once the strikes in the
/// window reach the limit.
pub(crate) fn strike(world: &mut World, ip: IpAddr, now_ms: i64) {
    let cfg = &world.auth_guard.cfg;
    let limit = cfg.unauthenticated_strikes_before_ban as usize;
    if limit == 0 {
        return;
    }
    let since = now_ms - cfg.unauthenticated_strike_window_ms as i64;
    let ban_minutes = cfg.unauthenticated_ban_minutes;
    let window_ms = cfg.unauthenticated_strike_window_ms;

    let strikes = &mut world.auth_guard.strikes;
    if strikes.len() > SWEEP_ABOVE {
        strikes.retain(|_, at| at.last().is_some_and(|&t| t > since));
    }
    let at = strikes.entry(ip).or_default();
    at.retain(|&t| t > since);
    at.push(now_ms);
    if at.len() < limit {
        return;
    }
    let count = at.len();
    strikes.remove(&ip);

    let expires_at = if ban_minutes > 0 {
        now_ms + ban_minutes * 60_000
    } else {
        0
    };
    let reason = format!(
        "automatic: {count} connections without authenticating within {} minutes",
        window_ms / 60_000
    );
    warn!("GameServer: banning {ip} — {reason}");
    commons::audit::record(
        commons::audit::Category::GmAudit,
        serde_json::json!({
            "event": "ip_ban",
            "source": "game_server",
            "ip": ip.to_string(),
            "expires_at": (expires_at > 0).then_some(expires_at),
            "reason": reason,
        }),
    );
    let _ = world.db.send(DbCommand::BanIp {
        ip: ip.to_string(),
        expires_at,
        reason,
    });
}
