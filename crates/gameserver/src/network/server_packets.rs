//! Outbound (server → client) packets. Ported 1:1 from
//! `gameserver/network/serverpackets`. Each builder returns the serialized body
//! (opcode + payload, unencrypted); the connection task encrypts and frames it.
//!
//! G1 covers only `KeyPacket`; the rest arrive with their milestones.

use commons::network::PacketWriter;

use crate::model::inventory::PaperdollSlot;
use crate::model::Player;

/// `ServerPackets` opcodes (the single-byte `_id1`).
pub mod opcodes {
    pub const CHARACTER_SELECTION_INFO: u8 = 0x09;
    pub const LOGIN_FAIL: u8 = 0x0A;
    pub const CHAR_SELECTED: u8 = 0x0B;
    pub const NEW_CHARACTER_SUCCESS: u8 = 0x0D;
    pub const CHAR_CREATE_SUCCESS: u8 = 0x0F;
    pub const CHAR_CREATE_FAIL: u8 = 0x10;
    pub const CHAR_DELETE_SUCCESS: u8 = 0x1D;
    pub const CHAR_DELETE_FAIL: u8 = 0x1E;
    pub const VERSION_CHECK: u8 = 0x2E;
    pub const STATUS_UPDATE: u8 = 0x18;
    pub const MAGIC_SKILL_USE: u8 = 0x48;
    pub const MAGIC_SKILL_LAUNCHED: u8 = 0x54;
    pub const SETUP_GAUGE: u8 = 0x6B;
    pub const ACQUIRE_SKILL_DONE: u8 = 0x94;

    /// Extended packets: opcode 0xFE + a 2-byte little-endian sub-opcode.
    pub const EX: u8 = 0xFE;
    pub const EX_IS_CHAR_NAME_CREATABLE: i16 = 0x10B;
    pub const EX_SEND_MANOR_LIST: i16 = 0x22;
    pub const EX_UI_SETTING: i16 = 0x71;
}

/// Port of `serverpackets/ExSendManorList` — the castles that have a manor.
/// TODO(G12): the real castle list from `CastleManager` (empty for now).
pub fn ex_send_manor_list() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SEND_MANOR_LIST);
    w.write_i32(0); // castle count
    w.into_bytes()
}

/// Port of `serverpackets/settings/ExUISetting` — the player's stored UI key
/// mapping. TODO(G-later): load the stored mapping; null → length 0 for now.
pub fn ex_ui_setting() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_UI_SETTING);
    w.write_i32(0); // no stored key-mapping
    w.into_bytes()
}

/// Port of `serverpackets/ExIsCharNameCreatable`. `allowed` = -1 when the name
/// may be used; 1..5 is a `RequestCharacterNameCreatable` failure reason.
pub fn ex_is_char_name_creatable(allowed: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_IS_CHAR_NAME_CREATABLE);
    w.write_i32(allowed);
    w.into_bytes()
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

/// Java `ServerPacket.PAPERDOLL_ORDER` — the 33-int equipment write order the
/// client expects (the `InventorySlot` wire order mapped to paperdoll slots;
/// `RHand` repeats where the LRHAND display component sits).
pub const PAPERDOLL_ORDER: [PaperdollSlot; 33] = [
    PaperdollSlot::Under,
    PaperdollSlot::REar,
    PaperdollSlot::LEar,
    PaperdollSlot::Neck,
    PaperdollSlot::RFinger,
    PaperdollSlot::LFinger,
    PaperdollSlot::Head,
    PaperdollSlot::RHand,
    PaperdollSlot::LHand,
    PaperdollSlot::Gloves,
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Feet,
    PaperdollSlot::Cloak,
    PaperdollSlot::RHand,
    PaperdollSlot::Hair,
    PaperdollSlot::Hair2,
    PaperdollSlot::RBracelet,
    PaperdollSlot::LBracelet,
    PaperdollSlot::Deco1,
    PaperdollSlot::Deco2,
    PaperdollSlot::Deco3,
    PaperdollSlot::Deco4,
    PaperdollSlot::Deco5,
    PaperdollSlot::Deco6,
    PaperdollSlot::Belt,
    PaperdollSlot::Brooch,
    PaperdollSlot::BroochJewel1,
    PaperdollSlot::BroochJewel2,
    PaperdollSlot::BroochJewel3,
    PaperdollSlot::BroochJewel4,
    PaperdollSlot::BroochJewel5,
    PaperdollSlot::BroochJewel6,
];

