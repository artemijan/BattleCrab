//! Vitals, zones, stats and death checks.

use super::*;

/// Whether an object currently stands inside `zone` — Java
/// `ZoneType.isInsideZone(object)`.
///
/// `false` for an object that has left the world. Every caller is sweeping a
/// boss lair for "who is in here", and something with no position is not in
/// here.
///
/// Takes a resolved `&Zone` rather than a zone id because the callers are
/// filters over a region's worth of objects: the lookup is hoisted out of the
/// loop, which is also what makes the missing-zone case theirs to decide. Those
/// that keep an id-based check split on it deliberately —
/// `is_some_and` for "is this player in the boss zone?" (no zone ⇒ no) against
/// `is_none_or` for "has the boss left its lair?" (no zone ⇒ don't drag it
/// back) — so folding them in here would have to pick one and silently change
/// the other.
pub(crate) fn in_zone(world: &World, object_id: i32, zone: &crate::data::zone_data::Zone) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Position>(&object_id)
        .is_some_and(|p| zone.contains(p.x, p.y, p.z))
}

/// A creature's `(current, maximum)` HP, the maximum widened to `f64` so the
/// pair can be divided or compared without a cast at every use.
///
/// `None` once the object has left the world. Callers that only want the ratio
/// should take [`hp_fraction`], which also handles a zero maximum.
pub(crate) fn hp_pair(world: &World, object_id: i32) -> Option<(f64, f64)> {
    world
        .objects
        .get_component::<Vitals>(&object_id)
        .map(|v| (v.cur_hp, v.max_hp as f64))
}

/// A creature's HP as a fraction of its maximum, `0.0..=1.0` — Java's
/// `getCurrentHp() / getMaxHp()`, the number behind every "below N%" gate.
///
/// `None` for a departed object **and** for a zero maximum. That second case is
/// not paranoia: `max_hp` is 0 between an NPC's spawn and its first stat
/// recompute, and dividing there yields a `NaN` that compares `false` against
/// every threshold — so a boss script silently behaves as though the mob were
/// at full health. Every caller guarded it separately, in four different ways,
/// and one had missed it.
///
/// The fraction is canonical rather than a percentage on purpose: what each
/// caller does with a missing answer is *theirs*. `npc_cast` treats it as
/// "healthy, don't heal", `cubic` as "dead, skip", and those are opposite
/// defaults that must not be folded into one helper.
pub(crate) fn hp_fraction(world: &World, object_id: i32) -> Option<f64> {
    world
        .objects
        .get_component::<Vitals>(&object_id)
        .filter(|v| v.max_hp > 0)
        .map(|v| v.cur_hp / v.max_hp as f64)
}

/// Whether a creature counts as dead — **`true` when it has no [`Vitals`] at
/// all**.
///
/// [`Vitals`] is attached once at NPC spawn and player load and is never
/// removed on its own, so "no Vitals" means the object has left the world or
/// was never a creature (a dropped item, a door). Every caller is a
/// "may I still act on this target?" guard, and for those, an object that
/// isn't there must answer the same way a corpse does.
pub(crate) fn is_dead(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<Vitals>(&object_id)
        .is_none_or(|v| v.dead)
}
pub(crate) fn is_friend(world: &World, owner_oid: i32, target_oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Friends>(&owner_oid)
        .is_some_and(|fl| fl.0.iter().any(|f| f.char_id == target_oid))
}

/// Charge a creature's MP, floored at zero — the write half of every "does it
/// have the MP for this?" check (skill and shot costs, NPC buff casts, zone and
/// toggle upkeep).
///
/// No-op for an object that has left the world, and deliberately **not** a
/// check: Java's callers compare against the cost themselves, each with its own
/// refusal (a system message, a toggle that switches off, a different HTML
/// page), so the affordability test stays with the caller and only the clamped
/// subtraction lives here.
pub(crate) fn spend_mp(world: &mut World, object_id: i32, amount: f64) {
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&object_id) {
        v.cur_mp = (v.cur_mp - amount).max(0.0);
    }
}

