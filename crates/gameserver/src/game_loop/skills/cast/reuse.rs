//! The reuse (cooldown) gate: check and set, keyed by shared reuse groups.

use crate::game_loop::helpers::send_sm_and_action_failed;
use crate::game_loop::helpers::send_sm_bare_to_player;
use crate::model::skill::OperateType;
use crate::model::skill::Skill;
use crate::network::server_packets;
use crate::scheduler::ms_to_ticks;
use crate::world::World;
/// Reuse gate shared by `use_magic_on` and the `ItemSkills` item handler
/// (Java `Player.isSkillDisabled`/`getSkillRemainingReuseTime`), keyed by the
/// shared reuse group when the skill has one. `true` means the skill is off
/// cooldown; a still-cooling skill sends the h/m/s breakdown (or SM 48 for
/// short reuses) plus `ActionFailed` and returns `false`.
pub(crate) fn check_skill_reuse(
    world: &World,
    client_id: u32,
    object_id: i32,
    skill: &Skill,
) -> bool {
    use server_packets::{SmParam, sm_ids};

    let Some(&crate::model::SkillReuse {
        until_tick,
        total_ms,
        ..
    }) = world
        .objects
        .get_component::<crate::model::components::Reuses>(&object_id)
        .and_then(|r| r.0.get(&skill.reuse_key()))
    else {
        return true;
    };
    if until_tick <= world.tick {
        return true;
    }
    let name_param = SmParam::SkillName {
        id: skill.id,
        level: skill.level,
    };
    if total_ms > 3000 {
        let remaining_ms = (until_tick - world.tick) * 100;
        let hours = (remaining_ms / 3_600_000) as i32;
        let minutes = ((remaining_ms % 3_600_000) / 60_000) as i32;
        let seconds = ((remaining_ms / 1000) % 60) as i32;
        if hours > 0 {
            send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::S2_HOURS_S3_MINUTES_S4_SECONDS_REMAINING_FOR_REUSE,
                &[
                    name_param,
                    SmParam::Int(hours),
                    SmParam::Int(minutes),
                    SmParam::Int(seconds),
                ],
            );
        } else if minutes > 0 {
            send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::S2_MINUTES_S3_SECONDS_REMAINING_FOR_REUSE,
                &[name_param, SmParam::Int(minutes), SmParam::Int(seconds)],
            );
        } else {
            send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::S2_SECONDS_REMAINING_FOR_REUSE,
                &[name_param, SmParam::Int(seconds)],
            );
        }
    } else {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::S1_IS_NOT_AVAILABLE_REUSE,
            &[name_param],
        );
    }
    false
}

/// Registers a skill's cooldown (Java `Player.addTimeStamp`), skipped when
/// trivially short (`> 10` ms, like Java). Shared by `start_casting` and the
/// `ItemSkills` item handler (immediate-effect items never enter
/// `start_casting`).
pub(crate) fn set_skill_reuse(world: &mut World, object_id: i32, skill: &Skill) {
    // Java gates on the **computed** delay (`getReuseTime`), not the raw one,
    // so a −99 % Super Haste can take a skill under the 10 ms floor and out of
    // the cooldown map entirely.
    let mut reuse_delay =
        crate::game_loop::skills::effects::reuse_time_for(world, object_id, skill);
    if reuse_delay <= 10 {
        return;
    }
    // `Formulas.calcSkillMastery` — Skill Mastery (330 STR / 331 INT) and
    // Focus Skill Mastery (334, the rate multiplier). On a proc the cooldown
    // collapses to 100 ms and the caster is told (G34 S4).
    //
    // Java's three exclusions all sit here and all matter: a **static** skill,
    // one cast from an **item** (`getReferenceItemId() != 0`) and anything that
    // is not `operateType A1` are never mastered — so it fires on ordinary
    // active skills only, not on toggles, buffs (A2) or potions.
    if reuse_delay > 10
        && skill.magic_type != 2
        && skill.item_consume_id == 0
        // `OperateType::Active` collapses A1 and A2, and `is_continuous` is
        // exactly the A2..A6/DA2..DA5 family — so `Active && !is_continuous`
        // is A1, which is what Java's `getOperateType() == A1` demands.
        && skill.operate_type == OperateType::Active
        && !skill.is_continuous
        && crate::game_loop::skills::effects::calc_skill_mastery(world, object_id)
    {
        reuse_delay = 100;
        send_sm_bare_to_player(
            world,
            object_id,
            server_packets::sm_ids::A_SKILL_IS_READY_TO_BE_USED_AGAIN,
        );
    }
    let until_tick = world.tick + ms_to_ticks(reuse_delay);
    // `reuses_mut` attaches the map when it is missing — without it this write
    // was a silent no-op on an NPC and `npc_cast`'s check, which treats an
    // absent component as "ready", always passed: NPC skill cooldowns never
    // applied at all and a mob could re-cast as fast as its AI ticked.
    if let Some(reuses) = crate::game_loop::helpers::reuses_mut(world, object_id) {
        reuses.0.insert(
            skill.reuse_key(),
            crate::model::SkillReuse {
                skill_level: skill.level,
                until_tick,
                total_ms: reuse_delay,
            },
        );
    }
}
