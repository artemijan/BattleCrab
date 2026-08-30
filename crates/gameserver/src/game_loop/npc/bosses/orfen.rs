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

use crate::game_loop::helpers::pos_of;
use crate::game_loop::helpers::set_position;
use crate::game_loop::npc::cast;
use crate::model::components::{Position, Vitals};
use crate::scheduler::ScheduledTask;
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

/// Java `DISTANCE_CHECK`: dragged more than this from her spawn, Orfen resets.
const LEASH_RANGE: f64 = 10000.0;
/// The 5 s leash-check beat.
const DISTANCE_TICK_TICKS: u64 = 50;

/// Marks an Orfen that has already used its half-HP relocation, so it happens
/// once per life rather than on every hit below the threshold, and remembers her
/// spawn anchor for the leash check.
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, Default)]
pub struct OrfenState {
    pub teleported: bool,
    /// The spawn point the leash pulls her back to (Java `npc.getSpawn()`).
    pub home: (i32, i32, i32),
}

/// Java's `startQuestTimer("DISTANCE_CHECK", 5000, orfen, …, true)` at spawn:
/// remember where she started and begin the leash beat.
pub(crate) fn on_orfen_spawned(world: &mut World, orfen_oid: i32) {
    // Java `spawnBoss` places four Raikel Leos beside Orfen — her `<minions>`
    // escort, which the grand-boss spawn path (`spawn_npc_at`) otherwise skips.
    crate::game_loop::npc::minions::spawn_minions(world, orfen_oid);

    let home = pos_of(world, orfen_oid).unwrap_or(HOME);
    world.objects.add_components(
        &orfen_oid,
        OrfenState {
            teleported: false,
            home,
        },
    );
    world.scheduler.schedule(
        world.tick + DISTANCE_TICK_TICKS,
        ScheduledTask::OrfenDistanceCheck { orfen_oid },
    );
}

/// Java `DISTANCE_CHECK`: dragged more than `LEASH_RANGE` from her spawn, Orfen
/// drops her hate and walks back — the anti-drag rule.
pub(crate) fn handle_distance_check(world: &mut World, orfen_oid: i32) {
    let home = world
        .objects
        .get_component::<OrfenState>(&orfen_oid)
        .map(|s| s.home)
        .unwrap_or(HOME);
    if !crate::game_loop::grand_boss::leash_to_home(world, orfen_oid, home, LEASH_RANGE) {
        return; // Java cancels the timer on death
    }
    world.scheduler.schedule(
        world.tick + DISTANCE_TICK_TICKS,
        ScheduledTask::OrfenDistanceCheck { orfen_oid },
    );
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
    crate::game_loop::ai::clear_aggro(world, orfen_oid);
    set_position(world, orfen_oid, (HOME.0, HOME.1, HOME.2));
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
    if !crate::game_loop::death::teleport_to_object(world, attacker_oid, orfen_oid) {
        return;
    }
    cast::cast_skill(world, orfen_oid, attacker_oid, PARALYSIS, 1);
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
    let Some(orfen_oid) = crate::game_loop::grand_boss::find_alive(world, ORFEN) else {
        return;
    };
    cast::cast_skill(world, minion_oid, orfen_oid, ORFEN_HEAL, 1);
}

fn distance_2d(world: &World, a: i32, b: i32) -> Option<f64> {
    let pa = world.objects.get_component::<Position>(&a)?;
    let pb = world.objects.get_component::<Position>(&b)?;
    let (dx, dy) = ((pa.x - pb.x) as f64, (pa.y - pb.y) as f64);
    Some((dx * dx + dy * dy).sqrt())
}
