//! Ports of `loginserver/network/serverpackets/*` needed for the handshake
//! (the rest arrive with M3/M4). Each returns the packet body (opcode +
//! payload) — encryption and framing happen in the connection task.

use commons::network::PacketWriter;

use crate::enums::LoginFailReason;

pub const PROTOCOL_REVISION: i32 = 0x0000c621;

/// `Init.java` — session id, scrambled RSA modulus, GG constants, Blowfish key.
pub fn init(session_id: i32, scrambled_modulus: &[u8; 0x80], blowfish_key: &[u8; 16]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x00);
    w.write_i32(session_id);
    w.write_i32(PROTOCOL_REVISION);
    w.write_bytes(scrambled_modulus);
    // GG related.
    w.write_i32(0x29DD954Eu32 as i32);
    w.write_i32(0x77C39CFCu32 as i32);
    w.write_i32(0x97ADB620u32 as i32);
    w.write_i32(0x07BDE0F7u32 as i32);
    w.write_bytes(blowfish_key);
    w.write_u8(0); // Null termination.
    w.into_bytes()
}

/// `GGAuth.java` — GameGuard authentication reply.
pub fn gg_auth(response: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x0b);
    w.write_i32(response);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0);
    w.into_bytes()
}

/// `LoginFail.java`.
pub fn login_fail(reason: LoginFailReason) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x01);
    w.write_u8(reason as u8);
    w.into_bytes()
}
