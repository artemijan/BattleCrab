//! Baium (`ai/bosses/Baium`) — the sleeping-stone awakening, archangels and the
//! strider debuff.
//!
//! Baium is unusual among the grand bosses: **at rest he is not a boss at all**
//! but a stone statue (`BAIUM_STONE`, 29025). A raid enters, wakes the statue,
//! and only then does the live Baium (29020) spawn — through a short cinematic
//! (earthquake, two poses, the waker ported in and struck by Baium's "gift"),
//! after which his five archangels join and the fight proper begins. So the
//! grand-boss lifecycle spawns the **stone** at ALIVE, and the live boss only
//! ever exists mid-fight (or when crash-recovery restores one).
//!
//! He uses the four-state status ladder (ALIVE 0 / WAITING 1 / IN_FIGHT 2 /
//! DEAD 3), like Antharas and Valakas.
//!
//! Baium's cinematic uses **no `SpecialCamera`** (unlike Valakas's 19 and
//! Antharas's 7) — only social actions and an earthquake — which is why it is
//! portable before the camera packet is universally wired.
//!
//! His threat table is the shared `boss_threat` one — Antharas keeps an
//! identical copy in Java. Only the skill ladder below is Baium's own.

use crate::game_loop::space::position::maybe_position;
use crate::game_loop::space::position::pos_of;
use crate::game_loop::time::TICKS_PER_SECOND;
use crate::model::components::{Immobilized, Position, Vitals};
use crate::model::grand_boss::GrandBoss;
use crate::scheduler::ScheduledTask;
use crate::world::World;

pub const BAIUM: i32 = 29020;
/// The sleeping statue Baium rests as — spawned at ALIVE, woken by a raid.
pub const BAIUM_STONE: i32 = 29025;
/// Archangel — five of them circle Baium.
pub const ARCHANGEL: i32 = 29021;

/// The Angelic Vortex (31862) — the entry NPC that reads the fight's state and
/// ferries a Blooded-Fabric bearer into the lair.
pub const ANG_VORTEX: i32 = 31862;
/// The teleport cube (31842) spawned when Baium dies — the way out.
pub const TELE_CUBE: i32 = 31842;
/// Blooded Fabric (4295) — the entry ticket, consumed on a successful cross.
pub const FABRIC: i32 = 4295;

// Status ladder (`GrandBossManager` values for Baium).
const ALIVE: i32 = 0;
const WAITING: i32 = 1;
const IN_FIGHT: i32 = 2;
const DEAD: i32 = 3;

/// `BAIUM_LOC` — where the statue (and, once woken, the boss) stands.
const BAIUM_LOC: (i32, i32, i32, i32) = (116_033, 17_447, 10_107, 40_188);
/// `BAIUM_GIFT_LOC` — the waker is ported here to receive Baium's "gift".
const BAIUM_GIFT_LOC: (i32, i32, i32) = (115_910, 17_337, 10_105);
/// `TELEPORT_IN_LOC` — where the vortex drops a fabric-bearer.
const TELEPORT_IN_LOC: (i32, i32, i32) = (114_077, 15_882, 10_078);
/// `TELEPORT_CUBIC_LOC` — where the exit cube stands after the kill.
const TELEPORT_CUBIC_LOC: (i32, i32, i32) = (115_017, 15_549, 10_090);
/// `TELEPORT_OUT_LOC` — the three surface points the cube scatters people to.
const TELEPORT_OUT_LOC: [(i32, i32, i32); 3] = [
    (108_784, 16_000, -4_928),
    (113_824, 10_448, -5_164),
    (115_488, 22_096, -5_168),
];
/// `BAIUM_PRESENT` (4136, "Baium's Gift") — the skill that greets the waker.
const BAIUM_PRESENT: i32 = 4136;
/// `BS01_D` — the death roar played when Baium falls.
const DEATH_SOUND: &str = "BS01_D";

