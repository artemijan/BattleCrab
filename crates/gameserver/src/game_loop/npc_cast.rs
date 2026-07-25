//! NPC skill casting — the "Cast skills" block of `AttackableAI.thinkAttack`
//! plus an NPC-side `SkillCaster.startCasting`.
//!
//! Until this slice a monster could only swing: 4831 of this dist's NPCs carry
//! at least one castable skill and none of them ever used it. The AI walks the
//! [`AiSkillScope`] buckets built at load
//! ([`crate::data::npc_ai_skills`]) in Java's priority order — heal, buff,
//! immobilize a fleeing target, mute a casting one, then short/long range, then
//! anything — casting the first skill that passes its conditions.
//!
//! **Deliberate narrowings** (each a `TODO(G21)` at the site):
//! - `skillTargetReconsider` — Java re-picks the heal/buff target across the
//!   caster's whole faction (and re-picks *any* target when the current one is
//!   unreachable). With no faction/clan-help plumbing yet, heal and buff
//!   resolve to the caster itself, which is what a solo mob would pick anyway.
//! - `AIType.ARCHER`'s kite move and the raid target-chaos shuffle.
//! - `SUICIDE`/`RES`/`NEGATIVE` buckets are filled but unused (no skill in this
//!   dist declares `isSuicideAttack`, and no resurrect effect is ported).

use rand::Rng;

use crate::data::npc_ai_skills::AiSkillScope;
use crate::data::npc_data::AiType;
use crate::model::components::{Casting, Position, RegionCell, Vitals};
use crate::model::npc::AggroList;
use crate::model::skill::Skill;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::helpers::ms_to_ticks;
use super::helpers::{broadcast_near_region_in, instance_of};
use super::skills::cast::set_skill_reuse;

/// Java's literal cut between the SHORT_RANGE and LONG_RANGE buckets.
const SHORT_RANGE: f64 = 150.0;

/// The cast block of `thinkAttack`. Returns `true` if a cast started, in which
/// case the caller skips the move/swing tail for this think.
pub(crate) fn try_cast(world: &mut World, npc_oid: i32, target_oid: i32) -> bool {
    // Already casting — one cast at a time.
    if world.objects.has_component::<Casting>(&npc_oid) {
        return false;
    }

    let Some(npc_id) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .map(|n| n.npc_id)
    else {
        return false;
    };
    let Some(template) = world.data.npc_data.get(npc_id) else {
        return false;
    };
    let (ai_type, min_chance, max_chance) = (
        template.ai_type,
        template.min_skill_chance,
        template.max_skill_chance,
    );

    // `(!npc.isMoving() && npc.hasSkillChance()) || (npc.getAiType() == AIType.MAGE)`
    // — a mage casts on every think, regardless of movement or the roll. 402
    // NPCs on this dist are MAGE.
    let moving = world
        .objects
        .has_component::<crate::model::components::Movement>(&npc_oid);
    let mage = ai_type == AiType::Mage;
    if !mage && (moving || !has_skill_chance(min_chance, max_chance)) {
        return false;
    }

    let Some(ai_skills) = world.data.npc_ai_skills.get(npc_id).cloned() else {
        return false;
    };

    // --- Java's priority ladder, in order. ---

    // 1. Heal — the most important skill, and Java reconsiders the target for
    //    it: the pack's healer looks for whoever is worst off, not just itself.
    //    The chance scales off *that* target's HP so it's certain below 33 %:
    //    `(100 - hpPercent) * 1.5`.
    if let Some(skill) = pick(world, &ai_skills, AiSkillScope::Heal, npc_oid) {
        if let Some(heal_target) = skill_target_reconsider(world, npc_oid, &skill, false) {
            let hp_pct = hp_percent(world, heal_target);
            let heal_chance = (100.0 - hp_pct) * 1.5;
            if (rand::thread_rng().gen_range(0..100) as f64) < heal_chance
                && check_skill_target(world, npc_oid, heal_target, &skill)
            {
                start_cast(world, npc_oid, heal_target, &skill);
                return true;
            }
        }
    }

    // 2. Buff — same reconsider, so a support mob buffs its pack rather than
    //    only itself. Java passes `insideCastRange = true` here.
    if let Some(skill) = pick(world, &ai_skills, AiSkillScope::Buff, npc_oid) {
        if let Some(buff_target) = skill_target_reconsider(world, npc_oid, &skill, true) {
            if check_skill_target(world, npc_oid, buff_target, &skill) {
                start_cast(world, npc_oid, buff_target, &skill);
                return true;
            }
        }
    }

    // 3. Immobilize a target that's running (kiting or fleeing).
    let target_moving = world
        .objects
        .has_component::<crate::model::components::Movement>(&target_oid);
    if target_moving {
        if let Some(skill) = pick(world, &ai_skills, AiSkillScope::Immobilize, npc_oid) {
            if check_skill_target(world, npc_oid, target_oid, &skill) {
                start_cast(world, npc_oid, target_oid, &skill);
                return true;
            }
        }
    }

    // 4. Mute a target that's mid-cast (Java's COT bucket).
    if world.objects.has_component::<Casting>(&target_oid) {
        if let Some(skill) = pick(world, &ai_skills, AiSkillScope::Cot, npc_oid) {
            if check_skill_target(world, npc_oid, target_oid, &skill) {
                start_cast(world, npc_oid, target_oid, &skill);
                return true;
            }
        }
    }

    // 5/6. Range-matched attack skills.
    let dist = distance_2d(world, npc_oid, target_oid).unwrap_or(f64::MAX);
    if dist <= SHORT_RANGE {
        if let Some(skill) = pick(world, &ai_skills, AiSkillScope::ShortRange, npc_oid) {
            if check_skill_target(world, npc_oid, target_oid, &skill) {
                start_cast(world, npc_oid, target_oid, &skill);
                return true;
            }
        }
    }
    if let Some(skill) = pick(world, &ai_skills, AiSkillScope::LongRange, npc_oid) {
        if check_skill_target(world, npc_oid, target_oid, &skill) {
            start_cast(world, npc_oid, target_oid, &skill);
            return true;
        }
    }

    // 7. Anything at all.
    if let Some(skill) = pick(world, &ai_skills, AiSkillScope::General, npc_oid) {
        if check_skill_target(world, npc_oid, target_oid, &skill) {
            start_cast(world, npc_oid, target_oid, &skill);
            return true;
        }
    }

    false
}

