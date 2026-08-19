//! NPC intention setters and hate-table entry points, shared by scripts,
//! quests and events (moved from helpers).

use super::move_npc_to;
use crate::game_loop::combat::ATTACK_TIMEOUT_TICKS;
use crate::model::npc::AggroList;
use crate::model::npc::NpcAi;
use crate::model::npc::NpcIntention;
use crate::world::World;
pub(crate) fn max_hate(world: &World, victim_oid: i32) -> f64 {
    world
        .objects
        .get_component::<AggroList>(&victim_oid)
        .map(|a| a.0.values().map(|i| i.hate).fold(0.0_f64, f64::max))
        .unwrap_or(0.0)
}
/// Java `setIntention(AI_INTENTION_ATTACK, target)` reduced to what this port
/// keeps on `NpcAi`: the intention flip plus a fresh attack-timeout stamp. No
/// target is passed — the ported AI reads its victim from the aggro list every
/// think tick, so seeding hate *is* the target hand-off.
///
/// The caller decides whether an NPC already in `Attack` should have its
/// timeout re-armed; this always re-arms.
pub(crate) fn set_attack_intention(world: &mut World, npc_oid: i32) {
    let deadline = world.tick + ATTACK_TIMEOUT_TICKS;
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.intention = NpcIntention::Attack;
        ai.attack_timeout_tick = deadline;
    }
}

/// The bare half of `setIntention(AI_INTENTION_ACTIVE)` — drop the AI back to
/// the scan loop and nothing else.
///
/// [`set_active`] is the fuller version, which also
/// reverts the move type to walking and broadcasts the `ChangeMoveType` that
/// goes with it. Callers here want only the intention: a servitor recalled to
/// its owner keeps whatever move type it was on, and a sown mob is left where
/// Java's `setIntention(AI_INTENTION_IDLE)` leaves it.
pub(crate) fn set_active_intention(world: &mut World, npc_oid: i32) {
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.intention = NpcIntention::Active;
    }
}

/// The NPC half of `setIntention(AI_INTENTION_MOVE_TO, destination)`: park the
/// AI on `MoveTo` and walk. The intention is what keeps `think` from re-issuing
/// a chase for the length of the walk (fear, flee-on-attack), so it is set
/// *before* the move and regardless of the outcome — Java flips the intention
/// whether or not the walk actually starts, and [`move_npc_to`] can bail on a
/// rooted mob, a missing speed or no path.
///
/// The player half is [`crate::game_loop::position::intention_move_to`], which
/// needs a client id and its own broadcast; callers that may hold either kind
/// of creature pick between the two.
///
/// [`move_npc_to`]: move_npc_to
pub(crate) fn set_move_to_intention(world: &mut World, npc_oid: i32, x: i32, y: i32, z: i32) {
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.intention = NpcIntention::MoveTo;
    }
    move_npc_to(world, npc_oid, x, y, z);
}

/// Java's taunt arithmetic (`Aggression` / `GetAgro`: `getHating(mostHated) -
/// getHating(effector) + 1`): puts `target_oid` one point above the current
/// most-hated entry rather than at an arbitrary huge constant, so the pull is
/// dominant but still breakable by the next real tank.
///
/// This *sets* the top of the list; [`minions::add_hate`]
/// *accumulates* a given amount. Returns `false` when `npc_oid` has no aggro
/// list at all (not an attackable — nothing to hate with).
pub(crate) fn set_top_hate(world: &mut World, npc_oid: i32, target_oid: i32) -> bool {
    let top = max_hate(world, npc_oid);
    let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) else {
        return false;
    };
    aggro.0.entry(target_oid).or_default().hate = top + 1.0;
    true
}

/// "Attack this, now" — [`set_top_hate`] plus the AI wake, the primitive behind
/// taunts (`GetAgro`), `Confuse`/`RandomizeHate` retargeting, and the servitor
/// attack order. A no-op on an NPC without an aggro list, which could not act
/// on the order anyway.
pub(crate) fn force_attack_target(world: &mut World, npc_oid: i32, target_oid: i32) {
    if set_top_hate(world, npc_oid, target_oid) {
        set_attack_intention(world, npc_oid);
    }
}