/// Social-action ids Baium plays during the awakening.
const SOCIAL_WAKE: i32 = 2;
const SOCIAL_STAND: i32 = 3;
const SOCIAL_ROAR: i32 = 1;

/// `HEAL_OF_BAIUM` (4135, "Baium Heal") — the self-heal the idle check casts.
const HEAL_OF_BAIUM: i32 = 4135;
/// The CHECK_ATTACK beat (Java 60 s).
const CHECK_ATTACK_TICKS: u64 = 600;
/// 30 minutes with no hit → the fight resets and Baium sleeps again.
const RESET_IDLE_TICKS: u64 = 18_000;
/// 5 minutes with no hit (and wounded) → Baium heals himself.
const HEAL_IDLE_TICKS: u64 = 3_000;
/// 15 minutes after the kill, the lair is force-emptied (cube + stragglers).
const CLEAR_ZONE_TICKS: u64 = 9_000;

/// The waker, held on the live Baium so the cinematic beats can reach them.
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy)]
pub struct BaiumWaker {
    pub player_oid: i32,
}

/// The `SELECT_TARGET` beat — the archangels re-pick every 5 s.
const SELECT_TARGET_TICKS: u64 = 50;
/// `getVisibleObjectsInRange(mob, Playable, 1000)` — the archangel's reach.
const ARCHANGEL_REACH: f64 = 1000.0;
/// `addDamageHate(target, 0, 999)` — the engage weight.
const ENGAGE_HATE: f64 = 999.0;

/// `ARCHANGEL_LOC` — five fixed points, with headings.
const ARCHANGEL_LOC: [(i32, i32, i32, i32); 5] = [
    (115_792, 16_608, 10_136, 0),
    (115_168, 17_200, 10_136, 0),
    (115_780, 15_564, 10_136, 13_620),
    (114_880, 16_236, 10_136, 5_400),
    (114_239, 17_168, 10_136, -1_992),
];

/// Bring out Baium's five archangels and arm their targeting beat.
///
/// Unlike Queen Ant's nurses these are **not** in a minion table — the script
/// places them, so nothing else would.
pub(crate) fn spawn_archangels(world: &mut World) {
    for (x, y, z, heading) in ARCHANGEL_LOC {
        crate::game_loop::npc::spawn_npc_at(world, ARCHANGEL, x, y, z, heading);
    }
    world.scheduler.schedule(
        world.tick + SELECT_TARGET_TICKS,
        ScheduledTask::BaiumSelectTarget,
    );
}

// ---------------------------------------------------------------------------
// Spawn / the sleeping stone
// ---------------------------------------------------------------------------

/// Baium's status-driven boot spawn (Java's constructor branch), reached from
/// `grand_boss::spawn_from_record`.
///
/// - **ALIVE / WAITING**: place the sleeping **stone** (29025) at `BAIUM_LOC`.
///   Java collapses `WAITING` to `ALIVE` here — a server that went down during
///   the 30-minute entry window comes back with the statue, not a half-fight.
/// - **IN_FIGHT**: crash-recovery — the server died mid-fight, so bring the
///   **live** boss back at its stored location and HP with its archangels
///   already circling.
pub(crate) fn spawn_from_record(world: &mut World, boss: &GrandBoss) {
    if boss.status == IN_FIGHT {
        let Some(oid) = crate::game_loop::npc::spawn_npc_at(
            world,
            BAIUM,
            boss.loc_x,
            boss.loc_y,
            boss.loc_z,
            boss.heading,
        ) else {
            return;
        };
        if boss.current_hp > 0.0
            && let Some(v) = world.objects.get_component_mut::<Vitals>(&oid)
        {
            v.cur_hp = boss.current_hp.min(v.max_hp as f64);
            v.cur_mp = boss.current_mp.min(v.max_mp as f64);
        }
        spawn_archangels(world);
        arm_combat_watch(world, oid);
        return;
    }

    // ALIVE or WAITING → the sleeping statue. Fold WAITING down to ALIVE so the
    // stored state matches what we actually spawned.
    if boss.status == WAITING {
        crate::game_loop::grand_boss::set_status(world, BAIUM, ALIVE);
    }
    crate::game_loop::npc::spawn_npc_at(
        world,
        BAIUM_STONE,
        BAIUM_LOC.0,
        BAIUM_LOC.1,
        BAIUM_LOC.2,
        BAIUM_LOC.3,
    );
}

