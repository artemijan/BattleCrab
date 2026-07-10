//! Inbound (client → server) packets. Ported 1:1 from
//! `gameserver/network/clientpackets`. G1 covers only the transport handshake
//! packet `ProtocolVersion`; gameplay packets are parsed/dispatched on the game
//! thread from G2 on.

use commons::network::PacketReader;

/// `ClientPackets` opcodes (single-byte `_id`).
pub mod opcodes {
    pub const PROTOCOL_VERSION: u8 = 0x0E;
    pub const AUTH_LOGIN: u8 = 0x2B;
    pub const CHARACTER_CREATE: u8 = 0x0C;
    pub const CHARACTER_DELETE: u8 = 0x0D;
    pub const CHARACTER_SELECT: u8 = 0x12;
    pub const NEW_CHARACTER: u8 = 0x13;
    pub const CHARACTER_RESTORE: u8 = 0x7B;
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

/// Port of `clientpackets/AuthLogin`. The account name and the two session-key
/// halves the client echoes from the login handoff. Field order matches
/// `readImpl`: name, playKey2, playKey1, loginKey1, loginKey2.
pub struct AuthLogin {
    pub login_name: String,
    pub play_key1: i32,
    pub play_key2: i32,
    pub login_key1: i32,
    pub login_key2: i32,
}

impl AuthLogin {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let login_name = r.read_string()?.to_lowercase();
        let play_key2 = r.read_i32()?;
        let play_key1 = r.read_i32()?;
        let login_key1 = r.read_i32()?;
        let login_key2 = r.read_i32()?;
        Some(Self { login_name, play_key1, play_key2, login_key1, login_key2 })
    }
}

/// Port of `clientpackets/CharacterCreate` (`cSdddddddddddd`).
pub struct CharacterCreate {
    pub name: String,
    pub is_female: bool,
    pub class_id: i32,
    pub hair_style: i32,
    pub hair_color: i32,
    pub face: i32,
}

impl CharacterCreate {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let name = r.read_string()?;
        r.read_i32()?; // race (ignored; derived from class)
        let is_female = r.read_i32()? != 0;
        let class_id = r.read_i32()?;
        for _ in 0..6 {
            r.read_i32()?; // int/str/con/men/dex/wit (ignored)
        }
        let hair_style = r.read_i32()? & 0xff;
        let hair_color = r.read_i32()? & 0xff;
        let face = r.read_i32()? & 0xff;
        Some(Self { name, is_female, class_id, hair_style, hair_color, face })
    }
}

/// `clientpackets/CharacterDelete` / `CharacterRestore` — both carry a char slot.
pub fn read_char_slot(body_after_opcode: &[u8]) -> Option<i32> {
    PacketReader::new(body_after_opcode).read_i32()
}
