//! Soul crystal abilities on an item — Java `Item.restoreSpecialAbilities`,
//! `applySpecialAbilities` and `clearSpecialAbilities`.
//!
//! An ensouled weapon carries up to two regular options and one special one
//! ([`crate::data::ensoul_data`]). Each option is nothing but a skill, granted
//! with `addSkill(skill, false)` while the weapon is equipped — `store = false`,
//! so it never reaches `character_skills`. Here they go into the transient
//! [`OptionSkills`] next to the augment actives, which is also what lists them
//! in `SkillList`. All of them are passives; their stats come through the
//! conditioned passive pump
//! ([`crate::game_loop::stats::passive_skills::recompute_conditioned_passives`]),
//! which reads `OptionSkills` as well as the book, so an option whose effects
//! carry a weapon-type or HP condition switches on and off like any learned
//! passive.

use commons::network::PacketReader;
use tracing::warn;

use crate::data::GameData;
use crate::data::item_data::kinds::ItemKind;
use crate::db::ItemRow;
use crate::game_loop::admin::refresh_skill_list;
use crate::game_loop::character::inventory::{self, item_id_of};
use crate::game_loop::character::sit_stand::is_sitting;
use crate::game_loop::combat::has_attack_stance;
use crate::game_loop::helpers::{
    is_dead, player_of, send_sm_bare_to_client, send_sm_to_client, send_to_client,
};
use crate::game_loop::items::{self, item_mana};
use crate::game_loop::stats::passive_skills::refresh_conditioned_passives;
use crate::model::components::commerce::{EnchantRequest, Trade};
use crate::model::components::player::FishingSession;
use crate::model::components::skills::{Buffs, OptionSkills, SkillBook};
use crate::model::inventory::{Inventory, ItemChange, ItemInstance};
use crate::network::server_packets::{self as sp, SmParam, sm_ids};
use crate::world::World;

/// Java `Item.restoreSpecialAbilities` — the data half of the load.
///
/// `load_items` has already put each `item_special_abilities` row in its slot;
/// this drops what Java's restore would not have filed: everything on an item
/// that is not equipable (Java only restores for `isEquipable()`), an option id
/// `EnsoulData` does not know, and a position past the template's slot count
/// (`addSpecialAbility`'s `position < getEnsoulSlots()`).
///
/// Java leaves the rejected rows in the table until the next
/// `updateSpecialAbilities` rewrites them; the port's store rewrites an item's
/// rows on every flush, so they go at the first one.
pub(crate) fn restore(data: &GameData, row: &mut ItemRow) {
    let Some(template) = data.item_data.get(row.item_id) else {
        return;
    };
    let keeps = |option_id: i32, position: i32, slots: i32| {
        option_id != 0 && data.ensoul.option(option_id).is_some() && position < slots
    };
    let equipable = template.body_part != crate::data::item_data::SLOT_NONE;
    for (position, option_id) in row.ensoul_options.iter_mut().enumerate() {
        if !equipable || !keeps(*option_id, position as i32, template.ensoul_normal_slots) {
            *option_id = 0;
        }
    }
    if !equipable || !keeps(row.ensoul_special_option, 0, template.ensoul_special_slots) {
        row.ensoul_special_option = 0;
    }
}

/// Every option id on an instance, regular slots first, zeroes skipped — Java's
/// walk of `_ensoulOptions` then `_ensoulSpecialOptions`.
pub(crate) fn option_ids(item: &ItemInstance) -> Vec<i32> {
    item.ensoul_options
        .iter()
        .copied()
        .chain(std::iter::once(item.ensoul_special_option))
        .filter(|&id| id != 0)
        .collect()
}

/// The `(skill_id, level)` an option grants — Java `EnsoulOption.getSkill()`,
/// which is `null` when the skill is not in `SkillData` (option 999's skill on
/// this dist).
fn option_skill(data: &GameData, option_id: i32) -> Option<(i32, i32)> {
    let option = data.ensoul.option(option_id)?;
    data.skill_data
        .get(option.skill_id, option.skill_level)
        .map(|_| (option.skill_id, option.skill_level))
}