// ---------------------------------------------------------------------------
// wakeUp — the stone becomes the boss
// ---------------------------------------------------------------------------

/// Java `wakeUp`: a raid talks to the sleeping statue and Baium rises.
///
/// Flips the status to `IN_FIGHT` (locking entry), removes the stone, spawns the
/// live boss at `BAIUM_LOC` **held still** for the cinematic (Java's
/// `disableCoreAI(true)` — he must not wander or swing mid-scene), remembers the
/// waker, and arms the cinematic chain. A no-op unless Baium is `ALIVE`, so two
/// raids can't wake him twice.
///
/// Returns the live boss's object id when the wake took, `None` otherwise.
pub(crate) fn wake_up(world: &mut World, stone_oid: i32, waker_oid: i32) -> Option<i32> {
    if crate::game_loop::grand_boss::status(world, BAIUM) != Some(ALIVE) {
        return None;
    }
    crate::game_loop::grand_boss::set_status(world, BAIUM, IN_FIGHT);

    despawn(world, stone_oid);

    let oid = crate::game_loop::npc::spawn_npc_at(
        world,
        BAIUM,
        BAIUM_LOC.0,
        BAIUM_LOC.1,
        BAIUM_LOC.2,
        BAIUM_LOC.3,
    )?;
    // `disableCoreAI(true)` — pin him while the scene plays.
    world.objects.add_components(&oid, Immobilized);
    world.objects.add_components(
        &oid,
        BaiumWaker {
            player_oid: waker_oid,
        },
    );
    arm_combat_watch(world, oid);

    schedule_beat(world, 0, WAKEUP_DELAY_MS);
    Some(oid)
}

/// Start the inactivity clock (`_lastAttack = now`) and the CHECK_ATTACK beat —
/// shared by a fresh wake and a crash-recovery of a live boss.
fn arm_combat_watch(world: &mut World, baium_oid: i32) {
    let now = world.tick;
    world.objects.add_components(
        &baium_oid,
        super::combat::BossCombat {
            last_attack_tick: now,
            ..Default::default()
        },
    );
    world.scheduler.schedule(
        world.tick + CHECK_ATTACK_TICKS,
        ScheduledTask::BaiumCheckAttack,
    );
}

/// One beat of the awakening. Java arms `WAKEUP_ACTION` at +50 ms and the rest
/// chain off `MANAGE_EARTHQUAKE`; the port keeps them as one relative chain.
struct CinematicBeat {
    /// Delay from the previous beat (ms).
    delay_ms: u64,
    /// A social action broadcast to the lair, if any.
    social: Option<i32>,
    /// An earthquake + roar sound is played this beat.
    earthquake: bool,
    /// The waker is ported to `BAIUM_GIFT_LOC` this beat.
    port_waker: bool,
    /// Baium greets the waker with his gift skill and takes his AI back.
    strike_waker: bool,
    /// The archangels join and the fight begins.
    spawn_archangels: bool,
}

const WAKEUP_DELAY_MS: u64 = 50;