/// `Npc.hasSkillChance()` — `Rnd.get(100) < Rnd.get(min, max)`.
fn has_skill_chance(min: i32, max: i32) -> bool {
    let mut rng = rand::thread_rng();
    let ceiling = if max > min {
        rng.gen_range(min..=max)
    } else {
        min
    };
    rng.gen_range(0..100) < ceiling
}

/// Pick a random skill from a bucket (Java `Rnd.get(list.size())`) and resolve
/// it, dropping any that fail `SkillCaster.checkUseConditions` (dead, already
/// casting, on reuse, not enough MP).
fn pick(
    world: &World,
    ai_skills: &crate::data::npc_ai_skills::NpcAiSkills,
    scope: AiSkillScope,
    npc_oid: i32,
) -> Option<Skill> {
    let bucket = ai_skills.get(scope);
    if bucket.is_empty() {
        return None;
    }
    let (id, level) = bucket[rand::thread_rng().gen_range(0..bucket.len())];
    let skill = world.data.skill_data.get(id, level)?;
    check_use_conditions(world, npc_oid, skill).then(|| skill.clone())
}

/// The use-condition gate (MP, mutes, cooldown), shared with the owner-ordered
/// [`ServitorSkillUse`] path so a commanded skill obeys the same rules as an
/// AI-chosen one.
///
/// [`ServitorSkillUse`]: crate::game_loop::servitor::use_servitor_skill
pub(crate) fn check_use_conditions_pub(world: &World, npc_oid: i32, skill: &Skill) -> bool {
    check_use_conditions(world, npc_oid, skill)
}

/// Test hook.
#[cfg(test)]
pub(crate) fn check_use_conditions_for_test(world: &World, npc_oid: i32, skill: &Skill) -> bool {
    check_use_conditions(world, npc_oid, skill)
}

/// `SkillCaster.checkUseConditions`, narrowed to what an NPC can trip: MP and
/// the reuse timer. (No shield/weapon conditions — those are item-gated player
/// checks — and no `isSkillDisabled` silence handling beyond the abnormal
/// flags below.)
fn check_use_conditions(world: &World, npc_oid: i32, skill: &Skill) -> bool {
    let Some(vitals) = world.objects.get_component::<Vitals>(&npc_oid) else {
        return false;
    };
    if vitals.dead || vitals.cur_mp < skill.mp_consume as f64 {
        return false;
    }
    // A muted NPC can't cast — the same abnormal gate players go through.
    if skill.magic_type == 1 {
        if super::abnormal::is_muted(world, npc_oid) {
            return false;
        }
    } else if super::abnormal::is_physical_muted(world, npc_oid) {
        return false;
    }
    if super::abnormal::is_blocked_from_actions(world, npc_oid) {
        return false;
    }
    if let Some(reuses) = world
        .objects
        .get_component::<crate::model::components::Reuses>(&npc_oid)
    {
        if let Some(r) = reuses.0.get(&skill.reuse_key()) {
            if r.until_tick > world.tick {
                return false;
            }
        }
    }
    true
}

