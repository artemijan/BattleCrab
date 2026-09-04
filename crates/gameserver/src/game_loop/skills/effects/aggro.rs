//! Hate-table effects: adding, clearing and redirecting an NPC's aggro.

use super::control::{confuse_chance_passes, random_bystander};
use crate::game_loop::helpers;
use crate::model::skill::Skill;
use crate::world::World;

/// `RandomizeHate.instant` — move the *caster's* accumulated hate onto a
/// random bystander, so the mob rounds on someone else instead of simply
/// forgetting (Confusion 2, Switch 12).
pub(crate) fn randomize_hate(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    chance: i32,
) {
    // Java: `if ((effected == effector) || !effected.isAttackable()) return;`
    if target_oid == caster_oid || !crate::game_loop::combat::is_npc_oid(target_oid) {
        return;
    }
    if !confuse_chance_passes(world, caster_oid, target_oid, skill, chance) {
        return;
    }
    // The exclusions are wider here than for `Confuse`: never the caster, and
    // never a same-faction attackable ("aggro cannot be transfered to a mob of
    // the same faction").
    let Some(victim) = random_bystander(world, target_oid, caster_oid, true) else {
        return;
    };
    if let Some(aggro) = world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&target_oid)
    {
        // `getHating` → `stopHating` → `addDamageHate(target, 0, hate)`: the
        // hate is *moved*, not duplicated.
        let hate = aggro.0.get(&caster_oid).map(|i| i.hate).unwrap_or(0.0);
        aggro.0.remove(&caster_oid);
        aggro.0.entry(victim).or_default().hate += hate;
    }
}

/// `TargetMe` / `TargetMeProbability` — the *playable*-side taunt. Java wraps
/// both in `if (effected.isPlayable())`, so taunting a **monster** through
/// these does nothing at all; a mob's aggro comes from the `AddHate`/`GetAgro`
/// effects the same skills carry. `chance` is `None` for `TargetMe` — the
/// continuous variant that also **locks** the target (cleared on expiry) —
/// and `Some` for the instant, chance-rolled, lock-free probability variant.
pub(crate) fn target_me(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    chance: Option<i32>,
) {
    if !world
        .objects
        .has_component::<crate::model::Player>(&target_oid)
    {
        return;
    }
    if let Some(chance) = chance
        && !confuse_chance_passes(world, caster_oid, target_oid, skill, chance)
    {
        return;
    }
    // `if (effected.getTarget() != effector) effected.setTarget(effector)` —
    // through the client-notifying setter so the selection ring actually
    // moves.
    let already = world
        .objects
        .get_component::<crate::model::components::combat::TargetRef>(&target_oid)
        .and_then(|t| t.0);
    if already != Some(caster_oid)
        && let Some(client_id) = helpers::client_for_player(world, target_oid)
    {
        crate::game_loop::combat::target::set_target(
            world,
            client_id,
            target_oid,
            Some(caster_oid),
        );
    }
    if chance.is_none() {
        world.objects.add_components(
            &target_oid,
            crate::model::components::combat::LockedTarget(caster_oid),
        );
    }
}

/// `AddHate.instant` — a flat hate change with no damage (positive:
/// Charm/Lure; negative: unused on this dist but supported). Mirrors the
/// add/reduce shape already used by `minions.rs`/`faction_call`.
///
/// No `Attackable.reduceHate` tail here (the −25 calm window +
/// `clearAggroList`), deliberately: Java can't reach it through this handler.
/// `AddHate.instant` passes `(int) -val` for a negative `val`, so a
/// `power=-1240` skill calls `reduceHate(effector, +1240)` →
/// `ai.addHate(+1240)` — the double negation makes Java's "negative AddHate"
/// *raise* hate, which never leaves `getMostHated() == null` and so never
/// arms the calm window. The only genuine `reduceHate` caller is
/// `TransferHate` (skill 489 Shift Target, off-chronicle here). Porting the
/// tail onto this branch's reduce semantics would invent a 25 s stand-down
/// Java never produces.
pub(crate) fn add_hate(world: &mut World, caster_oid: i32, target_oid: i32, power: f64) {
    let Some(aggro) = world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&target_oid)
    else {
        return;
    };
    if power >= 0.0 {
        aggro.0.entry(caster_oid).or_default().hate += power;
    } else if let Some(entry) = aggro.0.get_mut(&caster_oid) {
        entry.hate = (entry.hate + power).max(0.0);
    }
    if power > 0.0
        && world
            .objects
            .get_component::<crate::model::npc::NpcAi>(&target_oid)
            .is_some_and(|ai| ai.intention != crate::model::npc::NpcIntention::Attack)
    {
        crate::game_loop::ai::set_attack_intention(world, target_oid);
    }
}

/// `DeleteHate.instant` — wipe the *whole* aggro list and disengage (Java
/// `setWalking()` + `setIntention(ACTIVE)`).
///
/// The gate is **`calcSuccess`, not a bare roll**:
///
/// ```java
/// public boolean calcSuccess(Creature effector, Creature effected, Skill skill)
/// {
///     return Formulas.calcProbability(_chance, effector, effected, skill);
/// }
/// ```
///
/// So the declared `<chance>` is a *base* chance that the level difference, the
/// target's abnormal resistance, the skill's element and its trait all move —
/// the same formula `Confuse` and `RandomizeHate` already run here. Repose
/// (1034), Peace (1075) and Eva's Serenade (1273) are the learnable carriers,
/// and a flat roll made all three land on a boss exactly as often as on a rat.
pub(crate) fn delete_hate(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    chance: i32,
) {
    if !confuse_chance_passes(world, caster_oid, target_oid, skill, chance) {
        return;
    }
    crate::game_loop::ai::clear_aggro(world, target_oid);
    crate::game_loop::ai::set_active(world, target_oid);
}

/// `DeleteHateOfMe.instant` — `stopHating` just the caster's own entry, but
/// Java disengages the AI wholesale regardless of whatever other hate remains —
/// the next think tick re-picks the next-most-hated target on its own if any is
/// left.
///
/// Same `calcSuccess` gate as [`delete_hate`]; the learnable carriers here are
/// Trick (11), Bluff (358) and Forget (1156).
pub(crate) fn delete_hate_of_me(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    chance: i32,
) {
    if !confuse_chance_passes(world, caster_oid, target_oid, skill, chance) {
        return;
    }
    if let Some(aggro) = world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&target_oid)
        && let Some(entry) = aggro.0.get_mut(&caster_oid)
    {
        entry.hate = 0.0;
    }
    crate::game_loop::ai::set_active(world, target_oid);
}
