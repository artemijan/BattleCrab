//! Sailren (`ai/bosses/Sailren`) — the wave-summon raid.
//!
//! Sailren isn't a standing boss: a party leader offers a **Gazkh** at Shilen's
//! Stone Statue, and the fight climbs a ladder of dinosaurs — three
//! Velociraptors, then a Pterosaur, then a Tyrannosaurus, then Sailren himself.
//! Each rung spawns the next on death.
//!
//! The kill-chain is **stateless** — it counts the living tagged mobs rather
//! than a kill counter, so it needs no global fight state. The wave mobs also
//! spawn in the open world, so a [`SailrenWaveMob`] marker scopes the chain to
//! this fight (and doubles as the `IN_FIGHT` gate). Slice 1: the wave ladder +
//! Sailren's guarded entrance. Slice 2: the Statue (32109) entry — leader +
//! Gazkh + party teleport-in ([`crate::scripts::sailren_altar`]). The zone
//! time-out/decay and the respawn lock remain a later slice.
//!
//! [`SailrenWaveMob`]: SailrenWaveMob
use crate::game_loop::common::near_leader;
use crate::model::components::{AdminFlags, Immobilized, SailrenWaveMob, Vitals};
use crate::scheduler::ScheduledTask;
use crate::world::World;

const SAILREN: i32 = 29065;
const VELOCIRAPTOR: i32 = 22218;
const PTEROSAUR: i32 = 22199;
const TREX: i32 = 22217;
/// Teleportation Cubic — the exit spawned when Sailren falls.
const CUBIC: i32 = 32107;
/// Shilen's Stone Statue — the entry NPC.
pub(crate) const STATUE: i32 = 32109;
/// Gazkh — the party leader's entry ticket.
const GAZKH: i32 = 8784;

/// Where the admitted party lands (Java `teleToLocation(27549, -6638, -2008)`).
const ENTER_LOC: (i32, i32, i32) = (27549, -6638, -2008);
/// Java delays the first wave 60 s after the party enters.
const FIRST_WAVE_MS: u64 = 60_000;

/// Where the dinosaurs enter (Java `addSpawn(..., 27313, -6766, -1975)`).
const NEST: (i32, i32, i32) = (27313, -6766, -1975);
/// Sailren's own entrance spot.
const SAILREN_POS: (i32, i32, i32) = (27549, -6638, -2008);
/// The exit cube's spot.
const CUBIC_POS: (i32, i32, i32) = (27644, -6638, -2008);

/// Java `startQuestTimer("SPAWN_SAILREN", 180000)` after the Trex falls.
const SAILREN_DELAY_MS: u64 = 180_000;
/// Java `startQuestTimer("ATTACK", 24600, sailren)` — the intro's length.
const ATTACK_ENABLE_MS: u64 = 24_600;
/// The engage weight for `addAttackPlayerDesire`.
const ENGAGE_HATE: f64 = 999.0;

/// Java `onEvent("enter")`: the refusal html for a party that can't start (or
/// `None` when the leader may enter). A fight is "in progress" while any wave
/// mob is alive — the marker doubles as the `IN_FIGHT` status, so no global
/// state is needed.
pub(crate) fn entry_refusal(world: &mut World, leader_oid: i32) -> Option<&'static str> {
    let Some((leader, _)) = crate::game_loop::party::leader_and_members(world, leader_oid) else {
        return Some("32109-01.html"); // not in a party
    };
    if fight_active(world) {
        return Some("32109-05.html"); // a fight is already underway
    }
    if leader != leader_oid {
        return Some("32109-03.html"); // only the leader may enter
    }
    if gazkh_count(world, leader_oid) == 0 {
        return Some("32109-02.html"); // no Gazkh
    }
    None
}

/// Admit the leader's nearby party members and arm the first wave (Java teleports
/// each member within 1000, then `SPAWN_VELOCIRAPTOR` 60 s out). The Gazkh is
/// taken by the caller (it needs the inventory-aware `QuestCtx`).
pub(crate) fn enter_party(world: &mut World, leader_oid: i32) {
    let Some((_, members)) = crate::game_loop::party::leader_and_members(world, leader_oid) else {
        return;
    };
    // Gather the near members *before* teleporting anyone — teleporting the
    // leader first would move the reference point and strand the rest.
    let near: Vec<i32> = members
        .into_iter()
        .filter(|&m| near_leader(world, leader_oid, m))
        .collect();
    for member in near {
        crate::game_loop::death::teleport_player(
            world,
            member,
            ENTER_LOC.0,
            ENTER_LOC.1,
            ENTER_LOC.2,
        );
    }
    world.scheduler.schedule(
        world.tick + crate::scheduler::ms_to_ticks(FIRST_WAVE_MS),
        ScheduledTask::SailrenBeginFight,
    );
}

