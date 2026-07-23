//! Queen Ant (`ai/bosses/QueenAnt`) — the first grand-boss script.
//!
//! The respawn lifecycle is shared (`grand_boss`); what is hers is the **larva
//! and the nurses**: a separate creature spawned beside her, and six healer
//! minions that keep both of them up. Killing the Queen through a working nurse
//! rotation is the actual fight.

use crate::model::components::Vitals;
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

/// `HEAL1`/`HEAL2` — both "Recovery". The larva gets either at random, the
/// Queen only ever gets `HEAL1`.
const HEAL1: i32 = 4020;
const HEAL2: i32 = 4024;

/// `startQuestTimer("heal", 1000, …, true)` — a 1 s beat.
const HEAL_TICK_TICKS: u64 = 10;

/// Called when the Queen is placed in the world: bring out the larva and start
/// the heal beat.
pub(crate) fn on_queen_spawned(world: &mut World, queen_oid: i32) {
    let heading = world.roll(360);
    crate::model::npc::spawn_npc_at(world, LARVA, LARVA_X, LARVA_Y, LARVA_Z, heading);
    world.scheduler.schedule(
        world.tick + HEAL_TICK_TICKS,
        ScheduledTask::QueenAntHeal { queen_oid },
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

    let larva = find_alive(world, LARVA);
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

/// The first living NPC of `npc_id` — the larva is unique, so "first" is "the".
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
    let Some(skill) = world.data.skill_data.get(skill_id, 1).cloned() else {
        return;
    };
    if !crate::game_loop::npc_cast::check_use_conditions_pub(world, nurse_oid, &skill) {
        return;
    }
    crate::game_loop::npc_cast::start_cast(world, nurse_oid, target_oid, &skill);
}
