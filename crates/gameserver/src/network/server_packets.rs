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
    pub const ACTION_FAIL: u8 = 0x1F;
    pub const TARGET_SELECTED: u8 = 0x23;
    pub const TARGET_UNSELECTED: u8 = 0x24;
    pub const MOVE_TO_LOCATION: u8 = 0x2F;
    pub const STOP_MOVE: u8 = 0x47;
    pub const VALIDATE_LOCATION: u8 = 0x79;
    pub const STATUS_UPDATE: u8 = 0x18;
    pub const MAGIC_SKILL_USE: u8 = 0x48;
    pub const MAGIC_SKILL_CANCELED: u8 = 0x49;
    pub const MAGIC_SKILL_LAUNCHED: u8 = 0x54;
    pub const SYSTEM_MESSAGE: u8 = 0x62;
    pub const SETUP_GAUGE: u8 = 0x6B;
    pub const SKILL_COOL_TIME: u8 = 0xC7;
    pub const ACQUIRE_SKILL_DONE: u8 = 0x94;
    pub const MY_TARGET_SELECTED: u8 = 0xB9;

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

/// Port of `serverpackets/ActionFailed.STATIC_PACKET` — the "request consumed"
/// terminator Java sends after (almost) every `Action`/movement request,
/// success or not. `castingType` is always 0 (no `SkillCastingType` bar
/// tracking yet).
pub fn action_failed() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ACTION_FAIL);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/MyTargetSelected`, sent only to the selecting
/// player. `color` (level-diff, shown for attackable targets) is always 0 —
/// no monsters/attackable creatures exist yet.
pub fn my_target_selected(target_object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MY_TARGET_SELECTED);
    w.write_i32(1); // Grand Crusade
    w.write_i32(target_object_id);
    w.write_i16(0); // color
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/TargetSelected` — broadcast to other known players,
/// never to the selecting player themselves (they get `MyTargetSelected`).
pub fn target_selected(object_id: i32, target_object_id: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TARGET_SELECTED);
    w.write_i32(object_id);
    w.write_i32(target_object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/TargetUnselected` — unlike `target_selected`,
/// Java broadcasts this with includeSelf=true, so the deselecting player
/// receives it too (the client needs it to drop its target UI).
pub fn target_unselected(object_id: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TARGET_UNSELECTED);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/ValidateLocation` — the server's "you are actually
/// here" correction to a drifted client (`ValidatePosition` reply).
pub fn validate_location(object_id: i32, x: i32, y: i32, z: i32, heading: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::VALIDATE_LOCATION);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(heading);
    w.write_u8(0xff); // Java: trailing byte, "TODO: Find me!"
    w.into_bytes()
}

/// Port of `serverpackets/StopMove`.
pub fn stop_move(object_id: i32, x: i32, y: i32, z: i32, heading: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::STOP_MOVE);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(heading);
    w.into_bytes()
}

/// Port of `serverpackets/MoveToLocation` with an explicit destination —
/// unlike `enter_world::move_to_location` (which always sends dest==current
/// for the enter-world burst), this is the real move-start packet, broadcast
/// once to the mover *and* other known players (the client only starts
/// walking on the server's confirmation; Java's `Player.broadcastPacket`
/// includes self).
pub fn move_to_location(object_id: i32, dest_x: i32, dest_y: i32, dest_z: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MOVE_TO_LOCATION);
    w.write_i32(object_id);
    w.write_i32(dest_x);
    w.write_i32(dest_y);
    w.write_i32(dest_z);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}

/// Port of `serverpackets/MagicSkillUse` (no ground-targeted skills, no
/// `RequestActionUse` action id yet). `casting_bar_id` is
/// `SkillCastingType.NORMAL`'s client bar id (0). `hit_time` is the
/// client-displayed cast time (`_hitTime + _cancelTime`). Self-casts pass the
/// caster as `target`.
pub fn magic_skill_use(
    caster: &Player,
    target: &Player,
    skill_id: i32,
    skill_level: i32,
    hit_time: i32,
    reuse_delay: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_USE);
    w.write_i32(0); // casting bar: NORMAL
    w.write_i32(caster.object_id);
    w.write_i32(target.object_id);
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i32(hit_time);
    w.write_i32(0); // reuse group
    w.write_i32(reuse_delay);
    w.write_i32(caster.x);
    w.write_i32(caster.y);
    w.write_i32(caster.z);
    w.write_i16(0); // isGroundTargetSkill
    w.write_i16(0); // no ground location
    w.write_i32(target.x);
    w.write_i32(target.y);
    w.write_i32(target.z);
    w.write_i32(0); // actionId used
    w.write_i32(0); // actionId
    w.into_bytes()
}

/// Port of `serverpackets/MagicSkillLaunched`: the launch flourish, broadcast
/// with the resolved target list (`SkillCaster._targets`).
pub fn magic_skill_launched(caster_object_id: i32, skill_id: i32, skill_level: i32, targets: &[i32]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_LAUNCHED);
    w.write_i32(0); // casting bar: NORMAL
    w.write_i32(caster_object_id);
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i32(targets.len() as i32);
    for &t in targets {
        w.write_i32(t);
    }
    w.into_bytes()
}