/// `AttackableAI.checkSkillTarget` — the target-side conditions.
fn check_skill_target(world: &World, npc_oid: i32, target_oid: i32, skill: &Skill) -> bool {
    let Some(vitals) = world.objects.get_component::<Vitals>(&target_oid) else {
        return false;
    };
    if vitals.dead {
        return false;
    }

    // Cast range (Java `Util.checkIfInRange(castRange, …, true)` — collision
    // radii included; `combatant` carries them).
    if skill.cast_range > 0 {
        let reach = skill.cast_range as f64
            + collision_radius(world, npc_oid)
            + collision_radius(world, target_oid);
        if distance_2d(world, npc_oid, target_oid).is_none_or(|d| d > reach) {
            return false;
        }
    }

    if skill.is_continuous {
        // Don't re-apply an abnormal the target already has at >= this level.
        if has_abnormal_at_least(
            world,
            target_oid,
            &skill.abnormal_type,
            skill.abnormal_level,
        ) {
            return false;
        }
        // Java: "there are cases where bad skills (negative effect points) are
        // actually buffs and NPCs cast them on players, but they shouldn't."
        // The test is `target.isAutoAttackable(caster)` — refuse a *good*
        // continuous skill on an **enemy**. This read `target_oid != npc_oid`
        // while heal/buff were self-only (slice 1), which silently blocked
        // buffing a faction-mate once reconsider landed.
        if !(skill.is_debuff || skill.is_bad()) && is_auto_attackable_by_npc(world, target_oid) {
            return false;
        }
    }

    // A heal on an undamaged target is wasted.
    let heals = skill
        .effects
        .iter()
        .any(|e| matches!(e, crate::model::skill::SkillEffect::Heal { .. }));
    if heals && vitals.cur_hp >= vitals.max_hp as f64 {
        return false;
    }

    true
}

/// NPC-side `SkillCaster.startCasting`. Much shorter than the player path:
/// no shots, no queued action, no cast bar or `YOU_USE_S1` (nobody to send
/// them to), no MP-initial-consume message — just face, broadcast, schedule.
pub(crate) fn start_cast(world: &mut World, npc_oid: i32, target_oid: i32, skill: &Skill) {
    // Java `Summon.doCast` → `rechargeShots(false, true, false)`: a summon
    // charges its magic shot before casting, the mirror of the soulshot charge
    // the attack loop does before swinging. No-op for a plain monster.
    if skill.magic_type == 1 {
        super::servitor::recharge_spiritshots(world, npc_oid);
    }
    let Some((tx, ty, tz)) = position_of(world, target_oid) else {
        return;
    };
    let Some((cx, cy, cz)) = position_of(world, npc_oid) else {
        return;
    };

    // Cast timing from the NPC's own finalized speeds. The player path routes
    // through `calc_skill_time_factor` (class template + DEX/WIT bonuses);
    // an NPC's `CombatStats` already hold the finalized values, so the factor
    // is Java's plain `getMAtkSpd() / 333` (or `getPAtkSpd() / 300`).
    let Some(combat) = world
        .objects
        .get_component::<crate::model::components::CombatStats>(&npc_oid)
    else {
        return;
    };
    let factor = if skill.magic_type == 1 {
        (combat.m_atk_spd.max(1) as f64 / 333.0).max(0.01)
    } else {
        (combat.p_atk_spd.max(1) as f64 / 300.0).max(0.01)
    };
    let hit_ms = if skill.magic_type == 2 || skill.magic_type == 4 || skill.magic_type == 21 {
        skill.hit_time
    } else {
        (skill.hit_time as f64 / factor) as i32
    };
    let cool_ms = crate::model::formulas::calc_atk_spd(combat, skill, skill.cool_time as f64);

    // Face the target.
    let heading = crate::model::movement::calculate_heading((tx - cx) as f64, (ty - cy) as f64);
    if let Some(pos) = world.objects.get_component_mut::<Position>(&npc_oid) {
        pos.heading = heading;
    }

    // Only the *initial* consume happens here. The cast then rides the shared
    // `handle_skill_launch` -> `handle_skill_finish` path, which charges
    // `mp_consume` at landing exactly as it does for a player — charging it
    // here too would bill the NPC twice for one spell.
    if skill.mp_initial_consume > 0 {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&npc_oid) {
            v.cur_mp = (v.cur_mp - skill.mp_initial_consume as f64).max(0.0);
        }
    }

    set_skill_reuse(world, npc_oid, skill);

    if let Some(region) = world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    {
        broadcast_near_region_in(
            world,
            region,
            instance_of(world, npc_oid),
            &server_packets::magic_skill_use_raw(
                (npc_oid, cx, cy, cz),
                (target_oid, tx, ty, tz),
                skill.id,
                skill.level,
                hit_ms,
            ),
        );
    }

    let seq = {
        let ai = world
            .objects
            .get_component_mut::<crate::model::npc::NpcAi>(&npc_oid);
        match ai {
            Some(ai) => {
                ai.cast_seq += 1;
                ai.cast_seq
            }
            None => return,
        }
    };
    world.objects.add_components(
        &npc_oid,
        Casting(crate::model::CastState {
            skill_id: skill.id,
            skill_level: skill.level,
            skill_sub_level: 0,
            target_object_id: target_oid,
            seq,
            launched: false,
            // NPCs have no interrupt window of their own to model yet: the
            // whole cast is the hit phase, and the launch task lands the
            // effects immediately after.
            cancel_ms: 0,
            cool_ms,
        }),
    );
    world.scheduler.schedule(
        world.tick + ms_to_ticks(hit_ms),
        ScheduledTask::SkillLaunch {
            player_object_id: npc_oid,
            cast_seq: seq,
        },
    );
}

