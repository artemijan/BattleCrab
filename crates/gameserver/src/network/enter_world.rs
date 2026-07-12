//! The enter-world packet burst (`EnterWorld.runImpl`). Inventory is real as
//! of G5, skills as of G6, shortcuts/macros as of G9.6, friends as of G10,
//! quest lists as of G11 (those builders live in `server_packets.rs` or
//! here); lists that depend on systems not yet built (henna, mail) are
//! still sent **empty** with TODOs; stat/position/action/item packets carry
//! real values.
//!
//! Opcodes: plain packets use a single-byte id; extended packets use `0xFE` +
//! a 2-byte little-endian sub-opcode.

use commons::network::PacketWriter;

use crate::data::item_data::ItemTemplate;
use crate::data::GameData;
use crate::enums::InventorySlot;
use crate::model::inventory::ItemInstance;
use crate::model::Player;
use crate::network::masks;

const EX: u8 = 0xFE;

fn ex(sub: i16) -> PacketWriter {
    let mut w = PacketWriter::new();
    w.write_u8(EX);
    w.write_i16(sub);
    w
}

/// `AbstractItemPacket.writeItem`, shared by `ItemList` and `InventoryUpdate`:
/// mask (always 0 — augmentation/elemental/enchant-effect/visual-id are later
/// milestones), object id, item id, T1, count, type2, customType1, equipped,
/// body part, enchant level, customType2, mana, time, available.
fn write_item_entry(w: &mut PacketWriter, item: &ItemInstance, template: &ItemTemplate, equipped: bool) {
    w.write_u8(0); // mask
    w.write_i32(item.object_id);
    w.write_i32(item.item_id);
    w.write_u8(if equipped { 0xFF } else { 0 }); // T1
    w.write_i64(item.count);
    w.write_u8(template.type2 as u8);
    w.write_u8(item.custom_type1 as u8);
    w.write_i16(equipped as i16);
    w.write_i64(template.body_part as i64);
    w.write_u8(item.enchant_level as u8);
    w.write_u8(item.custom_type2 as u8);
    w.write_i32(item.mana_left);
    w.write_i32(item.time);
    w.write_u8(1); // available
}

// ---- plain packets ----

/// `ItemList` (0x11). Quest items are filtered out (none exist yet).
pub fn item_list(inventory: &crate::model::inventory::Inventory, data: &GameData) -> Vec<u8> {
    let entries: Vec<_> = inventory
        .items()
        .iter()
        .filter_map(|item| data.item_data.get(item.item_id).map(|t| (item, t)))
        .filter(|(_, t)| !t.is_quest_item)
        .collect();

    let mut w = PacketWriter::new();
    w.write_u8(0x11);
    w.write_i16(0); // show window (false)
    w.write_i16(entries.len() as i16);
    for (item, template) in &entries {
        let equipped = inventory.paperdoll_slot_of(item.object_id).is_some();
        write_item_entry(&mut w, item, template, equipped);
    }
    w.write_i16(0); // inventory block (none)
    w.into_bytes()
}

/// `InventoryUpdate` (0x21). `change=2` (modify) for every entry: equip/unequip
/// only moves an existing `Item` between `INVENTORY`/`PAPERDOLL`, it never
/// creates or destroys the object (matches Java's `addItems`/plain `ItemInfo`).
pub fn inventory_update(
    inventory: &crate::model::inventory::Inventory,
    data: &GameData,
    changed_object_ids: &[i32],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x21);
    w.write_i16(changed_object_ids.len() as i16);
    for &object_id in changed_object_ids {
        let Some(item) = inventory.items().iter().find(|i| i.object_id == object_id) else { continue };
        let Some(template) = data.item_data.get(item.item_id) else { continue };
        let equipped = inventory.paperdoll_slot_of(object_id).is_some();
        w.write_i16(2); // change type: modify
        write_item_entry(&mut w, item, template, equipped);
    }
    w.into_bytes()
}

