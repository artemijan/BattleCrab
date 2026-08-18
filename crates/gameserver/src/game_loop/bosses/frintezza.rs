//! Last Imperial Tomb (Frintezza) — the one Interlude instanced boss encounter
//! (Java `ai/bosses/Frintezza/LastImperialTomb`). Native state machine driven by
//! the thin [`crate::scripts::last_imperial_tomb`] QuestScript (talk/kill hooks),
//! mirroring Java's `LastImperialTomb extends AbstractInstance`.
//!
//! Playable end-to-end: entry + the room-crawl (`onKill` status 0→4, slice 1),
//! per-instance doors (slice 2), the intro cinematic (slice 3), Scarlet's
//! 80%/20% morphs → final form (slice 4), the fight loops — songs + demon/portrait
//! ecosystem + Dewdrop suicide (slice 4b) — and the finish cinematic (Frintezza's
//! death → doors reopen, slice 5), Scarlet's custom daemon-skill AI (Java
//! `ScarletVanHalisha`), and the crawl polish — the room aggro-nudge, the 5%
//! Dewdrop drop, and the song debuff (5008). Songs play at all four of Java's
//! sites — the intro, both Scarlet morphs, and the 90 s timer. The only gap
//! left is cosmetic: the exhaustive dummy-anchored `SpecialCamera` choreography
//! is abbreviated throughout.

use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::hp_pair;
use crate::game_loop::helpers::instance_of;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::helpers::skill_by_id;
use crate::game_loop::instances;
use crate::model::components::{AdminFlags, Movement, Position};
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::scheduler::ms_to_ticks;
use crate::world::World;

pub(crate) const TEMPLATE_ID: i32 = 136;
/// The instance guide who lets a scroll-holder in.
pub(crate) const GUIDE: i32 = 32011;
/// The teleport cube spawned on victory; talking to it exits the instance.
pub(crate) const CUBE: i32 = 29061;
/// The alarm whose death opens the first room (Java `HALL_ALARM`).
const HALL_ALARM: i32 = 18328;

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

/// Java `onKill` for the crawl monsters — advance the room progression. Only the
/// dungeon status machine (0→4) is handled here; the boss-fight kill branches
/// (Scarlet2, demons, portraits) arrive with slice 4.
pub(crate) fn on_monster_killed(world: &mut World, killer_oid: i32, npc_oid: i32, npc_id: i32) {
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
        aggro_room(world, &spawned, killer_oid);
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
                aggro_room(world, &spawned, killer_oid);
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

    // Java `if (getRandom(100) < 5) npc.dropItem(killer, DEWDROP, 1)` — the crawl
    // trash sometimes yields a Dewdrop of Destruction (used on the portraits).
    if world.roll(100) < 5
        && let Some(pos) = maybe_position(world, npc_oid)
    {
        crate::game_loop::ground_items::spawn_ground_item(
            world,
            DEWDROP_ITEM,
            1,
            0,
            pos.x,
            pos.y,
            pos.z,
            npc_oid,
            crate::game_loop::ground_items::DropSource::Npc,
        );
    }
}

/// `reduceCurrentHp(1, killer, null)` on a freshly-spawned room — a nudge that
/// makes every guard aggro the intruder at once.
fn aggro_room(world: &mut World, room: &[i32], killer_oid: i32) {
    for &mob in room {
        crate::game_loop::minions::add_hate(world, mob, killer_oid, 1.0);
    }
}

/// Dewdrop of Destruction — the portrait-slaying consumable dropped by the crawl.
const DEWDROP_ITEM: i32 = 8556;

/// Java sets `monstersCount = getAliveNpcs().size() - 1`; right after a
/// `spawnGroup` the alive NPCs are exactly the group just spawned.
fn set_monsters_count(world: &mut World, instance_id: i32, spawned: usize) {
    let count = (spawned as i64 - 1).max(0);
    world.instances.set_var(instance_id, "monstersCount", count);
}

// ---------------------------------------------------------------------------
// The intro cinematic (Java `FRINTEZZA_INTRO_*`).
//
// A step-for-step port of Java's `FRINTEZZA_INTRO_START` and `_1`..`_20`, on a
// `ScheduledTask::FrintezzaIntro` machine: the 10-minute wait, the player
// freeze, the five camera dummies, the staged spawns (Frintezza → demons →
// Scarlet → portraits), all 20 `SpecialCamera` shots, the prelude skill and
// social beats, and the hand-back that starts the fight.
//
// Two things to know before touching the shots: they use Java's *11-argument*
// `SpecialCamera` overload, which swaps two of its own parameters (see
// [`camera`]), and most of them are anchored on invisible `DUMMY` actors that
// are deleted the moment their last shot lands.
// ---------------------------------------------------------------------------

const FRINTEZZA: i32 = 29045;
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
/// Java `DUMMY` / `DUMMY2` — invisible actors that exist only to anchor camera
/// shots. The cinematic frames Frintezza's hall from points no real NPC stands
/// at, so the shots hang off these and Java deletes each one the moment its
/// last shot has played.
const DUMMY: i32 = 29052;
const DUMMY2: i32 = 29053;
const FRINTEZZA_DUMMY_POS: (i32, i32, i32, i32) = (-87784, -155083, -9087, 16048);
const OVERHEAD_DUMMY_POS: (i32, i32, i32, i32) = (-87784, -153298, -9175, 16384);
const PORTRAIT_DUMMY1_POS: (i32, i32, i32, i32) = (-89566, -153168, -9165, 16048);
const PORTRAIT_DUMMY3_POS: (i32, i32, i32, i32) = (-86004, -153168, -9165, 16048);
/// `sendPacketX`'s pivot — players west of this x get the left-hand shot.
const PORTRAIT_SPLIT_X: i32 = -87784;
/// The skill `scarletDummy` casts at `overheadDummy` as Scarlet rises.
const SCARLET_RISE_SKILL: i32 = 5004;
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
        world.tick + ms_to_ticks(delay_ms).max(1),
        ScheduledTask::FrintezzaIntro { instance_id, step },
    );
}

