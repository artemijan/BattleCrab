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
