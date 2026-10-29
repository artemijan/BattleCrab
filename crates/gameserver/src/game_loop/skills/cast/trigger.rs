//! `SkillCaster.triggerCast` — an instant, cost-free cast an effect fires on
//! someone's behalf, queued per object.

use super::lifecycle::{activate_on_target, apply_self_effects};
use super::reuse::{disable_skill, is_skill_disabled};
use crate::game_loop::helpers;
use crate::game_loop::net::broadcast;
use crate::game_loop::skills::conditions;
use crate::game_loop::space::position::maybe_position;
use crate::model::components::combat::Casting;
use crate::model::skill::Skill;
use crate::model::skill::target::OperateType;
use crate::network::server_packets;
use crate::world::World;

/// `SkillCaster.triggerCast(creature, target, skill)` — the three-argument
/// form: no item, and `ignoreTargetType = true`, so `target` is used as given
/// rather than re-resolved through the skill's own `targetType`.
///
/// Java offers the cast to the target's `WorldObject._triggerCastQueue` and
/// drains that queue under an `AtomicBoolean`: a trigger whose effects set off
/// another trigger aimed at the same object does not run inside the first, it
/// runs after it. [`World::trigger_casts`] is that queue — a key present means
/// the object is draining.
pub(crate) fn trigger_cast(world: &mut World, caster_oid: i32, target_oid: i32, skill: Skill) {
    if let Some(queue) = world.trigger_casts.get_mut(&target_oid) {
        queue.push_back((caster_oid, skill));
        return;
    }
    world.trigger_casts.insert(target_oid, Default::default());
    let mut next = Some((caster_oid, skill));
    while let Some((caster_oid, skill)) = next {
        run(world, caster_oid, target_oid, &skill);
        next = world
            .trigger_casts
            .get_mut(&target_oid)
            .and_then(|queue| queue.pop_front());
    }
    world.trigger_casts.remove(&target_oid);
}

/// `SkillCaster.triggerCast(TriggerCastInfo)` — one dequeued cast.
fn run(world: &mut World, caster_oid: i32, target_oid: i32, skill: &Skill) {
    // `creature == null`: the caster left the world while this sat queued.
    if maybe_position(world, caster_oid).is_none() {
        return;
    }
    // `skill.checkCondition(creature, target)`, whose refusal lines reach a
    // player caster; there is no `ActionFailed`, nothing was requested.
    if let Err(refusal) = conditions::check_cast(world, caster_oid, skill, target_oid) {
        if let Some(client_id) = helpers::client_for_player(world, caster_oid) {
            conditions::send_refusal_lines(
                world, client_id, caster_oid, skill, target_oid, &refusal,
            );
        }
        return;
    }
    if is_skill_disabled(world, caster_oid, skill) {
        return;
    }
    // The raw `getReuseDelay()` into `disableSkill` — no reuse-rate stats, no
    // Skill Mastery, no timestamp.
    if skill.reuse_delay > 0 {
        disable_skill(world, caster_oid, skill, skill.reuse_delay);
    }

    let targets =
        crate::game_loop::skills::affect::targets_affected(world, caster_oid, target_oid, skill);
    if !skill.not_broadcastable
        && !is_channeling(world, caster_oid)
        && let (Some(from), Some(to)) = (
            maybe_position(world, caster_oid),
            maybe_position(world, target_oid),
        )
    {
        // `new MagicSkillUse(creature, currentTarget, displayId, level, 0, 0)`
        // through `broadcastSkillPacket`, which is a plain `broadcastPacket`.
        let pkt = server_packets::magic_skill_use_raw(
            (caster_oid, from.x, from.y, from.z),
            (target_oid, to.x, to.y, to.z),
            skill.id,
            skill.level,
            0,
        );
        broadcast::broadcast_including_self(world, caster_oid, &pkt);
    }

    // `skill.activateSkill(creature, item, targets)`.
    for t in targets {
        activate_on_target(world, caster_oid, t, skill);
    }
    apply_self_effects(world, caster_oid, skill);

    // `OnCreatureSkillFinishCast`, carrying the trigger's own target.
    crate::game_loop::skills::effects::fire_magic_type_triggers(
        world, caster_oid, target_oid, skill,
    );
}

/// Java `Creature.isChanneling()` — the `SkillChannelizer` runs from
/// `startCasting` (`if (_skill.isChanneling())`) until the cast stops, so it is
/// the life of a channeling skill's `Casting` slot.
fn is_channeling(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<Casting>(&oid)
        .and_then(|c| world.data.skill_data.get(c.0.skill_id, c.0.skill_level))
        .is_some_and(|s| s.operate_type == OperateType::Channeling)
}
