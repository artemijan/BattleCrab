//! Post-cast consequences per target: reflection, PvP/PvE matchup effects,
//! flagging, hate and the AI wake.

use crate::game_loop::helpers::stat_add;
use crate::game_loop::npc::npc_template;
use crate::model::Player;
use crate::model::skill::Skill;
use crate::model::skill::effects::SkillEffect;
use crate::world::World;
/// `Formulas.calcBuffDebuffReflection` — the chance that `target` bounces this
/// skill back at its caster.
///
/// ```java
/// if (!skill.isDebuff() || (skill.getActivateRate() == -1)) return false;
/// return target.getStat().getValue(skill.isMagic() ? REFLECT_SKILL_MAGIC : REFLECT_SKILL_PHYSIC, 0) > Rnd.get(100);
/// ```
///
/// Two gates before the roll: the skill must be a **debuff**, and it must
/// declare an `activateRate` (a skill with the default `-1` — i.e. one that
/// always lands — is never reflected). Which of the two stats is read depends
/// on the *incoming skill's* `isMagic`, not on the defender.
pub(super) fn calc_buff_debuff_reflection(
    world: &mut World,
    target_oid: i32,
    skill: &Skill,
) -> bool {
    use crate::model::stats::Stat;
    if !skill.is_debuff || skill.activate_rate == -1 {
        return false;
    }
    let stat = if skill.magic_type == 1 {
        Stat::ReflectSkillMagic
    } else {
        Stat::ReflectSkillPhysic
    };
    let chance = stat_add(world, target_oid, stat);
    if chance <= 0.0 {
        return false;
    }
    chance > world.roll(100) as f64
}

/// Java's PVE/PVP scope selector:
///
/// ```java
/// effector.isPlayable() && effected.isAttackable() ? PVE
///   : effector.isPlayable() && effected.isPlayable() ? PVP : null
/// ```
///
/// Returns `None` when neither applies (an NPC caster, or a player hitting
/// something that is neither attackable nor playable).
pub(super) fn matchup_effects(
    world: &World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
) -> Option<Vec<SkillEffect>> {
    // `isPlayable()` — a player. Java's `Playable` covers summons too, but this
    // path is only ever reached for a player caster, so the narrowing is the
    // call site's rather than a missing subsystem (servitors landed with G29).
    if !world.objects.has_component::<Player>(&caster_oid) {
        return None;
    }
    if world.objects.has_component::<Player>(&target_oid) {
        return Some(skill.pvp_effects.clone());
    }
    let attackable = npc_template(world, target_oid).is_some_and(|t| t.is_attackable_class());
    attackable.then(|| skill.pve_effects.clone())
}

#[cfg(test)]
pub(crate) fn apply_bad_skill_aggro_for_test(
    world: &mut World,
    player_object_id: i32,
    target_oid: i32,
    skill: &Skill,
) {
    apply_cast_consequences(world, player_object_id, target_oid, skill);
}
/// The per-target half of Java's `callSkill`: PvP flagging, monster hate and
/// the AI wake. Split out of [`handle_skill_finish`] when affect scopes landed
/// so every creature an AoE touches gets the same treatment the single target
/// used to get.
pub(super) fn apply_cast_consequences(
    world: &mut World,
    player_object_id: i32,
    target_oid: i32,
    skill: &Skill,
) {
    let target_is_player = world.objects.has_component::<Player>(&target_oid);
    // Monster proxy: an NPC whose template is auto-attackable (same test the
    // targeting code uses for "is this a monster").
    let target_is_monster = !target_is_player
        && npc_template(world, target_oid).is_some_and(|t| t.is_auto_attackable());
    if skill.is_bad() {
        // Bad skill on a player → flag the caster against that target
        // (`updatePvPStatus(target)`). Monsters take hate + an AI wake, no flag.
        if target_is_player {
            crate::game_loop::combat::pvp::update_pvp_status_target(
                world,
                player_object_id,
                target_oid,
            );
        } else if target_is_monster {
            // `callSkill`: a bad skill on an attackable *always* adds hate
            // (`addDamageHate(caster, 0, -effectPoint)`) and wakes its AI
            // (`notifyEvent(EVT_ATTACKED, caster)`), right after `activateSkill`
            // and **independent of whether the effects landed**. That's why a
            // resisted or otherwise no-op debuff still makes the mob retaliate:
            // the wake belongs here in the `callSkill` analog, not in the effect
            // handlers (those only wake on damage / spoil, so a pure or resisted
            // debuff would never aggro). `npc_wake_on_attacked` is the EVT_ATTACKED
            // primitive (hate += 1 + switch to the attack intention); the explicit
            // `-effectPoint` hate is added on top, matching `addDamageHate`
            // (`-effect_point` is positive since a bad skill has `effect_point < 0`).
            // Java gates *only this notify* on `!skill.hasEffectType(HATE)`:
            // an aggro-shedding skill (Bluff, Forget, Trick, Repose, Peace,
            // Eva's Serenade) must not wake the very mob it just made forget
            // you. The `-effectPoint` hate below is **not** gated, and Bluff
            // (358) really does carry `effectPoint -1`, so it still adds 1
            // hate — it is the AI wake, not the hate, that Java suppresses.
            if !skill.has_hate_effect() {
                crate::game_loop::combat::npc_wake_on_attacked(world, target_oid, player_object_id);
            }
            let hate = (-skill.effect_point) as f64;
            if hate != 0.0
                && let Some(aggro) = world
                    .objects
                    .get_component_mut::<crate::model::npc::AggroList>(&target_oid)
            {
                aggro.0.entry(player_object_id).or_default().hate += hate;
            }
        }
    } else if target_oid != player_object_id {
        // Good/support skill (not self-cast): "supporting monsters or players
        // results in pvpflag" — buffing a monster, or a flagged/PK player,
        // self-flags the caster (`updatePvPStatus()`).
        let target_is_flagged = world
            .objects
            .get_component::<crate::model::components::combat::PvpState>(&target_oid)
            .is_some_and(|s| s.flag > 0)
            || world
                .objects
                .get_component::<Player>(&target_oid)
                .is_some_and(|p| p.reputation < 0);
        let flag_self = (skill.effect_point > 0 && target_is_monster)
            || (target_is_player && target_is_flagged);
        if flag_self {
            crate::game_loop::combat::pvp::update_pvp_status(world, player_object_id);
        }
    }
}
