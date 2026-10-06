//! Port of `gameserver/network` — the client-facing network layer.
//!
//! Transport mirrors the login server (tokio, 2-byte LE framing from
//! `commons::network`); the cipher and packet set are game-specific. The
//! connection task owns the cipher and the transport handshake
//! (`ProtocolVersion` → `KeyPacket`); decrypted gameplay packets are forwarded
//! to the game thread as [`NetEvent::Received`] (THREADING_MODEL §1).

pub mod cipher;
pub mod client_packets;
pub mod connection;
pub mod connection_state;
pub mod enter_world;
pub mod game_client;
pub mod masks;
pub mod server_packets;
pub mod trade;
pub mod user_info;

pub use connection_state::ConnectionState;

/// Outbound queue endpoint held by the **game thread** to push serialized packet
/// bodies (opcode + payload, unencrypted) to a connection. The connection task
/// encrypts and frames them.
///
/// Carries [`bytes::Bytes`], not `Vec<u8>`: a broadcast hands the *same* packet
/// to every player in a 3×3 block, and cloning `Bytes` is a refcount bump
/// instead of a heap allocation plus a memcpy per recipient. The copy that does
/// have to happen — the cipher needs a mutable buffer — now happens in the
/// connection task, on a tokio worker, instead of on the single game thread
/// that everything else is waiting for.
///
/// The channel itself is unbounded (the game thread must never block), but the
/// queue is *pressure-managed*: `depth` counts packets queued and not yet
/// written, and past `drop_threshold` any [`can_be_dropped`] packet is
/// discarded instead of queued — the port of Java `Client.packetCanBeDropped`
/// (`Network.ini` `DropPackets`/`DropPacketThreshold`). A slow or absent
/// reader therefore costs bounded memory for the spammy broadcast traffic,
/// while state-bearing packets still always queue.
#[derive(Clone)]
pub struct OutboundTx {
    tx: tokio::sync::mpsc::UnboundedSender<bytes::Bytes>,
    /// Packets sent and not yet dequeued by the connection task (which owns
    /// the decrement side) — Java `Client._estimateQueueSize`.
    depth: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// This connection's own traffic and start time, for the dashboard's
    /// client list (`docs/MONITORING.md` §10). Here because this handle is
    /// the one piece of the connection every session state already carries.
    stats: std::sync::Arc<commons::monitor::clients::ConnectionStats>,
    drop_packets: bool,
    drop_threshold: usize,
}

/// Dropped-under-pressure counter (Java drops silently; a drop that leaves no
/// trace looks like a network bug when a client misses a movement packet).
fn packets_dropped() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("packets_dropped"))
}

/// Wire-level traffic counters — `docs/MONITORING.md` §2 (P1). The raw in/out
/// this monitors, independent of what the game thread makes of it.
/// `packets_in` counts every frame that arrives off the socket (including ones
/// the rate limiter then rejects); `game_loop::net::packets_handled` counts
/// what actually reached a handler, so the gap between the two is itself a
/// signal.
fn packets_in() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("packets_in"))
}

fn bytes_in() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("bytes_in"))
}

/// Counted per coalesced *batch write*, not per queued packet: the connection
/// task already accumulates a tick's worth of packets into one `write_all`
/// (see `connection.rs`'s outbound arm), so this is one atomic add per socket
/// write rather than one per broadcast recipient.
fn packets_out() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("packets_out"))
}

fn bytes_out() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("bytes_out"))
}

/// Accept-time series. `connections_accepted` is the lifetime total;
/// `connections_open` is the live count, held per connection task through
/// [`commons::metrics::Gauge::hold`].
fn connections_accepted() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("connections_accepted"))
}

fn connections_open() -> &'static commons::metrics::Gauge {
    static G: std::sync::OnceLock<commons::metrics::Gauge> = std::sync::OnceLock::new();
    G.get_or_init(|| commons::metrics::gauge("connections_open"))
}

pub(crate) use traffic::{note_connection_opened, note_inbound_frame, note_outbound_batch};

/// Small wrapper module so `connection.rs` records traffic through named
/// verbs instead of reaching into these statics directly — the counter names
/// above stay this module's business.
mod traffic {
    use commons::monitor::clients::ConnectionStats;
    use commons::network::HEADER_SIZE;

    /// One frame received off the socket, counted before decryption or rate
    /// limiting — this is "what arrived on the wire", not "what a handler saw".
    pub(crate) fn note_inbound_frame(conn: &ConnectionStats, payload_len: usize) {
        let wire = (payload_len + HEADER_SIZE) as u64;
        super::packets_in().incr();
        super::bytes_in().add(wire);
        conn.note_in(wire);
    }

