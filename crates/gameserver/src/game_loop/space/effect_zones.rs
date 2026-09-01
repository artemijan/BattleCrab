//! `EffectZone` — port of `model/zone/type/EffectZone` and its `ApplySkill`
//! task. The Blazing Swamp burns you, the Sea of Spores poisons you, the Hot
//! Springs hand out Haste/Focus/Might.
//!
//! 218 of these exist on this dist; 204 declare a skill list. They were parsed
//! by nobody before — the zone loader only read `castleId`, and the files
//! carrying effect zones weren't loaded at all.
//!
//! **Shape difference from Java, deliberate.** Java starts a per-zone
//! `scheduleAtFixedRate` task the moment someone enters and cancels it when the
//! zone empties, which needs a live "characters inside" set per zone. The port
//! has no such set, so instead one global sweep runs every second, groups the
//! players by the zones they're standing in, and fires each zone whose own
//! reuse interval has elapsed. Same observable behaviour — per-zone cadence,
//! per-creature chance roll — without the enter/exit bookkeeping. A zone with
//! nobody in it costs one hash lookup and never advances its timer.

use crate::game_loop::skills::skill_by_id;
use crate::game_loop::space::position::maybe_position;
use commons::util::rnd;

use crate::data::zone_data::ZoneKind;
use crate::game_loop::helpers::spend_mp;
use crate::game_loop::helpers::stat_add;
use crate::model::components::{Position, Vitals, ZoneFlags};
use crate::scheduler::ms_to_ticks;
use crate::world::World;

/// How often the sweep runs (10 × 100 ms). Individual zones still fire at their
/// own `reuse`; this is just the resolution the timers are checked at, and it
/// is finer than the shortest `reuse` in the datapack (6000 ms).
pub(crate) const SWEEP_PERIOD: u64 = 10;

/// One pass: every player in an effect zone whose timer is due takes that
/// zone's skills.
pub(crate) fn effect_zone_tick(world: &mut World) {
    // Group living players by the effect zones they're standing in.
    let occupants = zone_occupants(world, ZoneKind::Effect);

    for (idx, players) in occupants {
        // Cheapest test first: an occupied zone whose timer hasn't elapsed
        // costs one hash probe — the params clone below only happens on the
        // ticks a zone actually fires (or is seen for the first time).
        let now = world.tick;
        if let Some(&due) = world.effect_zone_next_tick.get(&idx)
            && due > now
        {
            continue;
        }
        let Some(params) = world.data.zone_data.zones[idx].effect.clone() else {
            continue;
        };
        // `ApplySkill.run`: a disabled zone does nothing. `casts_on_players`
        // folds in Java's `targetClass`/`isPlayer()` pair — 27 zones target
        // NPCs and therefore reach nobody.
        if !params.enabled || !params.casts_on_players || params.skills.is_empty() {
            continue;
        }

        if !arm_timer(world, idx, now, params.initial_delay, params.reuse) {
            continue;
        }

        for oid in players {
            // Java rolls the chance once per creature, then applies every
            // skill — not once per skill.
            if rnd::get(100) >= params.chance {
                continue;
            }
            for &(skill_id, skill_level) in &params.skills {
                let Some(skill) = skill_by_id(world, skill_id, skill_level) else {
                    continue;
                };
                // `getAffectedSkillLevel(id) < skill.getLevel()` — don't
                // refresh a buff the player already has at this level or
                // better. Without this the Hot Springs trio would re-cast every
                // 6 s forever.
                if already_affected_at_least(world, oid, skill_id, skill_level) {
                    continue;
                }
                // The zone casts on the player *as* the player (Java
                // `skill.activateSkill(character, character)`), so damage and
                // buffs both resolve against them with no external caster.
                crate::game_loop::skills::effects::apply_skill_effects(world, oid, oid, &skill);
            }
        }
    }
}

/// Living players grouped by the `kind` zones they stand in. The cached
/// `ZoneFlags` mask (maintained by `zones::revalidate_zone`) rejects the
/// vast majority of players with one bit test before the per-zone grid walk
/// — only players actually inside a zone of this kind pay `zone_indices_at`.
fn zone_occupants(world: &mut World, kind: ZoneKind) -> Vec<(usize, Vec<i32>)> {
    let mut occupants: Vec<(usize, Vec<i32>)> = Vec::new();
    let mut pairs: Vec<(usize, i32)> = Vec::new();
    let World { objects, data, .. } = &mut *world;
    objects.for_each_mut::<(&crate::model::Player, &Position, &Vitals, &ZoneFlags)>(
        |(p, pos, v, flags)| {
            if v.dead || flags.mask & kind.bit() == 0 {
                return;
            }
            for idx in data.zone_data.zone_indices_at(pos.x, pos.y, pos.z) {
                if data.zone_data.zones[idx].kind == kind {
                    pairs.push((idx, p.object_id));
                }
            }
        },
    );
    pairs.sort_unstable();
    for (idx, oid) in pairs {
        match occupants.last_mut() {
            Some((i, list)) if *i == idx => list.push(oid),
            _ => occupants.push((idx, vec![oid])),
        }
    }
    occupants
}

