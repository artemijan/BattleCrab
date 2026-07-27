//! GS↔LS link packets from the **game server side**: builders for
//! `loginserverpackets/game/*` (GS→LS) and parsers for
//! `loginserverpackets/login/*` (LS→GS). Payload crypto is
//! `commons::crypt::gs_link`.

use commons::crypt::RsaPublicModulus;
use commons::network::{PacketReader, PacketWriter};

use crate::session::SessionKey;

// ---- GS → LS (builders) ----

/// `BlowFishKey` (0x00): the session Blowfish key, RSA-`nopadding`-encrypted
/// with the LS modulus from `InitLS`.
pub fn blowfish_key(blowfish_key: &[u8], modulus: &RsaPublicModulus) -> Vec<u8> {
    let encrypted = modulus.encrypt_raw(blowfish_key);
    let mut w = PacketWriter::new();
    w.write_u8(0x00);
    w.write_i32(encrypted.len() as i32);
    w.write_bytes(&encrypted);
    w.into_bytes()
}

/// `AuthRequest` (0x01).
#[allow(clippy::too_many_arguments)]
pub fn auth_request(
    id: i32,
    accept_alternate: bool,
    reserve_host: bool,
    port: u16,
    max_players: i32,
    hexid: &[u8],
    hosts: &[(String, String)], // (subnet, host)
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x01);
    w.write_u8(id as u8);
    w.write_u8(accept_alternate as u8);
    w.write_u8(reserve_host as u8);
    w.write_i16(port as i16);
    w.write_i32(max_players);
    w.write_i32(hexid.len() as i32);
    w.write_bytes(hexid);
    w.write_i32(hosts.len() as i32);
    for (subnet, host) in hosts {
        w.write_string(subnet);
        w.write_string(host);
    }
    w.into_bytes()
}

/// `PlayerInGame` (0x02).
pub fn player_in_game(accounts: &[String]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x02);
    w.write_i16(accounts.len() as i16);
    for a in accounts {
        w.write_string(a);
    }
    w.into_bytes()
}

/// `PlayerLogout` (0x03).
pub fn player_logout(account: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x03);
    w.write_string(account);
    w.into_bytes()
}

/// `ChangeAccessLevel` (0x04, G31): set an account's access level on the login
/// server — a negative level bans it (`LoginController.setAccountAccessLevel`),
/// so the login server refuses that account's next login.
pub fn change_access_level(account: &str, level: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x04);
    w.write_i32(level);
    w.write_string(account);
    w.into_bytes()
}

/// `PlayerAuthRequest` (0x05). Note the field order: playOk first, then loginOk.
pub fn player_auth_request(account: &str, key: &SessionKey) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x05);
    w.write_string(account);
    w.write_i32(key.play_ok1);
    w.write_i32(key.play_ok2);
    w.write_i32(key.login_ok1);
    w.write_i32(key.login_ok2);
    w.into_bytes()
}

/// `ServerStatus` (0x06): a list of (attribute id, value) pairs.
pub fn server_status(attributes: &[(i32, i32)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x06);
    w.write_i32(attributes.len() as i32);
    for (id, value) in attributes {
        w.write_i32(*id);
        w.write_i32(*value);
    }
    w.into_bytes()
}

/// `ReplyCharacters` (0x08): character count on this server + deletion times.
pub fn reply_characters(account: &str, chars: u8, del_times: &[i64]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x08);
    w.write_string(account);
    w.write_u8(chars);
    w.write_u8(del_times.len() as u8);
    for t in del_times {
        w.write_i64(*t);
    }
    w.into_bytes()
}

// ---- LS → GS (parsers) ----

/// `InitLS` (0x00): protocol revision + RSA modulus bytes.
pub struct InitLs {
    pub revision: i32,
    pub rsa_key: Vec<u8>,
}

impl InitLs {
    pub fn read(after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(after_opcode);
        let revision = r.read_i32()?;
        let size = r.read_i32()? as usize;
        let rsa_key = r.read_bytes(size)?.to_vec();
        Some(Self { revision, rsa_key })
    }
}

/// `AuthResponse` (0x02): assigned server id + name.
pub struct AuthResponse {
    pub server_id: i32,
    pub server_name: String,
}

impl AuthResponse {
    pub fn read(after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(after_opcode);
        let server_id = r.read_u8()? as i32;
        let server_name = r.read_string()?;
        Some(Self {
            server_id,
            server_name,
        })
    }
}

/// `PlayerAuthResponse` (0x03).
pub struct PlayerAuthResponse {
    pub account: String,
    pub authed: bool,
}

impl PlayerAuthResponse {
    pub fn read(after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(after_opcode);
        let account = r.read_string()?;
        let authed = r.read_u8()? != 0;
        Some(Self { account, authed })
    }
}

/// A single account-name field (`KickPlayer` 0x04, `RequestCharacters` 0x05).
pub fn read_account(after_opcode: &[u8]) -> Option<String> {
    PacketReader::new(after_opcode).read_string()
}

/// `LoginServerFail` (0x01): reason code.
pub fn read_login_server_fail(after_opcode: &[u8]) -> Option<u8> {
    PacketReader::new(after_opcode).read_u8()
}

/// `LoginServerFail` reason strings (index by reason code).
pub const FAIL_REASONS: [&str; 8] = [
    "None",
    "Reason: ip banned",
    "Reason: ip reserved",
    "Reason: wrong hexid",
    "Reason: id reserved",
    "Reason: no free ID",
    "Not authed",
    "Reason: already logged in",
];
