//! Antharas (`ai/bosses/Antharas`) — the minion escalation.
//!
//! Antharas's pressure comes from adds that arrive every five minutes in
//! **growing waves**, capped so the lair cannot be flooded without bound.

use crate::scheduler::ScheduledTask;
use crate::world::World;

pub const ANTHARAS: i32 = 29068;
const BEHEMOTH: i32 = 29069;
const TERASQUE: i32 = 29190;

/// The four-state ladder (Java `GrandBossManager` statuses for Antharas).
/// `DORMANT` and `DEAD` have no reader yet, but the ladder is kept whole —
/// the numeric values mirror Java's and dropping members would invite a
/// re-numbering bug when the remaining states land.
#[allow(dead_code)]
pub const DORMANT: i32 = 0;
pub const WAITING: i32 = 1;
pub const IN_FIGHT: i32 = 2;
#[allow(dead_code)]
pub const DEAD: i32 = 3;

/// Where an admitted player lands: `(179700+rnd(700), 113800+rnd(2100), -7709)`.
const ENTRY_POINT: (i32, i32, i32) = (179700, 113800, -7709);
/// `SPAWN_ANTHARAS`: the boss teleports to the fight platform.
const FIGHT_POINT: (i32, i32, i32, i32) = (181323, 114850, -7623, 32542);
/// `teleportOut`: `(79800+rnd(600), 151200+rnd(1100), -3534)` — Giran side.
const EXIT_POINT: (i32, i32, i32) = (79800, 151200, -3534);

const TICKS_PER_SECOND: u64 = 10;
/// `startQuestTimer("SPAWN_MINION", 300000, …)` — a wave every five minutes.
const WAVE_INTERVAL_SECS: u64 = 300;
/// The multiplier grows to at most 4, so waves top out at 8 adds.
const MAX_MULTIPLIER: i32 = 4;
/// `getRandom(100) > 10` — the multiplier grows on ~89% of waves.
const GROWTH_ROLL_ABOVE: i32 = 10;

/// Antharas's minion bookkeeping (Java's `_minionCount` / `minionMultipler`
/// statics — per-boss here rather than global, since two Antharas instances
/// sharing one counter is a bug waiting to happen).
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy)]
pub struct AntharasMinions {
    pub count: i32,
    pub multiplier: i32,
}

impl Default for AntharasMinions {
    fn default() -> Self {
        // Java starts the multiplier at 1: the first wave is a single pair.
        Self {
            count: 0,
            multiplier: 1,
        }
    }
}

/// Arm the first wave.
pub(crate) fn begin_waves(world: &mut World, antharas_oid: i32) {
    if world
        .objects
        .get_component::<AntharasMinions>(&antharas_oid)
        .is_none()
    {
        world
            .objects
            .add_components(&antharas_oid, AntharasMinions::default());
    }
    world.scheduler.schedule(
        world.tick + WAVE_INTERVAL_SECS * TICKS_PER_SECOND,
        ScheduledTask::AntharasMinionWave { antharas_oid },
    );
}