/// One beat of the intro — Java's `FRINTEZZA_INTRO_START` and `_1`..`_20`, one
/// step each and with Java's own delays, so the whole cinematic runs to the
/// same 82 s it does upstream. (It used to collapse into six functional
/// milestones with the camera work abbreviated; the collapse also drifted the
/// total by about 14 s.)
pub(crate) fn handle_intro_step(world: &mut World, instance_id: i32, step: u8) {
    if !world.instances.contains(instance_id) {
        return; // the instance was torn down mid-cinematic
    }
    let frintezza = || var_oid(world, instance_id, "frintezza");
    match step {
        // INTRO_START: the earth shakes. Java arms _1 and _2 as two independent
        // timers off this one, so the door/cube beat and the actor spawn are not
        // chained — they are 17 s and 20 s from here.
        0 => {
            instances::broadcast_to_instance(
                world,
                instance_id,
                &server_packets::earthquake(-87784, -155083, -9087, 45, 27),
            );
            schedule_intro(world, instance_id, 1, 17_000);
            schedule_intro(world, instance_id, 2, 20_000);
        }
        // INTRO_1: the arena seals and the teleport cube appears for anyone who
        // wants out.
        1 => {
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
        }
        // INTRO_2: the camera dummies go up, the party freezes, and Frintezza
        // rises with the four immobilised demons.
        2 => {
            for (key, id, pos) in [
                ("frintezzaDummy", DUMMY, FRINTEZZA_DUMMY_POS),
                ("overheadDummy", DUMMY, OVERHEAD_DUMMY_POS),
                ("portraitDummy1", DUMMY, PORTRAIT_DUMMY1_POS),
                ("portraitDummy3", DUMMY, PORTRAIT_DUMMY3_POS),
                ("scarletDummy", DUMMY2, OVERHEAD_DUMMY_POS),
            ] {
                if let Some(oid) =
                    instances::spawn_npc(world, instance_id, id, pos.0, pos.1, pos.2, pos.3)
                {
                    world.instances.set_var(instance_id, key, oid as i64);
                }
            }
            // `overheadDummy.setCollisionHeight(600)` + its `NpcInfo` resend —
            // the shot that looks down the hall is framed off this inflated
            // box, so without it the angle is wrong rather than merely missing.
            let overhead = var_oid(world, instance_id, "overheadDummy");
            if overhead != 0 {
                if let Some(c) = world
                    .objects
                    .get_component_mut::<crate::model::components::Collision>(&overhead)
                {
                    c.height = 600.0;
                }
                if let Some(pkt) = crate::game_loop::visibility::npc_info_bytes(world, overhead) {
                    instances::broadcast_to_instance(world, instance_id, &pkt);
                }
            }
            disable_players(world, instance_id);
            camera(
                world,
                instance_id,
                overhead,
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
            camera(
                world,
                instance_id,
                overhead,
                300,
                90,
                -10,
                6500,
                7000,
                0,
                0,
                1,
                0,
                0,
            );

            if let Some(f) = spawn_frozen(
                world,
                instance_id,
                FRINTEZZA,
                FRINTEZZA_POS.0,
                FRINTEZZA_POS.1,
                FRINTEZZA_POS.2,
                FRINTEZZA_POS.3,
                true,
            ) {
                world.instances.set_var(instance_id, "frintezza", f as i64);
            }
            for (i, sp) in PORTRAIT_SPAWNS.iter().enumerate() {
                if let Some(demon) = spawn_frozen(
                    world,
                    instance_id,
                    sp[0] + 2,
                    sp[5],
                    sp[6],
                    sp[7],
                    sp[8],
                    false,
                ) {
                    world
                        .instances
                        .set_var(instance_id, &format!("demon{i}"), demon as i64);
                }
            }
            schedule_intro(world, instance_id, 3, 6_500);
        }
        // INTRO_3/_4: the long push down the hall onto Frintezza's dais.
        3 => {
            let d = var_oid(world, instance_id, "frintezzaDummy");
            camera(
                world,
                instance_id,
                d,
                1800,
                90,
                8,
                6500,
                7000,
                0,
                0,
                1,
                0,
                0,
            );
            schedule_intro(world, instance_id, 4, 900);
        }
        4 => {
            let d = var_oid(world, instance_id, "frintezzaDummy");
            camera(
                world,
                instance_id,
                d,
                140,
                90,
                10,
                2500,
                4500,
                0,
                0,
                1,
                0,
                0,
            );
            schedule_intro(world, instance_id, 5, 4_000);
        }
        // INTRO_5: cut to Frintezza himself. Java sends the same framing twice,
        // a 1 s shot and a 12 s one — the first snaps, the second holds.
        5 => {
            let f = frintezza();
            camera(world, instance_id, f, 40, 75, -10, 0, 1000, 0, 0, 1, 0, 0);
            camera(world, instance_id, f, 40, 75, -10, 0, 12000, 0, 0, 1, 0, 0);
            schedule_intro(world, instance_id, 6, 1_350);
        }
        // INTRO_6: Frintezza stirs, and the dummy that framed the approach goes.
        6 => {
            social(world, instance_id, frintezza(), 2);
            delete_dummy(world, instance_id, "frintezzaDummy");
            schedule_intro(world, instance_id, 7, 8_000);
        }
        // INTRO_7/_8: the demons wake in pairs — the inner two, then the outer
        // two under a shot that swings from whichever side the viewer stands.
        7 => {
            for i in [1, 2] {
                social(
                    world,
                    instance_id,
                    var_oid(world, instance_id, &format!("demon{i}")),
                    1,
                );
            }
            schedule_intro(world, instance_id, 8, 400);
        }
        8 => {
            for i in [0, 3] {
                social(
                    world,
                    instance_id,
                    var_oid(world, instance_id, &format!("demon{i}")),
                    1,
                );
            }
            let d1 = var_oid(world, instance_id, "portraitDummy1");
            let d3 = var_oid(world, instance_id, "portraitDummy3");
            for dur in [1000, 10000] {
                let left = camera_packet(d1, 1000, 118, 0, 0, dur, 0, 0, 1, 0, 0);
                let right = camera_packet(d3, 1000, 62, 0, 0, dur, 0, 0, 1, 0, 0);
                send_packet_x(world, instance_id, &left, &right, PORTRAIT_SPLIT_X);
            }
            schedule_intro(world, instance_id, 9, 2_000);
        }
        // INTRO_9: back to Frintezza, who raises the baton; the portrait dummies
        // have played their part.
        9 => {
            let f = frintezza();
            camera(world, instance_id, f, 240, 90, 0, 0, 1000, 0, 0, 1, 0, 0);
            camera(
                world,
                instance_id,
                f,
                240,
                90,
                25,
                5500,
                10000,
                0,
                0,
                1,
                0,
                0,
            );
            social(world, instance_id, f, 3);
            delete_dummy(world, instance_id, "portraitDummy1");
            delete_dummy(world, instance_id, "portraitDummy3");
            schedule_intro(world, instance_id, 10, 4_500);
        }
        // INTRO_10/_11: the same over-the-shoulder framing held twice.
        10 => {
            camera(
                world,
                instance_id,
                frintezza(),
                100,
                195,
                35,
                0,
                10000,
                0,
                0,
                1,
                0,
                0,
            );
            schedule_intro(world, instance_id, 11, 700);
        }
        11 => {
            camera(
                world,
                instance_id,
                frintezza(),
                100,
                195,
                35,
                0,
                10000,
                0,
                0,
                1,
                0,
                0,
            );
            schedule_intro(world, instance_id, 12, 1_300);
        }
        // INTRO_12: the Mournful Chorale Prelude — screen text, the pull-back,
        // and the 34 s cast that carries the rest of the cinematic.
        12 => {
            let f = frintezza();
            instances::broadcast_to_instance(
                world,
                instance_id,
                &server_packets::ex_show_screen_message("Mournful Chorale Prelude", 2, 5000),
            );
            camera(
                world,
                instance_id,
                f,
                120,
                180,
                45,
                1500,
                10000,
                0,
                0,
                1,
                0,
                0,
            );
            if f != 0
                && let Some(p) = maybe_position(world, f)
            {
                let src = (f, p.x, p.y, p.z);
                instances::broadcast_to_instance(
                    world,
                    instance_id,
                    &server_packets::magic_skill_use_raw(src, src, PRELUDE_SKILL, 1, 34000),
                );
            }
            schedule_intro(world, instance_id, 13, 1_500);
        }
        13 => {
            camera(
                world,
                instance_id,
                frintezza(),
                520,
                135,
                45,
                8000,
                10000,
                0,
                0,
                1,
                0,
                0,
            );
            schedule_intro(world, instance_id, 14, 7_500);
        }
        14 => {
            camera(
                world,
                instance_id,
                frintezza(),
                1500,
                110,
                25,
                10000,
                13000,
                0,
                0,
                1,
                0,
                0,
            );
            schedule_intro(world, instance_id, 15, 9_500);
        }
        // INTRO_15: the camera lifts overhead and `scarletDummy` casts at it —
        // the effect that reads as the floor breaking open.
        15 => {
            let overhead = var_oid(world, instance_id, "overheadDummy");
            let scarlet_dummy = var_oid(world, instance_id, "scarletDummy");
            camera(
                world,
                instance_id,
                overhead,
                930,
                160,
                -20,
                0,
                1000,
                0,
                0,
                1,
                0,
                0,
            );
            camera(
                world,
                instance_id,
                overhead,
                600,
                180,
                -25,
                0,
                10000,
                0,
                0,
                1,
                0,
                0,
            );
            if overhead != 0
                && scarlet_dummy != 0
                && let (Some(sp), Some(tp)) = (
                    maybe_position(world, scarlet_dummy),
                    maybe_position(world, overhead),
                )
            {
                instances::broadcast_to_instance(
                    world,
                    instance_id,
                    &server_packets::magic_skill_use_raw(
                        (scarlet_dummy, sp.x, sp.y, sp.z),
                        (overhead, tp.x, tp.y, tp.z),
                        SCARLET_RISE_SKILL,
                        1,
                        5800,
                    ),
                );
            }
            schedule_intro(world, instance_id, 16, 5_000);
        }
        // INTRO_16: Scarlet takes the field, still invulnerable and immobile.
        16 => {
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
                social(world, instance_id, scarlet, 3);
            }
            let d = var_oid(world, instance_id, "scarletDummy");
            camera(
                world,
                instance_id,
                d,
                800,
                180,
                10,
                1000,
                10000,
                0,
                0,
                1,
                0,
                0,
            );
            schedule_intro(world, instance_id, 17, 2_100);
        }
        17 => {
            let scarlet = var_oid(world, instance_id, "activeScarlet");
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
            schedule_intro(world, instance_id, 18, 2_000);
        }
        // INTRO_18: the last shot, and the song starts under it.
        18 => {
            let scarlet = var_oid(world, instance_id, "activeScarlet");
            camera(
                world,
                instance_id,
                scarlet,
                500,
                90,
                10,
                3000,
                5000,
                0,
                0,
                1,
                0,
                0,
            );
            play_song(world, instance_id);
            schedule_intro(world, instance_id, 19, 3_000);
        }
        // INTRO_19: the four portrait pillars appear and the last dummies go.
        19 => {
            for (i, sp) in PORTRAIT_SPAWNS.iter().enumerate() {
                if let Some(portrait) =
                    instances::spawn_npc(world, instance_id, sp[0], sp[1], sp[2], sp[3], sp[4])
                {
                    world
                        .instances
                        .set_var(instance_id, &format!("portrait{i}"), portrait as i64);
                }
            }
            delete_dummy(world, instance_id, "overheadDummy");
            delete_dummy(world, instance_id, "scarletDummy");
            schedule_intro(world, instance_id, 20, 2_000);
        }
        // INTRO_20: hand control back — Scarlet and the demons wake, Frintezza
        // keeps its invulnerability, and the fight is on.
        20 => {
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
            // The four intro demons seed the count; the portraits emit the rest.
            world
                .instances
                .set_var(instance_id, "demonCount", PORTRAIT_SPAWNS.len() as i64);
            // The song itself already started at INTRO_18; this only arms the
            // 90 s repeat.
            schedule_song(world, instance_id);
            schedule_demons(world, instance_id);
        }
        _ => {}
    }
}

