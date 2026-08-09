//! Valakas (`ai/bosses/Valakas`) — the attack-side rules.
//!
//! Valakas uses the **four-state** status ladder rather than the two-state one
//! the simpler bosses share:
//!
//! | status | meaning |
//! |---|---|
//! | 0 `DORMANT` | spawned, nobody has entered; entry unlocked |
//! | 1 `WAITING` | someone entered, 30-minute window for others; entry unlocked |
//! | 2 `FIGHTING` | engaged; entry **locked** |
//! | 3 `DEAD` | killed; entry locked |
//!
//! The `onAttack` rules, the entry/teleport flow, the spawn/death cinematics,
//! the 60 s `regen_task` (escalating self-heal + a 15-min-idle reset) and the
//! 2 s `skill_task` combat-skill AI (his breath/AoE/utility skills) are ported.

use crate::game_loop::abnormal::has_buff;
use crate::game_loop::common::players_in_lair_oids;
use crate::game_loop::guard::position;
use crate::game_loop::helpers::hp_pair;
use crate::game_loop::helpers::in_zone;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::set_position;
use crate::game_loop::helpers::skill_by_id;
use crate::model::components::{Position, Vitals};
use crate::scheduler::ScheduledTask;
use crate::world::World;

pub const VALAKAS: i32 = 29028;

/// `getZoneById(12010)` — "Valakas Boss", a `ScriptZone`.
const BOSS_ZONE_ID: i32 = 12010;

/// `ATTACKER_REMOVE` — where a player attacking outside the fight is dumped.
const ATTACKER_REMOVE: (i32, i32, i32) = (150_037, -57_255, -2_976);

/// `VALAKAS_REGENERATION_LOC` — home, where an idle Valakas resets.
const VALAKAS_HOME: (i32, i32, i32) = (-105_200, -253_104, -15_264);
/// `VALAKAS_REGENERATION` (4691, "Valakas Recovery") — the escalating self-heal.
const VALAKAS_REGENERATION: i32 = 4691;
/// The regen beat (Java `regen_task`, 60 s).
const REGEN_TICK_TICKS: u64 = 600;
/// Java's inactivity window: 15 min with nobody landing a hit resets the fight.
const INACTIVITY_TICKS: u64 = 9_000;

/// Java's static `_timeTracker`/`_actualVictim` — the last tick a lair attacker
/// struck Valakas (so the regen task can measure inactivity) and the player the
/// skill AI is currently working over (`0` = none).
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, Default)]
pub struct ValakasCombat {
    pub last_attack_tick: u64,
    pub actual_victim: i32,
}

// The skill AI's skill pools (`getRandomSkill`).
/// `VALAKAS_LAVA_SKIN` (4680) — a reflect buff he priority-casts when hurt.
const LAVA_SKIN: i32 = 4680;
/// Used while above 50% HP.
const REGULAR_SKILLS: [i32; 4] = [4681, 4682, 4683, 4689];
/// Used at or below 50% HP — adds Meteor Storm (4690).
const LOWHP_SKILLS: [i32; 5] = [4681, 4682, 4683, 4689, 4690];
/// Used when he feels surrounded (≥20 players within 1200).
const AOE_SKILLS: [i32; 7] = [4683, 4684, 4685, 4686, 4688, 4689, 4690];

/// The `skill_task` cadence (Java re-arms it every 1–2 s; the port uses 2 s).
const SKILL_TASK_TICKS: u64 = 20;
/// `getVisibleObjectsInRange(npc, Player, 1200).size() >= 20` — "surrounded".
const SURROUND_RANGE: f64 = 1200.0;
const SURROUND_COUNT: usize = 20;
/// `(skill.getCastRange() < 600) ? 600 : skill.getCastRange()` — the floor.
const MIN_CAST_RANGE: i32 = 600;
/// The roam jitter when he has no target (`getRandom(-1400, 1400)`).
const ROAM_OFFSET: i32 = 1_400;

pub const DORMANT: i32 = 0;
pub const WAITING: i32 = 1;
pub const FIGHTING: i32 = 2;
// No reader yet; the ladder is kept whole so the Java-mirrored numbering
// can't drift when the remaining states land.
#[allow(dead_code)]
pub const DEAD: i32 = 3;