/// One wave.
///
/// The ladder is cap-aware, and the steps are **not** interchangeable:
///
/// 1. Room for a full wave (`count < 100 - multiplier*2`) → `multiplier` pairs.
/// 2. Else room for a pair (`count < 98`) → one pair.
/// 3. Else room for one (`count < 99`) → a **single, randomly chosen** dragon.
/// 4. Else nothing.
///
/// Step 3 is the interesting one: at 98 minions Antharas adds one more of a
/// random type rather than skipping the wave, so the lair fills to exactly 99
/// rather than stalling at an even number. Collapsing the ladder to "spawn a
/// pair if there is room for two" would lose that and cap the fight two adds
/// early.
pub(crate) fn handle_wave(world: &mut World, antharas_oid: i32) {
    let Some(state) = world
        .objects
        .get_component::<AntharasMinions>(&antharas_oid)
        .copied()
    else {
        return;
    };
    let mut spawned: Vec<i32> = Vec::new();

    if state.multiplier > 1 && state.count < 100 - (state.multiplier * 2) {
        for _ in 0..state.multiplier {
            spawned.push(BEHEMOTH);
            spawned.push(TERASQUE);
        }
    } else if state.count < 98 {
        spawned.push(BEHEMOTH);
        spawned.push(TERASQUE);
    } else if state.count < 99 {
        spawned.push(if world.roll(2) == 0 {
            BEHEMOTH
        } else {
            TERASQUE
        });
    }

    let pos = world
        .objects
        .get_component::<crate::model::components::Position>(&antharas_oid)
        .copied();
    if let Some(p) = pos {
        for npc_id in &spawned {
            crate::model::npc::spawn_npc_at(world, *npc_id, p.x, p.y, p.z, 0);
        }
    }

    // `getRandom(100) > 10 && multiplier < 4` — the waves grow, but stop at 4.
    let grow = world.roll(100) > GROWTH_ROLL_ABOVE;
    if let Some(s) = world
        .objects
        .get_component_mut::<AntharasMinions>(&antharas_oid)
    {
        s.count += spawned.len() as i32;
        if grow && s.multiplier < MAX_MULTIPLIER {
            s.multiplier += 1;
        }
    }

    world.scheduler.schedule(
        world.tick + WAVE_INTERVAL_SECS * TICKS_PER_SECOND,
        ScheduledTask::AntharasMinionWave { antharas_oid },
    );
}

// ---------------------------------------------------------------------------
// The entry cinematic
// ---------------------------------------------------------------------------

/// `SocialAction` ids Antharas plays during the sequence.
const SOCIAL_ROAR: i32 = 1;
const SOCIAL_SECOND: i32 = 2;

/// One beat: the camera args, the delay to the **next** beat, and any social
/// action played alongside.
struct Beat {
    camera: [i32; 11],
    next_delay_ms: u64,
    social: Option<i32>,
}

/// Antharas's five camera beats.
///
/// **Antharas chains; Valakas does not.** Valakas arms all ten of its beats up
/// front from the start of the sequence; Antharas has each step schedule the
/// next with a *relative* delay, and one step (`CAMERA_3`) forks a second timer
/// for a later social action. The two scripts genuinely differ, so this is
/// ported as a chain rather than reshaped to match the Valakas table — reusing
/// that shape here would have quietly changed the timing model.
const BEATS: [Beat; 5] = [
    Beat {
        camera: [700, 13, -19, 0, 10000, 20000, 0, 0, 0, 0, 0],
        next_delay_ms: 3_000,
        social: None,
    },
    Beat {
        camera: [700, 13, 0, 6000, 10000, 20000, 0, 0, 0, 0, 0],
        next_delay_ms: 10_000,
        social: None,
    },
    // `CAMERA_3` roars, schedules `CAMERA_4` at +200 **and** a second social at
    // +5200 — the only beat that forks.
    Beat {
        camera: [3700, 0, -3, 0, 10000, 10000, 0, 0, 0, 0, 0],
        next_delay_ms: 200,
        social: Some(SOCIAL_ROAR),
    },
    Beat {
        camera: [1100, 0, -3, 22000, 10000, 30000, 0, 0, 0, 0, 0],
        next_delay_ms: 10_800,
        social: None,
    },
    Beat {
        camera: [1100, 0, -3, 300, 10000, 7000, 0, 0, 0, 0, 0],
        next_delay_ms: 1_900,
        social: None,
    },
];

/// `CAMERA_3` forks this at +5200 ms.
const SECOND_SOCIAL_DELAY_MS: u64 = 5_200;

/// Where Antharas walks once the cinematic ends.
const MOVE_TO: (i32, i32, i32) = (179_011, 114_871, -7_704);

/// Start the entry sequence — `startQuestTimer("CAMERA_1", 23, …)`.
pub(crate) fn begin_cinematic(world: &mut World, antharas_oid: i32) {
    schedule_beat(world, antharas_oid, 0, 23);
}

fn schedule_beat(world: &mut World, antharas_oid: i32, step: u8, delay_ms: u64) {
    world.scheduler.schedule(
        world.tick + (delay_ms * TICKS_PER_SECOND / 1000).max(1),
        ScheduledTask::AntharasCinematic { antharas_oid, step },
    );
}

