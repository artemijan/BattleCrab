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
//! What once sat here as narrowings has closed: `skillTargetReconsider`
//! re-picks heal/buff targets across the caster's faction
//! ([`skill_target_reconsider`], with one argued deviation at its doc), the
//! `ARCHER` kite and raid target-chaos live in `npc_ai`, and the `SUICIDE`
//! bucket is wired (`isSuicideAttack` parses; bombers detonate below 30 %
//! HP). Still inert by data, not by gap: `RES` — a Resurrection effect *is*
//! ported, but no NPC on this dist carries a resurrect skill in its
//! `<skillList>`.

use crate::game_loop::guard::{in_zone, maybe_position};
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::npc_template;
use commons::util::rnd;

use crate::data::npc_ai_skills::AiSkillScope;
use crate::data::npc_data::AiType;
use crate::data::zone_data::ZoneKind;
use crate::game_loop::{abnormal, combat, servitor};
use crate::geo::distance::{distance_2d, distance_2d_xy, distance_3d, position_of};
use crate::model::components::{Casting, Position, Vitals};
use crate::model::npc::AggroList;
use crate::model::skill::Skill;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;

use crate::game_loop::helpers::ms_to_ticks;
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::helpers::{broadcast_near_region_in, instance_of};
use crate::game_loop::skills::cast::set_skill_reuse;

/// Java's literal cut between the SHORT_RANGE and LONG_RANGE buckets.
const SHORT_RANGE: f64 = 150.0;