/// Strider riders are debuffed on sight (skill 4258), once.
const STRIDER_DEBUFF: i32 = 4258;
/// Java `MountType.STRIDER`.
const MOUNT_STRIDER: u8 = 1;

/// What `Valakas.onAttack` decided to do about an attacker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackVerdict {
    /// Attacking from **outside the lair** — Java calls `attacker.doDie()`.
    /// A hard anti-exploit: you cannot plink at Valakas from safety.
    KilledForAttackingFromOutside,
    /// Attacking before the fight has started — bounced to `ATTACKER_REMOVE`.
    RemovedNotFighting,
    /// A normal hit.
    Allowed,
}

/// `Valakas.onAttack`, minus the timer bookkeeping.
///
/// The order is Java's and is load-bearing: the **zone check comes first**, so
/// an out-of-zone attacker dies whatever the boss's status — including while
/// Valakas is dead, when the status check would otherwise have merely teleported
/// them.
pub(crate) fn on_valakas_attacked(
    world: &mut World,
    valakas_oid: i32,
    attacker_oid: i32,
) -> AttackVerdict {
    if world
        .objects
        .get_component::<crate::model::Player>(&attacker_oid)
        .is_none()
    {
        return AttackVerdict::Allowed;
    }

    if !attacker_in_lair(world, attacker_oid) {
        // `attacker.doDie(attacker)` — self-inflicted, so it carries no PvP or
        // karma consequence for anyone.
        crate::game_loop::death::player_do_die(world, attacker_oid, attacker_oid);
        return AttackVerdict::KilledForAttackingFromOutside;
    }

    if crate::game_loop::grand_boss::status(world, VALAKAS) != Some(FIGHTING) {
        let (x, y, z) = ATTACKER_REMOVE;
        crate::game_loop::death::teleport_player(world, attacker_oid, x, y, z);
        return AttackVerdict::RemovedNotFighting;
    }

    // A strider-mounted attacker is debuffed, once — Java checks
    // `!isAffectedBySkill(4258)` so it isn't recast every swing.
    let on_strider = world
        .objects
        .get_component::<crate::model::Player>(&attacker_oid)
        .is_some_and(|p| p.mount_type == MOUNT_STRIDER);
    if on_strider && !already_debuffed(world, attacker_oid) {
        cast_debuff(world, valakas_oid, attacker_oid);
    }

    // `_timeTracker = System.currentTimeMillis()` — a valid hit resets the
    // inactivity clock the regen task watches.
    let now = world.tick;
    if let Some(c) = world
        .objects
        .get_component_mut::<ValakasCombat>(&valakas_oid)
    {
        c.last_attack_tick = now;
    }

    AttackVerdict::Allowed
}

/// Java `regen_task` (60 s while FIGHTING): a 15-minute-idle reset — Valakas
/// goes home, reverts to `DORMANT`, heals fully and empties the lair — and,
/// otherwise, the escalating self-heal buff (stronger the lower his health).
pub(crate) fn handle_regen(world: &mut World, valakas_oid: i32) {
    if crate::game_loop::grand_boss::status(world, VALAKAS) != Some(FIGHTING) {
        return; // the fight ended — stop ticking (Java cancels the timer)
    }

    // Inactivity: nobody has landed a hit in 15 minutes → reset the encounter.
    let idle = world
        .objects
        .get_component::<ValakasCombat>(&valakas_oid)
        .is_some_and(|c| world.tick.saturating_sub(c.last_attack_tick) >= INACTIVITY_TICKS);
    if idle {
        set_position(
            world,
            valakas_oid,
            (VALAKAS_HOME.0, VALAKAS_HOME.1, VALAKAS_HOME.2),
        );
        if let Some(a) = world
            .objects
            .get_component_mut::<crate::model::npc::AggroList>(&valakas_oid)
        {
            a.0.clear();
        }
        if let Some(b) = world.grand_bosses.get_mut(&VALAKAS) {
            b.status = DORMANT;
        }
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&valakas_oid) {
            v.cur_hp = v.max_hp as f64;
            v.cur_mp = v.max_mp as f64;
        }
        handle_remove_players(world);
        return; // don't re-arm; the reset ends the fight
    }

    // Otherwise refresh the recovery buff at the level his health calls for.
    if let Some((cur, max)) = hp_pair(world, valakas_oid) {
        let level = regen_level(cur, max);
        if let Some(skill) = skill_by_id(world, VALAKAS_REGENERATION, level) {
            crate::game_loop::skills::effects::apply_continuous_effects(
                world,
                valakas_oid,
                valakas_oid,
                &skill,
                None,
            );
        }
    }
    world.scheduler.schedule(
        world.tick + REGEN_TICK_TICKS,
        ScheduledTask::ValakasRegen { valakas_oid },
    );
}

