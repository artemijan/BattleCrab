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
pub(crate) fn write_item_entry(w: &mut PacketWriter, item: &ItemInstance, template: &ItemTemplate, equipped: bool) {
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

/// `ItemList` (0x11). Quest items are filtered out (none exist yet). `open`
/// is Java's `_showWindow`: the enter-world burst sends it false; a client
/// `RequestItemList` (inventory window opened) sends it true so the client
/// pops the inventory window.
pub fn item_list(inventory: &crate::model::inventory::Inventory, data: &GameData, open: bool) -> Vec<u8> {
    let entries: Vec<_> = inventory
        .items()
        .iter()
        .filter_map(|item| data.item_data.get(item.item_id).map(|t| (item, t)))
        .filter(|(_, t)| !t.is_quest_item)
        .collect();

    let mut w = PacketWriter::new();
    w.write_u8(0x11);
    w.write_i16(i16::from(open)); // show window
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

/// `EtcStatusUpdate` (0xF9). Charges/weight/death-penalty/souls are still 0
/// (not modeled yet); the weapon/armor grade-penalty bytes carry the levels
/// computed by `refresh_expertise_penalty`.
pub fn etc_status_update(weapon_grade_penalty: i32, armor_grade_penalty: i32, message_refusal: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0xF9);
    w.write_u8(0); // charges
    w.write_i32(0); // weight penalty
    w.write_u8(weapon_grade_penalty as u8); // weapon grade penalty [1-4]
    w.write_u8(armor_grade_penalty as u8); // armor grade penalty [1-4]
    w.write_u8(0); // death penalty
    w.write_u8(0); // charged souls
    // Mask (Java `EtcStatusUpdate._mask`): bit 0x01 = message-refusal / silence
    // / chat-ban, 0x02 = danger area, 0x04 = charm of courage. Only silence is
    // modeled; this is what draws the chat-block icon.
    w.write_u8(if message_refusal { 1 } else { 0 });
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
    // Passive stand-ins (grade penalties) drive stats but never show as an
    // abnormal icon — Java adds them via `addSkill`, not the effect list.
    let shown = buffs.0.iter().filter(|b| !b.passive);
    w.write_i16(shown.clone().count() as i16);
    for buff in shown {
        let remaining_secs = buff.expires_at_tick.saturating_sub(now_tick) / 10;
        w.write_i32(buff.skill_id);
        w.write_i16(buff.skill_level as i16);
        w.write_i16(0); // sub-level
        w.write_i32(buff.abnormal_type_client_id);
        w.write_i16(remaining_secs as i16);
    }
    w.into_bytes()
}

/// `ExAbnormalStatusUpdateFromTarget` (0xFE:0xE6): the buff/debuff row shown in
/// the *target window* — sent to every player who has `object_id` selected when
/// its effects change (Java `EffectList.updateEffectIcons` → the status
/// listeners). Toggles/passives are excluded, like the self bar. The effector
/// (caster) id is written as 0 — `ActiveBuff` doesn't track it, and it only
/// feeds the "cast by" tooltip.
pub fn ex_abnormal_status_update_from_target(
    object_id: i32,
    buffs: &crate::model::components::Buffs,
    now_tick: u64,
) -> Vec<u8> {
    let mut w = ex(0xE6);
    w.write_i32(object_id);
    let shown: Vec<_> = buffs.0.iter().filter(|b| !b.passive).collect();
    w.write_i16(shown.len() as i16);
    for buff in shown {
        let remaining_secs = (buff.expires_at_tick.saturating_sub(now_tick) / 10) as i32;
        w.write_i32(buff.skill_id); // displayId
        w.write_i16(buff.skill_level as i16); // displayLevel
        w.write_i16(0); // subLevel
        w.write_i16(buff.abnormal_type_client_id as i16); // abnormalType (short here)
        write_optional_int(&mut w, remaining_secs); // writeOptionalInt(time)
        w.write_i32(0); // effectorObjectId
    }
    w.into_bytes()
}

/// Java `ServerPacket.writeOptionalInt`: a value below `Short.MAX_VALUE` is one
/// short; otherwise a `Short.MAX_VALUE` marker short followed by the full int.
fn write_optional_int(w: &mut PacketWriter, value: i32) {
    if value >= i16::MAX as i32 {
        w.write_i16(i16::MAX);
        w.write_i32(value);
    } else {
        w.write_i16(value as i16);
    }
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

/// `ExStorageMaxCount` (0x2F) — the capacity figures the client's inventory
/// window reports as "X out of Y available"; without this packet the client
/// never learns a max and shows 0. `_inventory`/`_inventoryQuestItems` are
/// the two limits this port actually enforces (`Inventory::non_quest_size`/
/// `quest_size`); warehouse/freight/private-store/recipe slots come from
/// systems not implemented yet, so those fields carry Java's static config
/// defaults rather than anything computed.
pub fn ex_storage_max_count(race: i32, cfg: &crate::config::CharacterConfig) -> Vec<u8> {
    let is_dwarf = race == crate::enums::Race::Dwarf as i32;
    let mut w = ex(0x2F);
    w.write_i32(cfg.inventory_limit(race));
    w.write_i32(if is_dwarf { 120 } else { 100 }); // warehouse (unimplemented; Java defaults)
    w.write_i32(200); // freight (unimplemented; Java default)
    w.write_i32(150); // clan warehouse (unimplemented; Java default)
    w.write_i32(if is_dwarf { 4 } else { 3 }); // private sell (unimplemented; Java defaults)
    w.write_i32(if is_dwarf { 5 } else { 4 }); // private buy (unimplemented; Java defaults)
    w.write_i32(50); // dwarf recipe book (unimplemented; Java default)
    w.write_i32(50); // common recipe book (unimplemented; Java default)
    w.write_i32(0); // belt-granted extra inventory slots (Stat.INVENTORY_NORMAL not wired)
    w.write_i32(cfg.inventory_max_quest_items);
    w.write_i32(40);
    w.write_i32(40);
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
    // Java's `addAll=true` constructor: every slot component is in the mask.
    let masks = masks::build_mask::<5>(InventorySlot::VALUES.iter().map(|s| s.mask()));
    w.write_bytes(&masks);
    for slot in InventorySlot::VALUES {
        // Match Java `writeImpl`: only write a block for slots set in the mask,
        // so the body always follows the mask (no desync if the mask goes partial).
        if !masks::contains_mask(&masks, slot.mask()) {
            continue;
        }
        let pd = slot.slot();
        let augment = inventory.paperdoll_augmentation(pd);
        let object_id_val = inventory.paperdoll_object_id(pd);
        let item_id = inventory.paperdoll_item_id(pd);
        let visual_id = inventory.paperdoll_visual_id(pd);
        w.write_i16(22); // block length: 10 + 4 * 3
        w.write_i32(object_id_val);
        w.write_i32(item_id);
        w.write_i32(augment.map_or(0, |(opt1, _)| opt1));
        w.write_i32(augment.map_or(0, |(_, opt2)| opt2));
        w.write_i32(visual_id);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::item_data::{self, ItemData, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;

    fn earring(id: i32) -> ItemTemplate {
        ItemTemplate {
            item_id: id,
            name: format!("earring{id}"),
            kind: ItemKind::Armor,
            body_part: item_data::SLOT_L_EAR | item_data::SLOT_R_EAR,
            weight: 0,
            is_stackable: false,
            type1: 0,
            type2: 0,
            is_quest_item: false,
            price: 0,
            handler: item_data::ItemHandler::None,
            crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
        }
    }

    #[test]
    fn ex_user_info_equip_slot_reports_both_ear_slots() {
        let catalog = ItemData::from_templates(vec![earring(501), earring(502)]);
        let mut inv = Inventory::new();
        inv.add_item(&catalog, 100, 501, 1);
        inv.add_item(&catalog, 101, 502, 1);
        inv.equip_item(&catalog, 100);
        inv.equip_item(&catalog, 101);

        let bytes = ex_user_info_equip_slot(3001, &inv);
        // ex(0x156): 1 (EX) + 2 (sub) = 3; + 4 (object id) + 2 (slot count) + 5 (mask) = 14.
        let mut offset = 14usize;
        let mut found_rear = None;
        let mut found_lear = None;
        for slot in InventorySlot::VALUES {
            let block_len = i16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
            let obj_id = i32::from_le_bytes(bytes[offset + 2..offset + 6].try_into().unwrap());
            let item_id = i32::from_le_bytes(bytes[offset + 6..offset + 10].try_into().unwrap());
            match slot {
                InventorySlot::REar => found_rear = Some((obj_id, item_id)),
                InventorySlot::LEar => found_lear = Some((obj_id, item_id)),
                _ => {}
            }
            offset += block_len;
        }
        assert_eq!(offset, bytes.len(), "block lengths must account for every byte written");
        assert_eq!(found_lear, Some((100, 501)), "first earring fills LEar (equip_item fills left first)");
        assert_eq!(found_rear, Some((101, 502)), "second earring fills the free REar slot");
    }

    // ---- Java ground-truth golden (jewelry-in-inventory differential) ----
    //
    // Produced by `tests/java_golden/EquipMaskDump.java` (raw output kept in
    // `tests/java_golden/equip_dump.json`), which runs against the real
    // `InventorySlot` enum, `Inventory.PAPERDOLL_*` layout, and `ItemData.SLOTS`
    // bodypart table in the interlude_classic reference. Regenerate from that
    // repo with:
    //   javac -cp target/classes -d out crates/.../EquipMaskDump.java
    //   java  -cp target/classes:out EquipMaskDump
    // If any assertion here fails, the Rust port diverged from the Java client
    // wire format for equipped items (the reported jewelry display bug).

    /// The ExUserInfoEquipSlot mask when every one of the 33 components is set.
    const JAVA_MASK_ALL_SLOTS: [u8; 5] = [0xff, 0xff, 0xff, 0xff, 0x80];

    /// `(InventorySlot name, mask bit, backing paperdoll index)` in wire order.
    const JAVA_SLOTS: [(&str, usize, usize); 33] = [
        ("UNDER", 0, 0), ("REAR", 1, 8), ("LEAR", 2, 9), ("NECK", 3, 4),
        ("RFINGER", 4, 13), ("LFINGER", 5, 14), ("HEAD", 6, 1), ("RHAND", 7, 5),
        ("LHAND", 8, 7), ("GLOVES", 9, 10), ("CHEST", 10, 6), ("LEGS", 11, 11),
        ("FEET", 12, 12), ("CLOAK", 13, 23), ("LRHAND", 14, 5), ("HAIR", 15, 2),
        ("HAIR2", 16, 3), ("RBRACELET", 17, 16), ("LBRACELET", 18, 15),
        ("DECO1", 19, 17), ("DECO2", 20, 18), ("DECO3", 21, 19), ("DECO4", 22, 20),
        ("DECO5", 23, 21), ("DECO6", 24, 22), ("BELT", 25, 24), ("BROOCH", 26, 25),
        ("BROOCH_JEWEL", 27, 26), ("BROOCH_JEWEL2", 28, 27), ("BROOCH_JEWEL3", 29, 28),
        ("BROOCH_JEWEL4", 30, 29), ("BROOCH_JEWEL5", 31, 30), ("BROOCH_JEWEL6", 32, 31),
    ];

    /// `(object_id, item_id)` block each component reports when paperdoll slot
    /// `i` holds `(1000 + i, 2000 + i)`. RHAND and LRHAND repeat paperdoll 5.
    const JAVA_EQUIP_BLOCKS: [(i32, i32); 33] = [
        (1000, 2000), (1008, 2008), (1009, 2009), (1004, 2004), (1013, 2013),
        (1014, 2014), (1001, 2001), (1005, 2005), (1007, 2007), (1010, 2010),
        (1006, 2006), (1011, 2011), (1012, 2012), (1023, 2023), (1005, 2005),
        (1002, 2002), (1003, 2003), (1016, 2016), (1015, 2015), (1017, 2017),
        (1018, 2018), (1019, 2019), (1020, 2020), (1021, 2021), (1022, 2022),
        (1024, 2024), (1025, 2025), (1026, 2026), (1027, 2027), (1028, 2028),
        (1029, 2029), (1030, 2030), (1031, 2031),
    ];

    fn paperdoll_row(paperdoll_slot: i32, object_id: i32, item_id: i32) -> crate::character::ItemRow {
        crate::character::ItemRow {
            object_id,
            item_id,
            count: 1,
            enchant_level: 0,
            loc: "PAPERDOLL".to_string(),
            loc_data: paperdoll_slot,
            custom_type1: 0,
            custom_type2: 0,
            mana_left: -1,
            time: 0,
        }
    }

    #[test]
    fn inventory_slot_order_matches_java() {
        // Wire order, mask bit (= ordinal), and backing paperdoll index must all
        // line up with the Java enum, or jewelry lands in the wrong slot.
        assert_eq!(InventorySlot::VALUES.len(), JAVA_SLOTS.len());
        for (slot, &(name, bit, paperdoll)) in InventorySlot::VALUES.iter().zip(JAVA_SLOTS.iter()) {
            assert_eq!(slot.mask(), bit, "{name}: mask bit");
            assert_eq!(slot.slot() as usize, paperdoll, "{name}: backing paperdoll slot");
        }
        assert_eq!(
            masks::build_mask::<5>(InventorySlot::VALUES.iter().map(|s| s.mask())),
            JAVA_MASK_ALL_SLOTS,
            "all-slots mask bytes"
        );
    }

    #[test]
    fn ex_user_info_equip_slot_matches_java_golden() {
        // Fill every paperdoll slot with (1000+i, 2000+i) and byte-compare the
        // produced ExUserInfoEquipSlot against the Java dump.
        let rows: Vec<_> = (0..crate::model::inventory::PAPERDOLL_TOTAL_SLOTS as i32)
            .map(|i| paperdoll_row(i, 1000 + i, 2000 + i))
            .collect();
        let inv = Inventory::from_rows(&rows);

        let bytes = ex_user_info_equip_slot(3001, &inv);

        // Header: 1 (EX) + 2 (sub) + 4 (object id) + 2 (component count).
        assert_eq!(i16::from_le_bytes([bytes[7], bytes[8]]), 33, "component count");
        // Mask bytes.
        assert_eq!(&bytes[9..14], &JAVA_MASK_ALL_SLOTS, "mask bytes");

        // 33 blocks of 22 bytes: len(2) + obj(4) + item(4) + aug1(4) + aug2(4) + visual(4).
        let mut offset = 14usize;
        for (i, &(exp_obj, exp_item)) in JAVA_EQUIP_BLOCKS.iter().enumerate() {
            let block_len = i16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
            let obj = i32::from_le_bytes(bytes[offset + 2..offset + 6].try_into().unwrap());
            let item = i32::from_le_bytes(bytes[offset + 6..offset + 10].try_into().unwrap());
            assert_eq!(block_len, 22, "block {i} length");
            assert_eq!((obj, item), (exp_obj, exp_item), "block {i} ({})", JAVA_SLOTS[i].0);
            offset += block_len as usize;
        }
        assert_eq!(offset, bytes.len(), "no trailing bytes");
    }

    #[test]
    fn jewelry_item_list_slot_field_matches_java_golden() {
        // AbstractItemPacket.writeItem writes `getBodyPart()` as the item's
        // "Slot" (the field the inventory window reads to place jewelry). Golden
        // bitmasks come straight from the Java ItemData.SLOTS table.
        let cases: [(&str, i32, i32, i32); 3] = [
            // (label, body_part, expected slot long, expected type2)
            ("earring", item_data::SLOT_LR_EAR, 6, 2),
            ("ring", item_data::SLOT_LR_FINGER, 48, 2),
            ("necklace", item_data::SLOT_NECK, 8, 2),
        ];
        for (label, body_part, exp_slot, exp_type2) in cases {
            let mut t = earring(9000);
            t.body_part = body_part;
            t.type2 = exp_type2;
            let item = crate::model::inventory::ItemInstance {
                object_id: 5000,
                item_id: 9000,
                count: 1,
                enchant_level: 0,
                custom_type1: 0,
                custom_type2: 0,
                mana_left: -1,
                time: 0,
                augment_mineral: 0,
                augment_option1: 0,
                augment_option2: 0,
            };
            let mut w = PacketWriter::new();
            write_item_entry(&mut w, &item, &t, true);
            let bytes = w.into_bytes();
            // Layout (byte offsets): mask@0(1) obj@1(4) item@5(4) T1@9(1)
            // count@10(8) type2@18(1) ct1@19(1) equipped@20(2) bodypart@22(8) ...
            let type2 = bytes[18];
            let slot = i64::from_le_bytes(bytes[22..30].try_into().unwrap());
            assert_eq!(type2 as i32, exp_type2, "{label} type2");
            assert_eq!(slot as i32, exp_slot, "{label} body-part slot field");
        }
    }
}
