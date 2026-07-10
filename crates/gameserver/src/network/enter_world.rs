//! The enter-world packet burst (`EnterWorld.runImpl`). Ported to the extent G4
//! needs: the character renders and the loading screen completes. Lists that
//! depend on systems not yet built (inventory, skills, quests, macros, henna,
//! friends, clan, mail) are sent **empty** with TODOs; stat/position/action
//! packets carry real values.
//!
//! Opcodes: plain packets use a single-byte id; extended packets use `0xFE` +
//! a 2-byte little-endian sub-opcode.

use commons::network::PacketWriter;

use crate::data::GameData;
use crate::enums::InventorySlot;
use crate::model::Player;
use crate::network::masks;

const EX: u8 = 0xFE;

fn ex(sub: i16) -> PacketWriter {
    let mut w = PacketWriter::new();
    w.write_u8(EX);
    w.write_i16(sub);
    w
}

// ---- plain packets ----

/// `ItemList` (0x11) — empty inventory. TODO(G6).
pub fn item_list() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x11);
    w.write_i16(0); // show window (false)
    w.write_i16(0); // item count
    w.write_i16(0); // inventory block (none)
    w.into_bytes()
}

/// `ShortCutInit` (0x45) — no shortcuts. TODO(G-later).
pub fn shortcut_init() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x45);
    w.write_i32(0);
    w.into_bytes()
}

/// `SkillList` (0x5F) — empty (skills deferred by request). TODO(G7).
pub fn skill_list() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x5F);
    w.write_i32(0); // skill count
    w.write_i32(-1); // last learned skill id (none)
    w.into_bytes()
}

/// `AcquireSkillList` (0x90) — nothing learnable yet. TODO(G7).
pub fn acquire_skill_list() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x90);
    w.write_i16(0);
    w.into_bytes()
}

/// `HennaInfo` (0xE5) — no dyes.
pub fn henna_info() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0xE5);
    for _ in 0..8 {
        w.write_i16(0); // INT/STR/CON/MEN/DEX/WIT/LUC/CHA
    }
    w.write_i32(0); // used slots (3 - empty)
    w.write_i32(0); // henna count
    w.write_i32(0); // premium slot dye id
    w.write_i32(0); // premium slot dye time left
    w.write_i32(0); // premium slot dye valid
    w.into_bytes()
}

/// `EtcStatusUpdate` (0xF9) — no charges/penalties.
pub fn etc_status_update() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0xF9);
    w.write_u8(0); // charges
    w.write_i32(0); // weight penalty
    w.write_u8(0); // weapon grade penalty
    w.write_u8(0); // armor grade penalty
    w.write_u8(0); // death penalty
    w.write_u8(0); // charged souls
    w.write_u8(0); // mask
    w.into_bytes()
}

/// `QuestList` (0x86) — no active quests. TODO(G10).
pub fn quest_list() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x86);
    w.write_i16(0); // active quests
    w.write_bytes(&[0u8; 128]); // one-time quest mask
    w.into_bytes()
}

/// `SkillCoolTime` (0xC7) — no reuse timers.
pub fn skill_cool_time() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0xC7);
    w.write_i32(0);
    w.into_bytes()
}

/// `AbnormalStatusUpdate` (0x85) — no active effects.
pub fn abnormal_status_update() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x85);
    w.write_i16(0);
    w.into_bytes()
}

/// `FriendList` (0x58) — no friends. TODO(G9).
pub fn friend_list() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x58);
    w.write_i32(0);
    w.into_bytes()
}

/// `MoveToLocation` (0x2F): destination == current position (Java sends this on
/// enter so the client fixes the character's position).
pub fn move_to_location(p: &Player) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x2F);
    w.write_i32(p.object_id);
    w.write_i32(p.x);
    w.write_i32(p.y);
    w.write_i32(p.z);
    w.write_i32(p.x);
    w.write_i32(p.y);
    w.write_i32(p.z);
    w.into_bytes()
}

/// `SystemMessage` (0x62) with no parameters (e.g. the welcome message id 34).
pub fn system_message(message_id: i16) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x62);
    w.write_i16(message_id);
    w.write_u8(0); // parameter count
    w.into_bytes()
}

/// System message id `WELCOME_TO_THE_WORLD_OF_LINEAGE_II`.
pub const SM_WELCOME: i16 = 34;

// ---- extended packets ----