/// The recovery level scales with missing health (Java's HP-band ladder).
fn regen_level(cur: f64, max: f64) -> i32 {
    if cur < max * 0.25 {
        4
    } else if cur < max * 0.5 {
        3
    } else if cur < max * 0.75 {
        2
    } else {
        1
    }
}

fn attacker_in_lair(world: &World, attacker_oid: i32) -> bool {
    let Some(pos) = world.objects.get_component::<Position>(&attacker_oid) else {
        return false;
    };
    world
        .data
        .zone_data
        .by_id(BOSS_ZONE_ID)
        .is_some_and(|z| z.contains(pos.x, pos.y, pos.z))
}

fn already_debuffed(world: &World, oid: i32) -> bool {
    has_buff(world, oid, STRIDER_DEBUFF)
}

fn cast_debuff(world: &mut World, caster_oid: i32, target_oid: i32) {
    let Some(skill) = skill_by_id(world, STRIDER_DEBUFF, 1) else {
        return;
    };
    crate::game_loop::skills::effects::apply_continuous_effects(
        world, caster_oid, target_oid, &skill, None,
    );
}

// ---------------------------------------------------------------------------
// skill_task — the combat skill AI (`callSkillAI`)
// ---------------------------------------------------------------------------

/// Java `skill_task` (2 s, while FIGHTING): drive one beat of the skill AI, then
/// re-arm. The fight ending (reset or death) drops the beat.
pub(crate) fn handle_skill_task(world: &mut World, valakas_oid: i32) {
    if crate::game_loop::grand_boss::status(world, VALAKAS) != Some(FIGHTING) {
        return; // fight over — stop (Java cancels the timer on reset/death)
    }
    call_skill_ai(world, valakas_oid);
    world.scheduler.schedule(
        world.tick + SKILL_TASK_TICKS,
        ScheduledTask::ValakasSkillTask { valakas_oid },
    );
}

/// Java `callSkillAI`: keep or re-pick a victim, then either roam (no target),
/// cast a skill (in range) or give chase (out of range).
fn call_skill_ai(world: &mut World, valakas_oid: i32) {
    // Don't stomp on an in-progress cast (Java `npc.isCastingNow()`).
    if world
        .objects
        .has_component::<crate::model::components::Casting>(&valakas_oid)
    {
        return;
    }

    // Re-pick a victim if the current one is gone, out of the lair, or on the
    // 10% whim (`getRandom(10) == 0`).
    let current = world
        .objects
        .get_component::<ValakasCombat>(&valakas_oid)
        .map(|c| c.actual_victim)
        .unwrap_or(0);
    let keep = current != 0
        && !is_dead(world, current)
        && in_lair_zone(world, current)
        && world.roll(10) != 0;
    let victim = if keep {
        current
    } else {
        random_target_in_lair(world)
    };
    if let Some(c) = world
        .objects
        .get_component_mut::<ValakasCombat>(&valakas_oid)
    {
        c.actual_victim = victim;
    }

    // No target: a 1-in-10 chance to roam within ±1400, else idle.
    if victim == 0 {
        if world.roll(10) == 0
            && let Some(p) = position(world, valakas_oid)
        {
            let x = p.x + world.roll(ROAM_OFFSET * 2 + 1) - ROAM_OFFSET;
            let y = p.y + world.roll(ROAM_OFFSET * 2 + 1) - ROAM_OFFSET;
            crate::game_loop::npc_ai::move_npc_to(world, valakas_oid, x, y, p.z);
        }
        return;
    }

    let skill_id = choose_skill(world, valakas_oid);
    let cast_range = world
        .data
        .skill_data
        .get(skill_id, 1)
        .map(|s| s.cast_range)
        .unwrap_or(0)
        .max(MIN_CAST_RANGE);

    if within(world, valakas_oid, victim, cast_range as f64) {
        super::boss_threat::cast_boss_skill(world, valakas_oid, victim, skill_id, false);
    } else {
        // FOLLOW — close the distance before the next beat.
        if let Some(p) = position(world, victim) {
            crate::game_loop::npc_ai::move_npc_to(world, valakas_oid, p.x, p.y, p.z);
        }
    }
}