fn position_of(world: &World, oid: i32) -> Option<(i32, i32, i32)> {
    world
        .objects
        .get_component::<Position>(&oid)
        .map(|p| (p.x, p.y, p.z))
}

fn collision_radius(world: &World, oid: i32) -> f64 {
    super::combat::combatant(world, oid)
        .map(|c| c.collision_radius)
        .unwrap_or(0.0)
}

fn distance_2d(world: &World, a: i32, b: i32) -> Option<f64> {
    let (ax, ay, _) = position_of(world, a)?;
    let (bx, by, _) = position_of(world, b)?;
    Some((((bx - ax) as f64).powi(2) + ((by - ay) as f64).powi(2)).sqrt())
}

fn hp_percent(world: &World, oid: i32) -> f64 {
    world
        .objects
        .get_component::<Vitals>(&oid)
        .map(|v| {
            if v.max_hp > 0 {
                v.cur_hp / v.max_hp as f64 * 100.0
            } else {
                100.0
            }
        })
        .unwrap_or(100.0)
}

/// `EffectList.hasAbnormalType(type, i -> i.getSkill().getAbnormalLevel() >= level)`
/// — don't overwrite an abnormal the target already carries at this level or
/// better. Untyped skills ("NONE") never match, matching the stacking rule in
/// [`crate::model`].
fn has_abnormal_at_least(world: &World, oid: i32, abnormal_type: &str, level: i32) -> bool {
    if abnormal_type.is_empty() || abnormal_type == "NONE" {
        return false;
    }
    world
        .objects
        .get_component::<crate::model::components::Buffs>(&oid)
        .is_some_and(|b| {
            b.0.iter()
                .any(|e| e.abnormal_type == abnormal_type && e.abnormal_level >= level)
        })
}