    /// One socket write carrying `packet_count` frames in `wire_bytes` bytes
    /// — a coalesced batch, or `1` for a frame written on its own.
    pub(crate) fn note_outbound_batch(
        conn: &ConnectionStats,
        packet_count: u64,
        wire_bytes: usize,
    ) {
        super::packets_out().add(packet_count);
        super::bytes_out().add(wire_bytes as u64);
        conn.note_out(packet_count, wire_bytes as u64);
    }

    /// The returned guard is the connection's slot in `connections_open`;
    /// the connection task owns it, so the count drops when the task ends
    /// however it ends, including by panic.
    pub(crate) fn note_connection_opened() -> commons::metrics::GaugeHold {
        super::connections_accepted().incr();
        super::connections_open().hold()
    }
}

/// Registered at boot so every series reads `0` from the first snapshot
/// instead of being *absent* until first traffic.
pub fn register_metrics() {
    packets_dropped();
    packets_in();
    bytes_in();
    packets_out();
    bytes_out();
    connections_accepted();
    connections_open().set(0);
}

impl OutboundTx {
    pub fn new(
        tx: tokio::sync::mpsc::UnboundedSender<bytes::Bytes>,
        drop_packets: bool,
        drop_threshold: usize,
    ) -> Self {
        Self {
            tx,
            depth: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            stats: std::sync::Arc::default(),
            drop_packets,
            drop_threshold,
        }
    }