/// `getRandomSkill`: Lava Skin when hurt-and-lucky (and not already up), a mass
/// spell when surrounded, otherwise the HP-banded pool.
fn choose_skill(world: &mut World, valakas_oid: i32) -> i32 {
    let (cur, max) = hp_pair(world, valakas_oid).unwrap_or((1.0, 1.0));
    let hp_ratio = (cur / max) * 100.0;

    // Lava Skin has priority: below 75% HP, a 1-in-150 roll, not already active.
    if hp_ratio < 75.0
        && world.roll(150) == 0
        && !crate::game_loop::abnormal::has_buff(world, valakas_oid, LAVA_SKIN)
    {
        return LAVA_SKIN;
    }
    // Surrounded (≥20 players within 1200) → a mass spell.
    if players_within(world, valakas_oid, SURROUND_RANGE) >= SURROUND_COUNT {
        return AOE_SKILLS[world.roll(AOE_SKILLS.len() as i32) as usize];
    }
    if hp_ratio > 50.0 {
        REGULAR_SKILLS[world.roll(REGULAR_SKILLS.len() as i32) as usize]
    } else {
        LOWHP_SKILLS[world.roll(LOWHP_SKILLS.len() as i32) as usize]
    }
}

/// A random living player inside the lair (Java `getRandomTarget`), or `0`.
fn random_target_in_lair(world: &mut World) -> i32 {
    let alive: Vec<i32> = players_in_lair_oids(world, BOSS_ZONE_ID)
        .into_iter()
        .filter(|&oid| !is_dead(world, oid))
        .collect();
    if alive.is_empty() {
        return 0;
    }
    alive[world.roll(alive.len() as i32) as usize]
}

/// How many players sit within `range` (2D) of Valakas.
fn players_within(world: &World, valakas_oid: i32, range: f64) -> usize {
    let Some(origin) = position(world, valakas_oid) else {
        return 0;
    };
    players_in_lair_oids(world, BOSS_ZONE_ID)
        .into_iter()
        .filter(|&oid| {
            world
                .objects
                .get_component::<Position>(&oid)
                .is_some_and(|p| p.distance_2d(&origin) <= range)
        })
        .count()
}

/// Is `oid` within `range` (2D) of Valakas?
fn within(world: &World, valakas_oid: i32, oid: i32, range: f64) -> bool {
    let (Some(a), Some(b)) = (position(world, valakas_oid), position(world, oid)) else {
        return false;
    };
    a.distance_2d(&b) <= range
}

fn in_lair_zone(world: &World, oid: i32) -> bool {
    let Some(pos) = world.objects.get_component::<Position>(&oid) else {
        return false;
    };
    world
        .data
        .zone_data
        .by_id(BOSS_ZONE_ID)
        .is_none_or(|z| z.contains(pos.x, pos.y, pos.z))
}

// ---------------------------------------------------------------------------
// The entry cinematic
// ---------------------------------------------------------------------------

/// `VALAKAS_LAIR` — where Valakas is teleported before the cinematic runs.
const VALAKAS_LAIR: (i32, i32, i32) = (212_852, -114_842, -1_632);

const TICKS_PER_SECOND: u64 = 10;

/// The ten cinematic beats: `(delay_ms_from_start, camera args)`.
///
/// Transcribed literally from Java's `startQuestTimer("spawn_N", …)` chain and
/// its `SpecialCamera` calls, in the same argument order the packet takes —
/// including `range`, which the wire drops. Keeping the tables aligned with the
/// source is the whole reason the packet kept that parameter.
///
/// The final beat carries no camera: it flips the status to `FIGHTING`, which
/// is what actually starts the fight and locks entry.
const CINEMATIC: [(u64, Option<[i32; 11]>); 10] = [
    (
        1_700,
        Some([1800, 180, -1, 1500, 15000, 10000, 0, 0, 1, 0, 0]),
    ),
    (
        3_200,
        Some([1300, 180, -5, 3000, 15000, 10000, 0, -5, 1, 0, 0]),
    ),
    (
        6_500,
        Some([500, 180, -8, 600, 15000, 10000, 0, 60, 1, 0, 0]),
    ),
    (
        9_400,
        Some([800, 180, -8, 2700, 15000, 10000, 0, 30, 1, 0, 0]),
    ),
    (
        12_100,
        Some([200, 250, 70, 0, 15000, 10000, 30, 80, 1, 0, 0]),
    ),
    (
        12_430,
        Some([1100, 250, 70, 2500, 15000, 10000, 30, 80, 1, 0, 0]),
    ),
    (
        15_430,
        Some([700, 150, 30, 0, 15000, 10000, -10, 60, 1, 0, 0]),
    ),
    (
        16_830,
        Some([1200, 150, 20, 2900, 15000, 10000, -10, 30, 1, 0, 0]),
    ),
    (
        23_530,
        Some([750, 170, -10, 3400, 15000, 4000, 10, -15, 1, 0, 0]),
    ),
    (26_000, None), // status → FIGHTING
];

