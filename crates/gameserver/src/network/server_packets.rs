//! Outbound (server → client) packets. Ported 1:1 from
//! `gameserver/network/serverpackets`. Each builder returns the serialized body
//! (opcode + payload, unencrypted); the connection task encrypts and frames it.
//!
//! G1 covers only `KeyPacket`; the rest arrive with their milestones.

use commons::network::PacketWriter;

use crate::data::npc_data::NpcTemplate;
use crate::enums::NpcInfoType;
use crate::model::inventory::PaperdollSlot;
use crate::model::Player;
use crate::network::masks;

/// `ServerPackets` opcodes (the single-byte `_id1`).
pub mod opcodes {
    pub const DELETE_OBJECT: u8 = 0x08;
    pub const NPC_INFO: u8 = 0x0C;
    pub const NPC_HTML_MESSAGE: u8 = 0x19;
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
    pub const CHAR_INFO: u8 = 0x31;
    pub const STOP_MOVE: u8 = 0x47;
    pub const VALIDATE_LOCATION: u8 = 0x79;
    pub const STATUS_UPDATE: u8 = 0x18;
    pub const MAGIC_SKILL_USE: u8 = 0x48;
    pub const MAGIC_SKILL_CANCELED: u8 = 0x49;
    pub const MAGIC_SKILL_LAUNCHED: u8 = 0x54;
    pub const SYSTEM_MESSAGE: u8 = 0x62;
    pub const RESTART_RESPONSE: u8 = 0x71;
    pub const LOG_OUT_OK: u8 = 0x84;
    pub const SETUP_GAUGE: u8 = 0x6B;
    pub const SKILL_COOL_TIME: u8 = 0xC7;
    pub const ACQUIRE_SKILL_DONE: u8 = 0x94;
    pub const MY_TARGET_SELECTED: u8 = 0xB9;
    pub const DIE: u8 = 0x00;
    pub const REVIVE: u8 = 0x01;
    pub const TELEPORT_TO_LOCATION: u8 = 0x22;
    pub const AUTO_ATTACK_START: u8 = 0x25;
    pub const AUTO_ATTACK_STOP: u8 = 0x26;
    pub const SOCIAL_ACTION: u8 = 0x27;
    pub const CHANGE_MOVE_TYPE: u8 = 0x28;
    pub const ATTACK: u8 = 0x33;
    pub const MOVE_TO_PAWN: u8 = 0x72;

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

/// The lobby slot to highlight: the most-recently-accessed character.
/// Characters marked for deletion never become active; if every character is
/// marked (or the list is empty), none is highlighted (-1).
fn lobby_active_id(chars: &[crate::character::CharData]) -> i32 {
    chars
        .iter()
        .enumerate()
        .filter(|(_, c)| c.delete_time == 0)
        .max_by_key(|(_, c)| c.last_access)
        .map(|(i, _)| i as i32)
        .unwrap_or(-1)
}

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
    let active_id = if active_id == -1 { lobby_active_id(chars) } else { active_id };

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
pub fn char_selected(v: &crate::model::PlayerView, session_id: i32, game_time: i32) -> Vec<u8> {
    let crate::model::PlayerView { p, pos, vitals, .. } = v;
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
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    w.write_f64(vitals.cur_hp);
    w.write_f64(vitals.cur_mp);
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

/// Port of `serverpackets/RestartResponse` (`TRUE`/`FALSE` statics): whether
/// the `RequestRestart` was accepted — `true` sends the client back to the
/// character-selection screen (a `CharSelectionInfo` must follow).
pub fn restart_response(result: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::RESTART_RESPONSE);
    w.write_i32(result as i32);
    w.into_bytes()
}

/// Port of `serverpackets/LeaveWorld` (`LOG_OUT_OK`): the "safe to quit"
/// acknowledgement for the client's `Logout`; the server closes the connection
/// right after it.
pub fn leave_world() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::LOG_OUT_OK);
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
/// player. `color` is `player.level - target.level` for auto-attackable
/// targets (tints the target bar by level gap), 0 otherwise.
pub fn my_target_selected(target_object_id: i32, color: i16) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MY_TARGET_SELECTED);
    w.write_i32(1); // Grand Crusade
    w.write_i32(target_object_id);
    w.write_i16(color);
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