/// `InventoryUpdate` (0x21) from explicit [`ItemChange`]s — the shape quest
/// `takeItems` needs: modified stacks write their new count, removed
/// instances write change type 3 from the final snapshot (`remove_item`
/// returns it; the instance no longer exists in the inventory).
pub fn inventory_update_changes(
    data: &GameData,
    changes: &[crate::model::inventory::ItemChange],
) -> Vec<u8> {
    use crate::model::inventory::ItemChange;
    let mut w = PacketWriter::new();
    w.write_u8(0x21);
    w.write_i16(changes.len() as i16);
    for change in changes {
        let (kind, item) = match change {
            ItemChange::Modified(item) => (2i16, item),
            ItemChange::Removed(item) => (3i16, item),
        };
        let Some(template) = data.item_data.get(item.item_id) else { continue };
        w.write_i16(kind);
        write_item_entry(&mut w, item, template, false);
    }
    w.into_bytes()
}

/// `SkillList` (0x5F), one entry per known skill (Java `Player.sendSkillList`
/// via `SkillList.addSkill`): passive flag, level, sub-level, id, reuse-delay
/// group (`Skill.reuseDelayGroup`, -1 when ungrouped), disabled (clan
/// reputation gate — always false, no clans yet), enchanted (always false).
pub fn skill_list(skills: &crate::model::components::SkillBook, data: &GameData) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x5F);
    w.write_i32(skills.0.len() as i32);
    for (&skill_id, &level) in &skills.0 {
        let skill = data.skill_data.get(skill_id, level);
        let passive = skill.is_some_and(|s| s.operate_type == crate::model::skill::OperateType::Passive);
        w.write_i32(passive as i32);
        w.write_i16(level as i16);
        w.write_i16(0); // sub-level
        w.write_i32(skill_id);
        w.write_i32(skill.map_or(-1, |s| s.reuse_delay_group));
        w.write_u8(0); // disabled
        w.write_u8(0); // enchanted
    }
    w.write_i32(-1); // last learned skill id (none new this burst)
    w.into_bytes()
}

/// `AcquireSkillList` (0x90) — the class skills the player can currently
/// learn (Java `SkillTreeData.getAvailableSkills`, `CLASS` type only — see
/// `data/skill_tree.rs::available_skills`). Base-class skills never carry
/// required items/remove-skills/dual-class gates (confirmed empty in
/// `StartingClass/*.xml`), so those lists are always written empty.
pub fn acquire_skill_list(p: &Player, skills: &crate::model::components::SkillBook, data: &GameData) -> Vec<u8> {
    let learnable = data.skill_trees.available_skills(p.class_id, p.level, &skills.0);
    let mut w = PacketWriter::new();
    w.write_u8(0x90);
    w.write_i16(learnable.len() as i16);
    for s in &learnable {
        w.write_i32(s.skill_id);
        w.write_i16(s.skill_level as i16);
        w.write_i64(s.level_up_sp);
        w.write_u8(s.get_level as u8);
        w.write_u8(0); // dual class level
        w.write_u8(0); // required item count
        w.write_u8(0); // remove-skill count
    }
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

/// `QuestList` (0x86): every STARTED quest as `(id, condBitSet)` plus the
/// 128-byte one-time mask of COMPLETED quests (Java packs `id % 10000` into
/// it, skipping the 256..10255 and >11023 id ranges that don't fit the
/// client's table).
pub fn quest_list(
    quests: &crate::model::components::Quests,
    registry: &crate::game_loop::quests::QuestRegistry,
) -> Vec<u8> {
    let mut active: Vec<(i32, i32)> = Vec::new();
    let mut one_time_mask = [0u8; 128];
    for (name, qs) in &quests.0 {
        let Some(quest_id) = registry.quest_id(name) else { continue };
        if quest_id <= 0 {
            continue;
        }
        if qs.is_started() {
            active.push((quest_id, qs.cond_bit_set()));
        } else if qs.is_completed() && !((quest_id > 255 && quest_id < 10256) || quest_id > 11023) {
            one_time_mask[(quest_id % 10000) as usize / 8] |= 1 << (quest_id % 8);
        }
    }
    let mut w = PacketWriter::new();
    w.write_u8(0x86);
    w.write_i16(active.len() as i16);
    for (id, cond) in active {
        w.write_i32(id);
        w.write_i32(cond);
    }
    w.write_bytes(&one_time_mask);
    w.into_bytes()
}

/// `AbnormalStatusUpdate` (0x85): one entry per active buff (Java
/// `BuffInfo` list) — display id/level/sub-level, `AbnormalType` client id,
/// remaining seconds. `now_tick` is `world.tick` (10 ticks/s) so the
/// remaining time can be derived from each buff's `expires_at_tick`.
pub fn abnormal_status_update(buffs: &crate::model::components::Buffs, now_tick: u64) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x85);
    w.write_i16(buffs.0.len() as i16);
    for buff in &buffs.0 {
        let remaining_secs = buff.expires_at_tick.saturating_sub(now_tick) / 10;
        w.write_i32(buff.skill_id);
        w.write_i16(buff.skill_level as i16);
        w.write_i16(0); // sub-level
        w.write_i32(buff.abnormal_type_client_id);
        w.write_i16(remaining_secs as i16);
    }
    w.into_bytes()
}