/// `"beginning"` — teleport Valakas to his lair and arm the cinematic.
///
/// Every beat is scheduled up front from the **start** of the sequence, exactly
/// as Java does, rather than each step chaining the next. That matters: the
/// beats are not evenly spaced (330 ms between steps 5 and 6, 6.7 s between 8
/// and 9), and a chain of relative delays would be far easier to get subtly
/// wrong.
pub(crate) fn begin_cinematic(world: &mut World, valakas_oid: i32) {
    set_position(
        world,
        valakas_oid,
        (VALAKAS_LAIR.0, VALAKAS_LAIR.1, VALAKAS_LAIR.2),
    );
    // `"broadcast_spawn"`, which Java arms 100 ms in: the lair theme plus
    // Valakas' roar animation, to everyone inside. A hundred milliseconds is
    // under one tick here, so it goes out inline rather than through a task
    // that could only fire at the same tick anyway.
    broadcast_to_lair(world, &crate::network::server_packets::play_music("BS03_A"));
    broadcast_to_lair(
        world,
        &crate::network::server_packets::social_action(valakas_oid, 3),
    );
    for (i, (delay_ms, _)) in CINEMATIC.iter().enumerate() {
        world.scheduler.schedule(
            world.tick + (delay_ms / 1000 * TICKS_PER_SECOND).max(1),
            crate::scheduler::ScheduledTask::ValakasCinematic {
                valakas_oid,
                step: i as u8,
            },
        );
    }
}

/// Broadcast one cinematic camera keyframe to the lair.
///
/// The eleven `SpecialCamera` parameters live as an array in the CINEMATIC and
/// DEATH_CINEMATIC tables, so both step handlers just spread one row.
fn broadcast_camera(world: &mut World, valakas_oid: i32, a: [i32; 11]) {
    let pkt = crate::network::server_packets::special_camera(
        valakas_oid,
        a[0],
        a[1],
        a[2],
        a[3],
        a[4],
        a[5],
        a[6],
        a[7],
        a[8],
        a[9],
        a[10],
    );
    broadcast_to_lair(world, &pkt);
}

