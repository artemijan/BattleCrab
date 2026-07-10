//! Outbound (server → client) packets. Ported 1:1 from
//! `gameserver/network/serverpackets`. Each builder returns the serialized body
//! (opcode + payload, unencrypted); the connection task encrypts and frames it.
//!
//! G1 covers only `KeyPacket`; the rest arrive with their milestones.

use commons::network::PacketWriter;

/// `ServerPackets` opcodes (the single-byte `_id1`).
pub mod opcodes {
    pub const VERSION_CHECK: u8 = 0x2E;
}

/// Port of `serverpackets/KeyPacket` — the reply to `ProtocolVersion`. Hands the
/// client the first 8 bytes of the cipher key and the crypt/server flags.
///
/// * `key8` — first 8 bytes of the 16-byte cipher key (the static tail is
///   hard-coded in the client).
/// * `result` — 1 = protocol ok, 0 = wrong protocol.
pub fn key_packet(key8: &[u8; 8], result: u8, packet_encryption: bool, server_id: i32, is_classic: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::VERSION_CHECK);
    w.write_u8(result); // 0 - wrong protocol, 1 - protocol ok
    for b in key8 {
        w.write_u8(*b);
    }
    w.write_i32(packet_encryption as i32); // use blowfish encryption
    w.write_i32(server_id);
    w.write_u8(1);
    w.write_i32(0); // obfuscation key
    w.write_u8(is_classic as u8);
    w.into_bytes()
}