/// `CharSelectionInfo.PAPERDOLL_ORDER_VISUAL_ID` — this packet overrides the
/// `ServerPacket` default with its own 9-slot visual-id order.
const CHAR_SELECT_PAPERDOLL_VISUAL_ORDER: [PaperdollSlot; 9] = [
    PaperdollSlot::RHand,
    PaperdollSlot::LHand,
    PaperdollSlot::Gloves,
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Feet,
    PaperdollSlot::RHand,
    PaperdollSlot::Hair,
    PaperdollSlot::Hair2,
];

/// The five armor slots whose enchant effect the lobby shows, in write order.
const CHAR_SELECT_ENCHANT_ORDER: [PaperdollSlot; 5] = [
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Head,
    PaperdollSlot::Gloves,
    PaperdollSlot::Feet,
];

/// Port of `serverpackets/CharSelectionInfo`. Writes the real character rows
/// and paperdoll; augmentation/visual id are zero (later milestones).
pub fn char_selection_info(
    login_name: &str,
    session_id: i32,
    chars: &[crate::character::CharData],
    active_id: i32,
    max_characters: i32,
    exp: &crate::data::ExperienceData,
) -> Vec<u8> {
    let now = commons::util::now_millis();
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHARACTER_SELECTION_INFO);
    let size = chars.len() as i32;
    w.write_i32(size); // created character count
    w.write_i32(max_characters);
    w.write_u8((size == max_characters) as u8); // 1 = can't create new char
    w.write_u8(1); // 1 = can play free until level 85
    w.write_i32(2); // client region flag
    w.write_u8(0); // Balthus Knights / premium suggestion

    // If no active id was given, the most-recently-accessed character is active.
    let active_id = if active_id == -1 {
        chars
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| c.last_access)
            .filter(|_| !chars.is_empty())
            .map(|(i, _)| i as i32)
            .unwrap_or(-1)
    } else {
        active_id
    };

    for (i, c) in chars.iter().enumerate() {
        w.write_string(&c.name);
        w.write_i32(c.object_id);
        w.write_string(login_name);
        w.write_i32(session_id);
        w.write_i32(0); // clan id
        w.write_i32(0); // builder level
        w.write_i32(c.sex);
        w.write_i32(c.race);
        w.write_i32(c.base_class_id);
        w.write_i32(1); // game server name
        w.write_i32(c.x);
        w.write_i32(c.y);
        w.write_i32(c.z);
        w.write_f64(c.cur_hp);
        w.write_f64(c.cur_mp);
        w.write_i64(c.sp);
        w.write_i64(c.exp);
        w.write_f64(exp_percent(exp, c.exp, c.level));
        w.write_i32(c.level);
        w.write_i32(c.reputation);
        w.write_i32(c.pk_kills);
        w.write_i32(c.pvp_kills);
        for _ in 0..9 {
            w.write_i32(0); // 7 reserved + 2 Ertheia
        }
        // Per-character paperdoll (Java `CharSelectInfoPackage`), read from the
        // `items` rows loaded alongside the character.
        let inv = crate::model::inventory::Inventory::from_rows(&c.items);
        for slot in PAPERDOLL_ORDER {
            w.write_i32(inv.paperdoll_item_id(slot));
        }
        for slot in CHAR_SELECT_PAPERDOLL_VISUAL_ORDER {
            w.write_i32(inv.paperdoll_visual_id(slot)); // always 0 (appearance: later milestone)
        }
        for slot in CHAR_SELECT_ENCHANT_ORDER {
            w.write_i16(inv.paperdoll_enchant_level(slot) as i16);
        }
        w.write_i32(c.hair_style);
        w.write_i32(c.hair_color);
        w.write_i32(c.face);
        w.write_f64(c.max_hp as f64);
        w.write_f64(c.max_mp as f64);
        w.write_i32(if c.delete_time > 0 { ((c.delete_time - now) / 1000) as i32 } else { 0 });
        w.write_i32(c.class_id);
        w.write_i32((i as i32 == active_id) as i32);
        w.write_u8(inv.paperdoll_enchant_level(PaperdollSlot::RHand).min(127) as u8); // rhand weapon enchant (capped 127)
        w.write_i32(0); // augmentation option 1
        w.write_i32(0); // augmentation option 2
        w.write_i32(0); // transform
        w.write_i32(0); // pet npc id
        w.write_i32(0); // pet level
        w.write_i32(0); // pet food
        w.write_i32(0); // pet food level
        w.write_f64(0.0); // pet hp
        w.write_f64(0.0); // pet mp
        w.write_i32(c.vitality_points);
        w.write_i32(100); // vitality percent (RATE_VITALITY_EXP_MULTIPLIER * 100)
        w.write_i32(0); // remaining vitality item uses
        w.write_i32((c.access_level != -100) as i32); // char active
        w.write_u8(c.noble as u8);
        w.write_u8(0); // hero glow
        w.write_u8(1); // hair accessory enabled
    }
    w.into_bytes()
}