/// One beat of the chain. Step 5 is the tail: Antharas starts moving and the
/// fight proper begins.
pub(crate) fn handle_cinematic_step(world: &mut World, antharas_oid: i32, step: u8) {
    // Step 5 = `START_MOVE`, past the end of the camera table.
    let Some(beat) = BEATS.get(step as usize) else {
        start_move(world, antharas_oid);
        return;
    };

    let pkt = crate::network::server_packets::special_camera(
        antharas_oid,
        beat.camera[0],
        beat.camera[1],
        beat.camera[2],
        beat.camera[3],
        beat.camera[4],
        beat.camera[5],
        beat.camera[6],
        beat.camera[7],
        beat.camera[8],
        beat.camera[9],
        beat.camera[10],
    );
    broadcast_to_lair(world, &pkt);

    if let Some(action) = beat.social {
        let social = crate::network::server_packets::social_action(antharas_oid, action);
        broadcast_to_lair(world, &social);
        // The fork: a second social lands 5.2 s later, independent of the
        // camera chain.
        world.scheduler.schedule(
            world.tick + SECOND_SOCIAL_DELAY_MS * TICKS_PER_SECOND / 1000,
            ScheduledTask::AntharasSocial { antharas_oid },
        );
    }

    schedule_beat(world, antharas_oid, step + 1, beat.next_delay_ms);
}

/// The forked second social action.
pub(crate) fn handle_social(world: &mut World, antharas_oid: i32) {
    let pkt = crate::network::server_packets::social_action(antharas_oid, SOCIAL_SECOND);
    broadcast_to_lair(world, &pkt);
}

/// `START_MOVE` — the cinematic is over: Antharas takes his AI back and walks
/// into the lair.
fn start_move(world: &mut World, antharas_oid: i32) {
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::Position>(&antharas_oid)
    {
        p.x = MOVE_TO.0;
        p.y = MOVE_TO.1;
        p.z = MOVE_TO.2;
    }
    // The waves only start once he is actually fighting.
    begin_waves(world, antharas_oid);
}

/// The cinematic is shown to the lair, not the surrounding region — the same
/// rule as Valakas's.
fn broadcast_to_lair(world: &World, pkt: &[u8]) {
    for cs in world.clients.values() {
        if let crate::session::ClientSession::InGame(s) = cs {
            let _ = s;
            cs.send(pkt.to_vec());
        }
    }
}

// ---------------------------------------------------------------------------
// The entry gate (Heart of Warding)
// ---------------------------------------------------------------------------

/// `MAX_PEOPLE` — the lair holds 200.
const MAX_PEOPLE: usize = 200;
/// `STONE` — Portal Stone, the entry ticket.
pub const PORTAL_STONE: i32 = 3865;
/// Members must be within this of the Heart to be brought along.
const GATHER_RANGE: f64 = 1000.0;
/// `antaras_no_restart` (`no_restart.xml`) — Java's `getZoneById(70050,
/// NoRestartZone.class)`, the "Antharas Nest" the script broadcasts to and
/// counts occupancy against. (The old `12016` was a Talking Island script zone;
/// it read as empty, so occupancy silently failed open.)
const LAIR_ZONE_ID: i32 = 70050;

/// Why the Heart of Warding did or didn't let someone in. Each maps to one of
/// Java's html pages, and keeping them as an enum means the ladder can be
/// tested without the html plumbing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryVerdict {
    /// `13001-01` — Antharas is dead; nothing to fight.
    BossDead,
    /// `13001-02` — the fight is already underway; entry is locked.
    AlreadyFighting,
    /// `13001-04` — the lair is full, **or** the group would overfill it.
    LairFull,
    /// `13001-05` — only the party (or command-channel) leader may enter a
    /// group.
    NotLeader,
    /// `13001-03` — no Portal Stone.
    NoStone,
    /// Entry granted; the listed players are teleported in.
    Admitted(Vec<i32>),
}

/// `"enter"` on the Heart of Warding.
///
/// The ladder's order is Java's, and two rungs are easy to lose:
///
/// - **Only the leader may bring a group in**, and for a command channel it is
///   the *channel* leader, not the party leader — so a party leader inside a CC
///   is refused.
/// - **The whole group must fit**: `members > MAX_PEOPLE - inside` refuses
///   rather than admitting as many as will fit, so a raid is never split in
///   half by the doorway.
pub(crate) fn try_enter(world: &mut World, player_oid: i32) -> EntryVerdict {
    let inside = players_in_lair(world);
    try_enter_with_occupancy(world, player_oid, inside)
}

