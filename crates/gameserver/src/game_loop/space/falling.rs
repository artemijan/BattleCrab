//! Falling damage (`Config.EnableFallingDamage`) — the port of
//! `Player.isFalling` / `setFalling`, `Formulas.calcFallDam` and the 1.5 s
//! `_fallingDamageTask`.
//!
//! Java drives the whole thing from one client packet. `ValidatePosition`
//! reports where the client thinks it is roughly twice a second; when the
//! reported Z is far enough *below* the server's, the player is falling, and
//! `Player.isFalling(z)` does three things at once:
//!
//! 1. computes the damage, **once**, from the drop measured on the first such
//!    report (`if (_fallingDamage == 0)`) — a longer fall does not accumulate;
//! 2. arms (or re-arms) a 1.5 s task that applies it, so the HP comes off
//!    once the player has actually landed rather than mid-air;
//! 3. returns `true` for the next second (`FALLING_VALIDATION_DELAY`), which
//!    makes `ValidatePosition` bail before its reconciliation — Java's own
//!    comment is *"Disable validations during fall to avoid jumping"*.
//!
//! The third is the load-bearing one and is easy to miss: without it the
//! out-of-sync snap fights the client all the way down.
//!
//! **The safe height is a constant here.** `PlayerTemplate.getSafeFallHeight()`
//! reads `baseSafeFall` off `stats/chars/*.xml`, and **no template on this dist
//! declares it** — every class takes Java's `set.getInt("baseSafeFall", 333)`
//! default. So there is nothing to load and nothing to key by class; see
//! [`SAFE_FALL_HEIGHT`].
//!
//! **`Stat.FALL` reduces the damage, not the height** — despite `SafeFallHeight`
//! being the name of the effect that feeds it. See [`Stat::Fall`].
//!
//! **Nothing has to cancel the pending fall.** Java's only other `cancel` site
//! is `Player.stopAllTasks()`, reached from `Disconnection` alone — and there
//! the port's entity is despawned, taking [`FallingDamage`] with it. So the
//! two lifetimes already agree and there is no logout hook to write.

use crate::data::zone_data::ZoneKind;
use crate::model::Player;
use crate::model::components;

use crate::model::stats::Stat;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::world::World;

use crate::game_loop::helpers::{is_dead, send_sm_to_player, send_to_player};

/// `PlayerTemplate._baseSafeFallHeight` — `set.getInt("baseSafeFall", 333)`.
///
/// A constant rather than a per-class template field **because the datapack
/// makes it one**: `grep -c baseSafeFall dist/game/data/stats/chars/*.xml` is
/// zero across every file, so all 31 classes fall back to Java's literal 333.
/// Reading it per class would be a loader with one possible answer.
pub(crate) const SAFE_FALL_HEIGHT: i32 = 333;

/// `Player.FALLING_VALIDATION_DELAY` (1000 ms), in ticks.
const FALLING_VALIDATION_DELAY_TICKS: u64 = 10;

/// The `ThreadPool.schedule(…, 1500)` the damage rides on, in ticks.
const DAMAGE_DELAY_TICKS: u64 = 15;

/// Java `Formulas.calcFallDam(creature, fallHeight)`:
/// `getValue(Stat.FALL, (fallHeight * getMaxHp()) / 1000.0)`, and 0 when the
/// config is off or the height is negative.
///
/// `getValue` with a base runs `Stat.defaultValue(creature, stat, base)`, which
/// is **`mul * base + add`** — the multiplier applies to the base and the flat
/// term is added afterwards, *not* `(base + add) * mul`. Acrobatics (173) is
/// `DIFF -60` / `-100`, so it lands entirely in `add` and takes a flat 60 or
/// 100 off the damage.
///
/// [`crate::model::stat_finalize::finalize`] **is** `Stat.defaultValue` — the same arithmetic
/// plus the `//setparam` fixed-value short-circuit — so this defers to it
/// rather than spelling the maths out a second time.
///
/// The result is deliberately **not** clamped to zero here: Java doesn't, and
/// the caller's `if (_fallingDamage > 0)` is what makes a negative harmless.
pub(crate) fn calc_fall_dam(world: &World, object_id: i32, fall_height: i32) -> f64 {
    if !world.cfg.general.enable_falling_damage || fall_height < 0 {
        return 0.0;
    }
    let Some(max_hp) = world
        .objects
        .get_component::<components::stats::Vitals>(&object_id)
        .map(|v| v.max_hp)
    else {
        return 0.0;
    };
    let base = (fall_height as f64 * max_hp as f64) / 1000.0;
    match world
        .objects
        .get_component::<crate::model::components::stats::StatModifiers>(&object_id)
    {
        Some(mods) => crate::model::stat_finalize::finalize(mods, Stat::Fall, base),
        None => base,
    }
}

