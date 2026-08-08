//! `ai/others/Servitors/SinEater` — the Sin Eater's running commentary.
//!
//! Pure flavour, but it is the one pet in the game with a voice, and the
//! hooks are all one-liners on paths that already exist: the pet summon, the
//! damage entry, the pet-death penalty, plus a 60 s chatter beat.
//!
//! Java hangs these off `addSummonSpawnId` / `ON_CREATURE_ATTACKED` /
//! `ON_CREATURE_DEATH` / `onSummonTalk`; this port has no summon-event
//! registry, so each site calls in directly.

use crate::game_loop::helpers::region_cell_of;
use crate::model::npc::Npc;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// The Sin Eater's npc/display id.
pub(crate) const SIN_EATER: i32 = 12564;

/// `startQuestTimer("TALK", 60000, …)`, in ticks.
const TALK_BEAT_TICKS: u64 = 600;

/// `onSummonSpawn` — one of two greetings, then the chatter beat starts.
pub(crate) fn on_spawn(world: &mut World, pet_oid: i32) {
    if !is_sin_eater(world, pet_oid) {
        return;
    }
    let line = if world.roll(2) == 0 { 42231 } else { 42232 };
    say(world, pet_oid, line);
    world.scheduler.schedule(
        world.tick + TALK_BEAT_TICKS,
        ScheduledTask::SinEaterTalk { pet_oid },
    );
}

/// The `"TALK"` beat: a 30 % chance of one of five gripes, then re-arm. Stops
/// when the pet is gone (Java cancels the timer when the owner has no pet).
pub(crate) fn handle_talk_beat(world: &mut World, pet_oid: i32) {
    if !is_sin_eater(world, pet_oid) {
        return;
    }
    if world.roll(100) < 30 {
        let line = match world.roll(100) {
            0..=19 => 42243,
            20..=39 => 42244,
            40..=59 => 42245,
            60..=79 => 42246,
            _ => 42247,
        };
        say(world, pet_oid, line);
    }
    world.scheduler.schedule(
        world.tick + TALK_BEAT_TICKS,
        ScheduledTask::SinEaterTalk { pet_oid },
    );
}

/// `ON_CREATURE_ATTACKED` — a 30 % chance of complaining about the hit.
pub(crate) fn on_attacked(world: &mut World, pet_oid: i32) {
    if !is_sin_eater(world, pet_oid) || world.roll(100) >= 30 {
        return;
    }
    let line = match world.roll(100) {
        0..=34 => 42233,
        35..=69 => 42234,
        _ => 42235,
    };
    say(world, pet_oid, line);
}

/// `ON_CREATURE_DEATH` — always one of three parting shots.
pub(crate) fn on_death(world: &mut World, pet_oid: i32) {
    if !is_sin_eater(world, pet_oid) {
        return;
    }
    let line = match world.roll(100) {
        0..=29 => 42236,
        30..=69 => 42237,
        _ => 42238,
    };
    say(world, pet_oid, line);
}

/// Java's `onSummonTalk` — the owner interacting with their own Sin Eater
/// has a 10 % chance of one of four more lines, 25 % each (strings
/// 42239–42242). Reached from `interact_with_npc`'s own-summon branch, the
/// port's `ON_PLAYER_SUMMON_TALK`.
pub(crate) fn on_summon_talk(world: &mut World, pet_oid: i32) {
    if !is_sin_eater(world, pet_oid) {
        return;
    }
    if world.roll(100) >= 10 {
        return;
    }
    let line = match world.roll(100) {
        0..=24 => 42239,
        25..=49 => 42240,
        50..=74 => 42241,
        _ => 42242,
    };
    say(world, pet_oid, line);
}

fn is_sin_eater(world: &World, pet_oid: i32) -> bool {
    world
        .objects
        .get_component::<Npc>(&pet_oid)
        .is_some_and(|n| n.npc_id == SIN_EATER)
}

/// `summon.broadcastPacket(new NpcSay(objectId, NPC_GENERAL, id, string))`.
fn say(world: &World, pet_oid: i32, npc_string_id: i32) {
    let Some(region) = region_cell_of(world, pet_oid) else {
        return;
    };
    let pkt = server_packets::npc_say(pet_oid, SIN_EATER, npc_string_id);
    crate::game_loop::helpers::broadcast_near_region(world, region, &pkt);
}
