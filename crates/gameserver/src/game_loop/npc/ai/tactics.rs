//! Attack-think tactics: unstacking piled mobs, archer back-off and raid
//! target chaos. All share the "did I end this think?" bool contract.

use super::move_npc_to;
use super::target_reconsider_random;
use crate::data::npc_data::AiType;
use crate::game_loop::abnormal;
use crate::game_loop::combat;
use crate::game_loop::helpers::hp_fraction;
use crate::game_loop::npc::minions::MinionOf;
use crate::game_loop::npc::minions::Minions;
use crate::game_loop::npc::npc_template;
use crate::game_loop::space::position::region_cell_of;
use crate::model::components::space::Position;
use crate::model::components::stats::Vitals;
use crate::model::npc::AggroList;
use crate::model::npc::NpcAi;
use crate::world::World;
/// `npc.getAiType()`, defaulting to `FIGHTER` for a template we can't read.
pub(super) fn ai_type_of(world: &World, npc_oid: i32) -> AiType {
    npc_template(world, npc_oid)
        .map(|t| t.ai_type)
        .unwrap_or(AiType::Fighter)
}

/// `Creature.isMovementDisabled()` for a monster: the abnormal states that pin
/// it (root/stun/sleep/paralysis) *or* a template that cannot move at all.
pub(super) fn movement_disabled(world: &World, npc_oid: i32) -> bool {
    abnormal::is_movement_disabled(world, npc_oid)
        || !npc_template(world, npc_oid).is_some_and(|t| t.can_move)
}

/// `thinkAttack`'s "In case many mobs are trying to hit from same place, move a
/// bit, circling around the target" block.
///
/// A 3-in-100 roll per think, and only when another `Attackable` is standing
/// inside this mob's own collision radius: step to a fresh spot roughly
/// `combinedCollision + Rnd(40)` off the *target* on each axis, sign chosen at
/// random, geo-validated. It is what stops a pack from stacking into one pixel
/// while they all beat on the same player. Returns whether the think ends here
/// — Java `return`s whenever it found a crowding neighbour, **even if the
/// chosen spot was rejected**.
pub(super) fn shuffle_off_a_stacked_mob(world: &mut World, npc_oid: i32, target_oid: i32) -> bool {
    if movement_disabled(world, npc_oid) || world.roll(100) > 3 {
        return false;
    }
    let (Some(me), Some(target)) = (
        combat::combatant(world, npc_oid),
        combat::combatant(world, target_oid),
    ) else {
        return false;
    };
    let collision = me.collision_radius;
    let combined = collision + target.collision_radius;

    let Some(region) = region_cell_of(world, npc_oid) else {
        return false;
    };
    let crowder = world
        .npcs_visible_from(region)
        .into_iter()
        .filter(|&other| other != npc_oid && other != target_oid)
        .filter(|&other| {
            world
                .objects
                .get_component::<Vitals>(&other)
                .is_some_and(|v| !v.dead)
        })
        .find(|&other| {
            world
                .objects
                .get_component::<Position>(&other)
                .is_some_and(|p| {
                    let (dx, dy) = ((p.x - me.x) as f64, (p.y - me.y) as f64);
                    dx * dx + dy * dy <= collision * collision
                })
        });
    if crowder.is_none() {
        return false;
    }

    // `newX = combinedCollision + Rnd.get(40)`, then added to or subtracted
    // from the *target's* coordinate on a coin flip — per axis, so the mob can
    // end up on any of the four diagonals around whoever it is hitting.
    let (dx_step, dy_step) = (
        combined as i32 + world.roll(40),
        combined as i32 + world.roll(40),
    );
    let (flip_x, flip_y) = (world.roll(2) == 0, world.roll(2) == 0);
    let new_x = if flip_x {
        target.x + dx_step
    } else {
        target.x - dx_step
    };
    let new_y = if flip_y {
        target.y + dy_step
    } else {
        target.y - dy_step
    };
    // `if (!npc.isInsideRadius2D(newX, newY, 0, collision))` — don't bother
    // shuffling onto the spot we already occupy.
    let (dx, dy) = ((new_x - me.x) as f64, (new_y - me.y) as f64);
    if dx * dx + dy * dy > collision * collision {
        let new_z = me.z + 30;
        let (vx, vy, vz) = world
            .geo
            .get_valid_location(me.x, me.y, me.z, new_x, new_y, new_z);
        move_npc_to(world, npc_oid, vx, vy, vz);
    }
    true
}