pub(crate) fn restore_hp_mp(world: &mut World, object_id: i32) {
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&object_id) {
        v.cur_hp = v.max_hp as f64;
        v.cur_mp = v.max_mp as f64;
    }
}

/// Re-run [`Player::recalculate_stats`] for a player, folding the current
/// `BaseStats`/`StatModifiers`/paperdoll into `Speeds` + `CombatStats`.
///
/// No-op for anything that isn't a player with the full stat component set
/// (NPCs recompute through `recompute_npc_stats_from_buffs` instead).
///
/// Callers that mutate one of the inputs (`StatModifiers`, `BaseStats`) have to
/// do that in their own borrow *before* calling this; the recompute reads them
/// back through a fresh lookup.
pub(crate) fn recalculate_player_stats(world: &mut World, object_id: i32) {
    use crate::model::components::{BaseStats, CombatStats, Speeds};
    if let Some((p, base, mods, inventory, mut speeds, mut combat)) = world.objects.get_many_mut::<(
        &Player,
        &BaseStats,
        &StatModifiers,
        &Inventory,
        &mut Speeds,
        &mut CombatStats,
    )>(&object_id)
    {
        p.recalculate_stats(&world.data, base, mods, inventory, &mut speeds, &mut combat);
    }
}

/// [`recalculate_player_stats`] plus the max HP/MP/CP pass, which Java runs
/// inside the same `recalculateStats` but which lives on a separate path here
/// (`Vitals`/`PlayerVitals` aren't touched by `recalculate_stats`).
///
/// This is the one to call after anything that can move both the combat stats
/// and the vitals caps — transform/mount speed overrides, `//set` stat edits.
pub(crate) fn recalculate_player_stats_and_vitals(world: &mut World, object_id: i32) {
    recalculate_player_stats(world, object_id);
    crate::game_loop::skills::effects::recompute_max_vitals(world, object_id);
}

/// A player's [`Vitals`] and `PlayerVitals`, both copied out.
///
/// `None` unless **both** are present, which is what every caller wants: they
/// are feeding a `StatusUpdate` that carries HP, MP and CP together, and half a
/// gauge set is not a packet worth sending.
///
/// Copied rather than borrowed because every caller then needs `&mut World` to
/// broadcast.
pub(crate) fn vitals_pair(
    world: &World,
    player_oid: i32,
) -> Option<(Vitals, crate::model::components::PlayerVitals)> {
    Some((
        world
            .objects
            .get_component::<Vitals>(&player_oid)
            .copied()?,
        world
            .objects
            .get_component::<crate::model::components::PlayerVitals>(&player_oid)
            .copied()?,
    ))
}

pub(crate) fn absorb_into_hp(world: &mut World, attacker: i32, absorbed: f64) {
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&attacker) {
        v.cur_hp = (v.cur_hp + absorbed).min(v.max_hp as f64);
    }
}
/// The additive modifier standing on `stat`, defaulting to the additive
/// identity. Nothing with no [`StatModifiers`] has been buffed, so "absent"
/// and "zero" are the same answer.
pub(crate) fn stat_add(world: &World, object_id: i32, stat: Stat) -> f64 {
    world
        .objects
        .get_component::<StatModifiers>(&object_id)
        .and_then(|m| m.add.get(&stat).copied())
        .unwrap_or(0.0)
}

/// The multiplicative modifier standing on `stat`, defaulting to the
/// multiplicative identity — the [`stat_add`] counterpart, and the reason the
/// two cannot share one function: 0.0 and 1.0 are not interchangeable defaults.
pub(crate) fn stat_mul(world: &World, object_id: i32, stat: Stat) -> f64 {
    world
        .objects
        .get_component::<StatModifiers>(&object_id)
        .and_then(|m| m.mul.get(&stat).copied())
        .unwrap_or(1.0)
}