/// One cinematic beat.
pub(crate) fn handle_cinematic_step(world: &mut World, valakas_oid: i32, step: u8) {
    let Some((_, camera)) = CINEMATIC.get(step as usize).copied() else {
        return;
    };
    match camera {
        Some(a) => {
            broadcast_camera(world, valakas_oid, a);
        }
        None => {
            // The last beat: the fight is on, and entry locks behind it. Arm
            // the regen/inactivity task (Java `spawn_10` → `regen_task`).
            if let Some(b) = world.grand_bosses.get_mut(&VALAKAS) {
                b.status = FIGHTING;
            }
            world.objects.add_components(
                &valakas_oid,
                ValakasCombat {
                    last_attack_tick: world.tick,
                    actual_victim: 0,
                },
            );
            world.scheduler.schedule(
                world.tick + REGEN_TICK_TICKS,
                ScheduledTask::ValakasRegen { valakas_oid },
            );
            // Java `spawn_10` arms both `regen_task` and `skill_task`.
            world.scheduler.schedule(
                world.tick + SKILL_TASK_TICKS,
                ScheduledTask::ValakasSkillTask { valakas_oid },
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The death tail (`onKill` → `die_1`..`die_8` → `remove_players`)
// ---------------------------------------------------------------------------

/// The Teleport Cubic — its `teleportOut` talk is already routed by
/// `scripts::valakas_teleporters`; the kill just has to spawn the cubes.
const CUBE: i32 = 31759;

/// `addSpawn(..., 900000)` — the cubes' 15-minute despawn, in ticks.
const CUBE_LIFETIME_TICKS: u64 = 900_000 / 100;

/// `TELEPORT_CUBE_LOCATIONS` — the fifteen exit cubes `die_8` drops around the
/// lair (Java's array, verbatim).
const TELEPORT_CUBE_LOCATIONS: [(i32, i32, i32); 15] = [
    (214_880, -116_144, -1_644),
    (213_696, -116_592, -1_644),
    (212_112, -116_688, -1_644),
    (211_184, -115_472, -1_664),
    (210_336, -114_592, -1_644),
    (211_360, -113_904, -1_644),
    (213_152, -112_352, -1_644),
    (214_032, -113_232, -1_644),
    (214_752, -114_592, -1_644),
    (209_824, -115_568, -1_421),
    (210_528, -112_192, -1_403),
    (213_120, -111_136, -1_408),
    (215_184, -111_504, -1_392),
    (215_456, -117_328, -1_392),
    (213_200, -118_160, -1_424),
];

/// The eight death-cinematic beats: `(delay_ms_from_kill, camera args)`,
/// transcribed from Java's `die_N` `startQuestTimer`/`SpecialCamera` chain in
/// the packet's argument order (`range` included, as the entry table does).
/// The eighth beat also spawns the cubes and arms `remove_players`.
const DEATH_CINEMATIC: [(u64, [i32; 11]); 8] = [
    (300, [2000, 130, -1, 0, 15000, 10000, 0, 0, 1, 1, 0]),
    (600, [1100, 210, -5, 3000, 15000, 10000, -13, 0, 1, 1, 0]),
    (3_800, [1300, 200, -8, 3000, 15000, 10000, 0, 15, 1, 1, 0]),
    (8_200, [1000, 190, 0, 500, 15000, 10000, 0, 10, 1, 1, 0]),
    (8_700, [1700, 120, 0, 2500, 15000, 10000, 12, 40, 1, 1, 0]),
    (13_300, [1700, 20, 0, 700, 15000, 10000, 10, 10, 1, 1, 0]),
    (14_000, [1700, 10, 0, 1000, 15000, 10000, 20, 70, 1, 1, 0]),
    (16_500, [1700, 10, 0, 300, 15000, 250, 20, -20, 1, 1, 0]),
];

/// `startQuestTimer("remove_players", 900000)` — the lair empties 15 minutes
/// after the cubes appear.
const REMOVE_PLAYERS_SECS: u64 = 900;

/// `onKill` for Valakas: the death sound + opening camera, then the eight-beat
/// death cinematic scheduled up front from the kill (as Java does). The respawn
/// window and DEAD status are already set by `grand_boss::on_grand_boss_killed`,
/// which runs first on the shared death path.
pub(crate) fn on_valakas_killed(world: &mut World, valakas_oid: i32) {
    // `BOSS_ZONE.broadcastPacket(new PlaySound(1, "B03_D", 0, 0, 0, 0, 0))` —
    // the lair theme, type 1, not the type-0 quest-sound form.
    broadcast_to_lair(world, &crate::network::server_packets::play_music("B03_D"));
    let open = crate::network::server_packets::special_camera(
        valakas_oid,
        1200,
        20,
        -10,
        0,
        10000,
        13000,
        0,
        0,
        0,
        0,
        0,
    );
    broadcast_to_lair(world, &open);

    for (i, (delay_ms, _)) in DEATH_CINEMATIC.iter().enumerate() {
        world.scheduler.schedule(
            world.tick + (delay_ms / 1000 * TICKS_PER_SECOND).max(1),
            crate::scheduler::ScheduledTask::ValakasDeathCinematic {
                valakas_oid,
                step: i as u8,
            },
        );
    }
}

/// One death-cinematic beat. The eighth (`die_8`) also drops the fifteen exit
/// cubes and arms the 15-minute `remove_players` oust.
/// How many beats the death cinematic has — a test hook, so a test naming
/// "the last beat" cannot drift from the table.
#[cfg(test)]
pub(crate) fn death_cinematic_len() -> usize {
    DEATH_CINEMATIC.len()
}

pub(crate) fn handle_death_cinematic_step(world: &mut World, valakas_oid: i32, step: u8) {
    let Some((_, a)) = DEATH_CINEMATIC.get(step as usize).copied() else {
        return;
    };
    broadcast_camera(world, valakas_oid, a);

    if step as usize == DEATH_CINEMATIC.len() - 1 {
        for (x, y, z) in TELEPORT_CUBE_LOCATIONS {
            // `addSpawn(31759, loc, false, 900000)` — the cubes carry their own
            // 15-minute lifetime. Without it they stand in an empty lair until
            // the next server restart, and the next Valakas fight adds fifteen
            // more on top.
            if let Some(cube) = crate::model::npc::spawn_npc_at(world, CUBE, x, y, z, 0) {
                world.scheduler.schedule(
                    world.tick + CUBE_LIFETIME_TICKS,
                    crate::scheduler::ScheduledTask::DespawnNpc { npc_oid: cube },
                );
            }
        }
        world.scheduler.schedule(
            world.tick + REMOVE_PLAYERS_SECS * TICKS_PER_SECOND,
            crate::scheduler::ScheduledTask::ValakasRemovePlayers,
        );
    }
}

/// `remove_players` → `BOSS_ZONE.oustAllPlayers()`: teleport everyone still in
/// the lair out to the exit. The cubes outlive this by design — they are on
/// their own 15-minute `addSpawn` lifetime, armed where they spawn.
pub(crate) fn handle_remove_players(world: &mut World) {
    for player_oid in players_in_lair_oids(world, BOSS_ZONE_ID) {
        teleport_out(world, player_oid);
    }
}

/// `BOSS_ZONE.broadcastPacket` — the cinematic plays for everyone **in the
/// lair**, not everyone nearby: a player outside the zone sees nothing, which
/// is the point of running it on the zone rather than the boss's region.
fn broadcast_to_lair(world: &World, pkt: &[u8]) {
    let Some(zone) = world.data.zone_data.by_id(BOSS_ZONE_ID) else {
        return;
    };
    for cs in world.clients.values() {
        if let crate::session::ClientSession::InGame(s) = cs {
            let oid = s.player_object_id();
            let inside = in_zone(world, oid, zone);
            if inside {
                cs.send(pkt.to_vec());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The entry flow (Java `ai/others/ValakasTeleporters`), reached through
// `scripts::valakas_teleporters`. Slice 15 built the cinematic and never wired
// a caller; this is the caller.
// ---------------------------------------------------------------------------

/// The 200-player lifetime cap (Java `playerCount >= 200`).
pub const MAX_PEOPLE: u32 = 200;
/// `VACUALITE_FLOATING_STONE` — Klein's entry ticket.
pub const VACUALITE: i32 = 7267;
/// Java `qs.set("allowEnter", "1")` — the antechamber → lair gate flag.
const ALLOW_ENTER: &str = "VALAKAS_ALLOW_ENTER";

const HALL_OF_FLAMES: (i32, i32, i32) = (183_813, -115_157, -3_303);
/// `TELEPORT_INTO_VALAKAS_LAIR` — the base, offset by `rnd(600)` in x/y.
const LAIR_ENTRY: (i32, i32, i32) = (204_328, -111_874, 70);
/// `TELEPORT_OUT_OF_VALAKAS_LAIR` — the base, offset by `rnd(500)` in x/y.
const LAIR_EXIT: (i32, i32, i32) = (150_037, -57_720, -2_976);

const TICKS_PER_SECOND_ENTRY: u64 = 10;

/// The live Valakas NPC, if one stands in the world.
pub(crate) fn find_valakas(world: &World) -> Option<i32> {
    crate::game_loop::grand_boss::find_spawned(world, VALAKAS)
}

/// Watcher Klein's `on_talk` — the crowding message by lifetime entry count
/// (Java's `31540-01`..`-05` ladder). No teleport here; the antechamber
/// teleport is the `31540` sub-event.
pub(crate) fn klein_status_html(world: &World) -> &'static str {
    match world.valakas_entry_count {
        n if n < 50 => "31540-01.htm",
        n if n < 100 => "31540-02.htm",
        n if n < 150 => "31540-03.htm",
        n if n < MAX_PEOPLE => "31540-04.htm",
        _ => "31540-05.htm",
    }
}

/// Klein's `31540` sub-event — the Vacualite check, the Hall of Flames
/// teleport, and the `allowEnter` grant. Returns the refusal html, or `None`
/// when teleported (Java returns `""`, i.e. close the window).
pub(crate) fn enter_hall_of_flames(world: &mut World, player_oid: i32) -> Option<&'static str> {
    if quest_items_count(world, player_oid, VACUALITE) < 1 {
        return Some("31540-06.htm");
    }
    teleport_player_rand(world, player_oid, HALL_OF_FLAMES, 0);
    set_player_flag(world, player_oid, ALLOW_ENTER, 1);
    None
}

/// Heart of Volcano's `on_talk` — the lair door. Returns the refusal html, or
/// `None` when admitted (the teleport is the reply).
pub(crate) fn heart_enter(world: &mut World, player_oid: i32) -> Option<&'static str> {
    match crate::game_loop::grand_boss::status(world, VALAKAS) {
        // DORMANT / WAITING — entry open.
        Some(DORMANT) | Some(WAITING) => {}
        Some(FIGHTING) => return Some("31385-02.htm"),
        // DEAD (or no record) — the regen/dead window.
        _ => return Some("31385-01.htm"),
    }
    if world.valakas_entry_count >= MAX_PEOPLE {
        return Some("31385-03.htm");
    }
    if player_flag(world, player_oid, ALLOW_ENTER) != 1 {
        return Some("31385-04.htm");
    }
    // Admitted: consume the flag, teleport in, and count the entry.
    unset_player_flag(world, player_oid, ALLOW_ENTER);
    teleport_player_rand(world, player_oid, LAIR_ENTRY, 600);
    world.valakas_entry_count += 1;

    // The FIRST entry (DORMANT) starts the 30-minute window; a later entrant
    // during WAITING must not re-arm it (Java only arms on `status == 0`).
    if crate::game_loop::grand_boss::status(world, VALAKAS) == Some(DORMANT) {
        crate::game_loop::grand_boss::set_status(world, VALAKAS, WAITING);
        let wait_secs = world.cfg.grand_boss.valakas_wait_minutes.max(1) as u64 * 60;
        world.scheduler.schedule(
            world.tick + wait_secs * TICKS_PER_SECOND_ENTRY,
            crate::scheduler::ScheduledTask::ValakasBeginning,
        );
    }
    None
}

/// `"beginning"` — the window elapsed: Valakas takes the lair and the entry
/// cinematic runs (`begin_cinematic`, whose final beat flips FIGHTING).
pub(crate) fn handle_beginning_timer(world: &mut World) {
    // A GM could have killed him during the window; only a still-WAITING boss
    // begins the fight.
    if crate::game_loop::grand_boss::status(world, VALAKAS) != Some(WAITING) {
        return;
    }
    let Some(oid) = find_valakas(world) else {
        return;
    };
    begin_cinematic(world, oid);
}

/// The Teleportation Cubic's exit.
pub(crate) fn teleport_out(world: &mut World, player_oid: i32) {
    teleport_player_rand(world, player_oid, LAIR_EXIT, 500);
}

// -- small helpers, kept local so the entry flow reads as one unit ----------

fn teleport_player_rand(world: &mut World, player_oid: i32, base: (i32, i32, i32), spread: i32) {
    let (dx, dy) = if spread > 0 {
        (world.roll(spread), world.roll(spread))
    } else {
        (0, 0)
    };
    crate::game_loop::death::teleport_player(world, player_oid, base.0 + dx, base.1 + dy, base.2);
}

fn quest_items_count(world: &World, oid: i32, item_id: i32) -> i64 {
    world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&oid)
        .map(|inv| inv.count_of(item_id))
        .unwrap_or(0)
}

fn player_flag(world: &World, oid: i32, key: &str) -> i32 {
    world
        .objects
        .get_component::<crate::model::components::PlayerVariables>(&oid)
        .and_then(|v| v.0.get(key).and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

fn set_player_flag(world: &mut World, oid: i32, key: &str, value: i32) {
    if let Some(v) = world
        .objects
        .get_component_mut::<crate::model::components::PlayerVariables>(&oid)
    {
        v.0.insert(key.to_string(), value.to_string());
    }
}

fn unset_player_flag(world: &mut World, oid: i32, key: &str) {
    if let Some(v) = world
        .objects
        .get_component_mut::<crate::model::components::PlayerVariables>(&oid)
    {
        v.0.remove(key);
    }
}
