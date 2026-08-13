//! Global, player-less lifecycles from the `ai/areas` and `ai/others`
//! scripts — beats that cannot ride the quest-timer machinery because that is
//! anchored to a player (`QuestTimerSeqs`), while these run from boot with
//! nobody online.
//!
//! First resident: **Toma** (`ai/areas/DwarvenVillage/Toma`). Java spawns him
//! at one of three haunts and relocates him every 30 minutes
//! (`RESPAWN_TOMA`); his chat window is `scripts::toma`. The three **Mammon**
//! merchants (`ai/others/Mammons/*`) are the same shape and live here too.

use crate::game_loop::death::despawn_npc_by_oid;
use crate::game_loop::helpers::announce_to_all_online;
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::helpers::pos_of;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

pub(crate) const TOMA: i32 = 30556;

/// Java's `LOCATIONS` — the three spots Toma wanders between.
const TOMA_LOCS: [(i32, i32, i32, i32); 3] = [
    (151680, -174891, -1782, 0),
    (154153, -220105, -3402, 0),
    (178834, -184336, -355, 41400),
];

/// `TELEPORT_DELAY` (30 min) in ticks.
const TOMA_RELOCATE_TICKS: u64 = 30 * 60 * 10;

/// Boot: Toma is not in the spawn data — the script owns him entirely (Java
/// ctor fires `RESPAWN_TOMA` immediately, then every 30 minutes). Eilhalder
/// von Hellmann's day/night watch starts here too, treating boot as a
/// transition so a night boot spawns him right away. The three Mammon
/// merchants have the same "the ctor fires the beat once" shape.
pub(crate) fn spawn_at_boot(world: &mut World) {
    relocate_toma(world);
    for mammon in MAMMONS {
        relocate_mammon(world, mammon.npc_id);
    }
    let night = crate::game_loop::game_time::is_night_at(commons::util::now_millis());
    // `ai/others/Spawns/DayNightSpawns.onSpawnActivate` — place the half of
    // every day/night template that matches the phase we booted into.
    crate::game_loop::spawn_scripts::activate_at_boot(world);
    eilhalder_on_day_night_change(world, night);
    world.scheduler.schedule(
        world.tick + DAY_NIGHT_CHECK_TICKS,
        ScheduledTask::DayNightCheck { was_night: night },
    );
    world
        .scheduler
        .schedule(world.tick + FOG_REFRESH_TICKS, ScheduledTask::FogRefresh);
}

/// Forge of the Gods: the 15 s escalation-counter reset (Java's repeating
/// `"refresh"` quest timer).
const FOG_REFRESH_TICKS: u64 = 150;

pub(crate) fn handle_fog_refresh(world: &mut World) {
    world.fog_kill_count = 0;
    world
        .scheduler
        .schedule(world.tick + FOG_REFRESH_TICKS, ScheduledTask::FogRefresh);
}

/// The `RESPAWN_TOMA` beat: despawn the old Toma, spawn him at a random
/// haunt, re-arm.
pub(crate) fn relocate_toma(world: &mut World) {
    if let Some(oid) = find_toma(world) {
        despawn_npc_by_oid(world, oid);
    }
    let (x, y, z, heading) = TOMA_LOCS[world.roll(3) as usize];
    crate::model::npc::spawn_npc_at(world, TOMA, x, y, z, heading);
    world.scheduler.schedule(
        world.tick + TOMA_RELOCATE_TICKS,
        ScheduledTask::TomaRelocate,
    );
}

pub(crate) fn find_toma(world: &mut World) -> Option<i32> {
    find_by_npc_id(world, TOMA)
}

pub fn find_by_npc_id(world: &mut World, npc_id: i32) -> Option<i32> {
    let mut found = None;
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &crate::model::components::Position)>(
            |(n, _)| {
                if n.npc_id == npc_id {
                    found = Some(n.object_id);
                }
            },
        );
    found
}

// ---------------------------------------------------------------------------
// The Mammon merchants (`ai/others/Mammons/{Merchant,Blacksmith,Priest}`)
// ---------------------------------------------------------------------------

pub(crate) const MERCHANT_OF_MAMMON: i32 = 31113;
pub(crate) const BLACKSMITH_OF_MAMMON: i32 = 31126;
pub(crate) const PRIEST_OF_MAMMON: i32 = 33511;