/// The ladder, with the lair's occupancy passed in.
///
/// Split out so the "the group would overfill the lair" rung is reachable from
/// a test: filling a 200-player lair for real is impractical, and a test that
/// cannot reach a branch is not testing it.
pub(crate) fn try_enter_with_occupancy(
    world: &mut World,
    player_oid: i32,
    inside: usize,
) -> EntryVerdict {
    match crate::game_loop::grand_boss::status(world, ANTHARAS) {
        Some(3) => return EntryVerdict::BossDead,
        Some(2) => return EntryVerdict::AlreadyFighting,
        _ => {}
    }
    if inside >= MAX_PEOPLE {
        return EntryVerdict::LairFull;
    }

    let group = group_of(world, player_oid);
    if let Some((leader, members)) = group {
        if leader != player_oid {
            return EntryVerdict::NotLeader;
        }
        if !has_stone(world, player_oid) {
            return EntryVerdict::NoStone;
        }
        if members.len() > MAX_PEOPLE - inside {
            return EntryVerdict::LairFull;
        }
        // Only members actually gathered at the Heart come along.
        let near: Vec<i32> = members
            .into_iter()
            .filter(|m| near_leader(world, player_oid, *m))
            .collect();
        return EntryVerdict::Admitted(near);
    }

    if !has_stone(world, player_oid) {
        return EntryVerdict::NoStone;
    }
    EntryVerdict::Admitted(vec![player_oid])
}

/// `(leader, members)` for the player's command channel if they are in one,
/// else their party — `None` when solo.
///
/// The **command channel wins over the party**: a CC leader brings everyone,
/// and a party leader inside a CC is not a leader for this purpose.
fn group_of(world: &World, player_oid: i32) -> Option<(i32, Vec<i32>)> {
    let party_id = world
        .objects
        .get_component::<crate::model::components::PartyRef>(&player_oid)?
        .0;
    let party = world.parties.get(&party_id)?;
    Some((party.leader(), party.members.clone()))
}

fn has_stone(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&oid)
        .is_some_and(|inv| inv.count_of(PORTAL_STONE) > 0)
}

fn near_leader(world: &World, leader: i32, member: i32) -> bool {
    if leader == member {
        return true;
    }
    let (Some(a), Some(b)) = (
        world
            .objects
            .get_component::<crate::model::components::Position>(&leader),
        world
            .objects
            .get_component::<crate::model::components::Position>(&member),
    ) else {
        return false;
    };
    let (dx, dy) = ((a.x - b.x) as f64, (a.y - b.y) as f64);
    (dx * dx + dy * dy).sqrt() <= GATHER_RANGE
}

fn players_in_lair(world: &World) -> usize {
    let Some(zone) = world.data.zone_data.by_id(LAIR_ZONE_ID) else {
        return 0;
    };
    world
        .clients
        .values()
        .filter(|cs| matches!(cs, crate::session::ClientSession::InGame(_)))
        .filter_map(|cs| match cs {
            crate::session::ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        })
        .filter(|oid| {
            world
                .objects
                .get_component::<crate::model::components::Position>(oid)
                .is_some_and(|p| zone.contains(p.x, p.y, p.z))
        })
        .count()
}

// ---------------------------------------------------------------------------
// Skill selection (`manageSkills`)
// ---------------------------------------------------------------------------

/// Antharas's ten skills.
const ANTH_JUMP: i32 = 4106;
const ANTH_TAIL: i32 = 4107;
const ANTH_FEAR: i32 = 4108;
const ANTH_DEBUFF: i32 = 4109;
const ANTH_MOUTH: i32 = 4110;
const ANTH_BREATH: i32 = 4111;
const ANTH_NORM_ATTACK: i32 = 4112;
const ANTH_NORM_ATTACK_EX: i32 = 4113;
const ANTH_FEAR_SHORT: i32 = 5092;
const ANTH_METEOR: i32 = 5093;