/// One `SpecialCamera` shot to the instance, transcribing Java's **11-argument**
/// `new SpecialCamera(target, force, angle1, angle2, time, range, duration,
/// relYaw, relPitch, isWide, relAngle)` literally.
///
/// **That overload swaps two of its own parameters**, and every shot in this
/// script uses it. It forwards to the canonical constructor as
/// `this(creature, force, angle1, angle2, time, duration, range, …)` — so the
/// argument a caller writes as *range* is the one that lands in `_duration` and
/// goes out on the wire, while the argument written as *duration* is dropped
/// (the canonical `range` is stored nowhere). Reading the 6th number as the
/// real duration is the only way these shots make sense: `(…, 6500, 7000, 0,
/// …)` is a 7 s hold, not a 0 s one.
///
/// So the parameters here are named as the script writes them and the swap
/// happens at the call to `special_camera`, which takes the canonical order.
/// All 38 of the datapack's 11-arg call sites are in this file; the other
/// ported bosses (Antharas, Valakas, Dr. Chaos) use the 12-arg form and are
/// unaffected.
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
        &camera_packet(oid, force, a1, a2, time, range, dur, ry, rp, wide, rangle),
    );
}

/// [`camera`]'s packet, for the one shot that is not broadcast to everyone.
#[allow(clippy::too_many_arguments)]
fn camera_packet(
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
) -> Vec<u8> {
    // `range` and `dur` cross over here — see [`camera`].
    server_packets::special_camera(
        oid, force, a1, a2, time, dur, range, ry, rp, wide, rangle, 0,
    )
}

