//! Ports of `loginserver/network/serverpackets/*` needed for the handshake
//! (the rest arrive with M3/M4). Each returns the packet body (opcode +
//! payload) — encryption and framing happen in the connection task.

use commons::network::PacketWriter;

use crate::enums::{AccountKickedReason, LoginFailReason, PlayFailReason};
use crate::session::SessionKey;

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

/// `LoginOk.java` — the loginOk half of the session key.
pub fn login_ok(key: &SessionKey) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x03);
    w.write_i32(key.login_ok1);
    w.write_i32(key.login_ok2);
    w.write_i32(0x00);
    w.write_i32(0x00);
    w.write_i32(0x000003ea);
    w.write_i32(0x00);
    w.write_i32(0x00);
    w.write_i32(0x00);
    w.write_bytes(&[0u8; 16]);
    w.into_bytes()
}

/// `AccountKicked.java`.
pub fn account_kicked(reason: AccountKickedReason) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x02);
    w.write_i32(reason as i32);
    w.into_bytes()
}

/// `ServerList.java`. `chars_on_servers` is written only when present
/// (populated by ReplyCharacters — M5).
pub fn server_list(
    servers: &[crate::controller::ServerListEntry],
    last_server: i32,
    chars_on_servers: Option<&std::collections::HashMap<i32, i32>>,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x04);
    w.write_u8(servers.len() as u8);
    w.write_u8(last_server as u8);
    for s in servers {
        w.write_u8(s.server_id);
        w.write_bytes(&s.ip);
        w.write_i32(s.port);
        w.write_u8(s.age_limit);
        w.write_u8(if s.pvp { 0x01 } else { 0x00 });
        w.write_i16(s.current_players as i16);
        w.write_i16(s.max_players as i16);
        w.write_u8(if s.up { 0x01 } else { 0x00 });
        w.write_i32(s.server_type);
        w.write_u8(if s.brackets { 0x01 } else { 0x00 });
    }
    w.write_i16(0xA4u16 as i16); // unknown
    if let Some(chars) = chars_on_servers {
        for s in servers {
            w.write_u8(s.server_id);
            w.write_u8(*chars.get(&(s.server_id as i32)).unwrap_or(&0) as u8);
        }
    }
    w.into_bytes()
}

/// `PlayOk.java` — the playOk half of the session key.
pub fn play_ok(key: &SessionKey) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x07);
    w.write_i32(key.play_ok1);
    w.write_i32(key.play_ok2);
    w.into_bytes()
}

/// `PlayFail.java`.
pub fn play_fail(reason: PlayFailReason) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x06);
    w.write_u8(reason as u8);
    w.into_bytes()
}
