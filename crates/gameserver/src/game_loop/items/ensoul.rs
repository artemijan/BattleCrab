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

use crate::data::GameData;
use crate::db::ItemRow;
use crate::game_loop::admin::refresh_skill_list;
use crate::model::components::skills::{OptionSkills, SkillBook};
use crate::model::inventory::{Inventory, ItemInstance};
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
