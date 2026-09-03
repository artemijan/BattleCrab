//! The handshake and character-selection packets: protocol version, auth,
//! creation, and the hardware-id block that rides along with them.

use commons::network::PacketReader;

/// A client's hardware fingerprint (Java `ClientHardwareInfoHolder`, G31),
/// reported by `RequestHardWareInfo`. Keyed off the MAC address for HWID
/// punishments; the rest is shown by `//hwid`. Only the display-relevant fields
/// are kept.
#[derive(Debug, Clone, Default)]
pub struct HardwareInfo {
    pub mac_address: String,
    pub windows_platform_id: i32,
    pub windows_major_version: i32,
    pub windows_minor_version: i32,
    pub windows_build_number: i32,
    pub cpu_name: String,
    pub cpu_speed: i32,
    pub cpu_core_count: i32,
    pub vga_name: String,
    pub vga_driver_version: String,
}

impl HardwareInfo {
    /// Parse a `RequestHardWareInfo` body (the 19-field `cdddddddd…` layout).
    pub fn read(ex_body: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(ex_body);
        let mac_address = r.read_string()?;
        let windows_platform_id = r.read_i32()?;
        let windows_major_version = r.read_i32()?;
        let windows_minor_version = r.read_i32()?;
        let windows_build_number = r.read_i32()?;
        r.read_i32()?; // directxVersion
        r.read_i32()?; // directxRevision
        let cpu_name = r.read_string()?;
        let cpu_speed = r.read_i32()?;
        let cpu_core_count = r.read_i32()?;
        r.read_i32()?; // vgaCount
        r.read_i32()?; // vgaPcxSpeed
        r.read_i32()?; // physMemorySlot1
        r.read_i32()?; // physMemorySlot2
        r.read_i32()?; // physMemorySlot3
        r.read_i32()?; // videoMemory
        r.read_i32()?; // vgaVersion
        let vga_name = r.read_string()?;
        let vga_driver_version = r.read_string()?;
        Some(Self {
            mac_address,
            windows_platform_id,
            windows_major_version,
            windows_minor_version,
            windows_build_number,
            cpu_name,
            cpu_speed,
            cpu_core_count,
            vga_name,
            vga_driver_version,
        })
    }
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

/// The quest-zone id of `ExSendSelectedQuestZoneID` (`readInt`, after the
/// sub-opcode).
pub fn read_selected_quest_zone_id(ex_body: &[u8]) -> Option<i32> {
    PacketReader::new(ex_body).read_i32()
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
        Self {
            version: r.read_i32().unwrap_or(0),
        }
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
        Some(Self {
            login_name,
            play_key1,
            play_key2,
            login_key1,
            login_key2,
        })
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
        Some(Self {
            name,
            is_female,
            class_id,
            hair_style,
            hair_color,
            face,
        })
    }
}

/// `clientpackets/CharacterDelete` / `CharacterRestore` — both carry a char
/// slot. `RequestUnEquipItem`'s single `int` field (a body-part bitmask, not a
/// slot index — see `Inventory::unequip_slot`) has the same shape, so it
/// reuses this reader too.
pub fn read_char_slot(body_after_opcode: &[u8]) -> Option<i32> {
    PacketReader::new(body_after_opcode).read_i32()
}
