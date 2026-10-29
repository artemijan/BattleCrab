//! `FriendlyNpcAI` — the AI of Salvation's `FriendlyNpc` type (escorts and
//! combat assistants in instances, such as Labyrinth of Belis' Infiltration
//! Officer).
//!
//! It is `AttackableAI` with three overrides: being attacked or called for
//! aggression does nothing (`onActionAttacked` / `onActionAggression` are
//! empty), the attack intention stores its target and stops any follow, and
//! `thinkAttack` swings at that stored target rather than the most hated.
//! The inherited `thinkActive` never seeds hate from its scan — a
//! `FriendlyNpc` is not a monster, and the scan only hates playables, which
//! its `addDamageHate` refuses — and with `canReturnToSpawnPoint` off it ends
//! after the forget roll. Hate reaches it only from monsters' damage and from
//! scripts.

use super::follow::stop_follow;
use super::movement::chase;
use super::set_active;
use super::tactics::{ai_type_of, crowded_on, movement_disabled, shuffle_spot};
use crate::data::npc_data::AiType;
use crate::game_loop::abnormal;
use crate::game_loop::combat;
use crate::game_loop::helpers;
use crate::model::components::combat::Casting;
use crate::model::components::space::Movement;
use crate::model::npc::{AggroList, NpcAi, NpcIntention};
use crate::world::World;

pub(super) fn think(world: &mut World, npc_oid: i32) {
    let Some(intention) = world
        .objects
        .get_component::<NpcAi>(&npc_oid)
        .map(|ai| ai.intention)
    else {
        return;
    };
    match intention {
        NpcIntention::Active => think_active(world, npc_oid),
        NpcIntention::Attack => think_attack(world, npc_oid),
        NpcIntention::MoveTo => {}
    }
}

/// `AttackableAI.thinkActive` as a `FriendlyNpc` runs it.
fn think_active(world: &mut World, npc_oid: i32) {
    let global_aggro = {
        let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) else {
            return;
        };
        if ai.global_aggro != 0 {
            ai.global_aggro += if ai.global_aggro < 0 { 1 } else { -1 };
        }
        ai.global_aggro
    };
    if global_aggro >= 0
        && let Some(hated) = world
            .objects
            .get_component::<AggroList>(&npc_oid)
            .and_then(AggroList::most_hated)
    {
        let aggro = world
            .objects
            .get_component::<AggroList>(&npc_oid)
            .and_then(|a| a.0.get(&hated))
            .map_or(0.0, |i| i.hate);
        if aggro + f64::from(global_aggro) > 0.0 {
            super::set_running(world, npc_oid);
            set_attack_intention(world, npc_oid, hated);
        }
        return;
    }
    // "Chance to forget attackers after some time".
    let full = world
        .objects
        .get_component::<crate::model::components::stats::Vitals>(&npc_oid)
        .is_some_and(|v| v.cur_hp >= f64::from(v.max_hp) && v.cur_mp >= f64::from(v.max_mp));
    let attacked = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .is_some_and(|a| !a.0.is_empty());
    if full && attacked && world.roll(500) == 0 {
        super::clear_aggro(world, npc_oid);
    }
}

/// `FriendlyNpcAI.onIntentionAttack`: refused while it cannot act, else the
/// intention, the AI target, `stopFollow` and an immediate think.
pub(crate) fn set_attack_intention(world: &mut World, npc_oid: i32, target_oid: i32) {
    if abnormal::is_blocked_from_actions(world, npc_oid)
        || world.objects.has_component::<Casting>(&npc_oid)
        || abnormal::is_control_blocked(world, npc_oid)
    {
        return;
    }
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.intention = NpcIntention::Attack;
        ai.target = target_oid;
    }
    stop_follow(world, npc_oid);
    think_attack(world, npc_oid);
}

/// `FriendlyNpcAI.thinkAttack`.
fn think_attack(world: &mut World, npc_oid: i32) {
    if world.objects.has_component::<Casting>(&npc_oid) {
        return;
    }
    let target_oid = world
        .objects
        .get_component::<NpcAi>(&npc_oid)
        .map_or(0, |ai| ai.target);
    if target_oid == 0 || helpers::is_alike_dead(world, target_oid) {
        // `npc.stopHating(target)`, then ACTIVE at walking speed.
        if target_oid != 0
            && let Some(info) = world
                .objects
                .get_component_mut::<AggroList>(&npc_oid)
                .and_then(|a| a.0.get_mut(&target_oid))
        {
            info.hate = 0.0;
        }
        set_active(world, npc_oid);
        return;
    }
    let (Some(me), Some(target)) = (
        combat::combatant(world, npc_oid),
        combat::combatant(world, target_oid),
    ) else {
        return;
    };
    let collision = me.collision_radius;
    let combined = collision + target.collision_radius;

    // The crowd shuffle: like `AttackableAI`'s, but the spot is only walked
    // to when `canMoveToTarget` allows it, not clamped to a valid one.
    if !movement_disabled(world, npc_oid)
        && world.roll(100) <= 3
        && crowded_on(world, npc_oid, target_oid, &me)
    {
        let (new_x, new_y) = shuffle_spot(world, &target, combined);
        let (dx, dy) = (f64::from(new_x - me.x), f64::from(new_y - me.y));
        if dx * dx + dy * dy > collision * collision {
            let new_z = me.z + 30;
            if world
                .geo
                .can_move_to_target(me.x, me.y, me.z, new_x, new_y, new_z)
            {
                super::move_npc_to(world, npc_oid, new_x, new_y, new_z);
            }
        }
        return;
    }

    // Archer movement: back off with a MOVE_TO intention.
    if !movement_disabled(world, npc_oid)
        && ai_type_of(world, npc_oid) == AiType::Archer
        && world.roll(100) < 15
    {
        let (dx, dy) = (f64::from(target.x - me.x), f64::from(target.y - me.y));
        if (dx * dx + dy * dy).sqrt() <= 60.0 + combined {
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
                super::set_move_to_intention(world, npc_oid, pos_x, pos_y, pos_z);
            }
            return;
        }
    }

    let (dx, dy) = (f64::from(target.x - me.x), f64::from(target.y - me.y));
    let dist2 = (dx * dx + dy * dy).sqrt() as i32 - collision as i32;
    let mut range = me.atk_range + combined as i32;
    let target_moving = world.objects.has_component::<Movement>(&target_oid);
    if target_moving {
        range += 50;
        if world.objects.has_component::<Movement>(&npc_oid) {
            range += 50;
        }
    }
    let can_see = world
        .geo
        .can_see_target(me.x, me.y, me.z, target.x, target.y, target.z);
    if dist2 > range || !can_see {
        if target_moving {
            range -= 100;
        }
        chase(world, npc_oid, target_oid, f64::from(range.max(5)));
        return;
    }
    combat::do_auto_attack(world, npc_oid, target_oid);
}
