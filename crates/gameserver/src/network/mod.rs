//! Port of `gameserver/network` — the client-facing network layer.
//!
//! Transport mirrors the login server (tokio, 2-byte LE framing from
//! `commons::network`); the cipher and packet set are game-specific. The
//! connection task owns the cipher and the transport handshake
//! (`ProtocolVersion` → `KeyPacket`); decrypted gameplay packets are forwarded
//! to the game thread as [`NetEvent::Received`] (CONCURRENCY_MODEL §2.3).

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
/// encrypts and frames them. Unbounded for now; the drop policy
/// (`DropPackets`/`DropPacketThreshold`) is deferred (plan §4).
/// Outbound queue to one connection's task.
///
/// Carries [`bytes::Bytes`], not `Vec<u8>`: a broadcast hands the *same* packet
/// to every player in a 3×3 block, and cloning `Bytes` is a refcount bump
/// instead of a heap allocation plus a memcpy per recipient. The copy that does
/// have to happen — the cipher needs a mutable buffer — now happens in the
/// connection task, on a tokio worker, instead of on the single game thread
/// that everything else is waiting for.
pub type OutboundTx = tokio::sync::mpsc::UnboundedSender<bytes::Bytes>;
pub type OutboundRx = tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>;

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
    Received { client_id: u32, data: Vec<u8> },
    /// The connection closed (EOF, IO error, or server-side close).
    Disconnected { client_id: u32 },
}