/// Java `AttackableAI.skillTargetReconsider` — who should this skill actually
/// land on? Until now heal and buff resolved to the caster itself, because the
/// port had no faction data; slice 2 added it, so a pack's healer can now look
/// after the pack. **1040 NPCs on this dist carry a buff-bucket skill and 305 a
/// heal-bucket one**, so this is the difference between a support mob being
/// decorative and being a real problem to fight.
///
/// - **Bad skill** → the caster's own aggro list (whoever is fighting it).
/// - **Good skill** → nearby faction-mates plus itself; a *heal* picks the
///   lowest HP percentage, anything else picks at random.
///
/// **Deviation from Java, deliberate.** Java's good-skill candidate set is
/// `getVisibleObjectsInRange(npc, Creature.class, range)` — *every* creature
/// nearby. Its `checkSkillTarget` only rejects auto-attackable targets **inside
/// the `isContinuous()` branch**, and a heal is not continuous, so as written a
/// healer mob would happily heal a wounded **player** fighting it. That is
/// almost certainly unintended and would read as a port bug in-game, so the
/// candidate set here is scoped to the caster's faction (`shares_clan_with`)
/// plus itself. The scoping makes the AI do *less* than Java, never more.
/// TODO(G21): revisit if a capture ever shows retail mobs healing players.
fn skill_target_reconsider(
    world: &World,
    npc_oid: i32,
    skill: &Skill,
    inside_cast_range: bool,
) -> Option<i32> {
    // `isBad`: for a continuous skill the debuff flag decides, otherwise the
    // effect points do.
    let is_bad = if skill.is_continuous {
        skill.is_debuff
    } else {
        skill.is_bad()
    };

    if is_bad {
        // Anything already fighting this NPC is fair game.
        let candidates: Vec<i32> = world
            .objects
            .get_component::<AggroList>(&npc_oid)
            .map(|a| a.0.keys().copied().collect())
            .unwrap_or_default();
        let valid: Vec<i32> = candidates
            .into_iter()
            .filter(|&oid| check_skill_target(world, npc_oid, oid, skill))
            .collect();
        return pick_random(world, &valid);
    }

    // `insideCastRange ? castRange + collisionRadius : 2000` (Java's own
    // "TODO need some forget range" constant).
    let range = if inside_cast_range {
        skill.cast_range as f64 + collision_radius(world, npc_oid)
    } else {
        2000.0
    };

    let mut valid: Vec<i32> = faction_mates_in_range(world, npc_oid, range)
        .into_iter()
        .filter(|&oid| check_skill_target(world, npc_oid, oid, skill))
        .collect();
    // Java adds self explicitly — `getVisibleObjects` never returns you.
    if check_skill_target(world, npc_oid, npc_oid, skill) {
        valid.push(npc_oid);
    }
    if valid.is_empty() {
        return None;
    }

    // A heal goes to whoever is worst off, not to a random member.
    if skill
        .effects
        .iter()
        .any(|e| matches!(e, crate::model::skill::SkillEffect::Heal { .. }))
    {
        return valid
            .into_iter()
            .min_by(|&a, &b| hp_percent(world, a).total_cmp(&hp_percent(world, b)));
    }
    pick_random(world, &valid)
}

fn pick_random(_world: &World, candidates: &[i32]) -> Option<i32> {
    match candidates.len() {
        0 => None,
        n => Some(candidates[rand::thread_rng().gen_range(0..n)]),
    }
}

/// Living NPCs within `range` that share a faction with this one (the same
/// `shares_clan_with` test the help-call uses), excluding the caller.
fn faction_mates_in_range(world: &World, npc_oid: i32, range: f64) -> Vec<i32> {
    let Some(npc_id) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .map(|n| n.npc_id)
    else {
        return Vec::new();
    };
    let (Some(mine), Some(pos), Some(region)) = (
        world.data.npc_data.get(npc_id),
        world.objects.get_component::<Position>(&npc_oid).copied(),
        world
            .objects
            .get_component::<RegionCell>(&npc_oid)
            .map(|r| r.0),
    ) else {
        return Vec::new();
    };
    if mine.clans.is_empty() {
        return Vec::new();
    }

    (-1..=1)
        .flat_map(|dx| (-1..=1).map(move |dy| (dx, dy)))
        .filter_map(|(dx, dy)| world.npc_regions.get(&(region.0 + dx, region.1 + dy)))
        .flatten()
        .copied()
        .filter(|&other| other != npc_oid)
        .filter(|&other| {
            world
                .objects
                .get_component::<Vitals>(&other)
                .is_some_and(|v| !v.dead)
                && world
                    .objects
                    .get_component::<Position>(&other)
                    .is_some_and(|p| {
                        let d = (((p.x - pos.x) as f64).powi(2) + ((p.y - pos.y) as f64).powi(2))
                            .sqrt();
                        d <= range
                    })
                && world
                    .objects
                    .get_component::<crate::model::npc::Npc>(&other)
                    .and_then(|n| world.data.npc_data.get(n.npc_id))
                    .is_some_and(|theirs| mine.shares_clan_with(theirs))
        })
        .collect()
}

/// `target.isAutoAttackable(npc)` from a monster's point of view: players are,
/// other NPCs are not. That's what keeps a support mob from "buffing" the
/// player it's fighting while still letting it buff its pack.
fn is_auto_attackable_by_npc(world: &World, target_oid: i32) -> bool {
    world
        .objects
        .has_component::<crate::model::Player>(&target_oid)
}