/// Java `Item.applySpecialAbilities` for an item in `player_oid`'s bag. No-op
/// unless it is equipped (Java's own `isEquipped()` guard).
///
/// Each skill is granted unless the player already has it at that level
/// (`getSkillLevel(id) != level`). Stats follow on the caller's next passive
/// pump.
pub(crate) fn apply_item(world: &mut World, player_oid: i32, item_oid: i32) {
    let options = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .filter(|inv| inv.paperdoll_slot_of(item_oid).is_some())
        .and_then(|inv| inv.by_object_id(item_oid))
        .map(option_ids)
        .unwrap_or_default();
    let skills: Vec<(i32, i32)> = options
        .iter()
        .filter_map(|&id| option_skill(&world.data, id))
        .collect();
    if skills.is_empty() {
        return;
    }
    let mut changed = false;
    for (skill_id, level) in skills {
        let known = world
            .objects
            .get_component::<SkillBook>(&player_oid)
            .and_then(|b| b.0.get(&skill_id).copied())
            .or_else(|| {
                world
                    .objects
                    .get_component::<OptionSkills>(&player_oid)
                    .and_then(|o| o.0.get(&skill_id).copied())
            });
        if known == Some(level) {
            continue;
        }
        if let Some(o) = world.objects.get_component_mut::<OptionSkills>(&player_oid) {
            o.0.insert(skill_id, level);
            changed = true;
        }
    }
    if changed {
        refresh_skill_list(world, player_oid);
    }
}

/// Java `PlayerInventory.applyItemSkills`' ensoul half at enter-world: every
/// equipped item's abilities, **without** the expertise check the equip
/// listener makes — an over-grade ensouled weapon worn through a relog gets its
/// skills back, which re-equipping it would not.
pub(crate) fn apply_equipped(world: &mut World, player_oid: i32) {
    let equipped = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .map(|inv| inv.equipped_object_ids())
        .unwrap_or_default();
    for item_oid in equipped {
        apply_item(world, player_oid, item_oid);
    }
}

/// Java `Item.clearSpecialAbilities` — `removeSkill(skill, false, true)` for
/// each option, called from the unequip listener whatever the grade.
///
/// Takes the option ids rather than an object id so a destroyed item can pass
/// them from its snapshot. The skill's pump buff is dropped here too: the
/// passive pump only manages buffs whose skill is still known, so once the
/// skill leaves [`OptionSkills`] it would never take the buff off.
pub(crate) fn clear_options(world: &mut World, player_oid: i32, option_ids: &[i32]) {
    let skill_ids: Vec<i32> = option_ids
        .iter()
        .filter_map(|&id| option_skill(&world.data, id))
        .map(|(skill_id, _)| skill_id)
        .collect();
    if skill_ids.is_empty() {
        return;
    }
    if let Some(o) = world.objects.get_component_mut::<OptionSkills>(&player_oid) {
        for skill_id in &skill_ids {
            o.0.remove(skill_id);
        }
    }
    crate::game_loop::stats::context::with_stat_ctx(world, player_oid, |ctx| {
        for &skill_id in &skill_ids {
            ctx.remove(skill_id);
        }
    });
    crate::game_loop::skills::effects::recompute_max_vitals(world, player_oid);
    refresh_skill_list(world, player_oid);
}

/// [`clear_options`] for an item still in the bag.
pub(crate) fn clear_item(world: &mut World, player_oid: i32, item_oid: i32) {
    let options = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| inv.by_object_id(item_oid))
        .map(option_ids)
        .unwrap_or_default();
    clear_options(world, player_oid, &options);
}

/// Java `Item.addSpecialAbility(option, position, type, true)`.
///
/// A `position` outside the slot type's range is ignored. Any option already in
/// the slot loses its skill first (`removeSpecialAbility`, whether or not the
/// item is worn). The new option is filed only when `position` is under the
/// template's slot count; nothing re-grants skills here, that is the caller's
/// `applySpecialAbilities`.
pub(crate) fn add_special_ability(
    world: &mut World,
    player_oid: i32,
    item_oid: i32,
    option_id: i32,
    position: usize,
    slot_type: i32,
) {
    match slot_type {
        1 if position <= 1 => {}
        2 if position == 0 => {}
        _ => return,
    }
    let Some((regular, special, item_id)) = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| inv.by_object_id(item_oid))
        .map(|it| (it.ensoul_options, it.ensoul_special_option, it.item_id))
    else {
        return;
    };
    let Some((normal_slots, special_slots)) = world
        .data
        .item_data
        .get(item_id)
        .map(|t| (t.ensoul_normal_slots, t.ensoul_special_slots))
    else {
        return;
    };
    let old = if slot_type == 1 {
        regular[position]
    } else {
        special
    };
    if old != 0 {
        clear_options(world, player_oid, &[old]);
    }
    let (mut regular, mut special) = (regular, special);
    if slot_type == 1 && (position as i32) < normal_slots {
        regular[position] = option_id;
    } else if slot_type == 2 && (position as i32) < special_slots {
        special = option_id;
    }
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player_oid) {
        inv.set_ensoul_options(item_oid, regular, special);
    }
}

