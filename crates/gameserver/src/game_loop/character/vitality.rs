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
//! **Verified inert, not a gap:** `Stat.VITALITY_CONSUME_RATE` (a per-player
//! multiplier on the consumed amount) and `Stat.BONUS_EXP`/`BONUS_SP` (skill-
//! and item-granted flat exp/sp bonuses) are not in the modelled `Stat` set, so
//! each reads as its identity (1 / 0 / 0). **That is exact on this dist**: a
//! sweep of `data/stats/skills` finds **zero** skills granting any of the
//! three, so nothing could ever move them off their identity. They are noted
//! at the two arithmetic sites for whoever ports a later chronicle, not left
//! as TODOs — there is no work here to do.
//!
//! The daily (+`MAX/4`) and weekly (full) refills that
//! `DailyTaskManager.resetVitalityDaily`/`resetVitalityWeekly` apply at 06:30
//! land in [`reset_vitality`] (G33), driven by the `daily_tasks` scheduler — so
//! vitality no longer only ever drains.

use crate::game_loop::helpers::send_to_player;
use crate::model::components::PartyRef;
use crate::model::{MAX_VITALITY_POINTS, MIN_VITALITY_POINTS, Player};
use crate::network::server_packets::sm_ids;
use crate::world::World;

/// Java `Player.isLucky()` — the newbie skill that, under level 10, exempts a
/// character from both the death exp penalty and vitality consumption. Shared
/// with the death path rather than reimplemented: two copies of one predicate
/// is two places for the level bound to drift.
use crate::game_loop::combat::death::is_lucky;
use crate::game_loop::helpers::send_sm_bare_to_player as send_sm;

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
    // Java adds `(1 + getValue(Stat.BONUS_EXP, 0) / 100) - 1` here (and
    // `BONUS_SP` in the sp twin). Neither stat is modelled — and **no skill on
    // this dist grants either**, so the term is identically 0.
    bonus.max(1.0)
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
///
/// **Deliberate deviation, at operator request:** the `YOUR_VITALITY_HAS_DECREASED`
/// line is *not* sent. Every monster kill drains the pool, so Java's decrease
/// notice fires on essentially every kill and reads as chat spam. The increase
/// line and both edge lines (at-maximum / fully-exhausted) still fire — those
/// are rare — and the gauge/UserInfo/party updates below are untouched, so the
/// client still tracks the drain visually.
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
        // Java also sends `YOUR_VITALITY_HAS_DECREASED` on the `points < current`
        // leg; suppressed here on purpose (see the doc comment above).
        if points > current {
            send_sm(world, object_id, sm_ids::YOUR_VITALITY_HAS_INCREASED);
        }
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
    super::player_info::broadcast_user_info(world, object_id);
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
            // `double consumeRate = getMul(VITALITY_CONSUME_RATE, 1); if
            // (consumeRate <= 0) return; points *= consumeRate;` — the
            // Vitality Replenishing Herb family (skill 2580, -10 %) is what
            // grants it. The note that used to sit here said no skill on this
            // dist does; the herbs drop from the Schuttgart golems
            // (22801-22808), which this dist spawns, so it was reachable all
            // along.
            let rate = consume_rate(world, object_id);
            if rate <= 0.0 {
                // Java's early return. It is **shape, not behaviour**: falling
                // through with `points *= 0.0` reaches the same place, since a
                // zero delta leaves the total unchanged and this function
                // reports "nothing happened" for that. Kept so the two read
                // alike, not because a test can tell them apart.
                return false;
            }
            points *= rate;
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
    crate::game_loop::party::notify_party_vitality_points(world, object_id);
}

/// The daily-add step (`MAX_VITALITY_POINTS / 4`, Java `resetVitalityDaily`).
const VITALITY_DAILY_ADD: i32 = MAX_VITALITY_POINTS / 4;

/// Java `DailyTaskManager.resetVitalityDaily` / `resetVitalityWeekly`: the daily
/// refill that keeps vitality from only ever draining (G33). Called by the
/// daily-reset task. On `weekly` the pool is set to max; otherwise `MAX/4` is
/// added. Applies to online players (through `set_vitality_points`, so the
/// gauge and notices update) and the offline population (a DB `CASE WHEN`).
/// No-op unless `enable_vitality`.
pub(crate) fn reset_vitality(world: &mut World, weekly: bool) {
    if !world.cfg.character.enable_vitality {
        return;
    }

    let online: Vec<i32> = world.in_game_player_oids().collect();
    for oid in online {
        let target = if weekly {
            MAX_VITALITY_POINTS
        } else {
            vitality_points(world, oid) + VITALITY_DAILY_ADD
        };
        // Java passes `quiet = false` — players see the vitality-increased line.
        set_vitality_points(world, oid, target, false);
    }

    // Offline characters + every subclass row (Java's two `CASE WHEN` UPDATEs).
    let _ = world
        .db
        .send(crate::db::DbCommand::ResetVitality { weekly });
}

/// `getMul(Stat.VITALITY_CONSUME_RATE, 1)` — the multiplier on vitality
/// **loss**, 1.0 for a character with no such buff up.
fn consume_rate(world: &World, object_id: i32) -> f64 {
    world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&object_id)
        .map_or(1.0, |mods| {
            crate::model::stat_finalize::finalize(
                mods,
                crate::model::stats::Stat::VitalityConsumeRate,
                1.0,
            )
        })
}
