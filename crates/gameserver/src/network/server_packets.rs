//! Outbound (server → client) packets. Ported 1:1 from
//! `gameserver/network/serverpackets`. Each builder returns the serialized body
//! (opcode + payload, unencrypted); the connection task encrypts and frames it.
//!
//! G1 covers only `KeyPacket`; the rest arrive with their milestones.

use commons::network::PacketWriter;

/// `ServerPackets` opcodes (the single-byte `_id1`).
pub mod opcodes {
    pub const CHARACTER_SELECTION_INFO: u8 = 0x09;
    pub const LOGIN_FAIL: u8 = 0x0A;
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

/// Port of `serverpackets/LoginFail`. `LoginFail.LOGIN_SUCCESS` = `(-1, 0)`.
pub fn login_fail(success: i32, reason: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::LOGIN_FAIL);
    w.write_i32(success);
    w.write_i32(reason);
    w.into_bytes()
}

/// `LoginFail.LOGIN_SUCCESS`.
pub fn login_success() -> Vec<u8> {
    login_fail(-1, 0)
}

/// Port of `serverpackets/CharSelectionInfo` for an **empty** character list
/// (G2). The two per-character loops emit nothing when the count is 0, so the
/// packet is just the header. Full character rows arrive in G3.
pub fn char_selection_info_empty(max_characters: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHARACTER_SELECTION_INFO);
    w.write_i32(0); // created character count
    w.write_i32(max_characters); // max characters
    w.write_u8(0); // (count == max) → can't create new char; 0 for empty list
    w.write_u8(1); // 1 = can play free until level 85
    w.write_i32(2); // client region flag
    w.write_u8(0); // Balthus Knights / premium suggestion
    w.into_bytes()
}