/// Java `Item.removeSpecialAbility(position, type)` — empty one slot, taking
/// its skill off the owner.
///
/// Emptying regular slot 0 moves slot 1 down. Java does that by removing slot
/// 1's skill and re-adding the option at 0 through `addSpecialAbility`, which
/// never re-grants anything, and the extraction handler doesn't call
/// `applySpecialAbilities` either. **Kept:** on a worn weapon, the option that
/// moved down shows in its slot but grants nothing until the weapon is
/// re-equipped or the character relogs.
fn remove_special_ability(
    world: &mut World,
    player_oid: i32,
    item_oid: i32,
    position: usize,
    slot_type: i32,
) {
    let Some((regular, special)) = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| inv.by_object_id(item_oid))
        .map(|it| (it.ensoul_options, it.ensoul_special_option))
    else {
        return;
    };
    let set = |world: &mut World, regular: [i32; 2], special: i32| {
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player_oid) {
            inv.set_ensoul_options(item_oid, regular, special);
        }
    };
    if slot_type == 1 {
        let option = regular[position];
        if option == 0 {
            return;
        }
        clear_options(world, player_oid, &[option]);
        let mut regular = regular;
        regular[position] = 0;
        set(world, regular, special);
        if position == 0 && regular[1] != 0 {
            let second = regular[1];
            clear_options(world, player_oid, &[second]);
            regular[1] = 0;
            set(world, regular, special);
            add_special_ability(world, player_oid, item_oid, second, 0, slot_type);
        }
    } else if slot_type == 2 && special != 0 {
        clear_options(world, player_oid, &[special]);
        set(world, regular, 0);
    }
}