/// A Sailren fight is underway while any tagged wave mob is alive.
fn fight_active(world: &mut World) -> bool {
    let mut active = false;
    world
        .objects
        .for_each_mut::<(&SailrenWaveMob, &Vitals)>(|(_, v)| {
            if !v.dead {
                active = true;
            }
        });
    active
}

fn gazkh_count(world: &World, oid: i32) -> i64 {
    crate::game_loop::helpers::count_of(world, oid, GAZKH)
}

/// Begin the encounter: the first three Velociraptors enter the nest.
pub(crate) fn begin_fight(world: &mut World) {
    for _ in 0..3 {
        let dx = world.roll(150);
        let dy = world.roll(150);
        spawn_wave_mob(world, VELOCIRAPTOR, NEST.0 + dx, NEST.1 + dy, NEST.2);
    }
}

/// Java `onKill` for a tagged wave mob — advance the ladder.
pub(crate) fn on_wave_kill(world: &mut World, killer_oid: i32, npc_id: i32) {
    match npc_id {
        // The last of the three Velociraptors summons the Pterosaur.
        VELOCIRAPTOR => {
            if count_alive(world, VELOCIRAPTOR) == 0 {
                let ptero = spawn_wave_mob(world, PTEROSAUR, NEST.0, NEST.1, NEST.2);
                engage(world, ptero, killer_oid);
            }
        }
        // The Pterosaur yields the Tyrannosaurus.
        PTEROSAUR => {
            let trex = spawn_wave_mob(world, TREX, NEST.0, NEST.1, NEST.2);
            engage(world, trex, killer_oid);
        }
        // The Tyrannosaurus falling arms Sailren's entrance, 3 minutes out.
        TREX => {
            world.scheduler.schedule(
                world.tick + crate::scheduler::ms_to_ticks(SAILREN_DELAY_MS),
                ScheduledTask::SailrenSpawn,
            );
        }
        // Sailren himself: drop the exit cube (the respawn lock is a later slice).
        SAILREN => {
            crate::game_loop::npc::spawn_npc_at(
                world,
                CUBIC,
                CUBIC_POS.0,
                CUBIC_POS.1,
                CUBIC_POS.2,
                0,
            );
        }
        _ => {}
    }
}

/// `SPAWN_SAILREN`: Sailren enters invulnerable and rooted for the intro; the
/// `ATTACK` timer then lets him fight. (The `SpecialCamera` movie is abbreviated,
/// as with the other cinematic bosses.)
pub(crate) fn handle_spawn_sailren(world: &mut World) {
    let Some(sailren) = crate::game_loop::npc::spawn_npc_at(
        world,
        SAILREN,
        SAILREN_POS.0,
        SAILREN_POS.1,
        SAILREN_POS.2,
        0,
    ) else {
        return;
    };
    world.objects.add_components(&sailren, SailrenWaveMob);
    world.objects.add_components(
        &sailren,
        (
            AdminFlags {
                invul: true,
                ..Default::default()
            },
            Immobilized,
        ),
    );
    world.scheduler.schedule(
        world.tick + crate::scheduler::ms_to_ticks(ATTACK_ENABLE_MS),
        ScheduledTask::SailrenAttackEnable {
            sailren_oid: sailren,
        },
    );
}

/// `ATTACK`: the intro is over — Sailren can be hurt and can move.
pub(crate) fn handle_attack_enable(world: &mut World, sailren_oid: i32) {
    if let Some(f) = world.objects.get_component_mut::<AdminFlags>(&sailren_oid) {
        f.invul = false;
    }
    world.objects.remove_component::<Immobilized>(&sailren_oid);
}

/// Spawn a wave mob and tag it into this fight.
fn spawn_wave_mob(world: &mut World, npc_id: i32, x: i32, y: i32, z: i32) -> i32 {
    let oid = crate::game_loop::npc::spawn_npc_at(world, npc_id, x, y, z, 0).unwrap_or(0);
    if oid != 0 {
        world.objects.add_components(&oid, SailrenWaveMob);
    }
    oid
}

/// `addAttackPlayerDesire(mob, killer)` — the new arrival makes a beeline for
/// whoever felled the last one.
fn engage(world: &mut World, mob: i32, killer_oid: i32) {
    if mob != 0 {
        crate::game_loop::npc::minions::add_hate(world, mob, killer_oid, ENGAGE_HATE);
    }
}

/// Count the living tagged mobs of `npc_id` (the just-killed one is already
/// flagged dead, so it isn't counted).
fn count_alive(world: &mut World, npc_id: i32) -> usize {
    let mut n = 0;
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &Vitals, &SailrenWaveMob)>(|(mob, v, _)| {
            if mob.npc_id == npc_id && !v.dead {
                n += 1;
            }
        });
    n
}
