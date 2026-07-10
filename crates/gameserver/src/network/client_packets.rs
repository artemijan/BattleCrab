//! Inbound (client → server) packets. Ported 1:1 from
//! `gameserver/network/clientpackets`. G1 covers only the transport handshake
//! packet `ProtocolVersion`; gameplay packets are parsed/dispatched on the game
//! thread from G2 on.

use commons::network::PacketReader;

/// `ClientPackets` opcodes (single-byte `_id`).
pub mod opcodes {
    pub const PROTOCOL_VERSION: u8 = 0x0E;
    pub const AUTH_LOGIN: u8 = 0x2B;
}

/// Port of `clientpackets/ProtocolVersion`. Never encrypted (first packet).
/// A missing/short version reads as 0 (Java swallows the exception → `_version = 0`).
pub struct ProtocolVersion {
    pub version: i32,
}

impl ProtocolVersion {
    /// `readImpl`: the opcode byte has already been consumed by the dispatcher.
    pub fn read(body_after_opcode: &[u8]) -> Self {
        let mut r = PacketReader::new(body_after_opcode);
        Self { version: r.read_i32().unwrap_or(0) }
    }
}