/// `ExVitalityEffectInfo` (0x118).
pub fn ex_vitality_effect_info(p: &Player) -> Vec<u8> {
    let mut w = ex(0x118);
    w.write_i32(p.vitality_points);
    w.write_i32(0); // vitality bonus
    w.write_i16(0); // additional bonus %
    w.write_i16(0); // items remaining
    w.write_i16(0); // max items allowed
    w.into_bytes()
}

/// `ExGetBookMarkInfoPacket` (0x85) — no teleport bookmarks.
pub fn ex_get_bookmark_info() -> Vec<u8> {
    let mut w = ex(0x85);
    w.write_i32(0); // dummy
    w.write_i32(0); // bookmark slot
    w.write_i32(0); // bookmark count
    w.into_bytes()
}

/// `ExQuestItemList` (0xC7) — no quest items. TODO(G6/G10).
pub fn ex_quest_item_list() -> Vec<u8> {
    let mut w = ex(0xC7);
    w.write_i16(0); // item count
    w.write_i16(0); // inventory block (none)
    w.into_bytes()
}

/// `ExBasicActionList` (0x60) — the default action-bar ids from `ActionData`.
pub fn ex_basic_action_list(data: &GameData) -> Vec<u8> {
    let ids = data.action_data.action_ids();
    let mut w = ex(0x60);
    w.write_i32(ids.len() as i32);
    for &id in ids {
        w.write_i32(id);
    }
    w.into_bytes()
}

/// `ExSubjobInfo` (0xEA) — no subclasses (`NO_CHANGES`).
pub fn ex_subjob_info(p: &Player) -> Vec<u8> {
    let mut w = ex(0xEA);
    w.write_u8(0); // type = NO_CHANGES
    w.write_i32(p.class_id);
    w.write_i32(p.race);
    w.write_i32(0); // subclass count
    w.into_bytes()
}

/// `ExUserInfoInvenWeight` (0x166). TODO(G6): real current/max load.
pub fn ex_user_info_inven_weight(p: &Player) -> Vec<u8> {
    let mut w = ex(0x166);
    w.write_i32(p.object_id);
    w.write_i32(0); // current load
    w.write_i32(80_000); // max load (placeholder)
    w.into_bytes()
}

/// `ExAdenaInvenCount` (0x13E). TODO(G6): real adena / inventory size.
pub fn ex_adena_inven_count() -> Vec<u8> {
    let mut w = ex(0x13E);
    w.write_i64(0); // adena
    w.write_i16(0); // inventory size
    w.into_bytes()
}

/// `ExUserInfoEquipSlot` (0x156) — masked, all 33 `InventorySlot` components,
/// values read from the (still empty until G6) paperdoll.
pub fn ex_user_info_equip_slot(p: &Player) -> Vec<u8> {
    let mut w = ex(0x156);
    w.write_i32(p.object_id);
    w.write_i16(InventorySlot::VALUES.len() as i16);
    w.write_bytes(&masks::build_mask::<5>(InventorySlot::VALUES.iter().map(|s| s.mask())));
    for slot in InventorySlot::VALUES {
        let pd = slot.slot();
        let augment = p.inventory.paperdoll_augmentation(pd);
        w.write_i16(22); // block length: 10 + 4 * 3
        w.write_i32(p.inventory.paperdoll_object_id(pd));
        w.write_i32(p.inventory.paperdoll_item_id(pd));
        w.write_i32(augment.map_or(0, |(opt1, _)| opt1));
        w.write_i32(augment.map_or(0, |(_, opt2)| opt2));
        w.write_i32(p.inventory.paperdoll_visual_id(pd));
    }
    w.into_bytes()
}

/// `ExPledgeWaitingListAlarm` (0x147) — no body.
pub fn ex_pledge_waiting_list_alarm() -> Vec<u8> {
    ex(0x147).into_bytes()
}

/// `ExRotation` (0xC2).
pub fn ex_rotation(p: &Player) -> Vec<u8> {
    let mut w = ex(0xC2);
    w.write_i32(p.object_id);
    w.write_i32(p.heading);
    w.into_bytes()
}

/// `ExSetCompassZoneCode` (0x33).
pub fn ex_set_compass_zone_code(code: i32) -> Vec<u8> {
    let mut w = ex(0x33);
    w.write_i32(code);
    w.into_bytes()
}

/// `ExAutoSoulShot` (0x0C).
pub fn ex_auto_soul_shot(item_id: i32, enable: bool, kind: i32) -> Vec<u8> {
    let mut w = ex(0x0C);
    w.write_i32(item_id);
    w.write_i32(enable as i32);
    w.write_i32(kind);
    w.into_bytes()
}