/// Java's `sendPacketX`: members west of `x` get `left`, everyone else `right`.
/// Used once, to swing the portrait shot from whichever side the player is
/// standing on.
fn send_packet_x(world: &World, instance_id: i32, left: &[u8], right: &[u8], x: i32) {
    for member in instance_members(world, instance_id) {
        let Some(client_id) = crate::game_loop::helpers::client_for_player(world, member) else {
            continue;
        };
        let west = world
            .objects
            .get_component::<Position>(&member)
            .is_some_and(|p| p.x < x);
        crate::game_loop::helpers::send_to_client(
            world,
            client_id,
            if west { left.to_vec() } else { right.to_vec() },
        );
    }
}

/// Delete a cinematic dummy (Java `npc.deleteMe()`), clearing the instance var
/// so a torn-down cinematic cannot double-delete.
fn delete_dummy(world: &mut World, instance_id: i32, key: &str) {
    let oid = var_oid(world, instance_id, key);
    if oid == 0 {
        return;
    }
    world.instances.set_var(instance_id, key, 0);
    // Through the oid-deriving despawn — the old raw read fell back to region
    // (0, 0), silently corrupting that bucket of the npc-region index.
    crate::game_loop::death::despawn_npc_by_oid(world, oid);
}

/// `SocialAction` to the instance.
fn social(world: &World, instance_id: i32, oid: i32, action: i32) {
    if oid == 0 {
        return;
    }
    instances::broadcast_to_instance(
        world,
        instance_id,
        &server_packets::social_action(oid, action),
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

/// Set an NPC's paralyze + invulnerability directly (Java `setImmobilized` /
/// `disableAllSkills` + `setInvul`). The hand-back clears both, so an actor
/// spawned invulnerable actually becomes killable.
fn set_frozen(world: &mut World, oid: i32, paralyzed: bool, invul: bool) {
    let mut flags = admin_flags(world, oid);
    flags.paralyzed = paralyzed;
    flags.invul = invul;
    world.objects.add_components(&oid, flags);
}

fn set_paralyzed(world: &mut World, player_oid: i32, on: bool) {
    let mut flags = admin_flags(world, player_oid);
    flags.paralyzed = on;
    world.objects.add_components(&player_oid, flags);
    crate::game_loop::player_info::broadcast_user_info(world, player_oid);
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

// ---------------------------------------------------------------------------
// The boss fight (Java the `SCARLET_*_MORPH` / `onKill SCARLET2` branches).
//
// Slice 4: Scarlet's two morphs — at 80 % HP a first-morph cast, at 20 % the
// second morph that replaces Scarlet1 (29046) with its final form Scarlet2
// (29047) — and the finish trigger when Scarlet2 falls. Frintezza's songs, the
// demon/portrait spawn loops, the Dewdrop suicide and the full finish cinematic
// are later slices.
// ---------------------------------------------------------------------------

/// Scarlet's first form (morphs) and final form (Java `SCARLET1`/`SCARLET2`).
pub(crate) const SCARLET1: i32 = 29046;
pub(crate) const SCARLET2: i32 = 29047;
const FIRST_MORPH_SKILL: i32 = 5017;

// (`SCARLET1` is also the intro spawn id above.)

// Fight step ids.
const STEP_FIRST_MORPH: u8 = 1;
const STEP_SECOND_MORPH_A: u8 = 2;
const STEP_SECOND_MORPH_B: u8 = 3;

/// Java `onAttack(SCARLET1)`: cross the 80 % / 20 % HP thresholds once each,
/// arming the morphs. `scarlet_oid` is the struck Scarlet.
pub(crate) fn on_scarlet_attack(world: &mut World, scarlet_oid: i32, npc_id: i32) {
    let instance_id = instance_of(world, scarlet_oid);
    if instance_id == 0 {
        return;
    }
    // Both forms wake their skill AI on the first blow (Java `onAttack` arms the
    // ATTACK/RANDOM_TARGET timers).
    arm_scarlet_ai(world, instance_id);

    if npc_id != SCARLET1 {
        return; // only the first form morphs
    }
    let Some((cur, max)) = hp_pair(world, scarlet_oid) else {
        return;
    };
    let sv = world
        .objects
        .get_component::<crate::model::npc::Npc>(&scarlet_oid)
        .map_or(0, |n| n.script_value);

    if sv == 0 && cur < max * 0.80 {
        set_script_value(world, scarlet_oid, 1);
        schedule_fight(world, instance_id, STEP_FIRST_MORPH, 1000);
    } else if sv == 1 && cur < max * 0.20 {
        set_script_value(world, scarlet_oid, 2);
        schedule_fight(world, instance_id, STEP_SECOND_MORPH_A, 1000);
    }
}

/// Java `onKill(SCARLET2)`: the final form falls — cut Frintezza's song and roll
/// the finish cinematic (its death, then the doors reopen).
pub(crate) fn on_scarlet_killed(world: &mut World, killer_oid: i32) {
    let instance_id = instance_of(world, killer_oid);
    if instance_id == 0 {
        return;
    }
    // The song loops stop the moment the fight is won.
    world.instances.set_var(instance_id, "fightActive", 0);
    let frintezza = var_oid(world, instance_id, "frintezza");
    if frintezza != 0 {
        instances::broadcast_to_instance(
            world,
            instance_id,
            &server_packets::magic_skill_canceld(frintezza),
        );
    }
    schedule_finish(world, instance_id, 0, 500);
}

fn schedule_finish(world: &mut World, instance_id: i32, step: u8, delay_ms: u64) {
    world.scheduler.schedule(
        world.tick + ms_to_ticks(delay_ms).max(1),
        ScheduledTask::FrintezzaFinish { instance_id, step },
    );
}

/// The finish cinematic (Java `FINISH_CAMERA_1..5`), condensed to the functional
/// beats: the death shot, Frintezza's death ~7.4 s in, then the doors reopen so
/// the party can reach the exit cube. The camera choreography is abbreviated.
pub(crate) fn handle_finish_step(world: &mut World, instance_id: i32, step: u8) {
    if !world.instances.contains(instance_id) {
        return;
    }
    match step {
        // FINISH_CAMERA_1: a parting shot of the fallen Scarlet.
        0 => {
            let scarlet = var_oid(world, instance_id, "activeScarlet");
            if scarlet != 0 {
                camera(
                    world,
                    instance_id,
                    scarlet,
                    200,
                    0,
                    85,
                    4000,
                    10000,
                    0,
                    0,
                    1,
                    0,
                    0,
                );
            }
            schedule_finish(world, instance_id, 1, 7400);
        }
        // FINISH_CAMERA_2/3: Frintezza dies with its guardian.
        1 => {
            let frintezza = var_oid(world, instance_id, "frintezza");
            if frintezza != 0 {
                set_frozen(world, frintezza, false, false); // its death bypasses invul
                instances::broadcast_to_instance(
                    world,
                    instance_id,
                    &server_packets::die(frintezza, Default::default()),
                );
                camera(
                    world,
                    instance_id,
                    frintezza,
                    100,
                    120,
                    5,
                    0,
                    7000,
                    0,
                    0,
                    1,
                    0,
                    0,
                );
                crate::game_loop::death::despawn_npc_by_oid(world, frintezza);
            }
            schedule_finish(world, instance_id, 2, 16_000);
        }
        // FINISH_CAMERA_5: reopen every door and hand control back for the exit.
        2 => {
            for group in [
                FIRST_ROOM_DOORS,
                FIRST_ROUTE_DOORS,
                SECOND_ROOM_DOORS,
                SECOND_ROUTE_DOORS,
            ] {
                open_doors(world, instance_id, group);
            }
            enable_players(world, instance_id);
            world.instances.set_var(instance_id, "cleared", 1);
        }
        _ => {}
    }
}

fn handle_fight_step_inner(world: &mut World, instance_id: i32, step: u8) {
    match step {
        // SCARLET_FIRST_MORPH: the morph cast (cosmetic; Java also plays a song).
        STEP_FIRST_MORPH => {
            let scarlet = var_oid(world, instance_id, "activeScarlet");
            if scarlet != 0
                && let Some(p) = maybe_position(world, scarlet)
            {
                let src = (scarlet, p.x, p.y, p.z);
                instances::broadcast_to_instance(
                    world,
                    instance_id,
                    &server_packets::magic_skill_use_raw(src, src, FIRST_MORPH_SKILL, 1, 1000),
                );
            }
            // Java `SCARLET_FIRST_MORPH` ends with `playRandomSong(world)`.
            play_song(world, instance_id);
        }
        // SCARLET_SECOND_MORPH: freeze the party, then replace Scarlet1 with its
        // final form at the same spot.
        STEP_SECOND_MORPH_A => {
            disable_players(world, instance_id);
            // Java `SCARLET_SECOND_MORPH` plays a song too, right after the
            // freeze. This site carried no marker at all — only the first
            // morph did — so the gap here was invisible.
            play_song(world, instance_id);
            let scarlet1 = var_oid(world, instance_id, "activeScarlet");
            let (x, y, z, h) = world
                .objects
                .get_component::<Position>(&scarlet1)
                .map(|p| (p.x, p.y, p.z, p.heading))
                .unwrap_or(SCARLET_POS);
            if scarlet1 != 0 {
                let region = region_cell_of(world, scarlet1).unwrap_or((0, 0));
                crate::game_loop::death::despawn_npc(world, scarlet1, region);
            }
            if let Some(scarlet2) = spawn_frozen(world, instance_id, SCARLET2, x, y, z, h, true) {
                world
                    .instances
                    .set_var(instance_id, "activeScarlet", scarlet2 as i64);
                instances::broadcast_to_instance(
                    world,
                    instance_id,
                    &server_packets::social_action(scarlet2, 2),
                );
            }
            schedule_fight(world, instance_id, STEP_SECOND_MORPH_B, 9000);
        }
        // The final form wakes and control returns.
        STEP_SECOND_MORPH_B => {
            let scarlet2 = var_oid(world, instance_id, "activeScarlet");
            if scarlet2 != 0 {
                set_frozen(world, scarlet2, false, false);
            }
            enable_players(world, instance_id);
        }
        _ => {}
    }
}

/// Dispatch entry (guards a torn-down instance).
pub(crate) fn handle_fight_step(world: &mut World, instance_id: i32, step: u8) {
    if !world.instances.contains(instance_id) {
        return;
    }
    handle_fight_step_inner(world, instance_id, step);
}

fn schedule_fight(world: &mut World, instance_id: i32, step: u8, delay_ms: u64) {
    world.scheduler.schedule(
        world.tick + ms_to_ticks(delay_ms).max(1),
        ScheduledTask::FrintezzaFight { instance_id, step },
    );
}

fn set_script_value(world: &mut World, oid: i32, value: i32) {
    if let Some(n) = world
        .objects
        .get_component_mut::<crate::model::npc::Npc>(&oid)
    {
        n.script_value = value;
    }
}

// ---------------------------------------------------------------------------
// The fight loops (Java `PLAY_RANDOM_SONG`, `SPAWN_DEMONS`, the Dewdrop suicide
// and the demon/portrait `onKill` bookkeeping). Slice 4b.
// ---------------------------------------------------------------------------

const SONG_INTERVAL_MS: u64 = 90_000;
const DEMON_INTERVAL_MS: u64 = 20_000;
/// Java `MAX_DEMONS`.
const MAX_DEMONS: i64 = 24;
const SONG_SKILL: i32 = 5007;
/// The song's matching debuff (Java `skillEffect = SkillHolder(5008, random)`).
const SONG_EFFECT_SKILL: i32 = 5008;
const DEWDROP_SKILL: i32 = 2276;
/// The five song names (Java `SKILL_MSG`), shown as they play.
const SONG_NAMES: [&str; 5] = [
    "Requiem of Hatred",
    "Rondo of Solitude",
    "Frenetic Toccata",
    "Fugue of Jubilation",
    "Hypnotic Mazurka",
];

fn schedule_song(world: &mut World, instance_id: i32) {
    world.scheduler.schedule(
        world.tick + ms_to_ticks(SONG_INTERVAL_MS),
        ScheduledTask::FrintezzaSong { instance_id },
    );
}

fn schedule_demons(world: &mut World, instance_id: i32) {
    world.scheduler.schedule(
        world.tick + ms_to_ticks(DEMON_INTERVAL_MS),
        ScheduledTask::FrintezzaDemons { instance_id },
    );
}

/// The `PLAY_RANDOM_SONG` **timer**: play a song, then re-arm while the fight
/// lasts.
///
/// Kept separate from [`play_song`] on purpose. Java's `playRandomSong` only
/// performs; the 90 s re-arm lives in the timer case that calls it. The morph
/// steps call it too, and folding the re-arm in here would give every morph its
/// own duplicate song timer.
pub(crate) fn handle_song(world: &mut World, instance_id: i32) {
    if world.instances.get_var(instance_id, "fightActive") == 0 {
        return;
    }
    play_song(world, instance_id);
    schedule_song(world, instance_id);
}

/// Java `playRandomSong` — Frintezza performs one of five songs: the name on
/// screen, the 5007 animation, and the matching 5008 debuff on every player in
/// the instance.
///
/// Java's `isPlayingSong` guard is not ported because it cannot fire: the flag
/// is set true on entry and false again before the method returns, and nothing
/// else ever sets it, so the check only ever sees `false`.
fn play_song(world: &mut World, instance_id: i32) {
    let frintezza = var_oid(world, instance_id, "frintezza");
    let n = world.roll(SONG_NAMES.len() as i32) as usize;
    instances::broadcast_to_instance(
        world,
        instance_id,
        &server_packets::ex_show_screen_message(SONG_NAMES[n], 2, 4000),
    );
    if frintezza != 0 {
        if let Some(p) = maybe_position(world, frintezza) {
            let src = (frintezza, p.x, p.y, p.z);
            instances::broadcast_to_instance(
                world,
                instance_id,
                &server_packets::magic_skill_use_raw(src, src, SONG_SKILL, n as i32 + 1, 1000),
            );
        }
        // Java `for player: frintezza.setTarget(player); frintezza.doCast(5008)`
        // — the song's matching debuff lands on everyone (the animation above is
        // the 5007 half). Applied directly, since the cast is one-per-target.
        let level = n as i32 + 1;
        if let Some(skill) = skill_by_id(world, SONG_EFFECT_SKILL, level) {
            for player in instance_members(world, instance_id) {
                crate::game_loop::skills::effects::apply_skill_effects(
                    world, frintezza, player, &skill,
                );
            }
        }
    }
}

/// `SPAWN_DEMONS`: each still-standing portrait emits one demon (capped at
/// `MAX_DEMONS` alive), then the timer re-arms while any portrait remains.
pub(crate) fn handle_demon_spawn(world: &mut World, instance_id: i32) {
    if world.instances.get_var(instance_id, "fightActive") == 0 {
        return;
    }
    let mut any_portrait = false;
    for (i, s) in PORTRAIT_SPAWNS.iter().enumerate() {
        if var_oid(world, instance_id, &format!("portrait{i}")) == 0 {
            continue; // that portrait is down
        }
        any_portrait = true;
        if world.instances.get_var(instance_id, "demonCount") >= MAX_DEMONS {
            break;
        }
        if instances::spawn_npc(world, instance_id, s[0] + 2, s[5], s[6], s[7], s[8]).is_some() {
            let count = world.instances.get_var(instance_id, "demonCount");
            world
                .instances
                .set_var(instance_id, "demonCount", count + 1);
        }
    }
    if any_portrait {
        schedule_demons(world, instance_id);
    }
}

/// Java `onAttack`: the Dewdrop of Destruction (skill 2276) makes a portrait
/// suicide. `on_kill` then clears its slot.
pub(crate) fn on_portrait_attacked(
    world: &mut World,
    portrait_oid: i32,
    attacker_oid: i32,
    skill_id: Option<i32>,
) {
    if skill_id == Some(DEWDROP_SKILL) {
        crate::game_loop::death::npc_do_die(world, portrait_oid, attacker_oid);
    }
}

/// Java `onKill(PORTRAITS)`: a fallen portrait stops emitting demons.
pub(crate) fn on_portrait_killed(world: &mut World, killer_oid: i32, portrait_oid: i32) {
    let instance_id = instance_of(world, killer_oid);
    if instance_id == 0 {
        return;
    }
    for i in 0..PORTRAIT_SPAWNS.len() {
        if var_oid(world, instance_id, &format!("portrait{i}")) == portrait_oid {
            world
                .instances
                .set_var(instance_id, &format!("portrait{i}"), 0);
        }
    }
}

/// Java `onKill(DEMONS)`: one fewer demon counts against the cap.
pub(crate) fn on_demon_killed(world: &mut World, killer_oid: i32) {
    let instance_id = instance_of(world, killer_oid);
    if instance_id == 0 {
        return;
    }
    let count = world.instances.get_var(instance_id, "demonCount");
    world
        .instances
        .set_var(instance_id, "demonCount", (count - 1).max(0));
}

// ---------------------------------------------------------------------------
// Scarlet's combat skill AI (Java `ScarletVanHalisha`). A recurring tick, armed
// on the first blow, picks one of the daemon skills by the per-form probability
// table and casts it at a random in-range player. The daemon skills aren't in
// Scarlet's template skill list, so this drives them explicitly (the generic
// NPC AI can't).
// ---------------------------------------------------------------------------

const DAEMON_ATTACK: i32 = 5014;
const DAEMON_CHARGE: i32 = 5015;
const YOKE_OF_SCARLET: i32 = 5016;
const DAEMON_MORPH: i32 = 5018;
const DAEMON_FIELD: i32 = 5019;
/// Java `RANGED_SKILL_MIN_COOLTIME` (1 min) between the field/morph casts.
const RANGED_COOLDOWN_TICKS: u64 = 600;
/// How often Scarlet re-evaluates a cast (Java's ATTACK timer is 500 ms; a
/// slower cadence keeps the boss from skill-spamming while still feeling active).
const SCARLET_TICK_MS: u64 = 2000;

/// Arm the skill AI once (idempotent — repeated hits don't stack timers).
fn arm_scarlet_ai(world: &mut World, instance_id: i32) {
    if world.instances.get_var(instance_id, "scarletAi") == 1 {
        return;
    }
    world.instances.set_var(instance_id, "scarletAi", 1);
    schedule_scarlet(world, instance_id);
}

fn schedule_scarlet(world: &mut World, instance_id: i32) {
    world.scheduler.schedule(
        world.tick + ms_to_ticks(SCARLET_TICK_MS).max(1),
        ScheduledTask::ScarletSkill { instance_id },
    );
}

/// Java `getSkillAI`: while engaged and not already casting, pick a daemon skill
/// and cast it at a random in-range player, then re-arm.
pub(crate) fn handle_scarlet_skill(world: &mut World, instance_id: i32) {
    if world.instances.get_var(instance_id, "scarletAi") == 0
        || world.instances.get_var(instance_id, "fightActive") == 0
    {
        return; // the fight ended — stop ticking
    }
    let scarlet = var_oid(world, instance_id, "activeScarlet");
    let dead = is_dead(world, scarlet);
    if scarlet == 0 || dead {
        world.instances.set_var(instance_id, "scarletAi", 0);
        return;
    }

    // Skip while casting or (still) invulnerable, but keep the timer alive.
    let casting = world
        .objects
        .has_component::<crate::model::components::Casting>(&scarlet);
    let invul = world
        .objects
        .get_component::<AdminFlags>(&scarlet)
        .is_some_and(|f| f.invul);
    if casting || invul {
        schedule_scarlet(world, instance_id);
        return;
    }

    let npc_id = world
        .objects
        .get_component::<crate::model::npc::Npc>(&scarlet)
        .map_or(0, |n| n.npc_id);
    let (skill_id, level) = pick_daemon_skill(world, instance_id, npc_id);
    let range = skill_range(skill_id);
    if let Some(target) = pick_target_in_range(world, instance_id, scarlet, range)
        && let Some(skill) = skill_by_id(world, skill_id, level)
    {
        crate::game_loop::npc::cast::start_cast(world, scarlet, target, &skill);
    }
    schedule_scarlet(world, instance_id);
}

/// Java `getRndSkills` — the per-form probability table.
pub(crate) fn pick_daemon_skill(world: &mut World, instance_id: i32, npc_id: i32) -> (i32, i32) {
    if npc_id == SCARLET1 {
        if world.roll(100) < 10 {
            (DAEMON_CHARGE, 2)
        } else if world.roll(100) < 10 {
            (DAEMON_CHARGE, 5)
        } else if world.roll(100) < 2 {
            (YOKE_OF_SCARLET, 1)
        } else {
            (DAEMON_ATTACK, 2)
        }
    } else {
        // SCARLET2 — richer table, with the two ranged skills gated by cooldown.
        let ranged_ready = {
            let last = world.instances.get_var(instance_id, "scarletRanged") as u64;
            world.tick.saturating_sub(last) >= RANGED_COOLDOWN_TICKS
        };
        if world.roll(100) < 10 {
            (DAEMON_CHARGE, 3)
        } else if world.roll(100) < 10 {
            (DAEMON_CHARGE, 6)
        } else if world.roll(100) < 10 {
            (DAEMON_CHARGE, 2)
        } else if ranged_ready && world.roll(100) < 10 {
            world
                .instances
                .set_var(instance_id, "scarletRanged", world.tick as i64);
            (DAEMON_FIELD, 1)
        } else if ranged_ready && world.roll(100) < 10 {
            world
                .instances
                .set_var(instance_id, "scarletRanged", world.tick as i64);
            (DAEMON_MORPH, 1)
        } else if world.roll(100) < 2 {
            (YOKE_OF_SCARLET, 1)
        } else {
            (DAEMON_ATTACK, 3)
        }
    }
}

/// Java `getRandomTarget`'s per-skill range.
fn skill_range(skill_id: i32) -> f64 {
    match skill_id {
        DAEMON_CHARGE => 400.0,
        YOKE_OF_SCARLET => 200.0,
        DAEMON_MORPH | DAEMON_FIELD => 550.0,
        _ => 150.0, // DAEMON_ATTACK
    }
}

/// A random living instance member within `range` of Scarlet.
fn pick_target_in_range(
    world: &mut World,
    instance_id: i32,
    scarlet: i32,
    range: f64,
) -> Option<i32> {
    let origin = maybe_position(world, scarlet)?;
    let candidates: Vec<i32> = instance_members(world, instance_id)
        .into_iter()
        .filter(|&m| {
            let alive = world
                .objects
                .get_component::<crate::model::components::Vitals>(&m)
                .is_some_and(|v| !v.dead);
            let in_range = world
                .objects
                .get_component::<Position>(&m)
                .is_some_and(|p| p.distance_2d(&origin) <= range);
            alive && in_range
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    Some(candidates[world.roll(candidates.len() as i32) as usize])
}