/// A chosen skill and whether it is cast on the target or on Antharas himself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub skill_id: i32,
    /// Java's `castOnTarget == false` — the skill is cast **with Antharas as
    /// its own target**, because the tail sweep, the curse and the stomp are
    /// areas centred on him rather than on the player who drew them.
    pub on_self: bool,
}

const fn on_target(skill_id: i32) -> Choice {
    Choice {
        skill_id,
        on_self: false,
    }
}
const fn on_self(skill_id: i32) -> Choice {
    Choice {
        skill_id,
        on_self: true,
    }
}

/// `Util.calculateAngleFrom` — the angle from `a` to `b` in degrees, `0..360`.
fn angle_between(ax: i32, ay: i32, bx: i32, by: i32) -> f64 {
    let deg = ((by - ay) as f64).atan2((bx - ax) as f64).to_degrees();
    if deg < 0.0 {
        deg + 360.0
    } else {
        deg
    }
}

/// **The tail sweep and curse are gated on an absolute world angle, not on
/// where Antharas is facing.**
///
/// Java computes `npc.calculateDirectionTo(c2)`, which is
/// `atan2(targetY - npcY, targetX - npcX)` — the direction from Antharas to his
/// target **in world coordinates**. His `heading` never enters the comparison.
/// So "within 8° of 180" does not mean *behind him*; it means the target is
/// due **west** of him, whichever way he happens to be turned.
///
/// That reads like a bug — the windows are plainly shaped like a rear arc, and
/// every other cone check in the codebase (`Creature.isBehind`,
/// `Formulas.calcCastBreak`) subtracts `convertHeadingToDegree(getHeading())`
/// first. But the datapack is the specification, and a "fix" here would change
/// how often the tail lands, so it is ported exactly as written and pinned by a
/// test that puts the target due west while Antharas faces east.
fn in_arc(dist: f64, angle: f64, near: (f64, f64, f64), far: (f64, f64, f64)) -> bool {
    let (d1, lo1, hi1) = near;
    let (d2, lo2, hi2) = far;
    (dist < d1 && angle < hi1 && angle > lo1) || (dist < d2 && angle < hi2 && angle > lo2)
}

fn tail_arc(dist: f64, angle: f64) -> bool {
    in_arc(dist, angle, (1423.0, 172.0, 188.0), (802.0, 166.0, 194.0))
}

fn debuff_arc(dist: f64, angle: f64) -> bool {
    in_arc(dist, angle, (850.0, 150.0, 210.0), (425.0, 90.0, 270.0))
}