/// The six beats, in Java's order and timing (relative delays).
const BEATS: [CinematicBeat; 6] = [
    // WAKEUP_ACTION (+50 from wakeUp): the first pose.
    CinematicBeat {
        delay_ms: 0,
        social: Some(SOCIAL_WAKE),
        earthquake: false,
        port_waker: false,
        strike_waker: false,
        spawn_archangels: false,
    },
    // MANAGE_EARTHQUAKE (+~2000): the ground shakes.
    CinematicBeat {
        delay_ms: 1_950,
        social: None,
        earthquake: true,
        port_waker: false,
        strike_waker: false,
        spawn_archangels: false,
    },
    // SOCIAL_ACTION (+8000): the second pose.
    CinematicBeat {
        delay_ms: 8_000,
        social: Some(SOCIAL_STAND),
        earthquake: false,
        port_waker: false,
        strike_waker: false,
        spawn_archangels: false,
    },
    // PLAYER_PORT (+6000): the waker is drawn to Baium's feet.
    CinematicBeat {
        delay_ms: 6_000,
        social: None,
        earthquake: false,
        port_waker: true,
        strike_waker: false,
        spawn_archangels: false,
    },
    // PLAYER_KILL (+3000): the roar, the greeting, the gift skill.
    CinematicBeat {
        delay_ms: 3_000,
        social: Some(SOCIAL_ROAR),
        earthquake: false,
        port_waker: false,
        strike_waker: true,
        spawn_archangels: false,
    },
    // SPAWN_ARCHANGEL (+8000): the guardians arrive; the fight is on.
    CinematicBeat {
        delay_ms: 8_000,
        social: None,
        earthquake: false,
        port_waker: false,
        strike_waker: false,
        spawn_archangels: true,
    },
];

fn schedule_beat(world: &mut World, step: u8, delay_ms: u64) {
    world.scheduler.schedule(
        world.tick + (delay_ms * TICKS_PER_SECOND / 1000).max(1),
        ScheduledTask::BaiumCinematic { step },
    );
}

/// Run one cinematic beat and arm the next. The live Baium is found by id (there
/// is only ever one); if he has died mid-scene the chain simply stops.
pub(crate) fn handle_cinematic_step(world: &mut World, step: u8) {
    let Some(beat) = BEATS.get(step as usize) else {
        return;
    };
    let Some(baium) = crate::game_loop::grand_boss::find_alive(world, BAIUM) else {
        return; // Baium gone (aborted / killed) — drop the chain
    };
    let waker = world
        .objects
        .get_component::<BaiumWaker>(&baium)
        .map(|w| w.player_oid);

    if let Some(action) = beat.social {
        let pkt = crate::network::server_packets::social_action(baium, action);
        broadcast_to_lair(world, &pkt);
    }
    if beat.earthquake {
        if let Some((x, y, z)) = pos_of(world, baium) {
            let quake = crate::network::server_packets::earthquake(x, y, z, 40, 10);
            broadcast_to_lair(world, &quake);
        }
        let sound = crate::network::server_packets::play_sound("BS02_A");
        broadcast_to_lair(world, &sound);
    }
    if beat.port_waker
        && let Some(p) = waker
    {
        crate::game_loop::death::teleport_player(
            world,
            p,
            BAIUM_GIFT_LOC.0,
            BAIUM_GIFT_LOC.1,
            BAIUM_GIFT_LOC.2,
        );
    }
    if beat.strike_waker
        && let Some(p) = waker
    {
        super::boss_threat::cast_boss_skill(world, baium, p, BAIUM_PRESENT, false);
    }
    if beat.spawn_archangels {
        // Java `disableCoreAI(false)` — Baium takes his AI back, then the
        // guardians arrive and he engages the waker.
        world.objects.remove_component::<Immobilized>(&baium);
        spawn_archangels(world);
        if let Some(p) = waker {
            crate::game_loop::npc::minions::add_hate(world, baium, p, ENGAGE_HATE);
        }
    }

    if let Some(next) = BEATS.get(step as usize + 1) {
        schedule_beat(world, step + 1, next.delay_ms);
    }
}