/// The cast block of `thinkAttack`. Returns `true` if a cast started, in which
/// case the caller skips the move/swing tail for this think.
pub(crate) fn try_cast(world: &mut World, npc_oid: i32, target_oid: i32) -> bool {
    // Already casting — one cast at a time.
    if world.objects.has_component::<Casting>(&npc_oid) {
        return false;
    }

    let Some(npc_id) = npc_id_of(world, npc_oid) else {
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

    let Some(ai_skills) = world.data.npc_ai_skills.get(npc_id).cloned() else {
        return false;
    };

    // The SUICIDE block runs *before* the moving/mage gate (Java places it
    // right after `isCoreAIDisabled`, with its own `hasSkillChance()` roll):
    // below 30 % HP a bomber detonates on whoever it is fighting.
    if !ai_skills.get(AiSkillScope::Suicide).is_empty()
        && (hp_percent(world, npc_oid) as i32) < 30
        && has_skill_chance(min_chance, max_chance)
        && let Some(skill) = pick(world, &ai_skills, AiSkillScope::Suicide, npc_oid)
        && check_skill_target(world, npc_oid, target_oid, &skill)
        && cast_at(world, npc_oid, target_oid, &skill)
    {
        return true;
    }

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

    // --- Java's priority ladder, in order. ---

    // 1. Heal — the most important skill, and Java reconsiders the target for
    //    it: the pack's healer looks for whoever is worst off, not just itself.
    //    The chance scales off *that* target's HP so it's certain below 33 %:
    //    `(100 - hpPercent) * 1.5`.
    if let Some(skill) = pick(world, &ai_skills, AiSkillScope::Heal, npc_oid)
        && let Some(heal_target) = skill_target_reconsider(world, npc_oid, &skill, false)
    {
        let hp_pct = hp_percent(world, heal_target);
        let heal_chance = (100.0 - hp_pct) * 1.5;
        if rnd::chance(heal_chance) && cast_at(world, npc_oid, heal_target, &skill) {
            return true;
        }
    }

    // 2. Buff — same reconsider, so a support mob buffs its pack rather than
    //    only itself. Java passes `insideCastRange = true` here.
    if let Some(skill) = pick(world, &ai_skills, AiSkillScope::Buff, npc_oid)
        && let Some(buff_target) = skill_target_reconsider(world, npc_oid, &skill, true)
        && cast_at(world, npc_oid, buff_target, &skill)
    {
        return true;
    }

    // 3. Immobilize a target that's running (kiting or fleeing).
    let target_moving = world
        .objects
        .has_component::<crate::model::components::Movement>(&target_oid);
    if target_moving
        && let Some(skill) = pick(world, &ai_skills, AiSkillScope::Immobilize, npc_oid)
        && cast_at(world, npc_oid, target_oid, &skill)
    {
        return true;
    }

    // 4. Mute a target that's mid-cast (Java's COT bucket).
    if world.objects.has_component::<Casting>(&target_oid)
        && let Some(skill) = pick(world, &ai_skills, AiSkillScope::Cot, npc_oid)
        && cast_at(world, npc_oid, target_oid, &skill)
    {
        return true;
    }

    // 5/6. Range-matched attack skills.
    let dist = distance_2d(world, npc_oid, target_oid).unwrap_or(f64::MAX);
    if dist <= SHORT_RANGE
        && let Some(skill) = pick(world, &ai_skills, AiSkillScope::ShortRange, npc_oid)
        && cast_at(world, npc_oid, target_oid, &skill)
    {
        return true;
    }
    if let Some(skill) = pick(world, &ai_skills, AiSkillScope::LongRange, npc_oid)
        && cast_at(world, npc_oid, target_oid, &skill)
    {
        return true;
    }

    // 7. Anything at all.
    if let Some(skill) = pick(world, &ai_skills, AiSkillScope::General, npc_oid)
        && cast_at(world, npc_oid, target_oid, &skill)
    {
        return true;
    }

    false
}

/// One rung of the ladder: Java's `checkSkillTarget(skill, selected)` gate
/// followed by `npc.doCast(skill)`.
///
/// The two steps do **not** aim at the same object, and that is the whole point
/// of this helper. The AI checks the skill against the creature it is *thinking
/// about* — the most hated player, or the faction-mate `skillTargetReconsider`
/// picked — but `doCast` goes through `SkillCaster.castSkill`, which re-runs
/// `Skill.getTarget` and casts at whatever the **target-type handler** returns.
/// For a `SELF` skill that is the caster itself, no matter who the AI was
/// looking at.
///
/// Collapsing the two was a live parity bug: Catherok (21035) carries Stun
/// (4072), which is `targetType=SELF` / `affectScope=POINT_BLANK` /
/// `affectRange=150` — a self-centred shockwave. Casting it *at the player*
/// made the player the primary affected target, so the stun landed at any
/// distance; retail (and Java) sweep 150 units around the mob and hit nobody
/// when the player is out of that ring. 1332 of this dist's NPC skill entries
/// are `SELF`+`POINT_BLANK`, so this was every point-blank mob skill in the
/// game, not one monster.
fn cast_at(world: &mut World, npc_oid: i32, selected_oid: i32, skill: &Skill) -> bool {
    if !check_skill_target(world, npc_oid, selected_oid, skill) {
        return false;
    }
    let Some(cast_target) = resolve_npc_cast_target(world, npc_oid, selected_oid, skill) else {
        return false;
    };
    start_cast(world, npc_oid, cast_target, skill);
    true
}

/// `Npc.hasSkillChance()` — `Rnd.get(100) < Rnd.get(min, max)`.
fn has_skill_chance(min: i32, max: i32) -> bool {
    let ceiling = rnd::get_range(min, max);
    rnd::chance(ceiling as f64)
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
    let (id, level) = bucket[rnd::get(bucket.len() as i32) as usize];
    let skill = world.data.skill_data.get(id, level)?;
    check_use_conditions(world, npc_oid, skill).then(|| skill.clone())
}

/// `npc.setTarget(target); npc.doCast(skill)` for a skill the caller already
/// holds — the use-condition gate (MP, mutes, cooldown) plus the cast it must
/// always precede, said once.
///
/// Exporting the gate on its own is what let thirteen call sites each re-spell
/// this; the pairing is the invariant, so this is the only way out. The
/// owner-ordered [`ServitorSkillUse`] path comes through here too, so a
/// commanded skill obeys the same rules as an AI-chosen one.
///
/// Returns whether the cast started. Java's `doCast` runs its own checks and
/// quietly does nothing on failure, so a caller that ignores the result matches
/// it.
///
/// [`ServitorSkillUse`]: crate::game_loop::servitor::use_servitor_skill
pub(crate) fn cast_checked(
    world: &mut World,
    npc_oid: i32,
    target_oid: i32,
    skill: &Skill,
) -> bool {
    if !check_use_conditions(world, npc_oid, skill) {
        return false;
    }
    start_cast(world, npc_oid, target_oid, skill);
    true
}

/// [`cast_checked`] with the datapack lookup in front — the shape almost every
/// script and boss AI wants, since they name a skill by id rather than carrying
/// one.
///
/// `false` when the id is absent from this dist as well as when the conditions
/// refuse; the two are indistinguishable to Java's scripts, which call
/// `SkillData.getSkill(...)` straight into `doCast`.
pub(crate) fn cast_skill(
    world: &mut World,
    npc_oid: i32,
    target_oid: i32,
    skill_id: i32,
    level: i32,
) -> bool {
    let Some(skill) = crate::game_loop::helpers::skill_by_id(world, skill_id, level) else {
        return false;
    };
    cast_checked(world, npc_oid, target_oid, &skill)
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
        if abnormal::is_muted(world, npc_oid) {
            return false;
        }
    } else if abnormal::is_physical_muted(world, npc_oid) {
        return false;
    }
    if abnormal::is_blocked_from_actions(world, npc_oid) {
        return false;
    }
    if let Some(reuses) = world
        .objects
        .get_component::<crate::model::components::Reuses>(&npc_oid)
        && let Some(r) = reuses.0.get(&skill.reuse_key())
        && r.until_tick > world.tick
    {
        return false;
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

    // Java's *first* line: `if (skill.getTarget(npc, target, false,
    // npc.isMovementDisabled(), false) == null) return false`. Routing through
    // the target-type handlers is what puts an NPC's casts under the same
    // geodata rules as a player's — every non-self handler ends in
    // `GeoEngine.canSeeTarget`, so a mob can no more nuke through a wall than
    // you can. It also refuses the target types that simply don't resolve for
    // an NPC (GROUND), and the friend/foe mismatches.
    if resolve_npc_cast_target(world, npc_oid, target_oid, skill).is_none() {
        return false;
    }

    // Cast range — `AttackableAI.checkSkillTarget`'s
    // `Util.checkIfInRange(skill.getCastRange(), npc, target, true)`. Collision
    // radii included, and the `true` is `includeZAxis`: unlike the player's
    // `SkillCaster.castSkill` gate (2D), a mob measures in **3D**, so it won't
    // start a spell at something far above or below it on a cliff or a tower.
    if skill.cast_range > 0 {
        let reach = skill.cast_range as f64
            + collision_radius(world, npc_oid)
            + collision_radius(world, target_oid);
        if distance_3d(world, npc_oid, target_oid).is_none_or(|d| d > reach) {
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

/// `Skill.getTarget(npc, selected, forceUse = false, dontMove =
/// npc.isMovementDisabled(), sendMessage = false)` — the
/// `handlers/targethandlers/*.java` scripts from an NPC caster's side.
///
/// The player path lives in [`skills::cast::resolve_cast_target`] and takes a
/// `&Player`; NPC casts never reached it, so until now an NPC cast simply used
/// whatever creature the AI was thinking about. That skipped the handlers'
/// closing `GeoEngine.canSeeTarget` — the "mobs follow the same geodata rules
/// as players" half of this — and the `SELF` remap that decides where a
/// point-blank AoE is actually centred.
///
/// `sendMessage` is false for every AI cast, so no arm has a message to send.
///
/// [`skills::cast::resolve_cast_target`]: cast::resolve_cast_target
pub(crate) fn resolve_npc_cast_target(
    world: &World,
    npc_oid: i32,
    selected_oid: i32,
    skill: &Skill,
) -> Option<i32> {
    use crate::game_loop::{abnormal, servitor, target, zones};
    use crate::model::skill::TargetType;

    // Java passes `getActiveChar().isMovementDisabled()` as `dontMove`, which
    // turns the TARGET/ENEMY handlers' "walk into cast range" into a refusal —
    // a rooted mob can't close, so an out-of-range skill is dropped instead of
    // being cast from where it stands.
    let dont_move = abnormal::is_movement_disabled(world, npc_oid);

    let resolved = match skill.target_type {
        // `None.java`: the caster outright, with no peace-zone gate.
        TargetType::None_ => return Some(npc_oid),
        // `Self.java`: the caster; a *bad* self skill is refused in a peace
        // zone. This is the arm that fixes point-blank mob AoEs — the cast
        // lands on the mob, and `affectScope` sweeps outward from there.
        TargetType::Self_ => {
            if skill.is_bad() && in_zone(world, npc_oid, ZoneKind::Peace) {
                return None;
            }
            return Some(npc_oid);
        }
        // `Ground.java` is gated on `creature.isPlayable()`; an NPC caster
        // falls through to null there, so NPC GROUND skills are inert in Java
        // too.
        TargetType::Ground => return None,
        // `Others.java`: the NPC's own selection, never itself. No NPC on this
        // dist casts an `OTHERS` skill, so this is inert in practice.
        TargetType::Others => {
            if selected_oid == npc_oid {
                return None;
            }
            selected_oid
        }
        // `DoorTreasure.java` reads `creature.getTarget()` and accepts only a
        // door or a chest. No NPC on this dist casts Unlock, and an NPC has no
        // reason to pick a lock, so this is inert on both sides.
        TargetType::DoorTreasure => return None,
        // `Summon.java`: the caster's own servitor.
        TargetType::Summon => servitor::servitor_of(world, npc_oid)?,
        // `Target.java`: any creature, self included (self returns early and
        // skips every gate — "you can always target yourself").
        TargetType::Target => {
            if selected_oid == npc_oid {
                return Some(npc_oid);
            }
            if dont_move && !within_cast_range(world, npc_oid, selected_oid, skill) {
                return None;
            }
            selected_oid
        }
        // `Enemy.java` / `EnemyOnly.java`: never self, never a corpse, and the
        // target must be auto-attackable (`forceUse` is false for an AI cast,
        // so there is no override). A monster attacker makes every player
        // auto-attackable, which is what lets a mob nuke you at all.
        TargetType::Enemy | TargetType::EnemyOnly => {
            if selected_oid == npc_oid {
                return None;
            }
            if is_dead(world, selected_oid) && !skill.stay_after_death {
                return None;
            }
            if !target::is_auto_attackable(world, npc_oid, selected_oid) {
                return None;
            }
            if dont_move && !within_cast_range(world, npc_oid, selected_oid, skill) {
                return None;
            }
            selected_oid
        }
        // `EnemyNot.java`: the inverse gate — a hostile target is refused, self
        // is always allowed, and the dead are not excluded (a heal may land on
        // a fresh corpse ahead of a resurrection).
        TargetType::EnemyNot => {
            if selected_oid == npc_oid {
                return Some(npc_oid);
            }
            if target::is_auto_attackable(world, npc_oid, selected_oid) {
                return None;
            }
            selected_oid
        }
        // `NpcBody.java` / `PcBody.java`: a corpse of the matching kind.
        TargetType::NpcBody => {
            if !is_dead(world, selected_oid)
                || !world
                    .objects
                    .has_component::<crate::model::npc::Npc>(&selected_oid)
            {
                return None;
            }
            selected_oid
        }
        TargetType::PcBody => {
            if !is_dead(world, selected_oid)
                || !world
                    .objects
                    .has_component::<crate::model::Player>(&selected_oid)
            {
                return None;
            }
            selected_oid
        }
        // `targethandlers/OwnerPet.java` — `creature.getActingPlayer()`, and
        // `getActingPlayer()` on a summon is its **owner**. So a servitor
        // casting one of these aims at the player who owns it, never at
        // whatever the caster happens to have selected.
        //
        // The tamed-beast buffs (5186-5201) reach here with the tamer already
        // selected, so they resolved correctly even while this arm was folded
        // into `Other` — Master Recharge (4025, every Baby Kookaburra) did
        // not, which is what put this arm here.
        // Read straight off `ServitorOf` rather than through `acting_player`,
        // whose "not a servitor → itself" fallback would let a plain monster
        // self-target. Java has no such fallback: `getActingPlayer()` is null
        // for a bare `Npc`, so the cast finds no target and dies.
        TargetType::OwnerPet => {
            world
                .objects
                .get_component::<crate::model::components::ServitorOf>(&npc_oid)?
                .owner_object_id
        }
        // The handlers this port still collapses into `Other` (`OTHERS`,
        // `ARTILLERY`, `WYVERN_TARGET`, `ADVANCE_BASE`, …): passing the
        // selected target through matches each reachable carrier. `OTHERS`
        // ("not self") can only receive an aggro target here, never the
        // caster; and the siege-machine types (`ARTILLERY`, `WYVERN_TARGET`)
        // have no AI route on this dist — their casts come from scripts that
        // pick the target explicitly.
        TargetType::Other => selected_oid,
    };

    // "Geodata check when character is within range" — the closing gate of
    // `Target.java`, `Enemy.java` and `EnemyOnly.java`. `canSeeTarget` short-
    // circuits to true for a door (a closed gate occludes the ray to its own
    // centre), matching `GeoEngine.canSeeTarget(asker, target)`.
    let (Some(from), Some(to)) = (
        maybe_position(world, npc_oid),
        maybe_position(world, resolved),
    ) else {
        return None;
    };
    let target_is_door = world
        .objects
        .has_component::<crate::model::door::Door>(&resolved);
    if !target_is_door
        && !world
            .geo
            .can_see_target(from.x, from.y, from.z, to.x, to.y, to.z)
    {
        return None;
    }

    // `Enemy.java`/`EnemyOnly.java` close with the peace-zone refusal, after
    // the LOS check. It is a playable-on-playable rule, so it never fires for
    // a monster caster — ported for shape and for the servitor casts that do
    // reach this path.
    if matches!(skill.target_type, TargetType::Enemy | TargetType::EnemyOnly)
        && zones::is_inside_peace_zone(world, npc_oid, resolved)
    {
        return None;
    }

    Some(resolved)
}

/// `Util.checkIfInRange(skill.getCastRange(), npc, target, false)` as the
/// `dontMove` handlers use it — 2D centre distance, no collision radii (Java
/// compares `calculateDistance2D(target) > skill.getCastRange()` directly).
fn within_cast_range(world: &World, npc_oid: i32, target_oid: i32, skill: &Skill) -> bool {
    distance_2d(world, npc_oid, target_oid).is_some_and(|d| d <= skill.cast_range as f64)
}

/// NPC-side `SkillCaster.startCasting`. Much shorter than the player path:
/// no shots, no queued action, no cast bar or `YOU_USE_S1` (nobody to send
/// them to), no MP-initial-consume message — just face, broadcast, schedule.
pub(crate) fn start_cast(world: &mut World, npc_oid: i32, target_oid: i32, skill: &Skill) {
    // `Creature.doCast`'s first statement: "Attackables cannot cast while
    // moving." — `if (isAttackable() && isMoving()) return;`. Every entry into
    // an NPC cast funnels through `doCast` in Java, boss and quest scripts
    // included, so the refusal is here rather than in `try_cast`.
    //
    // This is the gate that matters for `AiType::Mage`. `thinkAttack`'s own
    // ladder guard is `(!npc.isMoving() && npc.hasSkillChance()) || (aiType ==
    // MAGE)` — the mage arm deliberately bypasses the `isMoving()` test (it is
    // there to skip the skill-chance roll), and `doCast` is the only thing that
    // then stops a running mage from casting. Without it all 402 MAGE templates
    // on this dist cast mid-sprint.
    //
    // Java's `thinkAttack` `return`s whether or not `doCast` actually cast, so
    // `cast_at` reporting `true` on a refusal is correct: the think ends, the
    // move already in flight carries the mob on, and the next think — by which
    // time it has arrived and dropped `Movement` — casts for real.
    if world
        .objects
        .has_component::<crate::model::components::Movement>(&npc_oid)
        && npc_template(world, npc_oid).is_some_and(|t| t.is_attackable_class())
    {
        return;
    }

    // `SkillCaster.startCasting`: "Stop movement when casting. Except instant
    // cast." → `caster.getAI().clientStopMoving(null)`, which drops the move
    // data and broadcasts `StopMove` so every observer pins the caster in
    // place. Attackables never reach this with move data (the `doCast` refusal
    // above already sent them away), but a servitor or pet does — `Summon` is
    // `Playable`, not `Attackable`, so only this half applies to it.
    //
    // SKIP(unreachable): Java skips the stop for an instant cast
    // (`SIMULTANEOUS` casting type, `abnormalInstant`, or `withoutAction`).
    // Censused against the whole datapack: 57 skills carry one of those
    // markers and **none** of them appears in any NPC's `<skillList>` (2159
    // distinct ids) or in any datapack script's `getSkill(...)`. So the
    // unconditional stop is not merely exact for today's callers — there is no
    // data on this dist that can reach the other branch.
    super::ai::stop_npc(world, npc_oid);

    // Java `Summon.doCast` → `rechargeShots(false, true, false)`: a summon
    // charges its magic shot before casting, the mirror of the soulshot charge
    // the attack loop does before swinging. No-op for a plain monster.
    if skill.magic_type == 1 {
        servitor::recharge_spiritshots(world, npc_oid);
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
    if skill.mp_initial_consume > 0
        && let Some(v) = world.objects.get_component_mut::<Vitals>(&npc_oid)
    {
        v.cur_mp = (v.cur_mp - skill.mp_initial_consume as f64).max(0.0);
    }

    set_skill_reuse(world, npc_oid, skill);

    if let Some(region) = region_cell_of(world, npc_oid) {
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
            // NPCs cast no item skills.
            trigger_item_object_id: 0,
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

fn collision_radius(world: &World, oid: i32) -> f64 {
    combat::combatant(world, oid)
        .map(|c| c.collision_radius)
        .unwrap_or(0.0)
}

/// `100.0` when the answer is unavailable — a departed target or one whose
/// stats have not been computed yet reads as *healthy*, so the heal and
/// self-buff gates below it decline rather than fire blind.
fn hp_percent(world: &World, oid: i32) -> f64 {
    crate::game_loop::helpers::hp_fraction(world, oid).map_or(100.0, |f| f * 100.0)
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
/// Revisit only if a retail capture ever shows mobs healing players.
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
    // constant, which Java's own comment flags as needing a real forget range).
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
        n => Some(candidates[rnd::get(n as i32) as usize]),
    }
}

/// Living NPCs within `range` that share a faction with this one (the same
/// `shares_clan_with` test the help-call uses), excluding the caller.
fn faction_mates_in_range(world: &World, npc_oid: i32, range: f64) -> Vec<i32> {
    let Some(npc_id) = npc_id_of(world, npc_oid) else {
        return Vec::new();
    };
    let (Some(mine), Some(pos), Some(region)) = (
        world.data.npc_data.get(npc_id),
        maybe_position(world, npc_oid),
        region_cell_of(world, npc_oid),
    ) else {
        return Vec::new();
    };
    if mine.clans.is_empty() {
        return Vec::new();
    }

    world
        .npcs_visible_from(region)
        .into_iter()
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
                        let d = distance_2d_xy(p.x, p.y, pos.x, pos.y);
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