fn exp_percent(exp: &crate::data::ExperienceData, current_exp: i64, level: i32) -> f64 {
    let base = exp.exp_for_level(level);
    let next = exp.exp_for_level(level + 1);
    let denom = next - base;
    if denom <= 0 {
        0.0
    } else {
        (current_exp - base) as f64 / denom as f64
    }
}

/// `serverpackets/NewCharacterSuccess` — the base-stat table for the creation
/// screen (one entry per offered template).
pub fn new_character_success(templates: &[(i32, crate::enums::Race, &crate::data::player_template::PlayerTemplate)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NEW_CHARACTER_SUCCESS);
    w.write_i32(templates.len() as i32);
    for (class_id, race, t) in templates {
        w.write_i32(race.ordinal());
        w.write_i32(*class_id);
        w.write_i32(99);
        w.write_i32(t.base_str);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_dex);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_con);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_int);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_wit);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_men);
        w.write_i32(1);
    }
    w.into_bytes()
}

/// `serverpackets/CharCreateOk`.
pub fn char_create_ok() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_CREATE_SUCCESS);
    w.write_i32(1);
    w.into_bytes()
}

/// `serverpackets/CharCreateFail` (`CharCreateFail.REASON_*`).
pub fn char_create_fail(reason: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_CREATE_FAIL);
    w.write_i32(reason);
    w.into_bytes()
}

/// `serverpackets/CharDeleteSuccess`.
pub fn char_delete_success() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_DELETE_SUCCESS);
    w.into_bytes()
}

/// `serverpackets/CharDeleteFail` (`CharacterDeleteFailType`).
pub fn char_delete_fail(reason: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_DELETE_FAIL);
    w.write_i32(reason);
    w.into_bytes()
}

/// Port of `serverpackets/CharSelected` — the reply to `CharacterSelect` that
/// starts the enter-world loading screen. `game_time` is minutes of the in-game
/// day (TODO(G4): real GameTimeTaskManager clock; 0 = midnight for now).
pub fn char_selected(p: &crate::model::Player, session_id: i32, game_time: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_SELECTED);
    w.write_string(&p.name);
    w.write_i32(p.object_id);
    w.write_string(&p.title);
    w.write_i32(session_id);
    w.write_i32(0); // clan id
    w.write_i32(0);
    w.write_i32(p.is_female as i32);
    w.write_i32(p.race);
    w.write_i32(p.class_id);
    w.write_i32(1); // active
    w.write_i32(p.x);
    w.write_i32(p.y);
    w.write_i32(p.z);
    w.write_f64(p.cur_hp);
    w.write_f64(p.cur_mp);
    w.write_i64(p.sp);
    w.write_i64(p.exp);
    w.write_i32(p.level);
    w.write_i32(p.reputation);
    w.write_i32(p.pk_kills);
    w.write_i32(game_time % (24 * 60));
    w.write_i32(0);
    w.write_i32(p.class_id);
    w.write_bytes(&[0u8; 16]);
    for _ in 0..9 {
        w.write_i32(0);
    }
    w.write_bytes(&[0u8; 28]);
    w.write_i32(0);
    w.into_bytes()
}

