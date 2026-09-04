//! Armor set bonuses — Java's `Inventory.ArmorSetListener` plus the two
//! `PaperdollCache` derivations that read sets (`getBaseStatValue`,
//! `getMaxSetEnchant`).
//!
//! Three things a worn set produces, and each has its own consumer:
//!
//! 1. **Skills.** Passive skills granted while enough pieces are worn. They go
//!    into the ordinary [`SkillBook`] and are filtered out of persistence by
//!    [`ArmorSetData::is_armor_set_skill`] — see that module's header for why
//!    the `SkillBook` is the right home and why the filter is unambiguous.
//! 2. **Base stats.** `<stats>` adds flat STR/DEX/CON/INT/WIT/MEN, exactly as a
//!    henna does, so [`set_stat_sums`] joins the henna sums wherever `BaseStats`
//!    is composed ([`crate::model::stat_finalize::compose_base_stats`]).
//! 3. **The min-enchant byte** in `UserInfo`/`CharInfo` ([`max_set_enchant`]),
//!    which is what makes a +6 set glow.
//!
//! ## Diff, not Java's remove-then-re-add
//!
//! Java's listener removes every skill whose conditions no longer validate and
//! then calls `applySkills` again to put back what still qualifies. Worse,
//! `ArmorsetSkillHolder.validateConditions` ends with `getSkillLevel() !=
//! _skillLevel`, so a skill the player *already has at exactly that level*
//! fails validation and gets stripped on any unequip.
//!
//! This computes the set of skills that should be granted right now and diffs
//! it against the ones currently granted. The outcome is identical on this dist
//! and the reason is measured, not assumed: **no armor-set skill is learnable**
//! (219 set-granted ids vs 758 tree ids, intersection empty), so Java's
//! "already knows it" clause can only ever fire against a skill another set
//! granted — and the diff recomputes the union of all worn sets anyway, which
//! is the behaviour Java's re-`applySkills` pass is groping toward. Re-check
//! that intersection before putting a set skill into a skill tree.

use crate::data::armor_set_data::{ArmorSetData, ArmorSetStats};
use crate::game_loop::admin::refresh_skill_list;
use crate::model::components::skills::SkillBook;
use crate::model::inventory::{Inventory, PaperdollSlot};
use crate::world::World;

/// Java `ArmorSet.ARMORSET_SLOTS` — the five slots `getLowestSetEnchant` walks.
/// Deliberately *not* every paperdoll slot: the shield is an optional item and
/// never contributes to the set's enchant floor.
const ARMOR_SET_SLOTS: [PaperdollSlot; 5] = [
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Head,
    PaperdollSlot::Gloves,
    PaperdollSlot::Feet,
];

/// Java `ArmorSet.getPiecesCountById` — how many *equipped* items are required
/// pieces of this set.
///
/// Counts paperdoll slots, not inventory stacks, and a full-armor item occupies
/// only `Chest` (Java's `SLOT_FULL_ARMOR` case sets `Legs` to null), so a
/// one-piece body armor counts once rather than twice.
fn pieces_count(inv: &Inventory, required: &[i32]) -> i32 {
    inv.equipped_items()
        .iter()
        .filter(|it| required.contains(&it.item_id))
        .count() as i32
}

/// Java `ArmorSet.getLowestSetEnchant` — 0 unless the set is complete, else the
/// **lowest** enchant among the required pieces in the five armor slots.
///
/// Java seeds the search with `Byte.MAX_VALUE` and maps "nothing matched" back
/// to 0; a `min()` over the matching slots is the same thing without the
/// sentinel.
fn lowest_set_enchant(inv: &Inventory, required: &[i32], minimum_pieces: i32) -> i32 {
    if pieces_count(inv, required) < minimum_pieces {
        return 0;
    }
    ARMOR_SET_SLOTS
        .iter()
        .filter_map(|&slot| inv.paperdoll_item(slot))
        .filter(|it| required.contains(&it.item_id))
        .map(|it| it.enchant_level)
        .min()
        .unwrap_or(0)
}

/// Java `ArmorSet.hasOptionalEquipped` — is one of the set's optional items
/// (the shield) worn?
fn has_optional_equipped(inv: &Inventory, optional: &[i32]) -> bool {
    inv.equipped_items()
        .iter()
        .any(|it| optional.contains(&it.item_id))
}

