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

use commons::util::rnd;

use crate::data::zone_data::ZoneKind;
use crate::model::components::{Position, Vitals};
use crate::world::World;

/// How often the sweep runs (10 × 100 ms). Individual zones still fire at their
/// own `reuse`; this is just the resolution the timers are checked at, and it
/// is finer than the shortest `reuse` in the datapack (6000 ms).
pub(crate) const SWEEP_PERIOD: u64 = 10;

/// One pass: every player in an effect zone whose timer is due takes that
/// zone's skills.
pub(crate) fn effect_zone_tick(world: &mut World) {
    // Group living players by the effect zones they're standing in.
    let mut occupants: Vec<(usize, Vec<i32>)> = Vec::new();
    {
        let mut pairs: Vec<(usize, i32)> = Vec::new();
        let crate::world::World { objects, data, .. } = &mut *world;
        objects.for_each_mut::<(&crate::model::Player, &Position, &Vitals)>(|(p, pos, v)| {
            if v.dead {
                return;
            }
            for idx in data.zone_data.zone_indices_at(pos.x, pos.y, pos.z) {
                if data.zone_data.zones[idx].kind == ZoneKind::Effect {
                    pairs.push((idx, p.object_id));
                }
            }
        });
        pairs.sort_unstable();
        for (idx, oid) in pairs {
            match occupants.last_mut() {
                Some((i, list)) if *i == idx => list.push(oid),
                _ => occupants.push((idx, vec![oid])),
            }
        }
    }

    for (idx, players) in occupants {
        let Some(params) = world.data.zone_data.zones[idx].effect.clone() else {
            continue;
        };
        // `ApplySkill.run`: a disabled zone does nothing. `casts_on_players`
        // folds in Java's `targetClass`/`isPlayer()` pair — 27 zones target
        // NPCs and therefore reach nobody.
        if !params.enabled || !params.casts_on_players || params.skills.is_empty() {
            continue;
        }

        // Per-zone cadence. First sight schedules `initialDelay` out (Java's
        // `scheduleAtFixedRate(task, initialDelay, reuse)`) rather than firing
        // immediately.
        let now = world.tick;
        match world.effect_zone_next_tick.get(&idx) {
            Some(&due) if due > now => continue,
            Some(_) => {
                world
                    .effect_zone_next_tick
                    .insert(idx, now + ms_to_ticks(params.reuse));
            }
            None => {
                world.effect_zone_next_tick.insert(
                    idx,
                    now + ms_to_ticks(params.initial_delay.max(params.reuse)),
                );
                continue;
            }
        }

        for oid in players {
            // Java rolls the chance once per creature, then applies every
            // skill — not once per skill.
            if rnd::get(100) >= params.chance {
                continue;
            }
            for &(skill_id, skill_level) in &params.skills {
                let Some(skill) = world.data.skill_data.get(skill_id, skill_level).cloned() else {
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
                super::skills::effects::apply_skill_effects(world, oid, oid, &skill);
            }
        }
    }
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

fn ms_to_ticks(ms: i32) -> u64 {
    (ms.max(0) as u64 / 100).max(1)
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
    let mut occupants: Vec<(usize, Vec<i32>)> = Vec::new();
    {
        let mut pairs: Vec<(usize, i32)> = Vec::new();
        let crate::world::World { objects, data, .. } = &mut *world;
        objects.for_each_mut::<(&crate::model::Player, &Position, &Vitals)>(|(p, pos, v)| {
            if v.dead {
                return;
            }
            for idx in data.zone_data.zone_indices_at(pos.x, pos.y, pos.z) {
                if data.zone_data.zones[idx].kind == ZoneKind::Damage {
                    pairs.push((idx, p.object_id));
                }
            }
        });
        pairs.sort_unstable();
        for (idx, oid) in pairs {
            match occupants.last_mut() {
                Some((i, list)) if *i == idx => list.push(oid),
                _ => occupants.push((idx, vec![oid])),
            }
        }
    }

    for (idx, players) in occupants {
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

        let now = world.tick;
        match world.effect_zone_next_tick.get(&idx) {
            Some(&due) if due > now => continue,
            Some(_) => {
                world
                    .effect_zone_next_tick
                    .insert(idx, now + ms_to_ticks(params.reuse));
            }
            None => {
                world.effect_zone_next_tick.insert(
                    idx,
                    now + ms_to_ticks(params.initial_delay.max(params.reuse)),
                );
                continue;
            }
        }

        for oid in players {
            // During a siege the castle's defenders are immune to its traps.
            if siege && super::siege::is_siege_defender(world, params.castle_id, oid) {
                continue;
            }
            if params.hp_per_tick > 0 {
                // Java's `DamageZone` calls the plain 3-arg `reduceCurrentHp`
                // (`isDOT` defaults `false`) — a damage zone *is* blocked by
                // `HP_BLOCK`, unlike an abnormal-effect DoT tick.
                super::combat::apply_physical_damage(
                    world,
                    oid,
                    oid,
                    params.hp_per_tick as f64,
                    false,
                );
            }
            if params.mp_per_tick > 0
                && let Some(v) = world.objects.get_component_mut::<Vitals>(&oid)
            {
                v.cur_mp = (v.cur_mp - params.mp_per_tick as f64).max(0.0);
            }
        }
    }
}

/// The `SwampZone` multiplier at a point — `SpeedFinalizer`'s
/// `baseValue *= zone.getMoveBonus()` for a playable inside one. `1.0` when
/// not in a swamp. Castle-trap swamps only bite during their siege.
pub(crate) fn swamp_multiplier_at(world: &World, oid: i32) -> f64 {
    let Some(pos) = world.objects.get_component::<Position>(&oid).copied() else {
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
            if super::siege::is_siege_defender(world, p.castle_id, oid) {
                continue;
            }
        }
        return p.move_bonus;
    }
    1.0
}
