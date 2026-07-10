//! Port of `gameserver/network/ConnectionState.java`.

/// The per-connection protocol state. Packet dispatch is keyed by
/// `(ConnectionState, opcode)`, exactly as `ClientPackets` restricts each packet
/// to the states it is valid in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Closing,
    Authenticated,
    Entering,
    InGame,
}