/// `manageSkills` — choose what Antharas does to his current top threat.
///
/// The ladder is **a chain of `else if`, not a weighted table**: each roll is
/// only taken when every roll above it has already failed, so the printed
/// percentages are conditional. `getRandomBoolean()` sitting two-thirds of the
/// way down means the options *below* it are reached less than half the time
/// they are eligible.
///
/// Four health bands, and the repertoire opens up as he is worn down. Only
/// below 25% does he use the Breath Attack at all — and he leads with it, at a
/// 30% chance, before anything else is considered.
pub(crate) fn choose_skill(
    world: &mut World,
    antharas_oid: i32,
    target_oid: i32,
) -> Option<Choice> {
    let (cur, max) = {
        let v = world
            .objects
            .get_component::<crate::model::components::Vitals>(&antharas_oid)?;
        (v.cur_hp, v.max_hp as f64)
    };
    let (dist, angle) = {
        let a = world
            .objects
            .get_component::<crate::model::components::Position>(&antharas_oid)?;
        let b = world
            .objects
            .get_component::<crate::model::components::Position>(&target_oid)?;
        let (dx, dy, dz) = ((a.x - b.x) as f64, (a.y - b.y) as f64, (a.z - b.z) as f64);
        (
            (dx * dx + dy * dy + dz * dz).sqrt(),
            angle_between(a.x, a.y, b.x, b.y),
        )
    };

    let fear = |w: &mut World| {
        if w.roll(2) == 0 {
            on_target(ANTH_FEAR)
        } else {
            on_target(ANTH_FEAR_SHORT)
        }
    };

    if cur < max * 0.25 {
        if world.roll(100) < 30 {
            return Some(on_target(ANTH_MOUTH));
        }
        if world.roll(100) < 80 && tail_arc(dist, angle) {
            return Some(on_self(ANTH_TAIL));
        }
        if world.roll(100) < 40 && debuff_arc(dist, angle) {
            return Some(on_self(ANTH_DEBUFF));
        }
        if world.roll(100) < 10 && dist < 1100.0 {
            return Some(on_self(ANTH_JUMP));
        }
        if world.roll(100) < 10 {
            return Some(on_target(ANTH_METEOR));
        }
        if world.roll(100) < 6 {
            return Some(on_target(ANTH_BREATH));
        }
        if world.roll(2) == 0 {
            return Some(on_target(ANTH_NORM_ATTACK_EX));
        }
        if world.roll(100) < 5 {
            return Some(fear(world));
        }
        return Some(on_target(ANTH_NORM_ATTACK));
    }

    if cur < max * 0.5 {
        if world.roll(100) < 80 && tail_arc(dist, angle) {
            return Some(on_self(ANTH_TAIL));
        }
        if world.roll(100) < 40 && debuff_arc(dist, angle) {
            return Some(on_self(ANTH_DEBUFF));
        }
        if world.roll(100) < 10 && dist < 1100.0 {
            return Some(on_self(ANTH_JUMP));
        }
        if world.roll(100) < 7 {
            return Some(on_target(ANTH_METEOR));
        }
        if world.roll(100) < 6 {
            return Some(on_target(ANTH_BREATH));
        }
        if world.roll(2) == 0 {
            return Some(on_target(ANTH_NORM_ATTACK_EX));
        }
        if world.roll(100) < 5 {
            return Some(fear(world));
        }
        return Some(on_target(ANTH_NORM_ATTACK));
    }

    if cur < max * 0.75 {
        // The curse drops out of this band — above half health Antharas never
        // casts it, so a party that burns him past 50% has *seen* it appear.
        if world.roll(100) < 80 && tail_arc(dist, angle) {
            return Some(on_self(ANTH_TAIL));
        }
        if world.roll(100) < 10 && dist < 1100.0 {
            return Some(on_self(ANTH_JUMP));
        }
        if world.roll(100) < 5 {
            return Some(on_target(ANTH_METEOR));
        }
        if world.roll(100) < 6 {
            return Some(on_target(ANTH_BREATH));
        }
        if world.roll(2) == 0 {
            return Some(on_target(ANTH_NORM_ATTACK_EX));
        }
        if world.roll(100) < 5 {
            return Some(fear(world));
        }
        return Some(on_target(ANTH_NORM_ATTACK));
    }

    // Above 75%: the stomp goes too, leaving the tail, meteor, breath and the
    // two ordinary attacks.
    if world.roll(100) < 80 && tail_arc(dist, angle) {
        return Some(on_self(ANTH_TAIL));
    }
    if world.roll(100) < 3 {
        return Some(on_target(ANTH_METEOR));
    }
    if world.roll(100) < 6 {
        return Some(on_target(ANTH_BREATH));
    }
    if world.roll(2) == 0 {
        return Some(on_target(ANTH_NORM_ATTACK_EX));
    }
    if world.roll(100) < 5 {
        return Some(fear(world));
    }
    Some(on_target(ANTH_NORM_ATTACK))
}

/// `manageSkills`' guard and body: skip while already casting, take the top
/// threat, choose, cast.
pub(crate) fn manage_and_cast(world: &mut World, antharas_oid: i32) {
    if world
        .objects
        .has_component::<crate::model::components::Casting>(&antharas_oid)
    {
        return;
    }
    let Some(target) = super::boss_threat::take_top_threat(world, antharas_oid) else {
        return;
    };
    let Some(choice) = choose_skill(world, antharas_oid, target) else {
        return;
    };
    super::boss_threat::cast_boss_skill(
        world,
        antharas_oid,
        target,
        choice.skill_id,
        choice.on_self,
    );
}

/// `Antharas.onAttack` — the threat half and the skill half, in Java's order.
pub(crate) fn on_antharas_damage(
    world: &mut World,
    antharas_oid: i32,
    attacker_oid: i32,
    damage: i32,
    is_melee: bool,
) {
    super::boss_threat::on_boss_damage(world, antharas_oid, attacker_oid, damage, is_melee);
    manage_and_cast(world, antharas_oid);
}