/// One rolled hit inside an `Attack` packet (Java `model/Hit`): flag bits
/// from `enums/AttackType` (miss 0x01 within flags... see `hit_flags`).
#[derive(Debug, Clone, Copy)]
pub struct AttackHit {
    pub target_object_id: i32,
    pub damage: i32,
    pub miss: bool,
    pub crit: bool,
}

/// Java `enums/AttackType` masks folded by `Hit`'s constructor: `MISSED` =
/// 0x01, `BLOCKED` = 0x02 (never set — no shield defence), `CRITICAL` = 0x04,
/// `SHOT_USED` = 0x08 (never — no soulshots).
fn hit_flags(hit: &AttackHit) -> i32 {
    if hit.miss {
        return 0x01;
    }
    if hit.crit {
        0x04
    } else {
        0
    }
}

/// Port of `serverpackets/Attack` (single-hit melee shape — the trailing
/// extra-hit list is empty, matching non-dual weapons).
pub fn attack(attacker_object_id: i32, hit: &AttackHit, ax: i32, ay: i32, az: i32, tx: i32, ty: i32, tz: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ATTACK);
    w.write_i32(attacker_object_id);
    w.write_i32(hit.target_object_id);
    w.write_i32(0); // soulshot visual substitute (brooch jewels)
    w.write_i32(hit.damage);
    w.write_i32(hit_flags(hit));
    w.write_i32(0); // ss grade
    w.write_i32(ax);
    w.write_i32(ay);
    w.write_i32(az);
    w.write_i16(0); // no additional hits
    w.write_i32(tx);
    w.write_i32(ty);
    w.write_i32(tz);
    w.into_bytes()
}

/// Port of `serverpackets/Die` — broadcast on any creature's death. Every
/// revive-destination flag is written explicitly; for NPCs they are all
/// false. `to_village` = `canRevive() && !isPendingRevive()` for players.
pub fn die(object_id: i32, to_village: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::DIE);
    w.write_i32(object_id);
    w.write_i32(to_village as i32); // to village
    w.write_i32(0); // to clan hall
    w.write_i32(0); // to castle
    w.write_i32(0); // to outpost / siege HQ
    w.write_i32(0); // sweepable
    w.write_i32(0); // use feather
    w.write_i32(0); // to fortress
    w.write_i32(0); // disables feather button timer
    w.write_i32(0); // adventure's song
    w.write_u8(0); // hide die animation
    w.write_i32(0); // items enabled
    w.write_i32(0); // item count
    w.into_bytes()
}

/// Port of `serverpackets/Revive`.
pub fn revive(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::REVIVE);
    w.write_i32(object_id);
    w.into_bytes()
}

/// Port of `serverpackets/AutoAttackStart` — combat stance begins.
pub fn auto_attack_start(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::AUTO_ATTACK_START);
    w.write_i32(object_id);
    w.into_bytes()
}

/// Port of `serverpackets/AutoAttackStop` — combat stance ends (15 s after
/// the last swing, `AttackStanceTaskManager.COMBAT_TIME`).
pub fn auto_attack_stop(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::AUTO_ATTACK_STOP);
    w.write_i32(object_id);
    w.into_bytes()
}

