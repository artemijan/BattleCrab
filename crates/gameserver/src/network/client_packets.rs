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
    pub const ENTER_WORLD: u8 = 0x11;
    pub const CHARACTER_SELECT: u8 = 0x12;
    pub const NEW_CHARACTER: u8 = 0x13;
    pub const REQUEST_SKILL_COOL_TIME: u8 = 0xA6;
    pub const CHARACTER_RESTORE: u8 = 0x7B;
    pub const REQUEST_UN_EQUIP_ITEM: u8 = 0x16;
    pub const USE_ITEM: u8 = 0x19;
    /// Extended packets: opcode 0xD0 + a 2-byte little-endian sub-opcode.
    pub const EX_PACKET: u8 = 0xD0;
}

/// Extended (`0xD0`) client sub-opcodes.
pub mod ex_opcodes {
    pub const REQUEST_MANOR_LIST: u16 = 0x01;
    pub const REQUEST_KEY_MAPPING: u16 = 0x21;
    pub const REQUEST_CHARACTER_NAME_CREATABLE: u16 = 0xA9;
    pub const REQUEST_USER_BAN_INFO: u16 = 0x138;
    pub const REQUEST_GOTO_LOBBY: u16 = 0x33;
}

/// Split an extended-packet body (after the `0xD0` opcode) into its 2-byte LE
/// sub-opcode and the remaining payload.
pub fn read_ex_opcode(body_after_opcode: &[u8]) -> Option<(u16, &[u8])> {
    if body_after_opcode.len() < 2 {
        return None;
    }
    let sub = u16::from_le_bytes([body_after_opcode[0], body_after_opcode[1]]);
    Some((sub, &body_after_opcode[2..]))
}

/// The name field of `RequestCharacterNameCreatable` (after the sub-opcode).
pub fn read_name_creatable(ex_body: &[u8]) -> Option<String> {
    PacketReader::new(ex_body).read_string()
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

/// `clientpackets/CharacterDelete` / `CharacterRestore` — both carry a char
/// slot. `RequestUnEquipItem`'s single `int` field (a body-part bitmask, not a
/// slot index — see `Inventory::unequip_slot`) has the same shape, so it
/// reuses this reader too.
pub fn read_char_slot(body_after_opcode: &[u8]) -> Option<i32> {
    PacketReader::new(body_after_opcode).read_i32()
}

/// Port of `clientpackets/UseItem` (`cdc`): the target item's object id, plus
/// a ctrl-pressed flag (used for split-stack prompts — not needed while gear
/// is the only thing `UseItem` acts on).
pub struct UseItem {
    pub object_id: i32,
    pub ctrl_pressed: bool,
}

impl UseItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let object_id = r.read_i32()?;
        let ctrl_pressed = r.read_i32()? != 0;
        Some(Self { object_id, ctrl_pressed })
    }
}
