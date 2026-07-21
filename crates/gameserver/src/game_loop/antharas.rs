//! Antharas (`ai/bosses/Antharas`) — the minion escalation.
//!
//! Antharas's pressure comes from adds that arrive every five minutes in
//! **growing waves**, capped so the lair cannot be flooded without bound.

use crate::scheduler::ScheduledTask;
use crate::world::World;

pub const ANTHARAS: i32 = 29068;
const BEHEMOTH: i32 = 29069;
const TERASQUE: i32 = 29190;

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
        Self { count: 0, multiplier: 1 }
    }
}

/// Arm the first wave.
pub(crate) fn begin_waves(world: &mut World, antharas_oid: i32) {
    if world.objects.get_component::<AntharasMinions>(&antharas_oid).is_none() {
        world.objects.add_components(&antharas_oid, AntharasMinions::default());
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
    let Some(state) = world.objects.get_component::<AntharasMinions>(&antharas_oid).copied() else {
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
        spawned.push(if world.roll(2) == 0 { BEHEMOTH } else { TERASQUE });
    }

    let pos = world.objects.get_component::<crate::model::components::Position>(&antharas_oid).copied();
    if let Some(p) = pos {
        for npc_id in &spawned {
            crate::model::npc::spawn_npc_at(world, *npc_id, p.x, p.y, p.z, 0);
        }
    }

    // `getRandom(100) > 10 && multiplier < 4` — the waves grow, but stop at 4.
    let grow = world.roll(100) > GROWTH_ROLL_ABOVE;
    if let Some(s) = world.objects.get_component_mut::<AntharasMinions>(&antharas_oid) {
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
    Beat { camera: [700, 13, -19, 0, 10000, 20000, 0, 0, 0, 0, 0], next_delay_ms: 3_000, social: None },
    Beat { camera: [700, 13, 0, 6000, 10000, 20000, 0, 0, 0, 0, 0], next_delay_ms: 10_000, social: None },
    // `CAMERA_3` roars, schedules `CAMERA_4` at +200 **and** a second social at
    // +5200 — the only beat that forks.
    Beat { camera: [3700, 0, -3, 0, 10000, 10000, 0, 0, 0, 0, 0], next_delay_ms: 200, social: Some(SOCIAL_ROAR) },
    Beat { camera: [1100, 0, -3, 22000, 10000, 30000, 0, 0, 0, 0, 0], next_delay_ms: 10_800, social: None },
    Beat { camera: [1100, 0, -3, 300, 10000, 7000, 0, 0, 0, 0, 0], next_delay_ms: 1_900, social: None },
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
        beat.camera[0], beat.camera[1], beat.camera[2], beat.camera[3], beat.camera[4],
        beat.camera[5], beat.camera[6], beat.camera[7], beat.camera[8], beat.camera[9],
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
    if let Some(p) = world.objects.get_component_mut::<crate::model::components::Position>(&antharas_oid) {
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
