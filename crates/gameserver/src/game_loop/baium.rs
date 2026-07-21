//! Baium (`ai/bosses/Baium`) — archangels and the strider debuff.
//!
//! Baium is the only one of the three great dragons' scripts with **no
//! cinematics at all** (`SpecialCamera` appears 19 times in Valakas and 7 in
//! Antharas, and zero here), which is why it is portable before the camera
//! packet is.
//!
//! # The threat table
//!
//! Baium does **not** use the ordinary aggro list. He keeps a **top-3 threat
//! table** (`c_quest0..2` / `i_quest0..2` on NPC variables), fed by a damage
//! weighting that shifts as he is worn down:
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
//! is safe early becomes worth noticing as Baium weakens. That asymmetry is the
//! fight, which is why the table is ported rather than approximated onto the
//! aggro list.

use crate::world::World;

pub const BAIUM: i32 = 29020;
/// Archangel — five of them circle Baium.
pub const ARCHANGEL: i32 = 29021;

/// `ANTI_STRIDER` (4258, "Hinder Strider").
const ANTI_STRIDER: i32 = 4258;
/// Java `MountType.STRIDER`.
const MOUNT_STRIDER: u8 = 1;

/// `getRandom(3000)` — the jitter added to every stored threat value.
const THREAT_JITTER: i32 = 3000;
/// The `aggro + 1000` floor an existing entry must fall below to be raised.
const THREAT_FLOOR_BONUS: i32 = 1000;

/// Baium's top-3 threat table (Java's `c_quest0..2` / `i_quest0..2`).
///
/// Three slots, and a newcomer displaces the **weakest** — so a fourth
/// attacker only registers by out-threatening someone already on it.
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, Default)]
pub struct BaiumThreat {
    /// `(attacker object id, threat value)`; `0` means an empty slot.
    pub slots: [(i32, i32); 3],
}

impl BaiumThreat {
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
}

/// `ARCHANGEL_LOC` — five fixed points, with headings.
const ARCHANGEL_LOC: [(i32, i32, i32, i32); 5] = [
    (115_792, 16_608, 10_136, 0),
    (115_168, 17_200, 10_136, 0),
    (115_780, 15_564, 10_136, 13_620),
    (114_880, 16_236, 10_136, 5_400),
    (114_239, 17_168, 10_136, -1_992),
];

/// Baium spawned: bring out his five archangels.
///
/// Unlike Queen Ant's nurses these are **not** in a minion table — the script
/// places them, so nothing else would.
pub(crate) fn on_baium_spawned(world: &mut World) {
    for (x, y, z, heading) in ARCHANGEL_LOC {
        crate::model::npc::spawn_npc_at(world, ARCHANGEL, x, y, z, heading);
    }
}

/// `Baium.onAttack`'s strider clause: a strider-mounted attacker is hindered,
/// **once** — Java checks both `!isAffectedBySkill(4258)` and that the skill
/// is off cooldown, so it is not recast every swing.
/// `refreshAiParams` — record an attacker's threat.
///
/// Two behaviours that are easy to flatten into "set the value":
///
/// - An attacker **already on the table** is only raised when its stored value
///   is below `aggro + 1000`, and is then set to `damage + rnd(3000)` — so
///   repeated small hits do not ratchet a threat upward indefinitely.
/// - An attacker **not** on the table replaces the **weakest** entry outright,
///   value and identity together.
pub(crate) fn refresh_threat(world: &mut World, baium_oid: i32, attacker_oid: i32, damage: i32, aggro: i32) {
    let new_val = damage + world.roll(THREAT_JITTER);
    let floor = aggro + THREAT_FLOOR_BONUS;

    if world.objects.get_component::<BaiumThreat>(&baium_oid).is_none() {
        world.objects.add_components(&baium_oid, BaiumThreat::default());
    }
    let Some(t) = world.objects.get_component_mut::<BaiumThreat>(&baium_oid) else { return };

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

/// `Baium.onAttack`'s weighting ladder. `skill_damage` is `None` for a melee
/// hit (Java's `skill == null`), which is worth vastly more threat.
pub(crate) fn on_baium_damage(world: &mut World, baium_oid: i32, attacker_oid: i32, damage: i32, is_melee: bool) {
    let (cur, max) = match world.objects.get_component::<crate::model::components::Vitals>(&baium_oid) {
        Some(v) => (v.cur_hp, v.max_hp as f64),
        None => return,
    };
    let weighted = if is_melee {
        damage * 1000
    } else if cur < max * 0.25 {
        (damage / 3) * 100
    } else if cur < max * 0.5 {
        damage * 20
    } else if cur < max * 0.75 {
        damage * 10
    } else {
        (damage / 3) * 20
    };
    refresh_threat(world, baium_oid, attacker_oid, weighted, weighted);
}

pub(crate) fn on_baium_attacked(world: &mut World, baium_oid: i32, attacker_oid: i32) {
    let on_strider = world
        .objects
        .get_component::<crate::model::Player>(&attacker_oid)
        .is_some_and(|p| p.mount_type == MOUNT_STRIDER);
    if !on_strider {
        return;
    }
    let already = world
        .objects
        .get_component::<crate::model::components::Buffs>(&attacker_oid)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == ANTI_STRIDER));
    if already {
        return;
    }
    let Some(skill) = world.data.skill_data.get(ANTI_STRIDER, 1).cloned() else { return };
    if !crate::game_loop::npc_cast::check_use_conditions_pub(world, baium_oid, &skill) {
        return;
    }
    crate::game_loop::npc_cast::start_cast(world, baium_oid, attacker_oid, &skill);
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