// ---------------------------------------------------------------------------
// The entry flow (Java `onEvent("enter")` / `SPAWN_ANTHARAS` /
// `teleportOut`), reached through `scripts::antharas_heart`.
// ---------------------------------------------------------------------------

fn set_status(world: &mut World, status: i32) {
    if let Some(b) = world.grand_bosses.get_mut(&ANTHARAS) {
        b.status = status;
    }
    crate::game_loop::grand_boss::persist(world, ANTHARAS);
}

/// The live Antharas NPC, if one stands in the world.
pub(crate) fn find_antharas(world: &World) -> Option<i32> {
    world.npc_regions.values().flatten().copied().find(|oid| {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(oid)
            .is_some_and(|n| n.npc_id == ANTHARAS)
    })
}

/// The Heart of Warding's `enter` bypass: run the ladder, teleport the
/// admitted group in, and arm the `SPAWN_ANTHARAS` window on the first entry.
/// Returns the refusal html name, `None` when admitted (the teleport is the
/// reply, like Java's null htmltext).
pub(crate) fn heart_enter(world: &mut World, player_oid: i32) -> Option<&'static str> {
    match try_enter(world, player_oid) {
        EntryVerdict::BossDead => Some("13001-01.html"),
        EntryVerdict::AlreadyFighting => Some("13001-02.html"),
        EntryVerdict::NoStone => Some("13001-03.html"),
        EntryVerdict::LairFull => Some("13001-04.html"),
        EntryVerdict::NotLeader => Some("13001-05.html"),
        EntryVerdict::Admitted(members) => {
            for member in members {
                let (dx, dy) = (world.roll(700), world.roll(2100));
                crate::game_loop::death::teleport_player(
                    world,
                    member,
                    ENTRY_POINT.0 + dx,
                    ENTRY_POINT.1 + dy,
                    ENTRY_POINT.2,
                );
            }
            // Only the FIRST admission arms the clock — a later party entering
            // during the window must not restart it (Java's
            // `if (getStatus() != WAITING)`).
            if crate::game_loop::grand_boss::status(world, ANTHARAS) != Some(WAITING) {
                set_status(world, WAITING);
                let wait_secs = world.cfg.grand_boss.antharas_wait_minutes.max(1) as u64 * 60;
                world.scheduler.schedule(
                    world.tick + wait_secs * TICKS_PER_SECOND,
                    ScheduledTask::AntharasSpawn,
                );
            }
            None
        }
    }
}

/// `SPAWN_ANTHARAS`: the window elapsed — Antharas takes the platform, the
/// fight starts, the lair hears `BS02_A`, and the camera chain begins (its
/// tail starts the minion waves).
pub(crate) fn handle_spawn_timer(world: &mut World) {
    let Some(oid) = find_antharas(world) else {
        return;
    };
    // A GM could have killed him during the window; a dead boss stays down.
    if crate::game_loop::grand_boss::status(world, ANTHARAS) != Some(WAITING) {
        return;
    }
    crate::game_loop::death::relocate_npc(
        world,
        oid,
        FIGHT_POINT.0,
        FIGHT_POINT.1,
        FIGHT_POINT.2,
        FIGHT_POINT.3,
    );
    set_status(world, IN_FIGHT);
    broadcast_to_lair(world, &crate::network::server_packets::play_sound("BS02_A"));
    begin_cinematic(world, oid);
}

/// The Teleportation Cubic's `teleportOut`.
pub(crate) fn teleport_out(world: &mut World, player_oid: i32) {
    let (dx, dy) = (world.roll(600), world.roll(1100));
    crate::game_loop::death::teleport_player(
        world,
        player_oid,
        EXIT_POINT.0 + dx,
        EXIT_POINT.1 + dy,
        EXIT_POINT.2,
    );
}

// ---------------------------------------------------------------------------
// The death tail (`onKill` + `CLEAR_ZONE`)
// ---------------------------------------------------------------------------