    /// The counter the connection task decrements as it dequeues.
    pub fn depth_handle(&self) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
        self.depth.clone()
    }

    /// The connection's traffic counters, which the connection task bumps.
    pub fn stats_handle(&self) -> std::sync::Arc<commons::monitor::clients::ConnectionStats> {
        self.stats.clone()
    }

    pub fn stats(&self) -> &commons::monitor::clients::ConnectionStats {
        &self.stats
    }

    /// Queue one packet body, or drop it under pressure. A send to a closed
    /// channel (connection task gone, session not yet reaped) is silently
    /// discarded, as before.
    pub fn send(&self, body: bytes::Bytes) {
        use std::sync::atomic::Ordering;
        if self.drop_packets
            && self.depth.load(Ordering::Relaxed) > self.drop_threshold
            && can_be_dropped(body.first().copied().unwrap_or(0))
        {
            packets_dropped().incr();
            return;
        }
        if self.tx.send(body).is_ok() {
            self.depth.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub type OutboundRx = tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>;

/// Test convenience: wrap a bare channel sender with the drop policy off, so
/// the hundreds of `Session::new(id, out_tx, …)` fixtures keep reading as
/// plain channels. Deliberately `cfg(test)`: production must decide its drop
/// policy explicitly via [`OutboundTx::new`].
#[cfg(test)]
impl From<tokio::sync::mpsc::UnboundedSender<bytes::Bytes>> for OutboundTx {
    fn from(tx: tokio::sync::mpsc::UnboundedSender<bytes::Bytes>) -> Self {
        Self::new(tx, false, 0)
    }
}

/// Port of the `WritablePacket.canBeDropped` overrides: the packet types Java
/// marks disposable under outbound pressure. All are high-frequency broadcast
/// state the next update of which supersedes the lost one (a missed
/// `StatusUpdate` is corrected by the next; a missed `MoveToLocation` by the
/// next move or the arrival `StopMove`). Everything else — inventory, dialog,
/// combat results — must never be dropped: the client has no way to recover
/// the state. Keyed by opcode because the game thread queues serialized
/// bodies; like `flood.rs`, a missing entry here is a missing table row, not a
/// silently unguarded call site.
pub fn can_be_dropped(opcode: u8) -> bool {
    use server_packets::opcodes as op;
    matches!(
        opcode,
        op::STATUS_UPDATE
            | op::AUTO_ATTACK_START
            | op::AUTO_ATTACK_STOP
            | op::SOCIAL_ACTION
            | op::MOVE_TO_PAWN
            | op::MOVE_TO_LOCATION
    )
}

/// Sender facade for the network's share of the unified service→game channel
/// ([`crate::events::GameEvent`]). `std::sync::mpsc` because the game thread
/// is a plain (non-async) thread that sleeps on the receiver between tick
/// boundaries; sends from the async connection tasks are non-blocking and
/// wake it.
#[derive(Clone)]
pub struct NetEventTx(pub crate::events::GameEventTx);

impl NetEventTx {
    /// An `Err` means the game thread is gone — callers treat it as shutdown.
    pub fn send(&self, event: NetEvent) -> Result<(), std::sync::mpsc::SendError<()>> {
        self.0
            .send(crate::events::GameEvent::Net(event))
            .map_err(|_| std::sync::mpsc::SendError(()))
    }
}

/// Events the network layer reports to the game thread.
pub enum NetEvent {
    /// A client finished connecting; carries the handle the game thread uses to
    /// send packets back and identify the client.
    Connected {
        client_id: u32,
        out: OutboundTx,
        addr: std::net::SocketAddr,
    },
    /// A decrypted gameplay packet body (opcode byte + payload) past the
    /// transport handshake. Opcode dispatch happens on the game thread (G2+).
    Received {
        client_id: u32,
        data: Vec<u8>,
        /// The connection's in-flight slot; dropping the handled event
        /// releases it, letting the read task pull the next frame
        /// (`connection::MAX_PACKETS_IN_FLIGHT`).
        permit: tokio::sync::OwnedSemaphorePermit,
    },
    /// The client announced its protocol version during the handshake.
    ///
    /// Purely informational on the game thread — the handshake itself already
    /// accepted or rejected the version connection-side. It is forwarded so
    /// `//charinfo`'s `%protocol%` has something to read, which is the one
    /// place Java surfaces `GameClient.getProtocolVersion()` to an operator.
    ProtocolVersion { client_id: u32, version: i32 },
    /// The connection closed (EOF, IO error, or server-side close).
    Disconnected { client_id: u32 },
}

#[cfg(test)]
mod outbound_tests {
    use super::*;

    fn body(opcode: u8) -> bytes::Bytes {
        bytes::Bytes::from(vec![opcode, 0, 0, 0, 0])
    }

    /// `CharInfo` — a state-bearing packet the client cannot recover if lost.
    const NOT_DROPPABLE: u8 = server_packets::opcodes::CHAR_INFO;

    /// The six `canBeDropped` overrides, and nothing else nearby.
    #[test]
    fn the_droppable_set_matches_javas_overrides() {
        use server_packets::opcodes as op;
        for op in [
            op::STATUS_UPDATE,
            op::AUTO_ATTACK_START,
            op::AUTO_ATTACK_STOP,
            op::SOCIAL_ACTION,
            op::MOVE_TO_PAWN,
            op::MOVE_TO_LOCATION,
        ] {
            assert!(can_be_dropped(op), "0x{op:02x} overrides canBeDropped");
        }
        assert!(!can_be_dropped(NOT_DROPPABLE));
        // `ActionFailed`'s override is commented out in Java — not droppable.
        assert!(!can_be_dropped(server_packets::opcodes::ACTION_FAIL));
    }

    /// Java `packetCanBeDropped`: strictly *over* the threshold, droppable
    /// packets are discarded; state-bearing packets always queue.
    #[test]
    fn past_the_threshold_only_droppable_packets_are_discarded() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let out = OutboundTx::new(tx, true, 2);
        use server_packets::opcodes as op;
        // Depths 0, 1, 2 — none strictly over 2, all queue.
        out.send(body(op::MOVE_TO_LOCATION));
        out.send(body(op::MOVE_TO_LOCATION));
        out.send(body(op::MOVE_TO_LOCATION));
        // Depth 3 > 2: the droppable is discarded, the state-bearing queues.
        out.send(body(op::STATUS_UPDATE));
        out.send(body(NOT_DROPPABLE));
        let mut queued = Vec::new();
        while let Ok(b) = rx.try_recv() {
            queued.push(b[0]);
        }
        assert_eq!(
            queued,
            vec![
                op::MOVE_TO_LOCATION,
                op::MOVE_TO_LOCATION,
                op::MOVE_TO_LOCATION,
                NOT_DROPPABLE
            ],
            "one StatusUpdate dropped, everything else in order"
        );
    }

    /// The connection task's decrement reopens the queue for droppables —
    /// pressure is a state, not a latch.
    #[test]
    fn draining_the_queue_lets_droppable_packets_flow_again() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let out = OutboundTx::new(tx, true, 0);
        use server_packets::opcodes as op;
        use std::sync::atomic::Ordering;
        out.send(body(op::MOVE_TO_LOCATION)); // depth 0 → queues
        out.send(body(op::MOVE_TO_LOCATION)); // depth 1 > 0 → dropped
        // The connection task dequeues (as its write loop does)…
        assert!(rx.try_recv().is_ok());
        out.depth_handle().fetch_sub(1, Ordering::Relaxed);
        // …and the next droppable queues again.
        out.send(body(op::MOVE_TO_LOCATION));
        assert!(rx.try_recv().is_ok());
    }

    /// `DropPackets = False` (Java's code default): never drop anything.
    #[test]
    fn with_the_policy_off_nothing_is_ever_dropped() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let out = OutboundTx::new(tx, false, 0);
        for _ in 0..10 {
            out.send(body(server_packets::opcodes::STATUS_UPDATE));
        }
        let mut n = 0;
        while rx.try_recv().is_ok() {
            n += 1;
        }
        assert_eq!(n, 10);
    }
}
