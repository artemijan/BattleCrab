//! Queen Ant (`ai/bosses/QueenAnt`) — the first grand-boss script.
//!
//! The respawn lifecycle is shared (`grand_boss`); what is hers is the **larva
//! and the nurses**: a separate creature spawned beside her, and six healer
//! minions that keep both of them up. Killing the Queen through a working nurse
//! rotation is the actual fight.

use crate::model::components::{AdminFlags, Vitals};
use crate::scheduler::ScheduledTask;
use crate::world::World;

pub const QUEEN: i32 = 29001;
/// Queen Ant Larva — spawned by the script, not by the minion table.
pub const LARVA: i32 = 29002;
/// The healer minions. Declared as the Queen's minions in the NPC data, so
/// they spawn with her; this module only drives their healing.
pub const NURSE: i32 = 29003;

/// `_larva = addSpawn(LARVA, -21600, 179482, -5846, …)`.
const LARVA_X: i32 = -21600;
const LARVA_Y: i32 = 179482;
const LARVA_Z: i32 = -5846;

/// The Queen's home (`QUEEN_X/Y/Z`) — the anchor the leash check pulls her back
/// to.
const QUEEN_HOME: (i32, i32, i32) = (-21610, 181594, -5734);
/// Java `DISTANCE_CHECK`: dragged more than this from home, she resets.
const LEASH_RANGE: f64 = 2000.0;
/// The 5 s leash-check beat.
const DISTANCE_TICK_TICKS: u64 = 50;

/// `HEAL1`/`HEAL2` — both "Recovery". The larva gets either at random, the
/// Queen only ever gets `HEAL1`.
const HEAL1: i32 = 4020;
const HEAL2: i32 = 4024;

/// `startQuestTimer("heal", 1000, …, true)` — a 1 s beat.
const HEAL_TICK_TICKS: u64 = 10;

/// Called when the Queen is placed in the world: bring out the larva and start
/// the heal beat.
pub(crate) fn on_queen_spawned(world: &mut World, queen_oid: i32) {
    // Java `onSpawn(QUEEN)`: `getMinionList().spawnMinions("Privates")` — the
    // six nurses and eight royal guards. The grand-boss spawn path
    // (`spawn_npc_at`) deliberately skips a leader's `<minions>` escort, so
    // without this the Queen stands alone: no healers, no guards, no fight.
    crate::game_loop::minions::spawn_minions(world, queen_oid);

    let heading = world.roll(360);
    if let Some(larva) =
        crate::model::npc::spawn_npc_at(world, LARVA, LARVA_X, LARVA_Y, LARVA_Z, heading)
    {
        // Java `onSpawn(LARVA)`: immobilized + undying — the larva can't move and
        // can't be killed, so the nurses always have it to heal. Burning the
        // Queen down before the nurses out-heal her *is* the fight.
        let mut flags = AdminFlags::default();
        flags.paralyzed = true;
        flags.undying = true;
        world.objects.add_components(&larva, flags);
    }
    world.scheduler.schedule(
        world.tick + HEAL_TICK_TICKS,
        ScheduledTask::QueenAntHeal { queen_oid },
    );
    world.scheduler.schedule(
        world.tick + DISTANCE_TICK_TICKS,
        ScheduledTask::QueenAntDistanceCheck { queen_oid },
    );
}

/// Java `onKill(QUEEN)` tail: the immortal larva is finally removed with its
/// mistress (the shared respawn timer is armed by `grand_boss`).
pub(crate) fn on_queen_killed(world: &mut World) {
    if let Some(larva) = crate::game_loop::grand_boss::find_alive(world, LARVA) {
        crate::game_loop::death::despawn_npc_by_oid(world, larva);
    }
}

/// Java `DISTANCE_CHECK`: dragged more than `LEASH_RANGE` from home, the Queen
/// drops her hate and walks back — the anti-drag rule.
pub(crate) fn handle_distance_check(world: &mut World, queen_oid: i32) {
    if !crate::game_loop::grand_boss::leash_to_home(world, queen_oid, QUEEN_HOME, LEASH_RANGE) {
        return; // Java cancels the timer on death
    }
    world.scheduler.schedule(
        world.tick + DISTANCE_TICK_TICKS,
        ScheduledTask::QueenAntDistanceCheck { queen_oid },
    );
}

/// The `"heal"` timer — the nurse rotation.
///
/// Priority is **larva first, Queen second**, and that ordering is the fight:
/// a party that leaves the larva alive is fighting a Queen whose healers are
/// busy elsewhere.
///
/// Java also skips a nurse whose leader is the larva when healing the Queen —
/// on this dist the larva declares **no minions**, so no nurse can have it as a
/// leader and that branch is unreachable. Left out rather than written as dead
/// code, and recorded here so its absence is deliberate.
pub(crate) fn handle_heal_tick(world: &mut World, queen_oid: i32) {
    // Queen gone (killed, or the world moved on) → the beat stops.
    let queen_alive = world
        .objects
        .get_component::<Vitals>(&queen_oid)
        .is_some_and(|v| !v.dead);
    if !queen_alive {
        return;
    }

    let larva = crate::game_loop::grand_boss::find_alive(world, LARVA);
    let larva_needs = larva.is_some_and(|oid| wounded(world, oid));
    let queen_needs = wounded(world, queen_oid);

    if larva_needs || queen_needs {
        // The Queen's nurses, alive and not already casting.
        let nurses = nurses_of(world, queen_oid);
        for nurse in nurses {
            let (target, skill_id) = if larva_needs {
                // `getRandomBoolean() ? HEAL1 : HEAL2`
                let heal = if world.roll(2) == 0 { HEAL1 } else { HEAL2 };
                (larva.unwrap(), heal)
            } else {
                (queen_oid, HEAL1)
            };
            cast_heal(world, nurse, target, skill_id);
        }
    }

    world.scheduler.schedule(
        world.tick + HEAL_TICK_TICKS,
        ScheduledTask::QueenAntHeal { queen_oid },
    );
}

fn wounded(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<Vitals>(&oid)
        .is_some_and(|v| !v.dead && v.cur_hp < v.max_hp as f64)
}

/// Living nurse minions of this Queen.
fn nurses_of(world: &mut World, queen_oid: i32) -> Vec<i32> {
    let mut out = Vec::new();
    world.objects.for_each_mut::<(
        &crate::model::npc::Npc,
        &Vitals,
        &crate::game_loop::minions::MinionOf,
    )>(|(n, v, m)| {
        if n.npc_id == NURSE && !v.dead && m.0 == queen_oid {
            out.push(n.object_id);
        }
    });
    out
}

/// `nurse.setTarget(x); nurse.useMagic(heal)` — routed through the ordinary NPC
/// cast path, so a nurse's heal obeys the same MP cost and cooldown as any
/// other NPC skill rather than being a privileged script effect.
fn cast_heal(world: &mut World, nurse_oid: i32, target_oid: i32, skill_id: i32) {
    crate::game_loop::npc::cast::cast_skill(world, nurse_oid, target_oid, skill_id, 1);
}