/// The two lists `ExEnsoulResult` / `ExEnSoulExtractionResult` carry — Java's
/// `getSpecialAbilities()` and `getAdditionalSpecialAbilities()`, the non-empty
/// slots in order.
fn option_lists(world: &World, player_oid: i32, item_oid: i32) -> (Vec<i32>, Vec<i32>) {
    world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| inv.by_object_id(item_oid))
        .map(|it| {
            (
                it.ensoul_options
                    .iter()
                    .copied()
                    .filter(|&id| id != 0)
                    .collect(),
                [it.ensoul_special_option]
                    .into_iter()
                    .filter(|&id| id != 0)
                    .collect(),
            )
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `EnsoulWindow` bypass: `show_ensoul_window` / `show_extract_ensoul_window`.
pub(crate) fn open_window(world: &mut World, client_id: u32, extraction: bool) {
    let packet = if extraction {
        sp::ex_show_ensoul_extraction_window()
    } else {
        sp::ex_show_ensoul_window()
    };
    send_to_client(world, client_id, packet);
}

/// One `RequestItemEnsoul` entry. `None` stands for an entry Java leaves
/// `null`: a position outside 1-2 or a type other than 1/2.
struct EnsoulEntry {
    slot_type: i32,
    /// The client's 1-based position.
    position: i32,
    crystal_oid: i32,
    option_id: i32,
}

/// `RequestItemEnsoul.readImpl`. Returns the item, Java's `_type` (the type
/// byte of the **last** entry read, valid or not) and the entries — `None`
/// when the count byte is outside 1-3, which Java leaves as a `null` array.
#[allow(clippy::type_complexity)]
fn read_item_ensoul(body: &[u8]) -> Option<(i32, i32, Option<Vec<Option<EnsoulEntry>>>)> {
    let mut r = PacketReader::new(body);
    let item_oid = r.read_i32()?;
    let count = r.read_u8()? as i8 as i32;
    let mut last_type = 0;
    if !(1..=3).contains(&count) {
        return Some((item_oid, last_type, None));
    }
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        last_type = r.read_u8()? as i8 as i32;
        let position = r.read_u8()? as i8 as i32;
        let crystal_oid = r.read_i32()?;
        let option_id = r.read_i32()?;
        entries.push(
            ((1..3).contains(&position) && (last_type == 1 || last_type == 2)).then_some(
                EnsoulEntry {
                    slot_type: last_type,
                    position,
                    crystal_oid,
                    option_id,
                },
            ),
        );
    }
    Some((item_oid, last_type, Some(entries)))
}

fn has_abnormal_type(world: &World, object_id: i32, abnormal_type: &str) -> bool {
    world
        .objects
        .get_component::<Buffs>(&object_id)
        .is_some_and(|b| b.0.iter().any(|x| x.abnormal_type == abnormal_type))
}

/// Java `Player.destroyItem(…, item, count, …, sendMessage = true)`: its own
/// `InventoryUpdate`, then the disappear message. Returns the change, or
/// `None` when nothing was taken (Java's `INCORRECT_ITEM_COUNT_2` branch).
fn destroy_with_message(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    item_oid: i32,
    count: i64,
) -> Option<crate::model::inventory::ItemChange> {
    let item_id = item_id_of(world, player_oid, item_oid)?;
    let Some(change) = items::destroy_item_by_object_id(world, player_oid, item_oid, count) else {
        send_sm_bare_to_client(world, client_id, sm_ids::INCORRECT_ITEM_COUNT_2);
        return None;
    };
    inventory::send_inventory_update(world, player_oid, vec![change]);
    send_disappeared(world, client_id, item_id, count);
    Some(change)
}

fn send_disappeared(world: &World, client_id: u32, item_id: i32, count: i64) {
    if count > 1 {
        send_sm_to_client(
            world,
            client_id,
            sm_ids::S2_S1_S_DISAPPEARED,
            &[SmParam::ItemName(item_id), SmParam::Long(count)],
        );
    } else {
        send_sm_to_client(
            world,
            client_id,
            sm_ids::S1_DISAPPEARED,
            &[SmParam::ItemName(item_id)],
        );
    }
}

/// `RequestItemEnsoul` (0xD0 0x108) — put soul crystals into a weapon.
///
/// Java's own quirks, kept:
/// * Frozen sends its refusal and **carries on**: the `FREEZING` check has no
///   `return`.
/// * The slot-count gate reads `_type`, the type of the last entry read.
/// * An entry Java left `null` throws at `itemOption.getPosition()`. Whatever
///   earlier entries already did stays done, and nothing after it is sent: no
///   `InventoryUpdate`, no re-grant, no result.
/// * A mystic entry at client position 2 indexes fee slot 3 and throws the
///   same way, unless the grade has no fee block at all (then it is just an
///   unknown fee).
/// * The fee follows the entry's type, but the slot it fills follows the
///   stone's `slotType`, so a mismatched stone is paid for and then ignored by
///   `addSpecialAbility`'s range check, still counting as a success.
/// * The gemstones are the first stack of the fee's id
///   (`getItemByItemId`), not the sum.
/// * The re-grant on a worn weapon has no expertise check.
pub(crate) fn handle_request_item_ensoul(world: &mut World, client_id: u32, body: &[u8]) {
    let Some((item_oid, last_type, entries)) = read_item_ensoul(body) else {
        return;
    };
    let Some(player_oid) = player_of(world, client_id) else {
        return;
    };
    let refuse = |world: &World, sm: i16| send_sm_bare_to_client(world, client_id, sm);
    let in_store = world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .is_some_and(|p| p.store_type != 0);
    if in_store {
        return refuse(
            world,
            sm_ids::CANNOT_USE_THE_SOUL_CRYSTAL_SYSTEM_WHILE_USING_THE_PRIVATE_STORE_WORKSHOP,
        );
    }
    if has_abnormal_type(world, player_oid, "FREEZING") {
        refuse(
            world,
            sm_ids::CANNOT_USE_THE_SOUL_CRYSTAL_SYSTEM_WHILE_FROZEN,
        );
    }
    if is_dead(world, player_oid) {
        return refuse(world, sm_ids::CANNOT_USE_THE_SOUL_CRYSTAL_SYSTEM_WHILE_DEAD);
    }
    if world.objects.has_component::<Trade>(&player_oid)
        || world.objects.has_component::<EnchantRequest>(&player_oid)
    {
        return refuse(
            world,
            sm_ids::CANNOT_USE_THE_SOUL_CRYSTAL_SYSTEM_WHILE_TRADING,
        );
    }
    if has_abnormal_type(world, player_oid, "PARALYZE") {
        return refuse(
            world,
            sm_ids::CANNOT_USE_THE_SOUL_CRYSTAL_SYSTEM_WHILE_PETRIFIED,
        );
    }
    if world
        .objects
        .get_component::<FishingSession>(&player_oid)
        .is_some_and(|f| f.is_fishing)
    {
        return refuse(
            world,
            sm_ids::CANNOT_USE_THE_SOUL_CRYSTAL_SYSTEM_WHILE_FISHING,
        );
    }
    if is_sitting(world, player_oid) {
        return refuse(
            world,
            sm_ids::CANNOT_USE_THE_SOUL_CRYSTAL_SYSTEM_WHILE_SEATED,
        );
    }
    if has_attack_stance(world, player_oid) {
        return refuse(
            world,
            sm_ids::CANNOT_USE_THE_SOUL_CRYSTAL_SYSTEM_WHILE_IN_BATTLE,
        );
    }

    let Some(item) = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| inv.by_object_id(item_oid))
        .copied()
    else {
        warn!("Player: {player_oid} attempting to ensoul item without having it!");
        return;
    };
    let Some(template) = world.data.item_data.get(item.item_id) else {
        return;
    };
    let refusal = if last_type == 1 && template.ensoul_normal_slots == 0 {
        Some("non ensoulable item")
    } else if last_type == 2 && template.ensoul_special_slots == 0 {
        Some("non special ensoulable item")
    } else if template.body_part == crate::data::item_data::SLOT_NONE {
        Some("non equippable item")
    } else if template.kind != ItemKind::Weapon {
        Some("item that's not a weapon")
    } else if template.is_common() {
        Some("common item")
    } else if item_mana::is_shadow_item(item.mana_left) {
        Some("shadow item")
    } else if template.is_hero_item() {
        Some("hero item")
    } else {
        None
    };
    if let Some(what) = refusal {
        warn!(
            "Player: {player_oid} attempting to ensoul {what}: {}!",
            item.item_id
        );
        return;
    }
    let grade = template.crystal_type;
    let Some(entries) = entries.filter(|e| !e.is_empty()) else {
        warn!(
            "Player: {player_oid} attempting to ensoul item without any special ability declared!"
        );
        return;
    };

    let mut success = false;
    let mut changes = Vec::new();
    for entry in entries {
        let Some(entry) = entry else {
            return;
        };
        let position = (entry.position - 1) as usize;
        let Some(crystal) = world
            .objects
            .get_component::<Inventory>(&player_oid)
            .and_then(|inv| inv.by_object_id(entry.crystal_oid))
            .copied()
        else {
            refuse(world, sm_ids::INVALID_SOUL_CRYSTAL);
            continue;
        };
        let Some(stone) = world.data.ensoul.stone(crystal.item_id) else {
            continue;
        };
        let stone_slot_type = stone.slot_type;
        if !stone.options.contains(&entry.option_id) {
            warn!(
                "Player: {player_oid} attempting to ensoul item option that stone doesn't contains!"
            );
            continue;
        }
        if world.data.ensoul.option(entry.option_id).is_none() {
            warn!("Player: {player_oid} attempting to ensoul item option that doesn't exist!");
            continue;
        }
        let (regular, special) = world
            .objects
            .get_component::<Inventory>(&player_oid)
            .and_then(|inv| inv.by_object_id(item_oid))
            .map_or(([0; 2], 0), |it| {
                (it.ensoul_options, it.ensoul_special_option)
            });
        let ensoul = &world.data.ensoul;
        let fee = if entry.slot_type == 1 {
            if regular[position] != 0 {
                ensoul.resoul_fee(grade, position)
            } else {
                ensoul.ensoul_fee(grade, position)
            }
        } else {
            // `_ensoulFee[3]` on a grade that has a block.
            if position + 2 > 2 && ensoul.has_fee_block(grade) {
                return;
            }
            if entry.position == 1 && special != 0 {
                ensoul.resoul_fee(grade, position + 2)
            } else {
                ensoul.ensoul_fee(grade, position + 2)
            }
        };
        let Some((fee_id, fee_count)) = fee else {
            warn!(
                "Player: {player_oid} attempting to ensoul item option that doesn't exist! (unknown fee)"
            );
            continue;
        };
        let Some(gemstones) = world
            .objects
            .get_component::<Inventory>(&player_oid)
            .and_then(|inv| inv.first_of_item(fee_id))
            .copied()
        else {
            continue;
        };
        if gemstones.count < fee_count {
            continue;
        }

        let crystal_change =
            destroy_with_message(world, client_id, player_oid, crystal.object_id, 1);
        let gemstone_change = crystal_change.and_then(|_| {
            destroy_with_message(world, client_id, player_oid, gemstones.object_id, fee_count)
        });
        if gemstone_change.is_some() {
            add_special_ability(
                world,
                player_oid,
                item_oid,
                entry.option_id,
                position,
                stone_slot_type,
            );
            success = true;
        }
        changes.push(crystal_change.unwrap_or(ItemChange::Modified(crystal)));
        changes.push(gemstone_change.unwrap_or(ItemChange::Modified(gemstones)));
        changes.extend(inventory::modified_changes(world, player_oid, &[item_oid]));
    }

    inventory::send_inventory_update(world, player_oid, changes);
    let worn = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .is_some_and(|inv| inv.paperdoll_slot_of(item_oid).is_some());
    if worn {
        apply_item(world, player_oid, item_oid);
        refresh_conditioned_passives(world, player_oid);
    }
    let (regular, special) = option_lists(world, player_oid, item_oid);
    send_to_client(
        world,
        client_id,
        sp::ex_ensoul_result(success, &regular, &special),
    );
}

