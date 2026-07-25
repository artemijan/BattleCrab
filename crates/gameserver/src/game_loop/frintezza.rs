//! Last Imperial Tomb (Frintezza) — the one Interlude instanced boss encounter
//! (Java `ai/bosses/Frintezza/LastImperialTomb`). Native state machine driven by
//! the thin [`crate::scripts::last_imperial_tomb`] QuestScript (talk/kill hooks),
//! mirroring Java's `LastImperialTomb extends AbstractInstance`.
//!
//! Landed: entry + the room-crawl progression (`onKill` status 0→4, slice 1),
//! per-instance doors (slice 2), and the intro cinematic step machine (slice 3).
//! The boss fight (Scarlet morphs, songs, demons/portraits) and the finish are
//! later slices (see `docs/PLAN_FRINTEZZA.md`).

use crate::game_loop::helpers::{instance_of, ms_to_ticks};
use crate::game_loop::instances;
use crate::model::components::{AdminFlags, Movement, Position};
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
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

// The four door groups the crawl opens as each room is cleared.
const FIRST_ROOM_DOORS: &[i32] = &[
    17130051, 17130052, 17130053, 17130054, 17130055, 17130056, 17130057, 17130058,
];
const SECOND_ROOM_DOORS: &[i32] = &[
    17130061, 17130062, 17130063, 17130064, 17130065, 17130066, 17130067, 17130068, 17130069,
    17130070,
];
const FIRST_ROUTE_DOORS: &[i32] = &[17130042, 17130043];
const SECOND_ROUTE_DOORS: &[i32] = &[17130045, 17130046];