/// Java `Player.isFalling(int z)` — **true means "swallow this position
/// report"**, not "is airborne". Called from
/// [`handle_validate_position`](crate::game_loop::space::position::handle_validate_position)
/// before any reconciliation.
///
/// Returns `true` only on the branch where a fall is already in progress and
/// its validation window has not expired. The report that *starts* the fall
/// returns `false` after arming everything, exactly like Java — that first
/// report still reconciles.
pub(crate) fn is_falling(world: &mut World, object_id: i32, client_z: i32) -> bool {
    // `isDead() || isFlying() || isFlyingMounted() || isInsideZone(ZoneId.WATER)`.
    // `isFlyingMounted()` is `checkTransformed(Transform::isFlying)` — a Gracia
    // flying *transformation*, of which this dist has none, so the wyvern check
    // in `Player::is_flying` covers the whole predicate here.
    if is_dead(world, object_id) {
        return false;
    }
    let Some(player) = world.objects.get_component::<Player>(&object_id) else {
        return false;
    };
    if player.is_flying() {
        return false;
    }
    if world
        .objects
        .get_component::<components::space::ZoneFlags>(&object_id)
        .is_some_and(|f| f.mask & ZoneKind::Water.bit() != 0)
    {
        return false;
    }

    let falling_until = player.falling_until_tick;
    if falling_until != 0 && world.tick < falling_until {
        return true;
    }

    let Some(pos) = world
        .objects
        .get_component::<components::space::Position>(&object_id)
        .copied()
    else {
        return false;
    };
    let delta_z = pos.z - client_z;

    // Within the safe height: not a fall at all, and the latch clears.
    if delta_z <= SAFE_FALL_HEIGHT {
        clear_falling(world, object_id);
        return false;
    }

    // "If there is no geodata loaded for the place we are, client Z correction
    // might cause falling damage." Without geo the server's Z is a guess, so a
    // large delta says nothing about a fall.
    if !world.geo.has_geo(pos.x, pos.y) {
        clear_falling(world, object_id);
        return false;
    }

    // `if (_fallingDamage == 0) _fallingDamage = calcFallDam(this, deltaZ);`
    // — the height of the *first* report of this fall is the one that counts.
    let existing = world
        .objects
        .get_component::<components::space::FallingDamage>(&object_id)
        .map(|f| f.damage)
        .unwrap_or(0);
    let damage = if existing == 0 {
        calc_fall_dam(world, object_id, delta_z) as i32
    } else {
        existing
    };
    // Java cancels the pending future and schedules a fresh one; overwriting
    // the component is the same thing without a stale heap entry to worry
    // about.
    world.objects.add_components(
        &object_id,
        components::space::FallingDamage {
            due_tick: world.tick + DAMAGE_DELAY_TICKS,
            damage,
        },
    );

    // "Prevent falling under ground" — push the server's authoritative
    // position back at the client, then latch the validation window.
    send_to_player(
        world,
        object_id,
        server_packets::validate_location(object_id, pos.x, pos.y, pos.z, pos.heading),
    );
    set_falling(world, object_id);

    false
}

/// Java `Player.setFalling()` — arm the validation window for 1 s.
pub(crate) fn set_falling(world: &mut World, object_id: i32) {
    let now = world.tick;
    if let Some(player) = world.objects.get_component_mut::<Player>(&object_id) {
        player.falling_until_tick = now + FALLING_VALIDATION_DELAY_TICKS;
    }
}

/// `_fallingTimestamp = 0` — the two early returns in `isFalling` that decide
/// the player is *not* falling after all.
fn clear_falling(world: &mut World, object_id: i32) {
    if let Some(player) = world.objects.get_component_mut::<Player>(&object_id) {
        player.falling_until_tick = 0;
    }
}

/// `_fallingDamageTask.run()` for everyone whose 1.5 s is up.
///
/// Java's task body reads `if ((_fallingDamage > 0) && !isInvul())`, applies
/// `min(_fallingDamage, getCurrentHp() - 1)` and then clears both fields
/// **unconditionally** — so an invulnerable player's pending fall is discarded,
/// not deferred.
///
/// The clamp is why falling cannot kill: at 1 HP it resolves to 0. The system
/// message still reports the *unclamped* `_fallingDamage`, which is Java's own
/// asymmetry and is ported as written.
pub(crate) fn falling_damage_tick(world: &mut World) {
    let now = world.tick;
    let mut due: Vec<(i32, i32)> = Vec::new();
    world
        .objects
        .for_each_mut::<(&Player, &components::space::FallingDamage)>(|(p, fall)| {
            if fall.due_tick <= now {
                due.push((p.object_id, fall.damage));
            }
        });

    for (oid, damage) in due {
        world
            .objects
            .remove_component::<components::space::FallingDamage>(&oid);
        if damage <= 0 {
            continue;
        }
        let invul = world
            .objects
            .get_component::<components::player::AdminFlags>(&oid)
            .is_some_and(|f| f.invul);
        if invul {
            continue;
        }
        let Some(cur_hp) = world
            .objects
            .get_component::<components::stats::Vitals>(&oid)
            .filter(|v| !v.dead)
            .map(|v| v.cur_hp)
        else {
            continue;
        };
        // `reduceCurrentHp(Math.min(_fallingDamage, getCurrentHp() - 1), this,
        // null, false, true, false, false)` — attacker is the faller, and
        // `directlyToHp` is **true**, so a full CP bar does not absorb a fall.
        let applied = (damage as f64).min(cur_hp - 1.0);
        crate::game_loop::combat::player_receive_damage_ex(world, oid, oid, applied, true);
        send_sm_to_player(
            world,
            oid,
            sm_ids::YOU_RECEIVED_S1_FALLING_DAMAGE,
            &[SmParam::Int(damage)],
        );
    }
}