/// Port of `serverpackets/SocialAction` (also carries the level-up effect,
/// `SocialAction.LEVEL_UP` = 2122).
pub const SOCIAL_ACTION_LEVEL_UP: i32 = 2122;
pub fn social_action(object_id: i32, action_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SOCIAL_ACTION);
    w.write_i32(object_id);
    w.write_i32(action_id);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/ChangeMoveType` — walk/run toggle broadcast (Java
/// `Creature.setRunning`/`setWalking`).
pub fn change_move_type(object_id: i32, running: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHANGE_MOVE_TYPE);
    w.write_i32(object_id);
    w.write_i32(if running { 1 } else { 0 });
    w.write_i32(0); // c2
    w.into_bytes()
}

/// Port of `serverpackets/MoveToPawn` — "walk toward that creature, stopping
/// at `distance`" (chasing/follow movement; plain destination moves use
/// `MoveToLocation`).
pub fn move_to_pawn(object_id: i32, target_object_id: i32, distance: i32, x: i32, y: i32, z: i32, tx: i32, ty: i32, tz: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MOVE_TO_PAWN);
    w.write_i32(object_id);
    w.write_i32(target_object_id);
    w.write_i32(distance);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(tx);
    w.write_i32(ty);
    w.write_i32(tz);
    w.into_bytes()
}

/// Port of `serverpackets/TeleportToLocation` (fade-style, like Java's
/// constant 0).
pub fn teleport_to_location(object_id: i32, x: i32, y: i32, z: i32, heading: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TELEPORT_TO_LOCATION);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(0); // fade 0, instant 1
    w.write_i32(heading);
    w.write_i32(0); // unknown
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
/// caster as `target`. `reuse_group` is the skill's `reuseDelayGroup` — -1
/// when ungrouped (the client greys *every* icon on 0, Java's constructor
/// default is -1).
pub fn magic_skill_use(
    caster: &Player,
    caster_pos: &crate::model::components::Position,
    target: (i32, i32, i32, i32), // (object_id, x, y, z) — player or NPC
    skill_id: i32,
    skill_level: i32,
    hit_time: i32,
    reuse_group: i32,
    reuse_delay: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_USE);
    w.write_i32(0); // casting bar: NORMAL
    w.write_i32(caster.object_id);
    w.write_i32(target.0);
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i32(hit_time);
    w.write_i32(reuse_group);
    w.write_i32(reuse_delay);
    w.write_i32(caster_pos.x);
    w.write_i32(caster_pos.y);
    w.write_i32(caster_pos.z);
    w.write_i16(0); // isGroundTargetSkill
    w.write_i16(0); // no ground location
    w.write_i32(target.1);
    w.write_i32(target.2);
    w.write_i32(target.3);
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
    pub const YOU_HAVE_OBTAINED_S1_ADENA: i16 = 28;
    pub const YOU_HAVE_OBTAINED_S2_S1: i16 = 29;
    pub const YOU_HAVE_OBTAINED_S1: i16 = 30;
    pub const YOU_HAVE_AVOIDED_C1_S_ATTACK: i16 = 42;
    pub const YOU_HAVE_MISSED: i16 = 43;
    pub const CRITICAL_HIT: i16 = 44;
    pub const YOUR_LEVEL_HAS_INCREASED: i16 = 96;
    pub const YOU_HAVE_ACQUIRED_S1_SP: i16 = 331;
    pub const YOUR_SP_HAS_DECREASED_BY_S1: i16 = 538;
    pub const YOUR_XP_HAS_DECREASED_BY_S1: i16 = 539;
    pub const C1_HAS_EVADED_C2_S_ATTACK: i16 = 2264;
    pub const C1_S_ATTACK_WENT_ASTRAY: i16 = 2265;
    pub const C1_LANDED_A_CRITICAL_HIT: i16 = 2266;
    pub const YOU_HAVE_ACQUIRED_S1_XP_BONUS_S2_AND_S3_SP_BONUS_S4: i16 = 3259;
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
    /// `TYPE_NPC_NAME` (2) — `addNpcName` (template id + 1000000).
    NpcName(i32),
    /// `TYPE_ITEM_NAME` (3) — `addItemName`.
    ItemName(i32),
    /// `TYPE_LONG_NUMBER` (6) — `addLong`.
    Long(i64),
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
            SmParam::NpcName(template_id) => {
                w.write_u8(2);
                w.write_i32(1_000_000 + *template_id);
            }
            SmParam::ItemName(item_id) => {
                w.write_u8(3);
                w.write_i32(*item_id);
            }
            SmParam::Long(v) => {
                w.write_u8(6);
                w.write_i64(*v);
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
/// whole seconds. The id written is the map key — the shared reuse group when
/// the skill has one, else the skill id (Java writes
/// `sharedReuseGroup > 0 ? group : skillId`). Sent on enter-world and on
/// `RequestSkillCoolTime`.
pub fn skill_cool_time(reuses: &crate::model::components::Reuses, now_tick: u64) -> Vec<u8> {
    let entries: Vec<(i32, i32, i32, i32)> = reuses
        .0
        .iter()
        .filter_map(|(&reuse_key, r)| {
            let remaining_ticks = r.until_tick.checked_sub(now_tick)?;
            if remaining_ticks == 0 {
                return None;
            }
            Some((reuse_key, r.skill_level, r.total_ms / 1000, (remaining_ticks / 10) as i32))
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

/// Port of `serverpackets/DeleteObject` — removes an object from the client's
/// screen when it leaves the observer's known area.
pub fn delete_object(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::DELETE_OBJECT);
    w.write_i32(object_id);
    w.write_u8(0); // c2
    w.into_bytes()
}

/// `CharInfo.PAPERDOLL_ORDER` — the 12-slot equipment view other clients get
/// (a subset of the 33-slot `UserInfo` order; `RHand` repeats for LRHAND).
const CHAR_INFO_PAPERDOLL_ORDER: [PaperdollSlot; 12] = [
    PaperdollSlot::Under,
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
];

/// `ServerPacket.PAPERDOLL_ORDER_AUGMENT`.
const CHAR_INFO_PAPERDOLL_ORDER_AUGMENT: [PaperdollSlot; 3] =
    [PaperdollSlot::RHand, PaperdollSlot::LHand, PaperdollSlot::RHand];

/// `ServerPacket.PAPERDOLL_ORDER_VISUAL_ID`.
const CHAR_INFO_PAPERDOLL_ORDER_VISUAL_ID: [PaperdollSlot; 9] = [
    PaperdollSlot::RHand,
    PaperdollSlot::LHand,
    PaperdollSlot::RHand,
    PaperdollSlot::Gloves,
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Feet,
    PaperdollSlot::Hair,
    PaperdollSlot::Hair2,
];

/// Port of `serverpackets/CharInfo` — how this player appears on *other*
/// players' clients (the counterpart of `UserInfo` for the owner). Values for
/// systems not yet modeled (clan, mounts, stores, cubics, fishing, abnormal
/// visual effects…) are their empty Java defaults; the vehicle branch and the
/// GM-sees-invisible variant are skipped (no boats/GM model).
pub fn char_info(v: &crate::model::PlayerView) -> Vec<u8> {
    let crate::model::PlayerView { p, pos, vitals, pvitals, speeds, collision, combat, inventory, .. } = v;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_INFO);
    w.write_u8(0); // Grand Crusade
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    w.write_i32(0); // vehicle object id
    w.write_i32(p.object_id);
    w.write_string(&p.name);
    w.write_i16(p.race as i16);
    w.write_u8(p.is_female as u8);
    w.write_i32(p.base_class_id); // root class id

    for slot in CHAR_INFO_PAPERDOLL_ORDER {
        w.write_i32(inventory.paperdoll_item_id(slot)); // display id
    }
    for slot in CHAR_INFO_PAPERDOLL_ORDER_AUGMENT {
        let augment = inventory.paperdoll_augmentation(slot);
        w.write_i32(augment.map_or(0, |a| a.0));
        w.write_i32(augment.map_or(0, |a| a.1));
    }
    w.write_u8(0); // armor min enchant
    for slot in CHAR_INFO_PAPERDOLL_ORDER_VISUAL_ID {
        w.write_i32(inventory.paperdoll_visual_id(slot));
    }

    w.write_u8(0); // pvp flag
    w.write_i32(p.reputation);
    w.write_i32(combat.m_atk_spd);
    w.write_i32(combat.p_atk_spd);
    w.write_i16(speeds.run_spd as i16);
    w.write_i16(speeds.walk_spd as i16);
    w.write_i16(speeds.swim_run_spd as i16);
    w.write_i16(speeds.swim_walk_spd as i16);
    w.write_i16(0); // fly run
    w.write_i16(0); // fly walk
    w.write_i16(0); // fly run (repeat)
    w.write_i16(0); // fly walk (repeat)
    w.write_f64(speeds.move_multiplier);
    w.write_f64(1.0); // attack speed multiplier
    w.write_f64(collision.radius);
    w.write_f64(collision.height);
    w.write_i32(p.hair_style); // visual hair
    w.write_i32(p.hair_color);
    w.write_i32(p.face);
    w.write_string(&p.title);
    w.write_i32(0); // clan id
    w.write_i32(0); // clan crest id
    w.write_i32(0); // ally id
    w.write_i32(0); // ally crest id
    w.write_u8(1); // !isSitting — standing
    w.write_u8(speeds.running as u8);
    w.write_u8(0); // in combat
    w.write_u8(0); // alike dead
    w.write_u8(0); // invisible
    w.write_u8(0); // mount type
    w.write_u8(0); // private store type
    w.write_i16(0); // cubic count (+ cubic ids)
    w.write_u8(0); // in matching room
    w.write_u8(0); // 1 water, 2 flying mount
    w.write_i16(0); // recom have
    w.write_i32(0); // mount npc id
    w.write_i32(p.class_id);
    w.write_i32(0); // TODO: Find me! (Java unknown)
    w.write_u8(inventory.paperdoll_enchant_level(PaperdollSlot::RHand) as u8); // weapon enchant
    w.write_u8(0); // team id
    w.write_i32(0); // clan crest large id
    w.write_u8(0); // noble
    w.write_u8(0); // hero
    w.write_u8(0); // fishing
    w.write_i32(0); // bait x
    w.write_i32(0); // bait y
    w.write_i32(0); // bait z
    w.write_i32(0xFFFFFF); // name color
    w.write_i32(pos.heading);
    w.write_u8(0); // pledge class
    w.write_i16(0); // pledge type
    w.write_i32(0xFFFF77); // title color
    w.write_u8(0); // cursed weapon level
    w.write_i32(0); // clan reputation score
    w.write_i32(0); // transformation display id
    w.write_i32(0); // agathion id
    w.write_u8(0); // nPvPRestrainStatus
    w.write_i32(pvitals.cur_cp.round() as i32);
    w.write_i32(vitals.max_hp);
    w.write_i32(vitals.cur_hp.round() as i32);
    w.write_i32(vitals.max_mp);
    w.write_i32(vitals.cur_mp.round() as i32);
    w.write_u8(0); // cBRLectureMark
    w.write_i32(0); // abnormal visual effect count (+ short ids)
    w.write_u8(0); // true hero (100 when true)
    w.write_u8(1); // hair accessory enabled
    w.write_u8(0); // used ability points
    w.into_bytes()
}

/// Port of `serverpackets/NpcInfo` (masked, 5 mask bytes / "mask_bits_37").
/// Component selection follows the Java constructor with the not-yet-modeled
/// state at its defaults: no summon animation, no water/fly/team/enchant/
/// clone/transform/abnormals, no clan, reputation 0, pvp flag 0. The
/// localisation pass (`MULTILANG_ENABLE`) is skipped.
pub fn npc_info(v: &crate::model::npc::NpcView, t: &NpcTemplate) -> Vec<u8> {
    let crate::model::npc::NpcView { npc, pos, vitals, speeds } = v;
    use NpcInfoType as T;

    // Java `NpcInfo._masks` starts with the two unnamed always-on component
    // pairs (0x0C/0x0D and 0x14/0x15) pre-set.
    let mut mask_bytes: [u8; 5] = [0x00, 0x0C, 0x0C, 0x00, 0x00];
    let mut init_size: i32 = 0;
    let mut block_size: i32 = 0;
    let mut add = |mask_bytes: &mut [u8; 5], ty: T| {
        masks::add_mask(mask_bytes, ty.mask());
        // `calcBlockSize`: ATTACKABLE/RELATIONS/TITLE go in block 1, the rest
        // in block 2; the string components add their chars on top.
        match ty {
            T::Attackable | T::Relations => init_size += ty.block_length(),
            T::Title => init_size += ty.block_length() + t.title.len() as i32 * 2,
            T::Name => block_size += ty.block_length() + t.name.len() as i32 * 2,
            _ => block_size += ty.block_length(),
        }
    };

    add(&mut mask_bytes, T::Attackable);
    add(&mut mask_bytes, T::Relations);
    add(&mut mask_bytes, T::Id);
    add(&mut mask_bytes, T::Position);
    add(&mut mask_bytes, T::StopMode);
    add(&mut mask_bytes, T::MoveMode);
    if pos.heading > 0 {
        add(&mut mask_bytes, T::Heading);
    }
    if t.base_p_atk_spd > 0 || t.base_m_atk_spd > 0 {
        add(&mut mask_bytes, T::AtkCastSpeed);
    }
    if t.base_run_spd > 0.0 {
        add(&mut mask_bytes, T::SpeedMultiplier);
    }
    if t.rhand > 0 || t.lhand > 0 {
        add(&mut mask_bytes, T::Equipped);
    }
    if vitals.max_hp > 0 {
        add(&mut mask_bytes, T::MaxHp);
    }
    if vitals.max_mp > 0 {
        add(&mut mask_bytes, T::MaxMp);
    }
    if vitals.cur_hp <= vitals.max_hp as f64 {
        add(&mut mask_bytes, T::CurrentHp);
    }
    if vitals.cur_mp <= vitals.max_mp as f64 {
        add(&mut mask_bytes, T::CurrentMp);
    }
    if t.server_side_name {
        add(&mut mask_bytes, T::Name);
    }
    if t.server_side_title {
        add(&mut mask_bytes, T::Title);
    }
    add(&mut mask_bytes, T::PetEvolutionId);
    // Status mask: 0x01 in combat, 0x02 dead, 0x04 targetable, 0x08 show name.
    let mut status_mask = 0u8;
    if t.targetable {
        status_mask |= 0x04;
    }
    if t.show_name {
        status_mask |= 0x08;
    }
    if status_mask != 0 {
        add(&mut mask_bytes, T::VisualState);
    }

    let contains = |ty: T| masks::contains_mask(&mask_bytes, ty.mask());

    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NPC_INFO);
    w.write_i32(npc.object_id);
    w.write_u8(0); // 0=teleported 1=default 2=summoned
    w.write_i16(37); // mask_bits_37
    w.write_bytes(&mask_bytes);

    // Block 1.
    w.write_u8(init_size as u8);
    w.write_u8(u8::from(t.is_attackable_class() && t.type_name != "Guard"));
    w.write_i32(0); // relations
    if contains(T::Title) {
        w.write_string(&t.title);
    }

    // Block 2.
    w.write_i16(block_size as i16);
    w.write_i32(t.display_id + 1_000_000);
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    if contains(T::Heading) {
        w.write_i32(pos.heading);
    }
    if contains(T::AtkCastSpeed) {
        w.write_i32(t.base_p_atk_spd);
        w.write_i32(t.base_m_atk_spd);
    }
    if contains(T::SpeedMultiplier) {
        // Current speed / template base speed — 1.0 until buffs/AI exist.
        w.write_f32(1.0); // movement speed multiplier
        w.write_f32(1.0); // attack speed multiplier
    }
    if contains(T::Equipped) {
        w.write_i32(t.rhand);
        w.write_i32(0); // armor id (Java writes 0)
        w.write_i32(t.lhand);
    }
    w.write_u8(1); // STOP_MODE: !isDead
    w.write_u8(speeds.running as u8); // MOVE_MODE
    w.write_i32(0); // PET_EVOLUTION_ID
    if contains(T::CurrentHp) {
        w.write_i32(vitals.cur_hp as i32);
    }
    if contains(T::CurrentMp) {
        w.write_i32(vitals.cur_mp as i32);
    }
    if contains(T::MaxHp) {
        w.write_i32(vitals.max_hp);
    }
    if contains(T::MaxMp) {
        w.write_i32(vitals.max_mp);
    }
    if contains(T::Name) {
        w.write_string(&t.name);
    }
    if contains(T::VisualState) {
        w.write_u8(status_mask);
    }
    w.into_bytes()
}

/// Port of `serverpackets/NpcHtmlMessage` — the NPC dialog window. `item_id`
/// stays 0 (item-triggered dialogs aren't a thing yet).
pub fn npc_html_message(npc_object_id: i32, html: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NPC_HTML_MESSAGE);
    w.write_i32(npc_object_id);
    w.write_string(html);
    w.write_i32(0); // item id
    w.write_i32(0); // show common board
    w.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::lobby_active_id;
    use crate::character::CharData;
    use commons::network::PacketWriter;

    /// `NpcInfo` byte layout, hand-computed against the Java constructor +
    /// `writeImpl` (no client capture available for NPCs yet, unlike the
    /// UserInfo test — the mask math is shared with that byte-verified path).
    #[test]
    fn npc_info_layout_matches_java() {
        let mut t = crate::data::npc_data::default_template(30001);
        t.name = "Gina".into();
        t.server_side_name = true;
        t.level = 5;
        t.base_hp_max = 100.0;
        t.base_mp_max = 50.0;
        // Defaults keep: p_atk_spd 300, m_atk_spd 333, run 120, rhand/lhand 0,
        // targetable + show_name true (→ status mask 0x0C), type Folk.
        let (npc, (mut pos, _region, vitals, speeds, _collision, _attack, _ai, _aggro)) =
            crate::model::npc::Npc::for_test(0x4000_0001, 30001, 100, 200, -300, 100, 50);
        pos.heading = 4000;
        let v = crate::model::npc::NpcView { npc: &npc, pos: &pos, vitals: &vitals, speeds: &speeds };

        let mut w = PacketWriter::new();
        w.write_u8(0x0C); // NPC_INFO
        w.write_i32(0x4000_0001);
        w.write_u8(0); // no summon animation
        w.write_i16(37);
        // Components: Id, Attackable, Relations, Name, Position, Heading,
        // AtkCastSpeed | SpeedMultiplier, StopMode, MoveMode (+ pre-set
        // 0x0C/0x0D) | PetEvolutionId (+ pre-set 0x14/0x15) | CurrentHp,
        // CurrentMp, MaxHp, MaxMp | VisualState(37).
        w.write_bytes(&[0xFD, 0xBC, 0x1C, 0xF0, 0x04]);
        w.write_u8(5); // init size: attackable(1) + relations(4)
        w.write_u8(0); // Folk is not in the Attackable subtree
        w.write_i32(0); // relations
        w.write_i16(69); // block 2 size
        w.write_i32(1_030_001); // display id + 1000000
        w.write_i32(100);
        w.write_i32(200);
        w.write_i32(-300);
        w.write_i32(4000); // heading
        w.write_i32(300); // p atk spd
        w.write_i32(333); // m atk spd
        w.write_f32(1.0); // movement multiplier
        w.write_f32(1.0); // attack speed multiplier
        w.write_u8(1); // stop mode: alive
        w.write_u8(0); // move mode: walking
        w.write_i32(0); // pet evolution id
        w.write_i32(100); // cur hp
        w.write_i32(50); // cur mp
        w.write_i32(100); // max hp
        w.write_i32(50); // max mp
        w.write_string("Gina");
        w.write_u8(0x0C); // visual state: targetable | show name
        let expected = w.into_bytes();

        assert_eq!(super::npc_info(&v, &t), expected);
    }

    fn chr(last_access: i64, delete_time: i64) -> CharData {
        CharData { last_access, delete_time, ..Default::default() }
    }

    #[test]
    fn active_id_is_most_recently_accessed_non_deleted() {
        let chars = [chr(100, 0), chr(300, commons::util::now_millis()), chr(200, 0)];
        assert_eq!(lobby_active_id(&chars), 2);
    }

    #[test]
    fn active_id_is_none_when_all_marked_for_deletion() {
        let now = commons::util::now_millis();
        let chars = [chr(100, now), chr(300, now)];
        assert_eq!(lobby_active_id(&chars), -1);
    }

    #[test]
    fn active_id_is_none_for_empty_list() {
        assert_eq!(lobby_active_id(&[]), -1);
    }
}