/// The awakening is shown to the lair, like the other bosses' cinematics.
/// (This used to broadcast to every player on the server.)
fn broadcast_to_lair(world: &World, pkt: &[u8]) {
    crate::game_loop::space::zones::broadcast_to_zone(world, BAIUM_ZONE_ID, pkt);
}

/// `getZoneById(70051)` — `baium_no_restart`, the boss-room zone (z 10061 –
/// 11061). Every archangel target pick is gated on it (Java
/// `zone.isInsideZone(creature)`): the gate is what stops an archangel from
/// locking onto a player on the tower floor *below* the lobby — 2D they stand
/// ~85 apart, but the player is ~930 z down and far outside the zone.
const BAIUM_ZONE_ID: i32 = 70051;

/// Is this object inside Baium's boss zone? Falls open when the zone table
/// isn't loaded (minimal test worlds) — the dist always carries 70051.
fn inside_baium_zone(world: &World, oid: i32) -> bool {
    super::common::in_boss_zone(world, BAIUM_ZONE_ID, oid)
}

/// Java `SELECT_TARGET`, per archangel every 5 s. The Archangels are passive
/// `Monster`s (no aggro range), so without this they never engage: each one
/// keeps the player it already hates *while that player is still inside the
/// boss zone*, else grabs the nearest in-zone player in reach (3D), else
/// regroups on Baium. When Baium falls they despawn.
pub(crate) fn handle_select_target(world: &mut World) {
    let Some(baium) = crate::game_loop::grand_boss::find_alive(world, BAIUM) else {
        // Baium is dead — the archangels leave with him (Java's `deleteMe`).
        for angel in archangels(world) {
            despawn(world, angel);
        }
        return; // don't re-arm
    };
    let baium_pos = pos_of(world, baium);
    for angel in archangels(world) {
        // A hated player who left the zone is abandoned. Java gets the same
        // outcome structurally — the `mostHated` keep-branch requires
        // `zone.isInsideZone(mostHated)` and the miss-branch parks the mob in
        // FOLLOW — but this AI has no FOLLOW intention, so the stale entry
        // must go or `think_attack` keeps chasing the departed player.
        drop_out_of_zone_hate(world, angel);
        // Already locked onto a living in-zone player? Leave it be.
        if has_living_player_target(world, angel) {
            continue;
        }
        match nearest_player_in_range(world, angel, ARCHANGEL_REACH) {
            Some(player) => {
                crate::game_loop::npc::minions::add_hate(world, angel, player, ENGAGE_HATE);
            }
            None => {
                if let Some((x, y, z)) = baium_pos {
                    crate::game_loop::ai::move_npc_to(world, angel, x, y, z);
                }
            }
        }
    }
    world.scheduler.schedule(
        world.tick + SELECT_TARGET_TICKS,
        ScheduledTask::BaiumSelectTarget,
    );
}

/// Every living Archangel.
fn archangels(world: &World) -> Vec<i32> {
    world
        .npcs_with_id(ARCHANGEL)
        .iter()
        .copied()
        .filter(|oid| {
            world
                .objects
                .get_component::<Vitals>(oid)
                .is_some_and(|v| !v.dead)
        })
        .collect()
}

fn despawn(world: &mut World, oid: i32) {
    crate::game_loop::npc::despawn_npc_by_oid(world, oid);
}

/// Does this archangel already hate a living player?
fn has_living_player_target(world: &World, angel: i32) -> bool {
    let Some(aggro) = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&angel)
    else {
        return false;
    };
    aggro.0.keys().any(|&oid| {
        world
            .objects
            .get_component::<crate::model::Player>(&oid)
            .is_some()
            && world
                .objects
                .get_component::<Vitals>(&oid)
                .is_some_and(|v| !v.dead)
            && inside_baium_zone(world, oid)
    })
}