/// `MoveToLocation` (0x2F): destination == current position (Java sends this on
/// enter so the client fixes the character's position).
pub fn move_to_location(object_id: i32, pos: &crate::model::components::Position) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x2F);
    w.write_i32(object_id);
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
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

/// `ExQuestItemList` (0xC7) — the quest-inventory tab: the `is_quest_item`
/// complement of `item_list`.
pub fn ex_quest_item_list(inventory: &crate::model::inventory::Inventory, data: &GameData) -> Vec<u8> {
    let entries: Vec<_> = inventory
        .items()
        .iter()
        .filter_map(|item| data.item_data.get(item.item_id).map(|t| (item, t)))
        .filter(|(_, t)| t.is_quest_item)
        .collect();
    let mut w = ex(0xC7);
    w.write_i16(entries.len() as i16);
    for (item, template) in &entries {
        write_item_entry(&mut w, item, template, false);
    }
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

/// `ExUserInfoInvenWeight` (0x166). Max load stays a placeholder — encumbrance
/// enforcement is out of scope.
pub fn ex_user_info_inven_weight(
    object_id: i32,
    inventory: &crate::model::inventory::Inventory,
    data: &GameData,
) -> Vec<u8> {
    let load: i64 = inventory
        .items()
        .iter()
        .map(|item| data.item_data.get(item.item_id).map_or(0, |t| t.weight as i64 * item.count))
        .sum();
    let mut w = ex(0x166);
    w.write_i32(object_id);
    w.write_i32(load as i32);
    w.write_i32(80_000); // max load (placeholder)
    w.into_bytes()
}

/// `ExAdenaInvenCount` (0x13E).
pub fn ex_adena_inven_count(inventory: &crate::model::inventory::Inventory) -> Vec<u8> {
    let mut w = ex(0x13E);
    w.write_i64(inventory.adena());
    w.write_i16(inventory.items().len() as i16);
    w.into_bytes()
}

/// `ExUserInfoEquipSlot` (0x156) — masked, all 33 `InventorySlot` components,
/// values read from the real paperdoll.
pub fn ex_user_info_equip_slot(object_id: i32, inventory: &crate::model::inventory::Inventory) -> Vec<u8> {
    let mut w = ex(0x156);
    w.write_i32(object_id);
    w.write_i16(InventorySlot::VALUES.len() as i16);
    w.write_bytes(&masks::build_mask::<5>(InventorySlot::VALUES.iter().map(|s| s.mask())));
    for slot in InventorySlot::VALUES {
        let pd = slot.slot();
        let augment = inventory.paperdoll_augmentation(pd);
        w.write_i16(22); // block length: 10 + 4 * 3
        w.write_i32(inventory.paperdoll_object_id(pd));
        w.write_i32(inventory.paperdoll_item_id(pd));
        w.write_i32(augment.map_or(0, |(opt1, _)| opt1));
        w.write_i32(augment.map_or(0, |(_, opt2)| opt2));
        w.write_i32(inventory.paperdoll_visual_id(pd));
    }
    w.into_bytes()
}

/// `ExPledgeWaitingListAlarm` (0x147) — no body.
pub fn ex_pledge_waiting_list_alarm() -> Vec<u8> {
    ex(0x147).into_bytes()
}

/// `ExRotation` (0xC2).
pub fn ex_rotation(object_id: i32, heading: i32) -> Vec<u8> {
    let mut w = ex(0xC2);
    w.write_i32(object_id);
    w.write_i32(heading);
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