/// `thinkAttack`'s "Calculate Archer movement" block: an `ARCHER` mob that has
/// been closed to inside `60 + combinedCollision` backs off 300 units on each
/// axis, away from its target, on a 15-in-100 roll — but only if the geodata
/// says it can actually walk there (`canMoveToTarget`, not `canSeeTarget`).
///
/// This is the kiting that makes bow mobs feel different from melee ones.
/// Returns whether the think ends here; Java returns as soon as the mob is
/// inside the trigger distance, whether or not the retreat was walkable.
pub(super) fn archer_backs_off(world: &mut World, npc_oid: i32, target_oid: i32) -> bool {
    if movement_disabled(world, npc_oid)
        || ai_type_of(world, npc_oid) != AiType::Archer
        || world.roll(100) >= 15
    {
        return false;
    }
    let (Some(me), Some(target)) = (
        combat::combatant(world, npc_oid),
        combat::combatant(world, target_oid),
    ) else {
        return false;
    };
    let combined = me.collision_radius + target.collision_radius;
    let (dx, dy) = ((target.x - me.x) as f64, (target.y - me.y) as f64);
    if dx * dx + dy * dy > (60.0 + combined) * (60.0 + combined) {
        return false;
    }

    // Straight away from the target on each axis, 300 units.
    let pos_x = if target.x < me.x {
        me.x + 300
    } else {
        me.x - 300
    };
    let pos_y = if target.y < me.y {
        me.y + 300
    } else {
        me.y - 300
    };
    let pos_z = me.z + 30;
    if world
        .geo
        .can_move_to_target(me.x, me.y, me.z, pos_x, pos_y, pos_z)
    {
        move_npc_to(world, npc_oid, pos_x, pos_y, pos_z);
    }
    true
}

/// `thinkAttack`'s "BOSS/Raid Minion Target Reconsider" block — the chaos
/// timer that makes a raid stop tunnelling its tank and lunge at someone else.
///
/// The chance climbs as the boss loses HP, on three different curves, and each
/// tier only starts rolling once `chaostime` has ticked past its config gate
/// (`RaidChaosTime`/`GrandChaosTime`/`MinionChaosTime`, all 10 on this dist —
/// i.e. ten thinks, ten seconds). A successful swap resets the counter and ends
/// the think. Returns whether the think ends here.
pub(super) fn raid_target_chaos(world: &mut World, npc_oid: i32) -> bool {
    let Some(template) = npc_template(world, npc_oid) else {
        return false;
    };
    let (is_raid, is_grand) = (template.is_raid(), template.type_name == "GrandBoss");
    let is_minion = world.objects.has_component::<MinionOf>(&npc_oid);
    if !is_raid && !is_grand && !is_minion {
        return false;
    }

    let chaos_time = {
        let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) else {
            return false;
        };
        ai.chaos_time += 1;
        ai.chaos_time
    };
    let hp_fraction = hp_fraction(world, npc_oid).unwrap_or(1.0);

    let cfg = &world.cfg.npc;
    // Java's ladder: GrandBoss first only because `instanceof RaidBoss` is
    // checked first and a GrandBoss is not a RaidBoss; the three arms are
    // mutually exclusive.
    let change = if is_grand && chaos_time > cfg.grand_chaos_time {
        let chaos_rate = 100.0 - hp_fraction * 300.0;
        (chaos_rate <= 10.0 && world.roll(100) <= 10)
            || (chaos_rate > 10.0 && (world.roll(100) as f64) <= chaos_rate)
    } else if is_raid && chaos_time > cfg.raid_chaos_time {
        // `hasMinions() ? 200 : 100` — a boss with an escort shuffles sooner.
        let multiplier = if world
            .objects
            .get_component::<Minions>(&npc_oid)
            .is_some_and(|m| !m.0.is_empty())
        {
            200.0
        } else {
            100.0
        };
        (world.roll(100) as f64) <= 100.0 - hp_fraction * multiplier
    } else if is_minion && chaos_time > cfg.minion_chaos_time {
        (world.roll(100) as f64) <= 100.0 - hp_fraction * 200.0
    } else {
        return false;
    };
    if !change {
        return false;
    }

    // `targetReconsider(true)` — a *random* valid attacker rather than the
    // most hated one. That randomness is the whole mechanic.
    let Some(new_target) = target_reconsider_random(world, npc_oid) else {
        return false;
    };
    // Java `setTarget(target); chaostime = 0; return;` — the swap is expressed
    // here by making the new pick dominant in the aggro list, since an NPC's
    // "target" in this port *is* its most-hated entry.
    let top = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .and_then(|a| {
            a.0.values()
                .map(|i| i.hate)
                .fold(None, |m: Option<f64>, h| Some(m.map_or(h, |m| m.max(h))))
        })
        .unwrap_or(0.0);
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        aggro.0.entry(new_target).or_default().hate = top + 1.0;
    }
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.chaos_time = 0;
    }
    true
}
