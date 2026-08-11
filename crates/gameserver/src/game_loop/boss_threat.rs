//! The grand bosses' top-3 threat table (`refreshAiParams` / `manageSkills`).
//!
//! Baium and Antharas do **not** use the ordinary aggro list. Each keeps three
//! slots of `(attacker, threat)` on NPC variables (`c_quest0..2` /
//! `i_quest0..2`), fed by a damage weighting that shifts as the boss is worn
//! down:
//!
//! | condition | weight |
//! |---|---|
//! | melee (`skill == null`) | `damage × 1000` |
//! | below 25% HP | `(damage / 3) × 100` |
//! | below 50% HP | `damage × 20` |
//! | below 75% HP | `damage × 10` |
//! | otherwise | `(damage / 3) × 20` |
//!
//! Melee threat is worth **fifty times** a caster's at full health, and the
//! caster weighting swings by a factor of ten across the bands — a caster who
//! is safe early becomes worth noticing as the boss weakens. That asymmetry is
//! the fight, which is why the table is ported rather than approximated onto
//! the aggro list.
//!
//! **The two scripts' tables are identical, line for line** — same weights,
//! same jitter, same 9000-unit prune, same 70% decay. They were ported
//! separately (Baium first) and the duplicate only became visible when
//! Antharas's turn came, so the machinery lives here and each boss keeps just
//! its own skill ladder.

use crate::game_loop::helpers::hp_pair;
use crate::world::World;

/// `getRandom(3000)` — the jitter added to every stored threat value.
const THREAT_JITTER: i32 = 3000;
/// The `aggro + 1000` floor an existing entry must fall below to be raised.
const THREAT_FLOOR_BONUS: i32 = 1000;
/// `calculateDistance3D(attacker) > 9000` — beyond this a threat is cleared.
const THREAT_RANGE: f64 = 9000.0;
/// After acting on the top threat, Java knocks it down to this 70% of the time.
const THREAT_DECAY_TO: i32 = 500;
const THREAT_DECAY_CHANCE: i32 = 70;

/// A boss's top-3 threat table.
///
/// Three slots, and a newcomer displaces the **weakest** — so a fourth
/// attacker only registers by out-threatening someone already on it.
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, Default)]
pub struct BossThreat {
    /// `(attacker object id, threat value)`; `0` means an empty slot.
    pub slots: [(i32, i32); 3],
}

impl BossThreat {
    /// The slot holding the least threat — Java's `getIndexOfMinValue`.
    fn weakest(&self) -> usize {
        let mut idx = 0;
        for i in 1..3 {
            if self.slots[i].1 < self.slots[idx].1 {
                idx = i;
            }
        }
        idx
    }

    /// The slot holding the most threat.
    fn strongest(&self) -> usize {
        let mut idx = 0;
        for i in 1..3 {
            if self.slots[i].1 > self.slots[idx].1 {
                idx = i;
            }
        }
        idx
    }
}

/// The shared `onAttack` weighting ladder. `is_melee` is Java's `skill == null`.
pub(crate) fn weighted_damage(
    world: &World,
    boss_oid: i32,
    damage: i32,
    is_melee: bool,
) -> Option<i32> {
    let (cur, max) = hp_pair(world, boss_oid)?;
    Some(if is_melee {
        damage * 1000
    } else if cur < max * 0.25 {
        (damage / 3) * 100
    } else if cur < max * 0.5 {
        damage * 20
    } else if cur < max * 0.75 {
        damage * 10
    } else {
        (damage / 3) * 20
    })
}