/// `calculateDistance3D(attacker) > 9000` — beyond this a threat is cleared.
const THREAT_RANGE: f64 = 9000.0;
/// After acting on the top threat, Java knocks it down to this 70% of the time.
const THREAT_DECAY_TO: i32 = 500;
const THREAT_DECAY_CHANCE: i32 = 70;

/// `manageSkills` — prune the table, pick the top threat, decay it, and choose
/// a skill for Baium's current health.
///
/// Returns `(target, skill_id)`, or `None` when there is nobody to act on.
pub(crate) fn manage_skills(world: &mut World, baium_oid: i32) -> Option<(i32, i32)> {
    prune_threat(world, baium_oid);

    let (target, value) = {
        let t = world.objects.get_component::<BaiumThreat>(&baium_oid)?;
        let mut best = 0usize;
        for i in 1..3 {
            if t.slots[i].1 > t.slots[best].1 {
                best = i;
            }
        }
        t.slots[best]
    };
    if value <= 0 || target == 0 {
        return None;
    }

    // **The rotation.** 70% of the time the top threat is knocked down to 500
    // *after* being chosen, so Baium does not lock onto one player for the whole
    // fight — the next-highest gets a turn. Without it he would tunnel the
    // single biggest damage dealer indefinitely.
    if world.roll(100) < THREAT_DECAY_CHANCE {
        if let Some(t) = world.objects.get_component_mut::<BaiumThreat>(&baium_oid) {
            for slot in t.slots.iter_mut() {
                if slot.0 == target {
                    slot.1 = THREAT_DECAY_TO;
                }
            }
        }
    }

    let skill = choose_skill(world, baium_oid);
    Some((target, skill))
}

/// Clear the entry for any attacker that has died or run beyond 9000 units —
/// so a fled or dead player stops holding a slot the living could use.
fn prune_threat(world: &mut World, baium_oid: i32) {
    let Some(t) = world.objects.get_component::<BaiumThreat>(&baium_oid).copied() else { return };
    let boss_pos = world.objects.get_component::<crate::model::components::Position>(&baium_oid).copied();
    let mut cleared = [false; 3];
    for (i, (oid, _)) in t.slots.iter().enumerate() {
        if *oid == 0 {
            continue;
        }
        let dead = world
            .objects
            .get_component::<crate::model::components::Vitals>(oid)
            .is_none_or(|v| v.dead);
        let far = match (boss_pos, world.objects.get_component::<crate::model::components::Position>(oid)) {
            (Some(a), Some(b)) => {
                let (dx, dy, dz) = ((a.x - b.x) as f64, (a.y - b.y) as f64, (a.z - b.z) as f64);
                (dx * dx + dy * dy + dz * dz).sqrt() > THREAT_RANGE
            }
            _ => true,
        };
        cleared[i] = dead || far;
    }
    if let Some(t) = world.objects.get_component_mut::<BaiumThreat>(&baium_oid) {
        for (i, c) in cleared.iter().enumerate() {
            if *c {
                t.slots[i].1 = 0;
            }
        }
    }
}

/// The skill ladder. Each 10% roll is taken **in order**, and the pool widens
/// as Baium weakens: two options above 75%, three above 50%, four above 25%,
/// and all four below — with `BAIUM_ATTACK` as the fallback throughout.
///
/// So a party sees Baium's repertoire grow as the fight goes on, which is the
/// same shape as his threat weighting.
fn choose_skill(world: &mut World, baium_oid: i32) -> i32 {
    let (cur, max) = match world.objects.get_component::<crate::model::components::Vitals>(&baium_oid) {
        Some(v) => (v.cur_hp, v.max_hp as f64),
        None => return BAIUM_ATTACK,
    };
    // Java's ladders share a tail, so the pool is expressed as the list of
    // skills tried before falling back — top band first.
    let pool: &[i32] = if cur > max * 0.75 {
        &[ENERGY_WAVE, EARTH_QUAKE]
    } else if cur > max * 0.5 {
        &[GROUP_HOLD, ENERGY_WAVE, EARTH_QUAKE]
    } else if cur > max * 0.25 {
        &[THUNDERBOLT, GROUP_HOLD, ENERGY_WAVE, EARTH_QUAKE]
    } else {
        &[THUNDERBOLT, GROUP_HOLD, ENERGY_WAVE, EARTH_QUAKE]
    };
    for skill in pool {
        if world.roll(100) < 10 {
            return *skill;
        }
    }
    BAIUM_ATTACK
}