/// The Teleportation Cubic (`html/default/31859.htm`, wired to `teleportOut`).
const CUBE: i32 = 31859;
/// `addSpawn(CUBE, 177615, 114941, -7709, 0, …)` — where the exit cube stands
/// after the kill (distinct from the entry cube location).
const DEATH_CUBE: (i32, i32, i32) = (177615, 114941, -7709);
/// `startQuestTimer("CLEAR_ZONE", 900000)` — the lair empties 15 minutes after
/// the kill.
const CLEAR_ZONE_SECS: u64 = 900;

/// `onKill` for Antharas: despawn the adds, play the death cinematic, drop the
/// exit cube, and arm the 15-minute zone clear. The respawn window and the DEAD
/// status are already set by `grand_boss::on_grand_boss_killed`, which runs
/// first on the shared death path.
pub(crate) fn on_antharas_killed(world: &mut World) {
    // `DESPAWN_MINIONS`: delete every Behemoth/Terasque left in the lair.
    for oid in lair_minions(world) {
        if let Some(region) = world
            .objects
            .get_component::<crate::model::components::RegionCell>(&oid)
            .map(|r| r.0)
        {
            crate::game_loop::death::despawn_npc(world, oid, region);
        }
    }

    // Death cinematic + sound, to the lair.
    let cam = crate::network::server_packets::special_camera(
        0, 1200, 20, -10, 0, 10000, 13000, 0, 0, 0, 0, 0,
    );
    broadcast_to_lair(world, &cam);
    broadcast_to_lair(world, &crate::network::server_packets::play_sound("BS01_D"));

    // The exit cube — `AntharasHeart` already routes its `teleportOut` talk.
    crate::model::npc::spawn_npc_at(world, CUBE, DEATH_CUBE.0, DEATH_CUBE.1, DEATH_CUBE.2, 0);

    world.scheduler.schedule(
        world.tick + CLEAR_ZONE_SECS * TICKS_PER_SECOND,
        ScheduledTask::AntharasClearZone,
    );
}

/// `CLEAR_ZONE`: teleport every lingering player out, then despawn every NPC
/// still in the lair (the cube and any straggler minions).
pub(crate) fn handle_clear_zone(world: &mut World) {
    for player_oid in players_in_lair_oids(world) {
        teleport_out(world, player_oid);
    }
    for oid in npcs_in_lair(world) {
        if let Some(region) = world
            .objects
            .get_component::<crate::model::components::RegionCell>(&oid)
            .map(|r| r.0)
        {
            crate::game_loop::death::despawn_npc(world, oid, region);
        }
    }
}

/// Behemoth/Terasque adds standing in the lair zone.
fn lair_minions(world: &World) -> Vec<i32> {
    let Some(zone) = world.data.zone_data.by_id(LAIR_ZONE_ID) else {
        return Vec::new();
    };
    world
        .npc_regions
        .values()
        .flatten()
        .copied()
        .filter(|oid| {
            let is_minion = world
                .objects
                .get_component::<crate::model::npc::Npc>(oid)
                .is_some_and(|n| n.npc_id == BEHEMOTH || n.npc_id == TERASQUE);
            is_minion
                && world
                    .objects
                    .get_component::<crate::model::components::Position>(oid)
                    .is_some_and(|p| zone.contains(p.x, p.y, p.z))
        })
        .collect()
}

/// Every NPC currently standing in the lair zone.
fn npcs_in_lair(world: &World) -> Vec<i32> {
    let Some(zone) = world.data.zone_data.by_id(LAIR_ZONE_ID) else {
        return Vec::new();
    };
    world
        .npc_regions
        .values()
        .flatten()
        .copied()
        .filter(|oid| {
            world
                .objects
                .get_component::<crate::model::components::Position>(oid)
                .is_some_and(|p| zone.contains(p.x, p.y, p.z))
        })
        .collect()
}

/// Object ids of the online players standing in the lair zone.
fn players_in_lair_oids(world: &World) -> Vec<i32> {
    let Some(zone) = world.data.zone_data.by_id(LAIR_ZONE_ID) else {
        return Vec::new();
    };
    world
        .clients
        .values()
        .filter_map(|cs| match cs {
            crate::session::ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        })
        .filter(|oid| {
            world
                .objects
                .get_component::<crate::model::components::Position>(oid)
                .is_some_and(|p| zone.contains(p.x, p.y, p.z))
        })
        .collect()
}