/// Port of `serverpackets/MagicSkillCanceled` — stops the cast animation
/// client-side. Broadcast (self included) by `stopCasting(aborted == true)`;
/// never sent on a quiet stop (finish-phase failures, natural end).
pub fn magic_skill_canceld(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_CANCELED);
    w.write_i32(object_id);
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

/// The `SystemMessageId` constants the cast pipeline sends (Java's enum has
/// ~6800 — added as handlers need them; the zero-parameter welcome message
/// keeps using `enter_world::system_message`).
pub mod sm_ids {
    pub const NOT_ENOUGH_HP: i16 = 23;
    pub const NOT_ENOUGH_MP: i16 = 24;
    pub const YOUR_CASTING_HAS_BEEN_INTERRUPTED: i16 = 27;
    pub const YOU_USE_S1: i16 = 46;
    pub const S1_IS_NOT_AVAILABLE_REUSE: i16 = 48;
    pub const INVALID_TARGET: i16 = 109;
    pub const CANNOT_SEE_TARGET: i16 = 181;
    pub const DISTANCE_TOO_FAR_CASTING_CANCELLED: i16 = 748;
    pub const S1_HP_HAS_BEEN_RESTORED: i16 = 1066;
    pub const S2_HP_HAS_BEEN_RESTORED_BY_C1: i16 = 1067;
    pub const M_CRITICAL: i16 = 1280;
    pub const C1_HAS_INFLICTED_S3_DAMAGE_ON_C2: i16 = 2261;
    pub const C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2: i16 = 2262;
    pub const S2_SECONDS_REMAINING_FOR_REUSE: i16 = 2303;
    pub const S2_MINUTES_S3_SECONDS_REMAINING_FOR_REUSE: i16 = 2304;
    pub const S2_HOURS_S3_MINUTES_S4_SECONDS_REMAINING_FOR_REUSE: i16 = 2305;
}

/// One `SystemMessage` parameter (Java `SystemMessage.SMParam`), scoped to the
/// types the cast pipeline emits.
pub enum SmParam {
    /// `TYPE_TEXT` (0) — `addString`.
    Text(String),
    /// `TYPE_INT_NUMBER` (1) — `addInt`.
    Int(i32),
    /// `TYPE_SKILL_NAME` (4) — `addSkillName` (id, level, sub-level 0).
    SkillName { id: i32, level: i32 },
    /// `TYPE_PLAYER_NAME` (12) — `addPcName`.
    PlayerName(String),
}

/// Port of `serverpackets/SystemMessage.writeImpl` (localisation branch
/// skipped — `MULTILANG_ENABLE` is off by default): message id, parameter
/// count, then each parameter as a type byte + payload.
pub fn system_message_with(message_id: i16, params: &[SmParam]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SYSTEM_MESSAGE);
    w.write_i16(message_id);
    w.write_u8(params.len() as u8);
    for param in params {
        match param {
            SmParam::Text(s) => {
                w.write_u8(0);
                w.write_string(s);
            }
            SmParam::Int(v) => {
                w.write_u8(1);
                w.write_i32(*v);
            }
            SmParam::SkillName { id, level } => {
                w.write_u8(4);
                w.write_i32(*id);
                w.write_i16(*level as i16);
                w.write_i16(0); // sub-level
            }
            SmParam::PlayerName(s) => {
                w.write_u8(12);
                w.write_string(s);
            }
        }
    }
    w.into_bytes()
}

/// Port of `serverpackets/SkillCoolTime`: every skill still on reuse
/// (`Player.reuses` entries with time remaining), total and remaining in
/// whole seconds. Sent on enter-world and on `RequestSkillCoolTime`.
pub fn skill_cool_time(p: &Player, now_tick: u64) -> Vec<u8> {
    let entries: Vec<(i32, i32, i32, i32)> = p
        .reuses
        .iter()
        .filter_map(|(&skill_id, &(until_tick, total_ms))| {
            let remaining_ticks = until_tick.checked_sub(now_tick)?;
            if remaining_ticks == 0 {
                return None;
            }
            let level = p.skills.get(&skill_id).copied().unwrap_or(1);
            Some((skill_id, level, total_ms / 1000, (remaining_ticks / 10) as i32))
        })
        .collect();
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SKILL_COOL_TIME);
    w.write_i32(entries.len() as i32);
    for (skill_id, level, total_secs, remaining_secs) in entries {
        w.write_i32(skill_id);
        w.write_i32(level);
        w.write_i32(total_secs);
        w.write_i32(remaining_secs);
    }
    w.into_bytes()
}

/// Port of `serverpackets/AcquireSkillDone` — no body.
pub fn acquire_skill_done() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ACQUIRE_SKILL_DONE);
    w.into_bytes()
}