/// The nearest living in-zone player within `range` (3D) of the archangel.
fn nearest_player_in_range(world: &mut World, angel: i32, range: f64) -> Option<i32> {
    let origin = maybe_position(world, angel)?;
    let World { objects, data, .. } = &mut *world;
    let zone = data.zone_data.by_id(BAIUM_ZONE_ID);
    let mut best: Option<(i32, f64)> = None;
    objects.for_each_mut::<(&crate::model::Player, &Position, &Vitals)>(|(p, pos, v)| {
        if v.dead {
            return;
        }
        // `zone.isInsideZone(creature)` — the floor below the lobby is out.
        if zone.is_some_and(|z| !z.contains(pos.x, pos.y, pos.z)) {
            return;
        }
        // 3D, like `getVisibleObjectsInRange` (`calculateDistance3D`).
        let dz = (pos.z - origin.z) as f64;
        let d2 = pos.distance_2d(&origin);
        let d = (d2 * d2 + dz * dz).sqrt();
        if d <= range && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((p.object_id, d));
        }
    });
    best.map(|(oid, _)| oid)
}

/// Zero out aggro entries for players no longer inside the boss zone (the
/// enforcement half of Java's `zone.isInsideZone(mostHated)` keep-condition).
fn drop_out_of_zone_hate(world: &mut World, angel: i32) {
    let Some(aggro) = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&angel)
    else {
        return;
    };
    let stale: Vec<i32> = aggro
        .0
        .keys()
        .copied()
        .filter(|oid| {
            world
                .objects
                .get_component::<crate::model::Player>(oid)
                .is_some()
                && !inside_baium_zone(world, *oid)
        })
        .collect();
    if stale.is_empty() {
        return;
    }
    if let Some(aggro) = world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&angel)
    {
        for oid in stale {
            aggro.0.remove(&oid);
        }
    }
}

/// `Baium.onAttack`'s strider clause: a strider-mounted attacker is hindered,
/// **once** — Java checks both `!isAffectedBySkill(4258)` and that the skill
/// is off cooldown, so it is not recast every swing.
/// `Baium.onAttack`'s threat half: weight the hit, then act on the table.
pub(crate) fn on_baium_damage(
    world: &mut World,
    baium_oid: i32,
    attacker_oid: i32,
    damage: i32,
    is_melee: bool,
) {
    // `_lastAttack = System.currentTimeMillis()` — a hit resets the inactivity
    // clock the CHECK_ATTACK beat watches for the reset and the self-heal.
    super::combat::touch(world, baium_oid);
    super::boss_threat::on_boss_damage(world, baium_oid, attacker_oid, damage, is_melee);
    manage_and_cast(world, baium_oid);
}

// ---------------------------------------------------------------------------
// Skill selection (`manageSkills`)
// ---------------------------------------------------------------------------

/// Baium's five skills.
const BAIUM_ATTACK: i32 = 4127;
const ENERGY_WAVE: i32 = 4128;
const EARTH_QUAKE: i32 = 4129;
const THUNDERBOLT: i32 = 4130;
const GROUP_HOLD: i32 = 4131;

/// `manageSkills` — prune the table, pick the top threat, decay it, and choose
/// a skill for Baium's current health.
///
/// Returns `(target, skill_id)`, or `None` when there is nobody to act on.
pub(crate) fn manage_skills(world: &mut World, baium_oid: i32) -> Option<(i32, i32)> {
    let target = super::boss_threat::take_top_threat(world, baium_oid)?;
    let skill = choose_skill(world, baium_oid);
    Some((target, skill))
}

/// `onAttack`'s skill half: pick a target and skill, then actually cast.
///
/// **This is the half that was missing.** `manage_skills` has existed since the
/// threat slice and nothing ever called it, so Baium chose skills into the void
/// and only ever swung — the "parsed but unconsumed" shape, one level up: a
/// whole decision procedure with no caller. Casting on the boss himself is not
/// applicable here (all five of Baium's are target-cast).
pub(crate) fn manage_and_cast(world: &mut World, baium_oid: i32) {
    if world
        .objects
        .has_component::<crate::model::components::Casting>(&baium_oid)
    {
        return;
    }
    let Some((target, skill_id)) = manage_skills(world, baium_oid) else {
        return;
    };
    super::boss_threat::cast_boss_skill(world, baium_oid, target, skill_id, false);
}

