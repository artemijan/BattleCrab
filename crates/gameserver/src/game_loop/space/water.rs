//! Drowning (`Config.ALLOW_WATER`) — the port of `Player.checkWaterState` /
//! `startWaterTask` / `stopWaterTask` and `actor/tasks/player/WaterTask`.
//!
//! Java hangs a `ScheduledFuture` off the player: `scheduleAtFixedRate(new
//! WaterTask(this), breath, 1000)`. The initial delay is the breath gauge (the
//! cyan `SetupGauge` bar the client draws), after which every second costs 1% of
//! max HP. Leaving the water cancels the future and blanks the bar.
//!
//! The port keeps the same shape as a [`WaterTask`] component holding the tick
//! of the next beat — "cancel the task" is removing the component, so the
//! several Java call sites that null out `_taskWater` (death, teleport,
//! `stopAllTimers` on logout) all map onto [`stop_water_task`].
//!
//! Note the two unrelated meanings of "in water" in Java: `Player.isInWater()`
//! is *this* task being active, while `Creature.moveToLocation`'s `isInWater`
//! local is the zone predicate (see [`crate::game_loop::space::position::is_in_water`]).

use crate::data::zone_data::ZoneKind;
use crate::game_loop::helpers::is_dead;
use crate::model::Player;
use crate::model::components::{Vitals, WaterTask};
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::world::World;

use crate::game_loop::helpers::{send_sm_to_player, send_to_player};

/// `SetupGauge`'s cyan bar (`new SetupGauge(objectId, 2, …)`) — the breath
/// meter. 0-blue, 1-red, 2-cyan, 3-green.
const GAUGE_CYAN: i32 = 2;

/// `getStat().getValue(Stat.BREATH, 60000)`'s **base** — the unmodified breath
/// gauge in ms. See [`breath_ms`] for the value actually used.
///
/// **Correction (G34 S4).** This constant used to be the whole story, behind a
/// comment stating that "no skill or item on this dist declares" `Stat.BREATH`.
/// That is wrong: **21 skills** carry a `Breath` effect — Boost Breath (195)
/// and Eva's Kiss (1073) are *learnable*, and the Doom armour set adds 19 item
/// skills. Third self-justifying deviation comment this epic has turned up
/// ([[l2r-deviation-comments-self-justify]]); the lesson is the same each time,
/// so check the claim before trusting the note.
pub(crate) const BREATH_BASE_MS: u64 = 60_000;

/// Java `getStat().getValue(Stat.BREATH, 60000)` — `Stat.defaultValue`, i.e.
/// **`mul × 60000 + add`**.
///
/// The order is the whole content of this function and it was wrong here: the
/// port computed `(60000 + add) × mul`, folding the flat term *inside* the
/// multiply, behind a comment claiming that was what Java does. It is not —
/// `Stat.defaultValue` is `(mul * baseValue) + add + moveTypeValue`, so a
/// multiplier applies to the base alone. The two agree whenever a stat carries
/// only one kind of modifier, which is why it survived: on this dist it took a
/// character holding *both* Eva's Kiss and Boost Breath to tell them apart, and
/// then only by ~1.8 s of breath.
///
/// The two modes still read very differently against that base: Eva's Kiss is
/// `PER 400` (×5 — five minutes underwater), while Boost Breath is `DIFF 180`,
/// which adds 0.18 s. The second looks like a datapack unit slip, but Java
/// computes exactly that, so it is ported as written
/// ([[l2r-port-behaviour-not-intent]]) rather than "corrected" into seconds.
///
/// Now routed through [`crate::model::stat_finalize::finalize`], which *is*
/// `Stat.defaultValue` and was sitting there the whole time — including the
/// `//setparam` fixed-value short-circuit this function never honoured.
pub(crate) fn breath_ms(world: &World, object_id: i32) -> u64 {
    use crate::model::stats::Stat;
    let Some(mods) = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&object_id)
    else {
        return BREATH_BASE_MS;
    };
    crate::model::stat_finalize::finalize(mods, Stat::Breath, BREATH_BASE_MS as f64).max(0.0) as u64
}

