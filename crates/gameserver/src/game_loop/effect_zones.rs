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

use rand::Rng;

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
        let Some(params) = world.data.zone_data.zones[idx].effect.clone() else { continue };
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
                world.effect_zone_next_tick.insert(idx, now + ms_to_ticks(params.reuse));
            }
            None => {
                world.effect_zone_next_tick.insert(idx, now + ms_to_ticks(params.initial_delay.max(params.reuse)));
                continue;
            }
        }

        for oid in players {
            // Java rolls the chance once per creature, then applies every
            // skill — not once per skill.
            if world.rng.gen_range(0..100) >= params.chance {
                continue;
            }
            for &(skill_id, skill_level) in &params.skills {
                let Some(skill) = world.data.skill_data.get(skill_id, skill_level).cloned() else { continue };
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
        .is_some_and(|b| b.0.iter().any(|e| e.skill_id == skill_id && e.skill_level >= level))
}

fn ms_to_ticks(ms: i32) -> u64 {
    (ms.max(0) as u64 / 100).max(1)
}
