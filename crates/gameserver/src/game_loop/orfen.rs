//! Orfen (`ai/bosses/Orfen`) — the third grand-boss script.
//!
//! Two mechanics, both reactions to being attacked:
//!
//! - **The drag.** An attacker between 300 and 1000 units away has a 1-in-10
//!   chance per hit of being teleported *onto* Orfen and paralysed. This is the
//!   fight: Orfen punishes ranged damage specifically, and the band matters —
//!   melee (inside 300) is never dragged, and out past 1000 you are out of
//!   reach entirely.
//! - **The half-HP relocation.** The first time Orfen drops below half health
//!   it teleports to its "home" spawn point, once per life.

use crate::model::components::{Position, Vitals};
use crate::world::World;

pub const ORFEN: i32 = 29014;
/// Riba Iren — the healer minion. Heals Orfen when **itself** wounded past
/// half, not when Orfen is.
pub const RIBA_IREN: i32 = 29018;

/// `PARALYSIS` (4064) — what the drag lands with.
const PARALYSIS: i32 = 4064;
/// `ORFEN_HEAL` (4516).
const ORFEN_HEAL: i32 = 4516;

/// `POS[0]` — the point the half-HP relocation uses.
const HOME: (i32, i32, i32) = (43728, 17220, -4342);

/// The drag band, from Java's `isInsideRadius2D(attacker, 1000) &&
/// !isInsideRadius2D(attacker, 300)`.
const DRAG_MIN: f64 = 300.0;
const DRAG_MAX: f64 = 1000.0;
/// `getRandom(10) == 0`.
const DRAG_CHANCE: i32 = 10;

/// Marks an Orfen that has already used its half-HP relocation, so it happens
/// once per life rather than on every hit below the threshold.
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, Default)]
pub struct OrfenState {
    pub teleported: bool,
}

/// `Orfen.onAttack`. Called after damage lands, with the damage that landed.
pub(crate) fn on_orfen_attacked(world: &mut World, orfen_oid: i32, attacker_oid: i32) {
    // The half-HP relocation wins when both could fire — Java's `if/else if`,
    // and the ordering matters: a boss that just relocated should not also drag
    // someone to where it no longer is.
    if try_half_hp_relocation(world, orfen_oid) {
        return;
    }
    try_drag(world, orfen_oid, attacker_oid);
}

/// `if (!_isTeleported && (hp - damage) < maxHp / 2)` — once per life.
fn try_half_hp_relocation(world: &mut World, orfen_oid: i32) -> bool {
    let already = world
        .objects
        .get_component::<OrfenState>(&orfen_oid)
        .is_some_and(|s| s.teleported);
    if already {
        return false;
    }
    let below_half = world
        .objects
        .get_component::<Vitals>(&orfen_oid)
        .is_some_and(|v| v.cur_hp < v.max_hp as f64 / 2.0);
    if !below_half {
        return false;
    }
    if world
        .objects
        .get_component::<OrfenState>(&orfen_oid)
        .is_none()
    {
        world
            .objects
            .add_components(&orfen_oid, OrfenState::default());
    }
    if let Some(s) = world.objects.get_component_mut::<OrfenState>(&orfen_oid) {
        s.teleported = true;
    }
    // `clearAggroList()` + `setIntention(IDLE)` — it disengages, then moves.
    if let Some(a) = world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&orfen_oid)
    {
        a.0.clear();
    }
    if let Some(p) = world.objects.get_component_mut::<Position>(&orfen_oid) {
        p.x = HOME.0;
        p.y = HOME.1;
        p.z = HOME.2;
    }
    true
}

/// The drag: yank a mid-range attacker onto Orfen and paralyse them.
fn try_drag(world: &mut World, orfen_oid: i32, attacker_oid: i32) {
    // Players only — a summon's damage does not drag its owner.
    if world
        .objects
        .get_component::<crate::model::Player>(&attacker_oid)
        .is_none()
    {
        return;
    }
    let Some(dist) = distance_2d(world, orfen_oid, attacker_oid) else {
        return;
    };
    if !(DRAG_MIN..=DRAG_MAX).contains(&dist) {
        return;
    }
    if world.roll(DRAG_CHANCE) != 0 {
        return;
    }
    let Some(to) = world.objects.get_component::<Position>(&orfen_oid).copied() else {
        return;
    };
    crate::game_loop::death::teleport_player(world, attacker_oid, to.x, to.y, to.z);
    cast_on(world, orfen_oid, attacker_oid, PARALYSIS);
}

/// Riba Iren heals Orfen when **the minion itself** drops below half — an
/// easy one to get backwards, since every other healer in the game reacts to
/// its master's health.
pub(crate) fn on_riba_iren_attacked(world: &mut World, minion_oid: i32) {
    let below_half = world
        .objects
        .get_component::<Vitals>(&minion_oid)
        .is_some_and(|v| v.cur_hp < v.max_hp as f64 / 2.0);
    if !below_half {
        return;
    }
    let Some(orfen_oid) = find_alive(world, ORFEN) else {
        return;
    };
    cast_on(world, minion_oid, orfen_oid, ORFEN_HEAL);
}

fn find_alive(world: &mut World, npc_id: i32) -> Option<i32> {
    let mut found = None;
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &Vitals)>(|(n, v)| {
            if n.npc_id == npc_id && !v.dead {
                found = Some(n.object_id);
            }
        });
    found
}

fn distance_2d(world: &World, a: i32, b: i32) -> Option<f64> {
    let pa = world.objects.get_component::<Position>(&a)?;
    let pb = world.objects.get_component::<Position>(&b)?;
    let (dx, dy) = ((pa.x - pb.x) as f64, (pa.y - pb.y) as f64);
    Some((dx * dx + dy * dy).sqrt())
}

fn cast_on(world: &mut World, caster_oid: i32, target_oid: i32, skill_id: i32) {
    let Some(skill) = world.data.skill_data.get(skill_id, 1).cloned() else {
        return;
    };
    if !crate::game_loop::npc_cast::check_use_conditions_pub(world, caster_oid, &skill) {
        return;
    }
    crate::game_loop::npc_cast::start_cast(world, caster_oid, target_oid, &skill);
}
