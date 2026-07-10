//! LS→GS packets (`loginserverpackets/*`). The GS-link payload encryption lives
//! in `commons::crypt::gs_link` (shared with the game server); re-exported here
//! so existing call sites keep working.

use commons::network::PacketWriter;

pub use commons::crypt::gs_link::{gs_decrypt, gs_encrypt, GS_STATIC_BLOWFISH_KEY};

/// `InitLS`: protocol revision + RSA modulus (Java `BigInteger.toByteArray()`).
pub fn init_ls(protocol_rev: i32, modulus_java_bytes: &[u8]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x00);
    w.write_i32(protocol_rev);
    w.write_i32(modulus_java_bytes.len() as i32);
    w.write_bytes(modulus_java_bytes);
    w.into_bytes()
}

/// `LoginServerFail`.
pub fn login_server_fail(reason: u8) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x01);
    w.write_u8(reason);
    w.into_bytes()
}

/// `AuthResponse`: assigned id + server name.
pub fn auth_response(server_id: i32, server_name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x02);
    w.write_u8(server_id as u8);
    w.write_string(server_name);
    w.into_bytes()
}

/// `PlayerAuthResponse`.
pub fn player_auth_response(account: &str, ok: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x03);
    w.write_string(account);
    w.write_u8(if ok { 1 } else { 0 });
    w.into_bytes()
}

/// `KickPlayer`.
pub fn kick_player(account: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x04);
    w.write_string(account);
    w.into_bytes()
}

/// `RequestCharacters`.
pub fn request_characters(account: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x05);
    w.write_string(account);
    w.into_bytes()
}

/// `ChangePasswordResponse`.
pub fn change_password_response(character_name: &str, message: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x06);
    w.write_string(character_name);
    w.write_string(message);
    w.into_bytes()
}
