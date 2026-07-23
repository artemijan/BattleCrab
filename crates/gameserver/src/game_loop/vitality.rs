//! The vitality system — port of the `PlayerStat` vitality half plus
//! `Attackable.getVitalityPoints`.
//!
//! Vitality is a per-character pool (0..=140 000, `characters.vitality_points`)
//! that monster kills drain and that multiplies exp/sp while any of it is left
//! (`RateVitalityExpMultiplier` = ×2 on this dist). `EnableVitality = True`
//! here, so the whole system is live.
//!
//! Java splits this across `PlayerStat` (the pool + the bonus multipliers) and
//! `Attackable` (how much a given kill costs); both live here, since the Rust
//! `Player` is a plain component rather than an object with a stat sub-object.
//!
//! **Not ported (`TODO(G19)`):** `Stat.VITALITY_CONSUME_RATE` (a per-player
//! multiplier on the consumed amount) and `Stat.BONUS_EXP`/`BONUS_SP` (skill- and
//! item-granted flat exp/sp bonuses) — none of the three is in the modelled
//! `Stat` set yet, so each reads as its identity (1 / 0 / 0). When the effect
//! breadth milestone adds them, fold them in at the two marked sites.
//!
//! **Not ported (`TODO(G33)`):** the daily (+25 %) and weekly (full) refills
//! that `DailyTaskManager.resetVitalityDaily`/`resetVitalityWeekly` apply at
//! 06:30. They need the wall-clock daily-task scheduler that G33 brings; the
//! recommendation system's `schedule_initial_daily_reset` is the pattern to
//! reuse. Until then vitality only ever drains, and only a fresh character (or
//! `//set_vitality_level`) refills it.

use crate::model::components::PartyRef;
use crate::model::{Player, MAX_VITALITY_POINTS, MIN_VITALITY_POINTS};
use crate::network::server_packets::{self, sm_ids};
use crate::world::World;

use super::helpers::client_for_player;

/// `CommonSkill.LUCKY` — the newbie "Lucky" skill that, under level 10,
/// exempts a character from vitality consumption entirely.
const LUCKY_SKILL_ID: i32 = 194;

/// `PlayerStat.getVitalityPoints` — the stored pool, clamped. (Java's
/// subclass branch is absent: no subclasses on this dist, `class_index` is
/// always 0.)
pub(crate) fn vitality_points(world: &World, object_id: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| {
            p.vitality_points
                .clamp(MIN_VITALITY_POINTS, MAX_VITALITY_POINTS)
        })
        .unwrap_or(0)
}

/// `PlayerStat.getVitalityExpBonus` — the exp/sp multiplier vitality itself
/// contributes: `RateVitalityExpMultiplier` while any points remain, else 1.
///
/// Java reads it as `getMul(Stat.VITALITY_EXP_RATE, RATE_VITALITY_EXP_MULTIPLIER)`,
/// i.e. the config value is the *default* a `VITALITY_EXP_RATE` modifier could
/// scale; nothing on this dist carries that stat, so the config value stands.
pub(crate) fn vitality_exp_bonus(world: &World, object_id: i32) -> f64 {
    if vitality_points(world, object_id) > 0 {
        world.cfg.rates.rate_vitality_exp_multiplier
    } else {
        1.0
    }
}

/// `PlayerStat.getExpBonusMultiplier` / `getSpBonusMultiplier` — both are the
/// same shape and, with `BONUS_EXP`/`BONUS_SP` unmodelled, the same value:
/// start at 1, add the vitality bonus's *excess over 1*, floor at 1.
///
/// Java also clamps to `Config.MAX_BONUS_EXP`/`MAX_BONUS_SP` when those are
/// non-zero; both are 0 (disabled) on this dist, so the clamp is a no-op and is
/// not ported.
pub(crate) fn exp_bonus_multiplier(world: &World, object_id: i32) -> f64 {
    let vitality = vitality_exp_bonus(world, object_id);
    let mut bonus = 1.0;
    if vitality > 1.0 {
        bonus += vitality - 1.0;
    }
    // TODO(G19): Java adds `(1 + getValue(Stat.BONUS_EXP, 0) / 100) - 1` here
    // (and `BONUS_SP` in the sp twin) — neither stat is modelled yet.
    bonus.max(1.0)
}

/// Java `Player.isLucky()`: level ≤ 9 **and** carrying the Lucky skill. Such a
/// character never spends vitality.
fn is_lucky(world: &World, object_id: i32) -> bool {
    let Some(p) = world.objects.get_component::<Player>(&object_id) else {
        return false;
    };
    p.level <= 9
        && world
            .objects
            .get_component::<crate::model::components::Buffs>(&object_id)
            .is_some_and(|b| b.0.iter().any(|x| x.skill_id == LUCKY_SKILL_ID))
}

