//! Port of `instancemanager/PcCafePointsManager` — earning PC-café ("PA")
//! points.
//!
//! The store already existed (`characters.pccafe_points`, the `//pccafepoints`
//! GM command, `ExPCCafePointInfo`); what was missing was every way a player
//! *earns* them. There are two mutually exclusive modes, picked by
//! `PcCafeRetailLike`:
//!
//! - **retail-like** (`True` on this dist): a per-player fixed-rate timer pays
//!   a flat `AcquisitionPointsRetailLikePoints` every `PcCafeRewardTime`,
//!   armed by [`run`] at login / on buying premium.
//! - **exp-proportional** ([`give_point`]): `exp * 0.0001 * rate` on every
//!   kill and quest reward. `givePcCafePoint` returns immediately when
//!   retail-like is on, so on this dist's config it is the timer or nothing.
//!
//! Both are behind `PcCafeEnabled`, which is **False** here — so nothing below
//! fires until an operator turns it on. It is ported anyway: the config is the
//! switch, not the reason to skip the code.

use crate::game_loop::helpers::send_to_player;
use crate::model::Player;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::world::World;

use crate::game_loop::helpers::client_for_player;

/// `Player.getPcCafePoints()`.
pub(crate) fn points_of(world: &World, object_id: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.pccafe_points)
        .unwrap_or(0)
}

/// Award `points`, announce it, and refresh the client's counter — the tail
/// both Java methods share verbatim: clamp to the ceiling, `addLong(points)`
/// on the already-chosen message, `setPcCafePoints`, `ExPCCafePointInfo`.
fn award(world: &mut World, object_id: i32, mut points: i32, message_id: i16) {
    let max = world.cfg.premium.pc_cafe_max_points;
    let current = points_of(world, object_id);
    if current + points > max {
        points = max - current;
    }
    send_to_player(
        world,
        object_id,
        server_packets::system_message_with(message_id, &[SmParam::Int(points)]),
    );
    let total = current + points;
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.pccafe_points = total;
    }
    send_to_player(
        world,
        object_id,
        server_packets::ex_pccafe_point_info(total, points, 1),
    );
}

/// Java's shared "double points?" coin flip, which also picks the message.
/// Returns `(points, message_id)`.
fn maybe_double(world: &mut World, points: i32, single_message: i16) -> (i32, i16) {
    if world.cfg.premium.pc_cafe_enable_double_points
        && world.roll(100) < world.cfg.premium.pc_cafe_double_points_chance
    {
        (points * 2, sm_ids::DOUBLE_POINTS_YOU_EARNED_S1_PA_POINT_S)
    } else {
        (points, single_message)
    }
}

/// `Rnd.get(points / 2, points)` — inclusive on both ends in Java.
fn randomize(world: &mut World, points: i32) -> i32 {
    let low = points / 2;
    if points <= low {
        return points;
    }
    low + world.roll(points - low + 1)
}

/// `PcCafePointsManager.run(player)` — arm the retail-like fixed-rate timer for
/// one player. A no-op unless the system is on *and* in retail-like mode.
///
/// Java re-runs this on every premium purchase without cancelling the previous
/// schedule, which stacks timers on one player. The port's `seq` guard (the
/// [`reco`](crate::game_loop::character::reco) pattern) makes the older schedule stale instead, so a
/// second purchase does **not** double the payout rate — a deliberate
/// divergence from a leak, noted rather than reproduced.
pub(crate) fn run(world: &mut World, player_object_id: i32) {
    if !world.cfg.premium.pc_cafe_enabled || !world.cfg.premium.pc_cafe_retail_like {
        return;
    }
    // Java's third guard is `player.hasEnteredWorld()`; here the call sites are
    // all past that point by construction, so the presence of the `Player`
    // component stands in for it.
    if world
        .objects
        .get_component::<Player>(&player_object_id)
        .is_none()
    {
        return;
    }
    let seq = world.next_pc_cafe_seq();
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_object_id) {
        p.pc_cafe_seq = seq;
    }
    let period = reward_period_ticks(world);
    world.scheduler.schedule(
        world.tick + period,
        ScheduledTask::PcCafeReward {
            player_object_id,
            seq,
        },
    );
}

/// `Config.PC_CAFE_REWARD_TIME` in 100 ms ticks, never below one tick (Java's
/// `ThreadPool.validate` floor is 0, which is what makes the reference server's
/// unassigned 0 blow up — see the config field's doc).
fn reward_period_ticks(world: &World) -> u64 {
    crate::scheduler::ms_to_ticks(world.cfg.premium.pc_cafe_reward_time).max(1)
}

/// The `PcCafeReward` task body: `giveRetailPcCafePont`, then reschedule.
pub(crate) fn handle_reward(world: &mut World, player_object_id: i32, seq: u64) {
    // Stale (logged out, or re-armed by a later `run`) → no-op, which is how the
    // per-session fixed-rate task is cancelled.
    let Some(p) = world.objects.get_component::<Player>(&player_object_id) else {
        return;
    };
    if p.pc_cafe_seq != seq {
        return;
    }
    give_retail_point(world, player_object_id);
    let period = reward_period_ticks(world);
    world.scheduler.schedule(
        world.tick + period,
        ScheduledTask::PcCafeReward {
            player_object_id,
            seq,
        },
    );
}