/// Java `StatusUpdateType` client ids used so far (grow as more stats need to
/// be pushed — regen, level-up, gear/buff changes, …).
pub mod status_update_type {
    pub const CUR_HP: u8 = 0x09;
    pub const MAX_HP: u8 = 0x0A;
    pub const CUR_MP: u8 = 0x0B;
    pub const MAX_MP: u8 = 0x0C;
    pub const CUR_CP: u8 = 0x21;
    pub const MAX_CP: u8 = 0x22;
}

/// Port of `serverpackets/StatusUpdate`. `updates` is a list of
/// `(StatusUpdateType client id, value)` pairs, in the order Java's
/// `LinkedHashMap` would iterate (insertion order). `isVisible`/caster id
/// (used to tell nearby players who's responsible for the change) are scoped
/// to "not visible" — self-only updates, no known-list broadcast yet (G7).
pub fn status_update(object_id: i32, updates: &[(u8, i32)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::STATUS_UPDATE);
    w.write_i32(object_id);
    w.write_i32(0); // caster id
    w.write_u8(0); // isVisible
    w.write_u8(updates.len() as u8);
    for &(kind, value) in updates {
        w.write_u8(kind);
        w.write_i32(value);
    }
    w.into_bytes()
}

/// Port of `serverpackets/MagicSkillUse`, self-cast only (`_target == _creature`,
/// no ground-targeted skills, no `RequestActionUse` action id — see the G6
/// cast-pipeline scope notes). `casting_bar_id` is `SkillCastingType.NORMAL`'s
/// client bar id (0).
pub fn magic_skill_use(p: &Player, skill_id: i32, skill_level: i32, hit_time: i32, reuse_delay: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_USE);
    w.write_i32(0); // casting bar: NORMAL
    w.write_i32(p.object_id); // caster
    w.write_i32(p.object_id); // target (self)
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i32(hit_time);
    w.write_i32(0); // reuse group
    w.write_i32(reuse_delay);
    w.write_i32(p.x);
    w.write_i32(p.y);
    w.write_i32(p.z);
    w.write_i16(0); // isGroundTargetSkill
    w.write_i16(0); // no ground location
    w.write_i32(p.x); // target x/y/z (self)
    w.write_i32(p.y);
    w.write_i32(p.z);
    w.write_i32(0); // actionId used
    w.write_i32(0); // actionId
    w.into_bytes()
}

/// Port of `serverpackets/MagicSkillLaunched`, self-cast only (single target =
/// the caster).
pub fn magic_skill_launched(p: &Player, skill_id: i32, skill_level: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_LAUNCHED);
    w.write_i32(0); // casting bar: NORMAL
    w.write_i32(p.object_id);
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i32(1); // target count
    w.write_i32(p.object_id);
    w.into_bytes()
}

/// Port of `serverpackets/SetupGauge` (the cast-bar packet). `color`: 0 = blue.
pub fn setup_gauge(object_id: i32, color: i32, time_ms: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SETUP_GAUGE);
    w.write_i32(object_id);
    w.write_i32(color);
    w.write_i32(time_ms);
    w.write_i32(time_ms);
    w.into_bytes()
}

/// Port of `serverpackets/AcquireSkillDone` — no body.
pub fn acquire_skill_done() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ACQUIRE_SKILL_DONE);
    w.into_bytes()
}