/// One Mammon: which NPC, where it may appear, and the announce line Java
/// broadcasts (`"… has been spawned near the Town of X."`).
struct Mammon {
    npc_id: i32,
    /// Java's `LOCATIONS` — `(x, y, z, heading)`.
    locations: &'static [(i32, i32, i32, i32)],
    /// The announce sentence with `{}` where the castle name goes. The Priest's
    /// line says "in Town of", the other two "near the Town of" — Java's
    /// wording differs per script, so keep all three verbatim.
    announce: &'static str,
}

/// `TELEPORT_DELAY` (30 min) in ticks — the same beat for all three.
const MAMMON_RELOCATE_TICKS: u64 = 30 * 60 * 10;

const MAMMONS: &[Mammon] = &[
    Mammon {
        npc_id: MERCHANT_OF_MAMMON,
        locations: &[
            (-52172, 78884, -4741, 0),  // Devotion
            (-41350, 209876, -5087, 0), // Sacrifice
            (-21657, 77164, -5173, 0),  // Patriots
            (45029, 123802, -5413, 0),  // Pilgrims
            (83175, 208998, -5439, 0),  // Saints
            (111337, 173804, -5439, 0), // Worship
            (118343, 132578, -4831, 0), // Martyrdom
            (172373, -17833, -4901, 0), // Disciple
        ],
        announce: "Merchant of Mammon has been spawned near the Town of {}.",
    },
    Mammon {
        npc_id: BLACKSMITH_OF_MAMMON,
        locations: &[
            (-19360, 13278, -4901, 0),   // Dark Omens
            (-53131, -250502, -7909, 0), // Heretic
            (46303, 170091, -4981, 0),   // Branded
            (-20485, -251008, -8165, 0), // Apostate
            (12669, -248698, -9581, 0),  // Forbidden Path
            (140519, 79464, -5429, 0),   // Witch
        ],
        announce: "Blacksmith of Mammon has been spawned near the Town of {}.",
    },
    Mammon {
        npc_id: PRIEST_OF_MAMMON,
        locations: &[
            (146882, 29665, -2264, 0),     // Aden
            (81284, 150155, -3528, 891),   // Giran
            (42784, -41236, -2192, 37972), // Rune
        ],
        announce: "Priest of Mammon has been spawned in Town of {}.",
    },
];

/// The `RESPAWN_*` beat (Java `onEvent`): delete the copy this script placed —
/// **not** whatever NPC of that id happens to be nearby, since the Priest also
/// has static spawns — place a new one at a random haunt, announce it when
/// `AnnounceMammonSpawn` is on, and re-arm 30 minutes out.
///
/// Java additionally passes a 30-minute despawn delay to `addSpawn`; that timer
/// and this beat expire together, so the relocation is the despawn.
pub(crate) fn relocate_mammon(world: &mut World, npc_id: i32) {
    let Some(mammon) = MAMMONS.iter().find(|m| m.npc_id == npc_id) else {
        return;
    };
    if let Some(oid) = world.mammon_spawns.remove(&npc_id) {
        despawn_npc_by_oid(world, oid);
    }
    let (x, y, z, heading) = mammon.locations[world.roll(mammon.locations.len() as i32) as usize];
    if let Some(oid) = crate::model::npc::spawn_npc_at(world, npc_id, x, y, z, heading) {
        world.mammon_spawns.insert(npc_id, oid);
        if world.cfg.npc.announce_mammon_spawn {
            let castle = nearest_castle_name(world, x, y, z);
            announce_to_all_online(world, &mammon.announce.replace("{}", &castle));
        }
    }
    world.scheduler.schedule(
        world.tick + MAMMON_RELOCATE_TICKS,
        ScheduledTask::MammonRelocate { npc_id },
    );
}

