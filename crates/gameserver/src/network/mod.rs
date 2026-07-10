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
pub mod server_packets;
pub mod user_info;

pub use connection_state::ConnectionState;

/// Outbound queue endpoint held by the **game thread** to push serialized packet
/// bodies (opcode + payload, unencrypted) to a connection. The connection task
/// encrypts and frames them. Unbounded for now; the drop policy
/// (`DropPackets`/`DropPacketThreshold`) is deferred (plan §4).
pub type OutboundTx = tokio::sync::mpsc::UnboundedSender<Vec<u8>>;
pub type OutboundRx = tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>;

/// Sender side of the network→game channel. `std::sync::mpsc` because the game
/// thread is a plain (non-async) thread draining it with `try_recv` each tick;
/// sends from the async connection tasks are non-blocking.
pub type NetEventTx = std::sync::mpsc::Sender<NetEvent>;
pub type NetEventRx = std::sync::mpsc::Receiver<NetEvent>;

/// Events the network layer reports to the game thread.
pub enum NetEvent {
    /// A client finished connecting; carries the handle the game thread uses to
    /// send packets back and identify the client.
    Connected { client_id: u32, out: OutboundTx, addr: std::net::SocketAddr },
    /// A decrypted gameplay packet body (opcode byte + payload) past the
    /// transport handshake. Opcode dispatch happens on the game thread (G2+).
    Received { client_id: u32, data: Vec<u8> },
    /// The connection closed (EOF, IO error, or server-side close).
    Disconnected { client_id: u32 },
}