/// The 1 s period of `scheduleAtFixedRate(…, timeinwater, 1000)`, in game ticks.
const DAMAGE_PERIOD_TICKS: u64 = 10;

/// Java `Player.isInWater()` — whether the drowning task is running. Distinct
/// from standing in a water zone: the task is only started while
/// `Config.ALLOW_WATER` is on and the player is alive.
pub(crate) fn is_drowning_task_active(world: &World, object_id: i32) -> bool {
    world.objects.has_component::<WaterTask>(&object_id)
}

/// Java `Player.checkWaterState()`, called from `revalidateZone` under
/// `Config.ALLOW_WATER`: start the clock inside a water zone, stop it outside.
pub(crate) fn check_water_state(world: &mut World, object_id: i32) {
    // The sole caller (`zones::revalidate_zone`) has just recomputed and
    // written the membership mask for this exact position, so read it back
    // instead of walking the zone grid a second time.
    let in_water_zone = world
        .objects
        .get_component::<crate::model::components::ZoneFlags>(&object_id)
        .is_some_and(|f| f.mask & ZoneKind::Water.bit() != 0);
    if in_water_zone {
        start_water_task(world, object_id);
    } else {
        stop_water_task(world, object_id);
    }
}

/// Java `Player.startWaterTask()`: idempotent (`_taskWater == null` guard) and
/// refused outright while dead — a corpse on the seabed does not drown.
pub(crate) fn start_water_task(world: &mut World, object_id: i32) {
    if world.objects.has_component::<WaterTask>(&object_id) {
        return;
    }
    if is_dead(world, object_id) {
        return;
    }
    let breath = breath_ms(world, object_id);
    let next_damage_tick = world.tick + breath / 100;
    world
        .objects
        .add_components(&object_id, WaterTask { next_damage_tick });
    send_to_player(
        world,
        object_id,
        server_packets::setup_gauge(object_id, GAUGE_CYAN, breath as i32),
    );
}

/// Java `Player.stopWaterTask()`: cancel and blank the gauge. The zero-length
/// `SetupGauge` is what makes the bar disappear, so it must go out on *every*
/// stop path — surfacing, dying, teleporting, logging out.
pub(crate) fn stop_water_task(world: &mut World, object_id: i32) {
    if !world.objects.has_component::<WaterTask>(&object_id) {
        return;
    }
    world.objects.remove_component::<WaterTask>(&object_id);
    send_to_player(
        world,
        object_id,
        server_packets::setup_gauge(object_id, GAUGE_CYAN, 0),
    );
}

/// `WaterTask.run()` for everyone whose breath has run out — 1% of max HP a
/// second (floored at 1), straight to HP, plus the "unable to breathe" message.
pub(crate) fn drown_tick(world: &mut World) {
    let now = world.tick;
    let mut due: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<(&Player, &mut WaterTask)>(|(p, mut task)| {
            if task.next_damage_tick > now {
                return;
            }
            // Fixed-rate, like `scheduleAtFixedRate`: the next beat is one
            // period on from the one just due, not from now.
            task.next_damage_tick += DAMAGE_PERIOD_TICKS;
            due.push(p.object_id);
        });

    for oid in due {
        let Some(max_hp) = world
            .objects
            .get_component::<Vitals>(&oid)
            .filter(|v| !v.dead)
            .map(|v| v.max_hp)
        else {
            continue;
        };
        // `double reduceHp = getMaxHp() / 100.0; if (reduceHp < 1) reduceHp = 1;`
        let reduce_hp = (max_hp as f64 / 100.0).max(1.0);
        // `reduceCurrentHp(reduceHp, _player, null, false, true, false, false)`
        // — attacker is the drowning player themself and `directlyToHp` is
        // **true**, so CP does not soak it. Routing this through the ordinary
        // player-damage path would have let a full CP bar hold its breath
        // indefinitely.
        crate::game_loop::combat::player_receive_damage_ex(world, oid, oid, reduce_hp, true);
        send_sm_to_player(
            world,
            oid,
            sm_ids::YOU_HAVE_TAKEN_S1_DAMAGE_BECAUSE_YOU_WERE_UNABLE_TO_BREATHE,
            &[SmParam::Int(reduce_hp as i32)],
        );
    }
}