/// Open every door in a group for this instance (Java `world.openCloseDoor`).
fn open_doors(world: &mut World, instance_id: i32, doors: &[i32]) {
    for &door_id in doors {
        instances::open_close_door(world, instance_id, door_id, true);
    }
}

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
        open_doors(world, instance_id, FIRST_ROOM_DOORS);
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
                open_doors(world, instance_id, FIRST_ROUTE_DOORS);
            }
            2 => {
                world.instances.set_status(instance_id, 3);
                let spawned = instances::spawn_group(world, instance_id, "room2_part2");
                set_monsters_count(world, instance_id, spawned.len());
                open_doors(world, instance_id, SECOND_ROOM_DOORS);
                // TODO(frintezza slice 1+): reduceCurrentHp(1) nudge to aggro.
            }
            3 => {
                world.instances.set_status(instance_id, 4);
                open_doors(world, instance_id, SECOND_ROUTE_DOORS);
                // The arena is cleared: after the wait, Frintezza's entrance.
                begin_intro(world, instance_id);
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

// ---------------------------------------------------------------------------
// The intro cinematic (Java `FRINTEZZA_INTRO_*`).
//
// Ported faithfully in structure — the 10-minute wait, the player freeze, the
// staged spawns (dummies → Frintezza → Scarlet → portraits), the prelude skill
// and social beats, and the hand-back that starts the fight — as a
// `ScheduledTask::FrintezzaIntro` step machine. The exhaustive dummy-anchored
// `SpecialCamera` choreography (~20 shots) is abbreviated to the establishing
// beats; the remaining cosmetics are a TODO.
// ---------------------------------------------------------------------------

const FRINTEZZA: i32 = 29045;
const SCARLET1: i32 = 29046;
/// `PORTRAIT_SPAWNS` — portrait `(id,x,y,z,heading)` then its demon (`id + 2`)
/// `(x,y,z,heading)`.
const PORTRAIT_SPAWNS: [[i32; 9]; 4] = [
    [
        29048, -89381, -153981, -9168, 3368, -89378, -153968, -9168, 3368,
    ],
    [
        29048, -86234, -152467, -9168, 37656, -86261, -152492, -9168, 37656,
    ],
    [
        29049, -89342, -152479, -9168, -5152, -89311, -152491, -9168, -5152,
    ],
    [
        29049, -86189, -153968, -9168, 29456, -86217, -153956, -9168, 29456,
    ],
];
const CUBE_POS: (i32, i32, i32) = (-87904, -141296, -9168);
const FRINTEZZA_POS: (i32, i32, i32, i32) = (-87780, -155086, -9080, 16384);
const SCARLET_POS: (i32, i32, i32, i32) = (-87789, -153295, -9176, 16384);
const PRELUDE_SKILL: i32 = 5006;
/// Java `FRINTEZZA_WAIT_TIME` — 10 minutes after the arena is cleared.
const INTRO_WAIT_MS: u64 = 10 * 60 * 1000;

/// Arm the intro (Java `startQuestTimer("FRINTEZZA_INTRO_START", 10 min)`).
fn begin_intro(world: &mut World, instance_id: i32) {
    schedule_intro(world, instance_id, 0, INTRO_WAIT_MS);
}

fn schedule_intro(world: &mut World, instance_id: i32, step: u8, delay_ms: u64) {
    world.scheduler.schedule(
        world.tick + ms_to_ticks(delay_ms as i32).max(1),
        ScheduledTask::FrintezzaIntro { instance_id, step },
    );
}

/// One beat of the intro. The steps collapse Java's `FRINTEZZA_INTRO_1..20` into
/// the functional milestones (the abbreviated cameras aside), preserving the
/// spawn order, the player freeze window, and the cumulative timing.
pub(crate) fn handle_intro_step(world: &mut World, instance_id: i32, step: u8) {
    if !world.instances.contains(instance_id) {
        return; // the instance was torn down mid-cinematic
    }
    match step {
        // INTRO_START (+INTRO_1): the earth shakes, the arena seals, and the
        // teleport cube appears for anyone who wants out.
        0 => {
            instances::broadcast_to_instance(
                world,
                instance_id,
                &server_packets::earthquake(-87784, -155083, -9087, 45, 27),
            );
            for group in [
                FIRST_ROOM_DOORS,
                FIRST_ROUTE_DOORS,
                SECOND_ROOM_DOORS,
                SECOND_ROUTE_DOORS,
            ] {
                for &door_id in group {
                    instances::open_close_door(world, instance_id, door_id, false);
                }
            }
            instances::spawn_npc(
                world,
                instance_id,
                CUBE,
                CUBE_POS.0,
                CUBE_POS.1,
                CUBE_POS.2,
                0,
            );
            schedule_intro(world, instance_id, 1, 20_000);
        }
        // INTRO_2: freeze the party and raise Frintezza (invulnerable) plus the
        // four immobilized demons.
        1 => {
            disable_players(world, instance_id);
            if let Some(frintezza) = spawn_frozen(
                world,
                instance_id,
                FRINTEZZA,
                FRINTEZZA_POS.0,
                FRINTEZZA_POS.1,
                FRINTEZZA_POS.2,
                FRINTEZZA_POS.3,
                true,
            ) {
                world
                    .instances
                    .set_var(instance_id, "frintezza", frintezza as i64);
                camera(
                    world,
                    instance_id,
                    frintezza,
                    0,
                    75,
                    -89,
                    0,
                    100,
                    0,
                    0,
                    1,
                    0,
                    0,
                );
            }
            for (i, s) in PORTRAIT_SPAWNS.iter().enumerate() {
                if let Some(demon) =
                    spawn_frozen(world, instance_id, s[0] + 2, s[5], s[6], s[7], s[8], false)
                {
                    world
                        .instances
                        .set_var(instance_id, &format!("demon{i}"), demon as i64);
                }
            }
            schedule_intro(world, instance_id, 2, 40_000);
        }
        // INTRO_12: the Mournful Chorale Prelude — screen text + Frintezza's cast.
        2 => {
            let frintezza = var_oid(world, instance_id, "frintezza");
            instances::broadcast_to_instance(
                world,
                instance_id,
                &server_packets::ex_show_screen_message("Mournful Chorale Prelude", 2, 5000),
            );
            if frintezza != 0 {
                if let Some(p) = world.objects.get_component::<Position>(&frintezza).copied() {
                    let src = (frintezza, p.x, p.y, p.z);
                    instances::broadcast_to_instance(
                        world,
                        instance_id,
                        &server_packets::magic_skill_use_raw(src, src, PRELUDE_SKILL, 1, 34000),
                    );
                }
                instances::broadcast_to_instance(
                    world,
                    instance_id,
                    &server_packets::social_action(frintezza, 3),
                );
            }
            schedule_intro(world, instance_id, 3, 25_000);
        }
        // INTRO_16: Scarlet takes the field (still invulnerable/immobile).
        3 => {
            if let Some(scarlet) = spawn_frozen(
                world,
                instance_id,
                SCARLET1,
                SCARLET_POS.0,
                SCARLET_POS.1,
                SCARLET_POS.2,
                SCARLET_POS.3,
                true,
            ) {
                world
                    .instances
                    .set_var(instance_id, "activeScarlet", scarlet as i64);
                instances::broadcast_to_instance(
                    world,
                    instance_id,
                    &server_packets::social_action(scarlet, 3),
                );
                camera(
                    world,
                    instance_id,
                    scarlet,
                    300,
                    60,
                    8,
                    0,
                    10000,
                    0,
                    0,
                    1,
                    0,
                    0,
                );
            }
            schedule_intro(world, instance_id, 4, 9_000);
        }
        // INTRO_19: the four portrait pillars appear.
        4 => {
            for (i, s) in PORTRAIT_SPAWNS.iter().enumerate() {
                if let Some(portrait) =
                    instances::spawn_npc(world, instance_id, s[0], s[1], s[2], s[3], s[4])
                {
                    world
                        .instances
                        .set_var(instance_id, &format!("portrait{i}"), portrait as i64);
                }
            }
            schedule_intro(world, instance_id, 5, 2_000);
        }
        // INTRO_20: hand control back — Scarlet and the demons wake, Frintezza
        // keeps its invulnerability, and the fight is on.
        5 => {
            for i in 0..PORTRAIT_SPAWNS.len() {
                let demon = var_oid(world, instance_id, &format!("demon{i}"));
                if demon != 0 {
                    set_frozen(world, demon, false, false);
                }
            }
            let scarlet = var_oid(world, instance_id, "activeScarlet");
            if scarlet != 0 {
                set_frozen(world, scarlet, false, false);
            }
            enable_players(world, instance_id);
            world.instances.set_var(instance_id, "fightActive", 1);
            // TODO(frintezza slice 4): arm PLAY_RANDOM_SONG + SPAWN_DEMONS and
            // Scarlet's morph/attack hooks.
        }
        _ => {}
    }
}

/// One establishing `SpecialCamera` shot to the instance (Java's `broadcastPacket
/// (world, new SpecialCamera(...))`).
#[allow(clippy::too_many_arguments)]
fn camera(
    world: &World,
    instance_id: i32,
    oid: i32,
    force: i32,
    a1: i32,
    a2: i32,
    time: i32,
    range: i32,
    dur: i32,
    ry: i32,
    rp: i32,
    wide: i32,
    rangle: i32,
) {
    instances::broadcast_to_instance(
        world,
        instance_id,
        &server_packets::special_camera(
            oid, force, a1, a2, time, range, dur, ry, rp, wide, rangle, 0,
        ),
    );
}

/// `disablePlayers` — freeze every member for the cinematic (Java also aborts
/// their cast/attack and clears the target; the paralyze flag blocks both).
fn disable_players(world: &mut World, instance_id: i32) {
    for member in instance_members(world, instance_id) {
        world.objects.remove_component::<Movement>(&member);
        set_paralyzed(world, member, true);
    }
}

/// `enablePlayers` — give control back once the cinematic ends.
fn enable_players(world: &mut World, instance_id: i32) {
    for member in instance_members(world, instance_id) {
        set_paralyzed(world, member, false);
    }
}

fn instance_members(world: &World, instance_id: i32) -> Vec<i32> {
    world
        .instances
        .get(instance_id)
        .map(|i| i.members.keys().copied().collect())
        .unwrap_or_default()
}

/// Spawn a cinematic actor already frozen (immobilized, and optionally
/// invulnerable) so it holds its pose until the fight begins.
fn spawn_frozen(
    world: &mut World,
    instance_id: i32,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    invul: bool,
) -> Option<i32> {
    let oid = instances::spawn_npc(world, instance_id, npc_id, x, y, z, heading)?;
    set_frozen(world, oid, true, invul);
    Some(oid)
}

/// Toggle an NPC's paralyze (+ invulnerability) — Java `setImmobilized` /
/// `disableAllSkills` (+ `setInvul`).
fn set_frozen(world: &mut World, oid: i32, frozen: bool, invul: bool) {
    let mut flags = admin_flags(world, oid);
    flags.paralyzed = frozen;
    if invul {
        flags.invul = frozen;
    }
    world.objects.add_components(&oid, flags);
}

fn set_paralyzed(world: &mut World, player_oid: i32, on: bool) {
    let mut flags = admin_flags(world, player_oid);
    flags.paralyzed = on;
    world.objects.add_components(&player_oid, flags);
    crate::game_loop::party::broadcast_user_info(world, player_oid);
}

fn admin_flags(world: &World, oid: i32) -> AdminFlags {
    world
        .objects
        .get_component::<AdminFlags>(&oid)
        .copied()
        .unwrap_or_default()
}

/// Read an object-ref parameter stored as an object id (0 when unset).
fn var_oid(world: &World, instance_id: i32, key: &str) -> i32 {
    world.instances.get_var(instance_id, key) as i32
}