/// The skill ladder. Each 10% roll is taken **in order**, and the pool widens
/// as Baium weakens: two options above 75%, three above 50%, four above 25%,
/// and all four below — with `BAIUM_ATTACK` as the fallback throughout.
///
/// So a party sees Baium's repertoire grow as the fight goes on, which is the
/// same shape as his threat weighting.
fn choose_skill(world: &mut World, baium_oid: i32) -> i32 {
    let (cur, max) = match world.objects.get_component::<Vitals>(&baium_oid) {
        Some(v) => (v.cur_hp, v.max_hp as f64),
        None => return BAIUM_ATTACK,
    };
    // Java's ladders share a tail, so the pool is expressed as the list of
    // skills tried before falling back — top band first.
    let pool: &[i32] = if cur > max * 0.75 {
        &[ENERGY_WAVE, EARTH_QUAKE]
    } else if cur > max * 0.5 {
        &[GROUP_HOLD, ENERGY_WAVE, EARTH_QUAKE]
    } else {
        // Java writes the ≤25 % band separately, but with the same four
        // skills as the 25-50 % one — collapsed here rather than duplicated.
        &[THUNDERBOLT, GROUP_HOLD, ENERGY_WAVE, EARTH_QUAKE]
    };
    for skill in pool {
        if world.roll(100) < 10 {
            return *skill;
        }
    }
    BAIUM_ATTACK
}

// ---------------------------------------------------------------------------
// Entry (Angelic Vortex) / exit (teleport cube) / the death tail
// ---------------------------------------------------------------------------

/// What the Angelic Vortex tells a would-be entrant (Java `onEvent("enter")`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryOutcome {
    /// Baium is dead — the lair is empty, cross freely (`31862-03.html`). Note
    /// the entrant still needs a fabric in Java's flow, but the dead branch is
    /// checked first, so the html shows regardless.
    Dead,
    /// A fight is underway; entry is locked (`31862-02.html`).
    InFight,
    /// No Blooded Fabric — the vortex is inert (`31862-01.html`).
    NoFabric,
    /// Cross over: take the fabric and teleport in.
    Admitted,
}

/// The vortex's decision, given whether the entrant holds a Blooded Fabric.
///
/// The order is Java's and matters: **dead** and **in-fight** are read before
/// the fabric check, so a player without a fabric still sees the "it's over" or
/// "it's busy" scene rather than the generic "you need something" one.
pub(crate) fn entry_outcome(world: &World, has_fabric: bool) -> EntryOutcome {
    match crate::game_loop::grand_boss::status(world, BAIUM) {
        Some(DEAD) => EntryOutcome::Dead,
        Some(IN_FIGHT) => EntryOutcome::InFight,
        _ if !has_fabric => EntryOutcome::NoFabric,
        _ => EntryOutcome::Admitted,
    }
}

/// Where the vortex drops an admitted entrant.
pub(crate) fn teleport_in_loc() -> (i32, i32, i32) {
    TELEPORT_IN_LOC
}

/// A random surface exit for the teleport cube — one of three points, jittered
/// by up to 100 on x/y (Java `dest + getRandom(100)`).
pub(crate) fn random_exit(world: &mut World) -> (i32, i32, i32) {
    let idx = world.roll(TELEPORT_OUT_LOC.len() as i32) as usize;
    let (x, y, z) = TELEPORT_OUT_LOC[idx];
    (x + world.roll(100), y + world.roll(100), z)
}