/// `Creature.getAffectedSkillLevel(skillId) >= level`.
fn already_affected_at_least(world: &World, oid: i32, skill_id: i32, level: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Buffs>(&oid)
        .is_some_and(|b| {
            b.0.iter()
                .any(|e| e.skill_id == skill_id && e.skill_level >= level)
        })
}

/// Per-zone cadence, shared by the effect and damage sweeps. Schedules the
/// zone's next fire and reports whether it may fire *now*: first sight schedules
/// `initialDelay` out (Java's `scheduleAtFixedRate(task, initialDelay, reuse)`)
/// and returns `false` rather than firing immediately.
///
/// The shared `ms_to_ticks` rounds **up** where this used to truncate, and it
/// returns 0 for 0 where this floored at one tick. Neither difference reaches
/// this dist: every `initialDelay`/`reuse` in `data/zones` is a multiple of
/// 100 ms and none is below 1000, so the two agree on every value shipped. The
/// `.max(1)` stays as the guard it always was — a zone declaring `reuse="0"`
/// would otherwise re-arm on the same tick forever.
fn arm_timer(world: &mut World, idx: usize, now: u64, initial_delay: i32, reuse: i32) -> bool {
    use std::collections::hash_map::Entry;
    match world.effect_zone_next_tick.entry(idx) {
        Entry::Occupied(mut e) => {
            e.insert(now + ms_to_ticks(reuse).max(1));
            true
        }
        Entry::Vacant(e) => {
            e.insert(now + ms_to_ticks(initial_delay.max(reuse)).max(1));
            false
        }
    }
}

/// `DamageZone.ApplyDamage` — flat HP/MP loss for players standing in lava,
/// acid or a castle trap. Same sweep shape as the effect tick above (one pass,
/// grouped by zone, each zone on its own `reuse`).
///
/// **Castle traps are siege-only and spare the defenders.** A zone with a
/// `castleId` does nothing unless that castle's siege is in progress, and even
/// then skips anyone defending it — otherwise the castle's own garrison would
/// cook itself on its own traps.
pub(crate) fn damage_zone_tick(world: &mut World) {
    let occupants = zone_occupants(world, ZoneKind::Damage);

    for (idx, players) in occupants {
        // Same order as the effect tick: hash probe before params clone.
        let now = world.tick;
        if let Some(&due) = world.effect_zone_next_tick.get(&idx)
            && due > now
        {
            continue;
        }
        let Some(params) = world.data.zone_data.zones[idx].damage.clone() else {
            continue;
        };
        if !params.enabled {
            continue;
        }
        // Castle traps: only during that castle's siege.
        let siege = params.castle_id > 0;
        if siege && !world.sieges.contains_key(&params.castle_id) {
            continue;
        }

        if !arm_timer(world, idx, now, params.initial_delay, params.reuse) {
            continue;
        }

        for oid in players {
            // During a siege the castle's defenders are immune to its traps.
            if siege && crate::game_loop::siege::is_siege_defender(world, params.castle_id, oid) {
                continue;
            }
            if params.hp_per_tick > 0 {
                // Java's `DamageZone` calls the plain 3-arg `reduceCurrentHp`
                // (`isDOT` defaults `false`) — a damage zone *is* blocked by
                // `HP_BLOCK`, unlike an abnormal-effect DoT tick.
                // `DamageZone`: `multiplier = 1 + (DAMAGE_ZONE_VULN / 100)`.
                // Iron Body (295) and Dance of Protection (311) grant it
                // *negative* (−40 / −30), so despite the stat's name the
                // learnable sources are mitigation (G34 S4).
                let vuln =
                    1.0 + stat_add(world, oid, crate::model::stats::Stat::DamageZoneVuln) / 100.0;
                crate::game_loop::combat::apply_physical_damage(
                    world,
                    oid,
                    oid,
                    params.hp_per_tick as f64 * vuln,
                    false,
                    // Java's zone damage passes a null skill.
                    false,
                );
            }
            if params.mp_per_tick > 0 {
                // Java applies the same multiplier to the MP tick.
                let vuln =
                    1.0 + stat_add(world, oid, crate::model::stats::Stat::DamageZoneVuln) / 100.0;
                spend_mp(world, oid, params.mp_per_tick as f64 * vuln);
            }
        }
    }
}

/// The `SwampZone` multiplier at a point — `SpeedFinalizer`'s
/// `baseValue *= zone.getMoveBonus()` for a playable inside one. `1.0` when
/// not in a swamp. Castle-trap swamps only bite during their siege.
pub(crate) fn swamp_multiplier_at(world: &World, oid: i32) -> f64 {
    let Some(pos) = maybe_position(world, oid) else {
        return 1.0;
    };
    for idx in world.data.zone_data.zone_indices_at(pos.x, pos.y, pos.z) {
        let zone = &world.data.zone_data.zones[idx];
        if zone.kind != ZoneKind::Swamp {
            continue;
        }
        let Some(p) = zone.swamp.as_ref() else {
            continue;
        };
        if !p.enabled {
            continue;
        }
        if p.castle_id > 0 {
            if !world.sieges.contains_key(&p.castle_id) {
                continue;
            }
            if crate::game_loop::siege::is_siege_defender(world, p.castle_id, oid) {
                continue;
            }
        }
        return p.move_bonus;
    }
    1.0
}