/// `npc.getCastle().getName()` — the nearest castle by
/// `CastleManager.findNearestCastle`. Java would NPE on a null castle; the
/// castle list is loaded at boot and every Mammon haunt has one nearby, so an
/// empty name only shows up in a test world with no castles.
fn nearest_castle_name(world: &World, x: i32, y: i32, z: i32) -> String {
    world
        .data
        .zone_data
        .nearest_castle_at(x, y, z)
        .and_then(|id| world.castles.iter().find(|c| c.id == id))
        .map(|c| c.name.clone())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// The castle mass gatekeeper (`ai/others/CastleTeleporter`'s `MASS_TELEPORT`)
// ---------------------------------------------------------------------------

/// `NpcStringId.THE_DEFENDERS_OF_S1_CASTLE_WILL_BE_TELEPORTED_TO_THE_INNER_CASTLE`.
const DEFENDERS_TELEPORTED_STRING_ID: i32 = 1_000_443;
/// `ChatType.NPC_SHOUT`.
const NPC_SHOUT: i32 = 23;

/// Java `startQuestTimer("MASS_TELEPORT", time, npc, null)`. The timer is
/// npc-anchored with **no player**, so it cannot ride `QuestTimerSeqs` (which
/// hangs off the player) — it is a plain scheduled task, like the other beats
/// in this module. Re-arming is prevented by the caller's `scriptValue` gate,
/// the same way Java's is.
pub(crate) fn arm_castle_mass_teleport(world: &mut World, npc_oid: i32, delay_ms: u64) {
    world.scheduler.schedule(
        world.tick + delay_ms.div_ceil(100),
        ScheduledTask::CastleMassTeleport { npc_oid },
    );
}

/// The `MASS_TELEPORT` event: shout the warning, oust everyone standing in the
/// castle's owner-restart territory into the inner castle, and reset the
/// gatekeeper so it can be armed again.
pub(crate) fn handle_castle_mass_teleport(world: &mut World, npc_oid: i32) {
    let Some((npc_id, x, y, z)) = npc_id_of(world, npc_oid)
        .zip(pos_of(world, npc_oid))
        .map(|(id, (x, y, z))| (id, x, y, z))
    else {
        return; // the gatekeeper died/despawned before the timer fired
    };
    let Some(castle_id) = world.data.zone_data.nearest_castle_at(x, y, z) else {
        return;
    };

    let name = nearest_castle_name(world, x, y, z);
    let say = server_packets::npc_say_param_typed(
        npc_oid,
        npc_id,
        NPC_SHOUT,
        DEFENDERS_TELEPORTED_STRING_ID,
        &name,
    );
    // Java scopes the shout to `MapRegionManager.getMapRegionLocId` — every
    // player in the gatekeeper's map region hears it, whether or not they can
    // see the NPC. Same bucket rule as the SHOUT chat path: two off-map
    // players share Java's `0` region.
    let from_region = world
        .data
        .map_region
        .region_at(x, y)
        .map(|r| r.name.clone());
    for cs in world.clients.values() {
        let ClientSession::InGame(s) = cs else {
            continue;
        };
        let other_pos = world
            .objects
            .get_component::<crate::model::components::Position>(&s.player_object_id());
        let Some(p) = other_pos else { continue };
        let other_region = world
            .data
            .map_region
            .region_at(p.x, p.y)
            .map(|r| r.name.clone());
        if from_region == other_region {
            cs.send(say.clone());
        }
    }

    crate::game_loop::siege::oust_all_players(world, castle_id);

    // `npc.setScriptValue(0)` — the gatekeeper is armable again.
    if let Some(n) = world
        .objects
        .get_component_mut::<crate::model::npc::Npc>(&npc_oid)
    {
        n.script_value = 0;
    }
}

// ---------------------------------------------------------------------------
// Eilhalder von Hellmann — the Forest of the Dead night raid boss
// ---------------------------------------------------------------------------

pub(crate) const EILHALDER: i32 = 25328;
const EILHALDER_SPAWN: (i32, i32, i32) = (59090, -42188, -3003);

/// The day/night poll cadence (1 real minute = 6 game-minutes; transitions
/// land within one beat, like Java's minute-tick task manager).
const DAY_NIGHT_CHECK_TICKS: u64 = 600;
/// Java's `startQuestTimer("despawn", 30000, …)` retry while he fights.
const EILHALDER_DESPAWN_RETRY_TICKS: u64 = 300;

/// The minute beat: fire the transition handler when day/night flipped,
/// re-arm carrying the new state (state lives in the task itself).
pub(crate) fn handle_day_night_check(world: &mut World, was_night: bool) {
    let night = crate::game_loop::game_time::is_night_at(commons::util::now_millis());
    if night != was_night {
        // Java fires `OnDayNightChange` to every listener: the day/night spawn
        // groups swap, and Eilhalder von Hellmann comes or goes.
        crate::game_loop::spawn_scripts::on_day_night_change(world, night);
        eilhalder_on_day_night_change(world, night);
        // `NightStatModify`'s global `OnDayNightChange` listener: re-pump every
        // bearer's night-gated stat and message the Shadow Sense holders.
        crate::game_loop::night_stats::on_day_night_change(world, night);
    }
    world.scheduler.schedule(
        world.tick + DAY_NIGHT_CHECK_TICKS,
        ScheduledTask::DayNightCheck { was_night: night },
    );
}

fn npc_in_combat(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::npc::AggroList>(&oid)
        .is_some_and(|a| !a.0.is_empty())
}

/// Java `onDayNightChange`: day + alive → despawn (30 s retry while he is
/// fighting); otherwise if he is absent or dead, spawn him.
pub(crate) fn eilhalder_on_day_night_change(world: &mut World, night: bool) {
    let alive = find_by_npc_id(world, EILHALDER).and_then(|oid| {
        world
            .objects
            .get_component::<crate::model::components::Vitals>(&oid)
            .map(|v| (oid, !v.dead))
    });
    match alive {
        Some((oid, true)) if !night => {
            if npc_in_combat(world, oid) {
                world.scheduler.schedule(
                    world.tick + EILHALDER_DESPAWN_RETRY_TICKS,
                    ScheduledTask::EilhalderDespawnRetry,
                );
            } else {
                despawn_npc_by_oid(world, oid);
            }
        }
        Some((_, true)) => {}
        _ => {
            crate::model::npc::spawn_npc_at(
                world,
                EILHALDER,
                EILHALDER_SPAWN.0,
                EILHALDER_SPAWN.1,
                EILHALDER_SPAWN.2,
                0,
            );
        }
    }
}

/// The `"despawn"` retry: still fighting → try again in 30 s, else vanish.
pub(crate) fn handle_eilhalder_despawn_retry(world: &mut World) {
    let Some(oid) = find_by_npc_id(world, EILHALDER) else {
        return;
    };
    if npc_in_combat(world, oid) {
        world.scheduler.schedule(
            world.tick + EILHALDER_DESPAWN_RETRY_TICKS,
            ScheduledTask::EilhalderDespawnRetry,
        );
    } else {
        despawn_npc_by_oid(world, oid);
    }
}

// ---------------------------------------------------------------------------
// RandomWalkingGuards (`ai/others/RandomWalkingGuards`)
// ---------------------------------------------------------------------------

/// `MIN_WALK_DELAY` / `MAX_WALK_DELAY` (15–45 s) in ticks.
const GUARD_WALK_MIN_TICKS: u64 = 150;
const GUARD_WALK_MAX_TICKS: u64 = 450;

/// Java `startQuestTimer("RANDOM_WALK", getRandom(MIN, MAX), npc, null)` — an
/// NPC-anchored timer with no player, so it rides the scheduler directly like
/// this module's other beats.
pub(crate) fn arm_guard_walk(world: &mut World, npc_oid: i32) {
    let span = (GUARD_WALK_MAX_TICKS - GUARD_WALK_MIN_TICKS) as i32;
    let delay = GUARD_WALK_MIN_TICKS + world.roll(span + 1) as u64;
    world.scheduler.schedule(
        world.tick + delay,
        ScheduledTask::GuardRandomWalk { npc_oid },
    );
}

/// The `RANDOM_WALK` beat: a guard out of combat strolls to a random point
/// within `MaxDriftRange` of its post, then re-arms. A guard in combat skips
/// the stroll but keeps the beat, exactly like Java.
pub(crate) fn handle_guard_random_walk(world: &mut World, npc_oid: i32) {
    let Some(spawn) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .map(|n| n.spawn_loc)
    else {
        return; // despawned — the beat dies with it
    };
    let alive = world
        .objects
        .get_component::<crate::model::components::Vitals>(&npc_oid)
        .is_some_and(|v| !v.dead);
    if alive && !npc_in_combat(world, npc_oid) {
        // `Util.getRandomPosition(spawnLoc, 0, MAX_DRIFT_RANGE)`: an
        // independent offset per axis, rotated by a random angle.
        let drift = world.cfg.npc.max_drift_range;
        let (rx, ry) = (world.roll(drift + 1), world.roll(drift + 1));
        let angle = (world.roll(360) as f64).to_radians();
        let dest_x = spawn.0 + (rx as f64 * angle.cos()) as i32;
        let dest_y = spawn.1 + (ry as f64 * angle.sin()) as i32;
        let (vx, vy, vz) = world
            .geo
            .get_valid_location(spawn.0, spawn.1, spawn.2, dest_x, dest_y, spawn.2);
        crate::game_loop::ai::move_npc_to(world, npc_oid, vx, vy, vz);
    }
    arm_guard_walk(world, npc_oid);
}
