//! Wire-traffic counters for the login server — `docs/MONITORING.md` §2 (P1).
//!
//! The login server had no `commons::metrics` series at all before this —
//! `docs/LOGGING.md`'s gap list named it explicitly. These mirror the
//! gameserver's `network::{packets_in, bytes_in, packets_out, bytes_out,
//! connections_accepted, connections_open}` (`gameserver/src/network/mod.rs`)
//! so the two services graph the same shape side by side.
//!
//! The login server does not coalesce outbound writes the way the game
//! server's connection task does — one `send()` call is one frame — so
//! `packets_out` is simpler here: one `note_outbound_frame` per send.

use commons::monitor::clients::ConnectionStats;
use commons::network::HEADER_SIZE;

fn packets_in() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("packets_in"))
}

fn bytes_in() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("bytes_in"))
}

fn packets_out() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("packets_out"))
}

fn bytes_out() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("bytes_out"))
}

fn connections_accepted() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("connections_accepted"))
}

fn connections_open() -> &'static commons::metrics::Gauge {
    static G: std::sync::OnceLock<commons::metrics::Gauge> = std::sync::OnceLock::new();
    G.get_or_init(|| commons::metrics::gauge("connections_open"))
}

/// Where each open connection is in the login flow. A connection holds exactly
/// one of these at a time ([`LoginStage`]), so together they sum to
/// `connections_open`: `sessions_handshaking` is everyone before a successful
/// `RequestAuthLogin` (key exchange, GameGuard, the login form),
/// `sessions_logged_in` everyone past it, looking at the server list. After
/// `PlayOk` the client leaves for the game server and the connection closes.
fn sessions_handshaking() -> &'static commons::metrics::Gauge {
    static G: std::sync::OnceLock<commons::metrics::Gauge> = std::sync::OnceLock::new();
    G.get_or_init(|| commons::metrics::gauge("sessions_handshaking"))
}

fn sessions_logged_in() -> &'static commons::metrics::Gauge {
    static G: std::sync::OnceLock<commons::metrics::Gauge> = std::sync::OnceLock::new();
    G.get_or_init(|| commons::metrics::gauge("sessions_logged_in"))
}

/// A connection's slot in the stage gauges above. Owned by the session, so
/// the slot is given back when the connection task ends, however it ends.
pub struct LoginStage {
    _slot: commons::metrics::GaugeHold,
}

impl LoginStage {
    pub fn handshaking() -> Self {
        Self {
            _slot: sessions_handshaking().hold(),
        }
    }

    /// The handshake slot is released when the caller overwrites it with this.
    pub fn logged_in() -> Self {
        Self {
            _slot: sessions_logged_in().hold(),
        }
    }
}

/// One frame read off the socket, counted before decrypt — "what arrived on
/// the wire", regardless of what `dispatch` later makes of it.
pub fn note_inbound_frame(conn: &ConnectionStats, payload_len: usize) {
    let wire = (payload_len + HEADER_SIZE) as u64;
    packets_in().incr();
    bytes_in().add(wire);
    conn.note_in(wire);
}

/// One frame handed to `write_frame`, counted after encryption (the encrypted
/// body is what actually goes on the wire).
pub fn note_outbound_frame(encrypted_len: usize) {
    let wire = (encrypted_len + HEADER_SIZE) as u64;
    packets_out().incr();
    bytes_out().add(wire);
    // The connection's own count: `send` has no session to hand, so the
    // connection task's registration is found through a task-local.
    crate::clients::note_outbound(wire);
}

/// The returned guard is the connection's slot in `connections_open`; the
/// connection task owns it, so the count drops when the task ends however it
/// ends, including by panic.
pub fn note_connection_opened() -> commons::metrics::GaugeHold {
    connections_accepted().incr();
    connections_open().hold()
}

/// Registered at boot so every series reads `0` from the first snapshot
/// instead of being *absent* until first traffic (same reasoning as
/// `game_loop::net::register_metrics`).
pub fn register_metrics() {
    packets_in();
    bytes_in();
    packets_out();
    bytes_out();
    connections_accepted();
    connections_open().set(0);
    sessions_handshaking().set(0);
    sessions_logged_in().set(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inbound_and_outbound_frames_add_the_header_back() {
        // These are process-global counters (commons::metrics is a shared
        // registry), so assert on deltas rather than absolute values — this
        // test runs alongside others touching the same series.
        let before_in = packets_in().get();
        let before_bytes_in = bytes_in().get();
        let conn = ConnectionStats::new();
        note_inbound_frame(&conn, 10);
        assert_eq!(packets_in().get(), before_in + 1);
        assert_eq!(bytes_in().get(), before_bytes_in + 10 + HEADER_SIZE as u64);
        assert_eq!(conn.traffic().bytes_in, 10 + HEADER_SIZE as u64);

        let before_out = packets_out().get();
        let before_bytes_out = bytes_out().get();
        note_outbound_frame(20);
        assert_eq!(packets_out().get(), before_out + 1);
        assert_eq!(
            bytes_out().get(),
            before_bytes_out + 20 + HEADER_SIZE as u64
        );
    }

    #[test]
    fn a_connection_holds_one_stage_at_a_time() {
        let (hs, li) = (sessions_handshaking().get(), sessions_logged_in().get());
        let mut session = (LoginStage::handshaking(),);
        assert_eq!(sessions_handshaking().get(), hs + 1);
        // How `client_connection` moves a session on: overwrite in place.
        session.0 = LoginStage::logged_in();
        assert_eq!(sessions_handshaking().get(), hs);
        assert_eq!(sessions_logged_in().get(), li + 1);
        drop(session);
        assert_eq!(sessions_logged_in().get(), li);
    }

    #[test]
    fn connections_open_tracks_the_live_count() {
        let start = connections_open().get();
        let a = note_connection_opened();
        let b = note_connection_opened();
        assert_eq!(connections_open().get(), start + 2);
        drop(a);
        assert_eq!(connections_open().get(), start + 1);
        drop(b);
    }
}
