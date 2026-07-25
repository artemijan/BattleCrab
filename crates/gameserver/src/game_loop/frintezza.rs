//! Last Imperial Tomb (Frintezza) — the one Interlude instanced boss encounter
//! (Java `ai/bosses/Frintezza/LastImperialTomb`). Native state machine driven by
//! the thin [`crate::scripts::last_imperial_tomb`] QuestScript (talk/kill hooks),
//! mirroring Java's `LastImperialTomb extends AbstractInstance`.
//!
//! Slice 1 (this): entry + the room-crawl progression (`onKill` status 0→4).
//! The doors, the intro cinematic, and the boss fight are later slices (see
//! `docs/PLAN_FRINTEZZA.md`).

use crate::game_loop::helpers::instance_of;
use crate::game_loop::instances;
use crate::world::World;

pub(crate) const TEMPLATE_ID: i32 = 136;
/// The instance guide who lets a scroll-holder in.
pub(crate) const GUIDE: i32 = 32011;
/// The teleport cube spawned on victory; talking to it exits the instance.
pub(crate) const CUBE: i32 = 29061;
/// The alarm whose death opens the first room (Java `HALL_ALARM`).
const HALL_ALARM: i32 = 18328;

/// Monsters whose death drives the crawl (Java `ON_KILL_MONSTERS`): the alarm,
/// the suicidal soldier, and the room trash.
pub(crate) const ON_KILL_MONSTERS: &[i32] = &[
    HALL_ALARM, 18333, // HALL_KEEPER_SUICIDAL_SOLDIER
    18329, 18330, 18331, 18334, 18335, 18336, 18337, 18338, 18339,
];

/// GUIDE talk holding the scroll: build instance 136 and move the player in
/// (Java `onTalk` GUIDE → `enterInstance`). The default group (HALL_ALARM)
/// spawns with the instance. Returns whether the player was let in.
pub(crate) fn try_enter(world: &mut World, player_oid: i32) -> bool {
    let Some(instance_id) = instances::create_from_template(world, TEMPLATE_ID) else {
        return false;
    };
    instances::enter(world, player_oid, instance_id);
    true
}

/// CUBE talk: send the player back out (Java `teleportPlayerOut`).
pub(crate) fn exit(world: &mut World, player_oid: i32) {
    instances::exit(world, player_oid);
}

/// Java `onKill` for the crawl monsters — advance the room progression. Only the
/// dungeon status machine (0→4) is handled here; the boss-fight kill branches
/// (Scarlet2, demons, portraits) arrive with slice 4.
pub(crate) fn on_monster_killed(world: &mut World, killer_oid: i32, npc_id: i32) {
    let instance_id = instance_of(world, killer_oid);
    if instance_id == 0 {
        return;
    }
    let status = world.instances.status(instance_id);

    // The alarm falls: open the first room and pour its guards out.
    if npc_id == HALL_ALARM && status == 0 {
        world.instances.set_status(instance_id, 1);
        let spawned = instances::spawn_group(world, instance_id, "room1");
        set_monsters_count(world, instance_id, spawned.len());
        // TODO(frintezza slice 2): open FIRST_ROOM_DOORS.
        // TODO(frintezza slice 1+): reduceCurrentHp(1) nudge to aggro the room.
        return;
    }

    // A room-trash kill: Java reads the counter, decrements it, and advances the
    // status when the *old* value has already reached 0 (the last mob).
    let kill_count = world.instances.get_var(instance_id, "monstersCount");
    world
        .instances
        .set_var(instance_id, "monstersCount", kill_count - 1);
    if kill_count <= 0 {
        match status {
            1 => {
                world.instances.set_status(instance_id, 2);
                let spawned = instances::spawn_group(world, instance_id, "room2_part1");
                set_monsters_count(world, instance_id, spawned.len());
                // TODO(frintezza slice 2): open FIRST_ROUTE_DOORS.
            }
            2 => {
                world.instances.set_status(instance_id, 3);
                let spawned = instances::spawn_group(world, instance_id, "room2_part2");
                set_monsters_count(world, instance_id, spawned.len());
                // TODO(frintezza slice 2): open SECOND_ROOM_DOORS + aggro nudge.
            }
            3 => {
                world.instances.set_status(instance_id, 4);
                // TODO(frintezza slice 2): open SECOND_ROUTE_DOORS.
                // TODO(frintezza slice 3): arm FRINTEZZA_INTRO_START (10 min) →
                // the intro cinematic.
            }
            _ => {}
        }
    }

    // TODO(frintezza slice 4): 5% Dewdrop of Destruction drop (8556) — only
    // useful once the portrait/demon fight exists.
}

/// Java sets `monstersCount = getAliveNpcs().size() - 1`; right after a
/// `spawnGroup` the alive NPCs are exactly the group just spawned.
fn set_monsters_count(world: &mut World, instance_id: i32, spawned: usize) {
    let count = (spawned as i64 - 1).max(0);
    world.instances.set_var(instance_id, "monstersCount", count);
}
