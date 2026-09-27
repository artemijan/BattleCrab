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

/// One frame read off the socket, counted before decrypt — "what arrived on
/// the wire", regardless of what `dispatch` later makes of it.
pub fn note_inbound_frame(payload_len: usize) {
    packets_in().incr();
    bytes_in().add((payload_len + HEADER_SIZE) as u64);
}

/// One frame handed to `write_frame`, counted after encryption (the encrypted
/// body is what actually goes on the wire).
pub fn note_outbound_frame(encrypted_len: usize) {
    packets_out().incr();
    bytes_out().add((encrypted_len + HEADER_SIZE) as u64);
}

pub fn note_connection_opened() {
    connections_accepted().incr();
    connections_open().incr();
}

pub fn note_connection_closed() {
    connections_open().decr();
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
        note_inbound_frame(10);
        assert_eq!(packets_in().get(), before_in + 1);
        assert_eq!(bytes_in().get(), before_bytes_in + 10 + HEADER_SIZE as u64);

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
    fn connections_open_tracks_the_live_count() {
        let start = connections_open().get();
        note_connection_opened();
        note_connection_opened();
        assert_eq!(connections_open().get(), start + 2);
        note_connection_closed();
        assert_eq!(connections_open().get(), start + 1);
    }
}