/// `PlayerStat.setVitalityPoints(value, quiet)` — clamp, store, and (unless
/// quiet) tell the player what changed. Returns true if the pool moved.
///
/// The notification set is Java's, in Java's order: the increased/decreased
/// line first, then the at-maximum / fully-exhausted edge line, then
/// `ExVitalityPointInfo`, the `broadcastUserInfo` pair (Java scopes it to the
/// `VITA_FAME` component; this port resends the whole packet — the same
/// approach the rest of the port takes to component-scoped UserInfo updates),
/// and finally the party window's vitality field.
pub(crate) fn set_vitality_points(
    world: &mut World,
    object_id: i32,
    value: i32,
    quiet: bool,
) -> bool {
    let points = value.clamp(MIN_VITALITY_POINTS, MAX_VITALITY_POINTS);
    let current = vitality_points(world, object_id);
    if points == current {
        return false;
    }

    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.vitality_points = points;
    } else {
        return false;
    }

    if !quiet {
        let sm = if points < current {
            sm_ids::YOUR_VITALITY_HAS_DECREASED
        } else {
            sm_ids::YOUR_VITALITY_HAS_INCREASED
        };
        send_sm(world, object_id, sm);
        if points == MIN_VITALITY_POINTS {
            send_sm(world, object_id, sm_ids::YOUR_VITALITY_IS_FULLY_EXHAUSTED);
        } else if points == MAX_VITALITY_POINTS {
            send_sm(world, object_id, sm_ids::YOUR_VITALITY_IS_AT_MAXIMUM);
        }
    }

    // Java sends these regardless of `quiet` — the client's vitality gauge must
    // track the pool even on a silent change (`//set_vitality` passes quiet).
    send_to_player(
        world,
        object_id,
        crate::network::enter_world::ex_vitality_point_info(points),
    );
    // Java `broadcastUserInfo(UserInfoType.VITA_FAME)`: UserInfo to self *and*
    // CharInfo to everyone who can see them.
    super::party::broadcast_user_info(world, object_id);
    notify_party_vitality(world, object_id);
    true
}

/// `PlayerStat.updateVitalityPoints(value, useRates, quiet)` — apply a *delta*
/// to the pool, optionally through the gain/lost rate config.
///
/// `value` is signed: negative consumes (a monster kill), positive restores (a
/// vitality item, the daily refill). Returns true if the pool moved.
pub(crate) fn update_vitality_points(
    world: &mut World,
    object_id: i32,
    value: i32,
    use_rates: bool,
    quiet: bool,
) -> bool {
    if value == 0 || !world.cfg.character.enable_vitality {
        return false;
    }

    let mut points = value as f64;
    if use_rates {
        if is_lucky(world, object_id) {
            return false;
        }
        if points < 0.0 {
            // TODO(G19): Java scales by `getMul(Stat.VITALITY_CONSUME_RATE, 1)`
            // here and bails out entirely when that rate is <= 0; the stat is
            // unmodelled, so the rate is its identity (1) and the bail-out is
            // unreachable.
            points *= 1.0;
        }
        // Java's two branches read `points > 0` *after* the consume scaling, so
        // a rate that flipped the sign would take the other branch; kept as-is.
        if points > 0.0 {
            points *= world.cfg.rates.rate_vitality_gain;
        } else {
            points *= world.cfg.rates.rate_vitality_lost;
        }
    }

    let current = vitality_points(world, object_id);
    let target = if points > 0.0 {
        (current as f64 + points).min(MAX_VITALITY_POINTS as f64)
    } else {
        (current as f64 + points).max(MIN_VITALITY_POINTS as f64)
    };
    if (target - current as f64).abs() <= 1e-6 {
        return false;
    }
    set_vitality_points(world, object_id, target as i32, quiet)
}

/// `Attackable.getVitalityPoints(level, exp, isBoss)` — what a kill costs, as a
/// **negative** delta (Java returns `-points`; a zero means "no change").
///
/// `npc_level`/`exp_reward` are the killed monster's; `player_level` the
/// killer's. Below level 85 the divisor is a hard-coded 1000 — which is every
/// character on an Interlude server — so `VitalityConsumeByMob`/`ByBoss` only
/// matter for the 85+ branch, ported for faithfulness.
pub(crate) fn kill_vitality_delta(
    world: &World,
    npc_level: i32,
    npc_exp_reward: f64,
    player_level: i32,
    exp: f64,
    is_boss: bool,
) -> i32 {
    let consume_by_boss = world.cfg.npc.vitality_consume_by_boss;
    if npc_level <= 0 || npc_exp_reward <= 0.0 || (is_boss && consume_by_boss == 0) {
        return 0;
    }
    let level_gap = (player_level - npc_level).max(1) as f64;
    let divisor = if player_level < 85 {
        1000.0
    } else if is_boss {
        consume_by_boss as f64
    } else {
        world.cfg.npc.vitality_consume_by_mob as f64
    };
    // Java: `Math.max((int) ((exp / divisor) * levelGap), 1)` — the int cast
    // truncates before the max, so any sub-1 result still costs 1 point.
    let points = ((exp / divisor) * level_gap) as i32;
    -points.max(1)
}

// ---------------------------------------------------------------------------
// Local send helpers (the same shape `reco.rs` uses)
// ---------------------------------------------------------------------------

fn send_to_player(world: &World, object_id: i32, packet: Vec<u8>) {
    if let Some(cid) = client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(packet);
        }
    }
}

fn send_sm(world: &World, object_id: i32, message_id: i16) {
    send_to_player(
        world,
        object_id,
        server_packets::system_message_with(message_id, &[]),
    );
}

/// The `PartySmallWindowUpdate` vitality piggyback (Java adds the
/// `VITALITY_POINTS` component type and broadcasts to the other members).
fn notify_party_vitality(world: &World, object_id: i32) {
    if world
        .objects
        .get_component::<PartyRef>(&object_id)
        .is_none()
    {
        return;
    }
    super::party::notify_party_vitality_points(world, object_id);
}