/// Baium's death tail (Java `onKill`): drop the exit cube where the raid can
/// find it and roar. The generic `on_grand_boss_killed` has already flipped the
/// status to DEAD and armed the respawn; this is only the Baium-specific part.
pub(crate) fn on_baium_killed(world: &mut World) {
    crate::game_loop::npc::spawn_npc_at(
        world,
        TELE_CUBE,
        TELEPORT_CUBIC_LOC.0,
        TELEPORT_CUBIC_LOC.1,
        TELEPORT_CUBIC_LOC.2,
        0,
    );
    let roar = crate::network::server_packets::play_sound(DEATH_SOUND);
    broadcast_to_lair(world, &roar);
    // Java arms `CLEAR_ZONE` at +900 s: the cube is a lift home for a quarter of
    // an hour, then the lair is emptied — the cube despawns and stragglers are
    // sent out.
    world
        .scheduler
        .schedule(world.tick + CLEAR_ZONE_TICKS, ScheduledTask::BaiumClearZone);
}

// ---------------------------------------------------------------------------
// CHECK_ATTACK — the inactivity reset and the self-heal
// ---------------------------------------------------------------------------

/// Java `CHECK_ATTACK` (60 s while Baium lives):
///
/// - **30 minutes without a hit** → the fight is abandoned: clear the zone
///   (despawn the boss and angels, oust any stragglers), put the sleeping stone
///   back and revert to `ALIVE`. The beat does not re-arm.
/// - **5 minutes without a hit, and below 75% HP** → Baium heals himself
///   (`HEAL_OF_BAIUM`, 4135), then the beat re-arms.
/// - otherwise → just re-arm.
///
/// If Baium is already gone (killed, or a prior reset) the beat simply stops.
pub(crate) fn handle_check_attack(world: &mut World) {
    let Some(baium) = crate::game_loop::grand_boss::find_alive(world, BAIUM) else {
        return; // Baium gone — nothing to watch
    };
    let idle = super::combat::idle_ticks(world, baium);

    if idle >= RESET_IDLE_TICKS {
        clear_zone(world);
        crate::game_loop::npc::spawn_npc_at(
            world,
            BAIUM_STONE,
            BAIUM_LOC.0,
            BAIUM_LOC.1,
            BAIUM_LOC.2,
            BAIUM_LOC.3,
        );
        crate::game_loop::grand_boss::set_status(world, BAIUM, ALIVE);
        return; // don't re-arm — the fight is over
    }

    if idle >= HEAL_IDLE_TICKS && wounded_below(world, baium, 0.75) {
        super::boss_threat::cast_boss_skill(world, baium, baium, HEAL_OF_BAIUM, true);
    }
    world.scheduler.schedule(
        world.tick + CHECK_ATTACK_TICKS,
        ScheduledTask::BaiumCheckAttack,
    );
}

/// Is `oid` below `fraction` of its max HP?
fn wounded_below(world: &World, oid: i32, fraction: f64) -> bool {
    world
        .objects
        .get_component::<Vitals>(&oid)
        .is_some_and(|v| v.cur_hp < v.max_hp as f64 * fraction)
}

/// Java `CLEAR_ZONE`: empty the lair — despawn every NPC inside it (Baium and
/// his angels) and scatter any players back to the surface. Fired by the
/// post-kill `BaiumClearZone` task.
pub(crate) fn clear_zone(world: &mut World) {
    let mut npcs = Vec::new();
    world
        .objects
        .for_each_mut::<&crate::model::npc::Npc>(|n| npcs.push(n.object_id));
    for oid in npcs {
        if inside_baium_zone(world, oid) {
            despawn(world, oid);
        }
    }

    let mut players = Vec::new();
    world
        .objects
        .for_each_mut::<&crate::model::Player>(|p| players.push(p.object_id));
    for oid in players {
        if inside_baium_zone(world, oid) {
            let (x, y, z) = random_exit(world);
            crate::game_loop::death::teleport_player(world, oid, x, y, z);
        }
    }
}