/// `RequestTryEnSoulExtraction` (0xD0 0x128) — take one option back out as
/// its soul crystal, for the grade's removal fee.
///
/// **No grade on this dist has a removal fee** (no `<remove>` in
/// `ensoulFees.xml`), so every request stops at Java's empty-fee return before
/// anything is checked or taken.
///
/// A position outside the slot type's array throws in Java: silently dropped
/// here. Regular position 0 falls back to slot 1 when slot 0 is empty.
pub(crate) fn handle_request_try_ensoul_extraction(world: &mut World, client_id: u32, body: &[u8]) {
    let mut r = PacketReader::new(body);
    let (Some(item_oid), Some(slot_type), Some(position)) = (
        r.read_i32(),
        r.read_u8().map(|b| b as i8 as i32),
        r.read_u8().map(|b| b as i8 as i32 - 1),
    ) else {
        return;
    };
    let Some(player_oid) = player_of(world, client_id) else {
        return;
    };
    let Some(item) = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| inv.by_object_id(item_oid))
        .copied()
    else {
        return;
    };
    let (option, position) = match (slot_type, position) {
        (1, 0..=1) => {
            let at = item.ensoul_options[position as usize];
            if at == 0 && position == 0 && item.ensoul_options[1] != 0 {
                (item.ensoul_options[1], 1)
            } else {
                (at, position as usize)
            }
        }
        (2, 0) => (item.ensoul_special_option, 0),
        // `_ensoulOptions[position]` / `_ensoulSpecialOptions[position]` out
        // of bounds.
        (1 | 2, _) => return,
        _ => (0, 0),
    };
    if option == 0 {
        return;
    }
    let Some(grade) = world
        .data
        .item_data
        .get(item.item_id)
        .map(|t| t.crystal_type)
    else {
        return;
    };
    let removal_fee = world.data.ensoul.removal_fee(grade).to_vec();
    if removal_fee.is_empty() {
        return;
    }
    for &(fee_id, fee_count) in &removal_fee {
        if inventory::count_of(world, player_oid, fee_id) < fee_count {
            send_sm_bare_to_client(world, client_id, sm_ids::INCORRECT_ITEM_COUNT);
            send_to_client(
                world,
                client_id,
                sp::ex_ensoul_extraction_result(false, &[], &[]),
            );
            return;
        }
    }
    // `destroyItemByItemId(…, sendMessage = true)`, one fee line at a time.
    for &(fee_id, fee_count) in &removal_fee {
        if inventory::take_items(world, client_id, player_oid, fee_id, fee_count) {
            send_disappeared(world, client_id, fee_id, fee_count);
        }
    }

    remove_special_ability(world, player_oid, item_oid, position, slot_type);
    let mut changes = inventory::modified_changes(world, player_oid, &[item_oid]);
    let rune_id = world.data.ensoul.stone_for_option(slot_type, option);
    if rune_id > 0
        && let Some(added) = inventory::add_inventory_item_changes(world, player_oid, rune_id, 1)
    {
        send_to_client(world, client_id, sp::obtained_item_sm(rune_id, 1));
        changes.extend(added);
    }
    inventory::send_inventory_update(world, player_oid, changes);
    inventory::send_inventory_item_list(world, player_oid);
    let (regular, special) = option_lists(world, player_oid, item_oid);
    send_to_client(
        world,
        client_id,
        sp::ex_ensoul_extraction_result(true, &regular, &special),
    );
}