/// The skills every worn set should be granting right now, as `(id, level)`.
///
/// Java's per-skill gates in `validateConditions`, in its order: enough pieces,
/// then the set's enchant floor, then the optional item. Where two sets grant
/// the same skill id, the higher level wins — Java reaches the same place by
/// skipping a grant when the player already has `>=` that level, and this way
/// the outcome doesn't depend on which set is visited first.
pub(crate) fn granted_skills_for(data: &ArmorSetData, inv: &Inventory) -> Vec<(i32, i32)> {
    let mut best: std::collections::BTreeMap<i32, i32> = std::collections::BTreeMap::new();
    // Only sets some equipped item belongs to can qualify — Java's
    // `getSets(item.getId())` per paperdoll item. Visiting sets rather than
    // items keeps a set from being evaluated once per worn piece.
    let mut seen_sets = std::collections::BTreeSet::new();
    for it in inv.equipped_items() {
        for &set_id in data.sets_for_item(it.item_id) {
            if !seen_sets.insert(set_id) {
                continue;
            }
            let Some(set) = data.get(set_id) else {
                continue;
            };
            // The visual branch is unported (no appearance stones on this
            // dist); a visual set must not behave like a real one.
            if set.visual {
                continue;
            }
            let pieces = pieces_count(inv, &set.required_items);
            if pieces < set.minimum_pieces {
                continue;
            }
            let lowest = lowest_set_enchant(inv, &set.required_items, set.minimum_pieces);
            let has_optional = has_optional_equipped(inv, &set.optional_items);
            for skill in &set.skills {
                if skill.minimum_pieces > pieces
                    || skill.minimum_enchant > lowest
                    || (skill.optional && !has_optional)
                {
                    continue;
                }
                // SKIP(census): Java's `applySkills` also arms an equip reuse
                // delay and re-sends `SkillCoolTime` for *active* set skills.
                // All 84 skills on the 37 sets a player can assemble on this
                // dist are `operateType="P"`, so no reachable set has an active
                // to arm. Re-census before adding one.
                let e = best.entry(skill.skill_id).or_insert(skill.level);
                *e = (*e).max(skill.level);
            }
        }
    }
    best.into_iter().collect()
}

/// Re-derive the armor-set skills on `oid` from the worn gear, applying the
/// difference. Returns whether the granted set changed.
///
/// Does **not** touch the stat maps or send anything; callers pair it with a
/// stat recompute and a `SkillList`/`UserInfo` — see
/// [`refresh_armor_sets`], which is what equip/unequip uses.
pub(crate) fn recompute_armor_set_skills(world: &mut World, oid: i32) -> bool {
    let desired = match world.objects.get_component::<Inventory>(&oid) {
        Some(inv) => granted_skills_for(&world.data.armor_sets, inv),
        None => Vec::new(),
    };
    let Some(book) = world.objects.get_component::<SkillBook>(&oid) else {
        return false;
    };
    let sets = &world.data.armor_sets;
    let mut current: Vec<(i32, i32)> = book
        .0
        .iter()
        .filter(|(id, _)| sets.is_armor_set_skill(**id))
        .map(|(&id, &lvl)| (id, lvl))
        .collect();
    current.sort_unstable();
    if current == desired {
        return false;
    }
    let Some(book) = world.objects.get_component_mut::<SkillBook>(&oid) else {
        return false;
    };
    for (id, _) in &current {
        book.0.remove(id);
    }
    for &(id, level) in &desired {
        book.0.insert(id, level);
    }
    // `Inventory.ArmorSetListener`: "Active, non offensive, skills start with
    // reuse on equip" — otherwise completing a set hands you a ready active,
    // and re-equipping a piece hands it to you again. The skill's own reuse
    // wins when it has one; `ArmorSetEquipActiveSkillReuse` is the fallback
    // for the ones that declare none.
    //
    // Java also guards on `player.hasEnteredWorld()`, because its inventory
    // restore runs this listener during login. The port's path is
    // equip-driven only (`refresh_after_paperdoll_change`), so there is no
    // login pass to exclude.
    let newly_granted: Vec<(i32, i32)> = desired
        .iter()
        .filter(|entry| !current.contains(entry))
        .copied()
        .collect();
    stamp_equip_reuse(world, oid, &newly_granted);
    true
}

/// Test hook for [`stamp_equip_reuse`] — the grant path itself needs a full
/// armour set worn, which is a lot of fixture for a rule about *one* number.
#[cfg(test)]
pub(crate) fn stamp_equip_reuse_for_test(world: &mut World, oid: i32, granted: &[(i32, i32)]) {
    stamp_equip_reuse(world, oid, granted);
}

/// The reuse stamp above, split out so the borrow of `SkillBook` is finished
/// before the `Reuses` write.
fn stamp_equip_reuse(world: &mut World, oid: i32, granted: &[(i32, i32)]) {
    let fallback_ms = world.cfg.character.armor_set_equip_active_skill_reuse_ms;
    if fallback_ms <= 0 || granted.is_empty() {
        return;
    }
    let stamps: Vec<(i32, i32, i32)> = granted
        .iter()
        .filter_map(|&(id, level)| {
            let skill = world.data.skill_data.get(id, level)?;
            // Java: `!isBad() && !isTransformation()`. A passive can pass this
            // test in Java too — the stamp is simply never consulted for one.
            if skill.is_bad()
                || skill.effects.iter().any(|e| {
                    matches!(
                        e,
                        crate::model::skill::effects::SkillEffect::Transform { .. }
                    )
                })
            {
                return None;
            }
            let delay = if skill.reuse_delay > 0 {
                skill.reuse_delay
            } else {
                fallback_ms
            };
            Some((skill.reuse_key(), level, delay))
        })
        .collect();
    if stamps.is_empty() {
        return;
    }
    let now = world.tick;
    if let Some(reuses) = crate::game_loop::helpers::reuses_mut(world, oid) {
        for (key, level, delay_ms) in stamps {
            reuses.0.insert(
                key,
                crate::model::SkillReuse {
                    skill_level: level,
                    until_tick: now + crate::scheduler::ms_to_ticks(delay_ms),
                    total_ms: delay_ms,
                },
            );
        }
    }
}