/// `PcCafePointsManager.giveRetailPcCafePont(player)` — the flat timed award.
pub(crate) fn give_retail_point(world: &mut World, player_object_id: i32) {
    let cfg = &world.cfg.premium;
    if !cfg.pc_cafe_enabled || !cfg.pc_cafe_retail_like {
        return;
    }
    // Java's `isOnlineInt() == 0` / `isInOfflineMode()` pair: an offline-shop
    // character is still in the world but must not earn.
    if client_for_player(world, player_object_id).is_none()
        || world.offline_traders.contains_key(&player_object_id)
    {
        return;
    }
    if cfg.pc_cafe_only_premium
        && !crate::game_loop::admin::premium::has_premium_status(world, player_object_id)
    {
        return;
    }

    let mut points = world.cfg.premium.acquisition_pc_cafe_retail_like_points;
    // **Java compares the *award* to the ceiling, not the player's balance** —
    // `if (points >= Config.PC_CAFE_MAX_POINTS)`. With the dist's 10 vs 200 000
    // this never trips, so a player at the cap keeps being told they earned
    // points while `award`'s clamp quietly hands them 0. Reproduced as written;
    // "fixing" it here would diverge from the reference server on any config
    // where the two are comparable.
    if points >= world.cfg.premium.pc_cafe_max_points {
        send_to_player(
            world,
            player_object_id,
            server_packets::system_message_with(
                sm_ids::YOU_HAVE_EARNED_THE_MAXIMUM_NUMBER_OF_PA_POINTS,
                &[],
            ),
        );
        return;
    }
    if world.cfg.premium.pc_cafe_random_point {
        points = randomize(world, points);
    }
    let (points, message_id) = maybe_double(world, points, sm_ids::YOU_EARNED_S1_PA_POINT_S);
    award(world, player_object_id, points, message_id);
}

/// `PcCafePointsManager.givePcCafePoint(player, exp)` — the exp-proportional
/// award, run after every kill reward and quest XP grant.
///
/// Inert while `PcCafeRetailLike` is on (its own first guard), which is this
/// dist's configuration.
pub(crate) fn give_point(world: &mut World, player_object_id: i32, exp: f64) {
    let cfg = &world.cfg.premium;
    if cfg.pc_cafe_retail_like || !cfg.pc_cafe_enabled {
        return;
    }
    // No points from a peace/PVP/siege zone, from a jailed player, or from
    // someone who isn't really here.
    let in_zone = |kind: ZoneKind| {
        world
            .objects
            .get_component::<crate::model::components::ZoneFlags>(&player_object_id)
            .is_some_and(|f| f.contains(kind))
    };
    use crate::data::zone_data::ZoneKind;
    if in_zone(ZoneKind::Peace) || in_zone(ZoneKind::Pvp) || in_zone(ZoneKind::Siege) {
        return;
    }
    if client_for_player(world, player_object_id).is_none()
        || world
            .objects
            .get_component::<Player>(&player_object_id)
            .is_some_and(|p| p.jailed)
    {
        return;
    }
    if world.cfg.premium.pc_cafe_only_premium
        && !crate::game_loop::admin::premium::has_premium_status(world, player_object_id)
    {
        return;
    }
    // Unlike the retail-like path, *this* max check reads the balance.
    if points_of(world, player_object_id) >= world.cfg.premium.pc_cafe_max_points {
        send_to_player(
            world,
            player_object_id,
            server_packets::system_message_with(
                sm_ids::YOU_HAVE_EARNED_THE_MAXIMUM_NUMBER_OF_PA_POINTS,
                &[],
            ),
        );
        return;
    }

    let mut points = (exp * 0.0001 * world.cfg.premium.pc_cafe_point_rate) as i32;
    if world.cfg.premium.pc_cafe_random_point {
        points = randomize(world, points);
    }
    // A kill too small to earn anything still pays 1 point, `RewardLowExpKills`
    // percent of the time.
    if points == 0
        && exp > 0.0
        && world.cfg.premium.pc_cafe_reward_low_exp_kills
        && world.roll(100) < world.cfg.premium.pc_cafe_low_exp_kills_chance
    {
        points = 1;
    }
    if points <= 0 {
        return;
    }
    // **Java's else branch sends the *double-points* message too** — both arms
    // of `givePcCafePoint`'s if/else assign
    // `DOUBLE_POINTS_YOU_EARNED_S1_PA_POINT_S`, where the sibling
    // `giveRetailPcCafePont` correctly uses the single-point string. A
    // copy-paste slip upstream, but it is what a player on the reference server
    // sees, so it is what they see here.
    let (points, message_id) = maybe_double(
        world,
        points,
        sm_ids::DOUBLE_POINTS_YOU_EARNED_S1_PA_POINT_S,
    );
    award(world, player_object_id, points, message_id);
}

/// Java `EnterWorld.runImpl`'s `PcCafePointsManager.getInstance().run(player)`.
pub(crate) fn on_enter_world(world: &mut World, player_object_id: i32) {
    run(world, player_object_id);
}
