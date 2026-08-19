//! Armor-conditioned passive skills — the `ConditionUsingItemType` (`<armorType>`)
//! effects on passive skills whose stat contribution depends on the worn armor.
//!
//! Java applies a passive skill's effects when it's learned (`Player.addSkill` →
//! `EffectList`) and re-checks each effect's condition on every stat recompute.
//! The only passives that reach the ported stat engine so far are the mystic
//! robe passives — **Spellcraft (163)** (`+50%` casting speed in a robe, `−50%`
//! in non-robe armor) and **Magician's Movement (118)** (`−20%` atk speed in
//! non-robe armor).
//!
//! Enter-world folds these in up front (`Player::from_char` →
//! [`crate::model::conditioned_passive_buffs`], so the first `UserInfo` already
//! carries them). This runs afterward, on every equip/unequip, to re-evaluate
//! the conditions as a robe is worn or removed — mirroring [`super::expertise`].
//! No-op when the applicable set is unchanged; resends `UserInfo` when it flips.

use crate::model::components::SkillBook;
use crate::model::inventory::Inventory;
use crate::model::skill::StatModifierEffect;
use crate::world::World;

/// Re-derive the armor-conditioned passive buffs from the player's known
/// passives versus currently-worn gear and (re)apply them. Resends `UserInfo`
/// only when the applied set actually changed (a robe swap moving
/// casting/attack speed). Call after any equip/unequip. Sends a fresh
/// `UserInfo` only when the applied set actually changed — a no-op otherwise.
pub(crate) fn refresh_conditioned_passives(world: &mut World, object_id: i32) {
    if recompute_conditioned_passives(world, object_id) {
        crate::game_loop::player_info::send_user_info(world, object_id);
    }
}

/// The HP-conditioned half of the same mechanism —
/// `AbstractConditionalHpEffect`'s `ON_CREATURE_HP_CHANGE` listener, which
/// forces a stat recompute whenever `currentHpPercent <= hpPercent` flips.
///
/// Java attaches that listener per effect; the port calls this from the paths
/// that actually move a player's HP. The gate up front is what keeps it free
/// for everyone else: only **Final Frenzy (290)** and **Final Fortress (291)**
/// are learnable carriers on this dist, so for every other character this is a
/// single pass over the skill book with no allocation and no recompute.
pub(crate) fn refresh_on_hp_change(world: &mut World, object_id: i32) {
    use crate::model::skill::SkillEffect;
    let carries = world
        .objects
        .get_component::<SkillBook>(&object_id)
        .is_some_and(|book| {
            book.0.iter().any(|(&id, &level)| {
                world.data.skill_data.get(id, level).is_some_and(|s| {
                    s.effects
                        .iter()
                        .any(|e| matches!(e, SkillEffect::StatModifier(m) if m.hp_percent > 0))
                })
            })
        });
    if !carries {
        return;
    }
    refresh_conditioned_passives(world, object_id);
}

/// Re-derive the armor-conditioned passive contributions in place, **without**
/// sending any packet. Returns whether the applied set actually changed (so a
/// caller that will broadcast its own stat update — e.g. `set_level`'s
/// `UserInfo` on a delevel — doesn't send a redundant second one). Callers that
/// aren't already refreshing the client use [`refresh_conditioned_passives`].
pub(crate) fn recompute_conditioned_passives(world: &mut World, object_id: i32) -> bool {
    // Rate-carrying passives (`MagicMpCost`/`Reuse`) live in their own tables,
    // not in `StatModifiers`, so they are rebuilt here rather than riding the
    // buff diff below — which returns early when the *stat* set is unchanged
    // and would skip them. Idempotent, so the extra calls cost nothing.
    crate::game_loop::skills::effects::refresh_passive_skill_rates(world, object_id);

    // --- read phase: the buffs that should be applied now, and the ones that
    // currently are (passive buffs whose skill is in the book — the expertise
    // penalty buffs 6209/6213 aren't learned skills, so they're left alone). ---
    let Some(book) = world.objects.get_component::<SkillBook>(&object_id) else {
        return false;
    };
    let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
        return false;
    };
    // `AbstractConditionalHpEffect` reads the live HP percentage, so this
    // recompute is also what turns Final Frenzy / Final Fortress on and off as
    // the bar crosses 30 %.
    let hp_percent = world
        .objects
        .get_component::<crate::model::components::Vitals>(&object_id)
        .map(|v| crate::model::hp_percent_of(v.cur_hp, v.max_hp))
        .unwrap_or(100);
    let desired = crate::model::conditioned_passive_buffs(&world.data, book, inventory, hp_percent);
    let desired_pairs: Vec<(i32, Vec<StatModifierEffect>)> = desired
        .iter()
        .map(|b| (b.skill_id, b.effects.clone()))
        .collect();

    let current: Vec<(i32, Vec<StatModifierEffect>)> =
        crate::game_loop::skills::effects::buffs_snapshot(world, object_id, |buff| {
            (buff.passive && book.0.contains_key(&buff.skill_id))
                .then(|| (buff.skill_id, buff.effects.clone()))
        });
    if same_buff_set(&current, &desired_pairs) {
        return false;
    }

    // The universe to remove before re-adding: everything currently applied plus
    // everything now desired (so a passive that just lost all its effects, e.g.
    // a robe removed, still gets its stale buff dropped).
    let mut managed_ids: Vec<i32> = current.iter().map(|(id, _)| *id).collect();
    for b in &desired {
        if !managed_ids.contains(&b.skill_id) {
            managed_ids.push(b.skill_id);
        }
    }

    // --- apply phase: drop the managed passive buffs, re-add those that apply.
    // `remove_buff`/`apply_buff` rebuild the modifier maps from the remaining
    // buffs, composing with the expertise penalty buffs (distinct skill ids). ---
    let swapped = crate::game_loop::stat_ctx::with_stat_ctx(world, object_id, |ctx| {
        for &skill_id in &managed_ids {
            ctx.remove(skill_id);
        }
        for buff in desired {
            ctx.apply(buff);
        }
    });
    if swapped.is_none() {
        return false;
    }
    // Max HP/MP/CP live on a separate path from `recalculate_stats` (they are
    // cached on `Vitals`/`PlayerVitals` rather than derived per read), so a
    // passive carrying `MaxHp`/`MaxMp`/`MaxCp` only reaches the bar through
    // this call — the same follow-up `apply_skill_effects` and the clan-skill
    // pump already make. Without it the modifier sits in `StatModifiers`
    // unnoticed until some *other* recompute folds it in, which is how a
    // cursed weapon's `+MaxCp` first appeared at the moment the curse *ended*.
    crate::game_loop::skills::effects::recompute_max_vitals(world, object_id);
    true
}

/// Order-insensitive equality of two `(skill_id, effects)` buff sets.
fn same_buff_set(
    a: &[(i32, Vec<StatModifierEffect>)],
    b: &[(i32, Vec<StatModifierEffect>)],
) -> bool {
    a.len() == b.len() && a.iter().all(|entry| b.contains(entry))
}