/// The flat base-stat bonus of every *complete* worn set (Java
/// `PaperdollCache.getBaseStatValue`, which sums `getStatsBonus` over each
/// applied set exactly once — the `appliedSets` guard).
pub(crate) fn set_stat_sums(world: &World, oid: i32) -> ArmorSetStats {
    let Some(inv) = world.objects.get_component::<Inventory>(&oid) else {
        return ArmorSetStats::default();
    };
    set_stat_sums_for(&world.data.armor_sets, inv)
}

/// [`set_stat_sums`] against borrowed data — for the login path, where neither
/// the `Inventory` nor a `World` is available yet.
pub(crate) fn set_stat_sums_for(data: &ArmorSetData, inv: &Inventory) -> ArmorSetStats {
    let mut total = ArmorSetStats::default();
    let mut applied = std::collections::BTreeSet::new();
    for it in inv.equipped_items() {
        for &set_id in data.sets_for_item(it.item_id) {
            if !applied.insert(set_id) {
                continue;
            }
            let Some(set) = data.get(set_id) else {
                continue;
            };
            if set.visual || set.stats.is_zero() {
                continue;
            }
            if pieces_count(inv, &set.required_items) >= set.minimum_pieces {
                total += set.stats;
            }
        }
    }
    total
}

/// Java `Inventory.getArmorMinEnchant` → `PaperdollCache.getMaxSetEnchant`: the
/// **highest** `getLowestSetEnchant` across every worn set.
///
/// The naming is worth reading twice — "armor *min* enchant" is a max over
/// per-set minima, so a player wearing two sets gets the better one's floor.
/// This is the byte `UserInfo`/`CharInfo` carry, and it drives the +6 set glow.
pub(crate) fn max_set_enchant(world: &World, oid: i32) -> i32 {
    world
        .objects
        .get_component::<Inventory>(&oid)
        .map_or(0, |inv| max_set_enchant_for(&world.data.armor_sets, inv))
}

/// [`max_set_enchant`] against borrowed data (packet builders hold an
/// `Inventory` and a `GameData`, but no `World`).
pub(crate) fn max_set_enchant_for(data: &ArmorSetData, inv: &Inventory) -> i32 {
    let mut best = 0;
    let mut seen = std::collections::BTreeSet::new();
    for it in inv.equipped_items() {
        for &set_id in data.sets_for_item(it.item_id) {
            if !seen.insert(set_id) {
                continue;
            }
            let Some(set) = data.get(set_id) else {
                continue;
            };
            if set.visual {
                continue;
            }
            best = best.max(lowest_set_enchant(
                inv,
                &set.required_items,
                set.minimum_pieces,
            ));
        }
    }
    best
}

/// The equip/unequip entry point: re-derive the set skills, and when they moved,
/// re-apply the passive stat contributions they carry and refresh the client.
///
/// Ordering matters and mirrors Java's listener: the skills change *first*, so
/// the passive re-pump below sees the new book. `recompute_conditioned_passives`
/// is what turns a passive in the book into stat modifiers, so it is also what
/// makes a set's passives take effect — the same call the robe passives ride.
pub(crate) fn refresh_armor_sets(world: &mut World, oid: i32) {
    let skills_changed = recompute_armor_set_skills(world, oid);
    // The base-stat bonus moves with the *pieces*, not with the skills, so it
    // has to be re-composed even when the granted skills are unchanged (a
    // 3-piece set staying complete while a 4th piece is swapped changes
    // nothing; completing a `<stats>` set changes STR without changing skills).
    let stats_changed = refresh_set_base_stats(world, oid);
    if !skills_changed && !stats_changed {
        return;
    }
    // Re-pump the passives so a newly granted set passive lands in the stat
    // maps (and a dropped one leaves). This sends its own `UserInfo` when the
    // applied set moved.
    crate::game_loop::stats::passive_skills::refresh_conditioned_passives(world, oid);
    if skills_changed {
        refresh_skill_list(world, oid);
    }
}

/// Re-compose `BaseStats` from the class template plus hennas plus set bonuses.
/// Returns whether the value actually moved, so the caller can skip a redundant
/// stat recompute and `UserInfo`.
fn refresh_set_base_stats(world: &mut World, oid: i32) -> bool {
    let Some(new_base) = crate::model::stat_finalize::compose_base_stats(world, oid) else {
        return false;
    };
    let changed = world
        .objects
        .get_component::<crate::model::components::stats::BaseStats>(&oid)
        .is_some_and(|b| *b != new_base);
    if changed {
        world.objects.add_components(&oid, new_base);
    }
    changed
}