/// `refreshAiParams` — record an attacker's threat.
///
/// Two behaviours that are easy to flatten into "set the value":
///
/// - An attacker **already on the table** is only raised when its stored value
///   is below `aggro + 1000`, and is then set to `damage + rnd(3000)` — so
///   repeated small hits do not ratchet a threat upward indefinitely.
/// - An attacker **not** on the table replaces the **weakest** entry outright,
///   value and identity together.
pub(crate) fn refresh_threat(
    world: &mut World,
    boss_oid: i32,
    attacker_oid: i32,
    damage: i32,
    aggro: i32,
) {
    let new_val = damage + world.roll(THREAT_JITTER);
    let floor = aggro + THREAT_FLOOR_BONUS;

    if world
        .objects
        .get_component::<BossThreat>(&boss_oid)
        .is_none()
    {
        world
            .objects
            .add_components(&boss_oid, BossThreat::default());
    }
    let Some(t) = world.objects.get_component_mut::<BossThreat>(&boss_oid) else {
        return;
    };

    for slot in t.slots.iter_mut() {
        if slot.0 == attacker_oid {
            if slot.1 < floor {
                slot.1 = new_val;
            }
            return;
        }
    }
    let idx = t.weakest();
    t.slots[idx] = (attacker_oid, new_val);
}

/// Apply the weighting and record it — the whole `onAttack` threat half.
pub(crate) fn on_boss_damage(
    world: &mut World,
    boss_oid: i32,
    attacker_oid: i32,
    damage: i32,
    is_melee: bool,
) {
    let Some(weighted) = weighted_damage(world, boss_oid, damage, is_melee) else {
        return;
    };
    refresh_threat(world, boss_oid, attacker_oid, weighted, weighted);
}

/// Prune the table, then take the top threat — the opening of `manageSkills`.
///
/// Returns the chosen attacker, or `None` when there is nobody to act on.
pub(crate) fn take_top_threat(world: &mut World, boss_oid: i32) -> Option<i32> {
    prune_threat(world, boss_oid);

    let (target, value) = {
        let t = world.objects.get_component::<BossThreat>(&boss_oid)?;
        t.slots[t.strongest()]
    };
    if value <= 0 || target == 0 {
        return None;
    }

    // **The rotation.** 70% of the time the top threat is knocked down to 500
    // *after* being chosen, so the boss does not lock onto one player for the
    // whole fight — the next-highest gets a turn. Without it he would tunnel
    // the single biggest damage dealer indefinitely.
    if world.roll(100) < THREAT_DECAY_CHANCE
        && let Some(t) = world.objects.get_component_mut::<BossThreat>(&boss_oid)
    {
        for slot in t.slots.iter_mut() {
            if slot.0 == target {
                slot.1 = THREAT_DECAY_TO;
            }
        }
    }
    Some(target)
}

/// Clear the entry for any attacker that has died or run beyond 9000 units —
/// so a fled or dead player stops holding a slot the living could use.
fn prune_threat(world: &mut World, boss_oid: i32) {
    let Some(t) = world
        .objects
        .get_component::<BossThreat>(&boss_oid)
        .copied()
    else {
        return;
    };
    let mut cleared = [false; 3];
    for (i, (oid, _)) in t.slots.iter().enumerate() {
        if *oid == 0 {
            continue;
        }
        let dead = world
            .objects
            .get_component::<crate::model::components::Vitals>(oid)
            .is_none_or(|v| v.dead);
        let far = !crate::geo::distance::within_3d(world, boss_oid, *oid, THREAT_RANGE);
        cleared[i] = dead || far;
    }
    if let Some(t) = world.objects.get_component_mut::<BossThreat>(&boss_oid) {
        for (i, c) in cleared.iter().enumerate() {
            if *c {
                t.slots[i].1 = 0;
            }
        }
    }
}

/// Fire the chosen skill. `on_self` is Java's `castOnTarget == false`, which
/// casts with the **boss itself** as the target — the AoE skills are centred on
/// him, not on the player who drew them.
pub(crate) fn cast_boss_skill(
    world: &mut World,
    boss_oid: i32,
    target_oid: i32,
    skill_id: i32,
    on_self: bool,
) -> bool {
    let target = if on_self { boss_oid } else { target_oid };
    crate::game_loop::npc::cast::cast_skill(world, boss_oid, target, skill_id, 1)
}
