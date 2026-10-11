//! `SkillCaster.handleSkillFly` — the rush half of a fly-type skill
//! (`SkillOperateType.isFlyType()`): as the cast launches, the caster is slid
//! to where the skill puts it.

use crate::game_loop::space::position;
use crate::game_loop::space::position::maybe_position;
use crate::model::Player;
use crate::model::components;
use crate::model::skill::Skill;
use crate::model::skill::target::SkillFly;
use crate::network::server_packets;
use crate::world::World;

/// Port of `SkillCaster.handleSkillFly`: work out the landing point,
/// geo-validate it, drop the caster's intention and slide it there.
///
/// Java sends `FlyToLocation` with a trailing `flySpeed 0, flyDelay 0,
/// animationSpeed 333`; the Interlude packet has no such tail (see
/// `server_packets::fly_to_location`), so only the fly type survives.
pub(super) fn handle_skill_fly(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    fly: SkillFly,
) {
    let Some(from) = maybe_position(world, caster_oid) else {
        return;
    };
    let (x, y, z) = match fly {
        SkillFly::ToTarget => {
            if caster_oid == target_oid {
                // `course = 180°` on top of the `Math.PI` already in the
                // formula: a full turn, i.e. `castRange` straight ahead.
                let radian =
                    crate::game_loop::skills::affect::heading_to_degree(from.heading).to_radians();
                let angle = std::f64::consts::PI + radian + std::f64::consts::PI;
                (
                    from.x + (angle.cos() * skill.cast_range as f64) as i32,
                    from.y + (angle.sin() * skill.cast_range as f64) as i32,
                    from.z,
                )
            } else {
                let Some(to) = maybe_position(world, target_oid) else {
                    return;
                };
                (to.x, to.y, to.z)
            }
        }
    };

    // `creature.isFlying() ? new Location(x, y, z) : getValidLocation(...)` —
    // a wyvern rider is not held to the ground mesh.
    let flying = world
        .objects
        .get_component::<Player>(&caster_oid)
        .is_some_and(Player::is_flying);
    let dest = if flying {
        (x, y, z)
    } else {
        world
            .geo
            .get_valid_location(from.x, from.y, from.z, x, y, z)
    };

    // `getAI().setIntention(AI_INTENTION_IDLE)`. The cast itself is not an
    // intention here (it lives in `Casting`), so this only drops what was
    // queued behind it; `resume_action_after_cast` re-arms the attack for a
    // `nextAction ATTACK` skill, as Java's `finishSkill` does.
    world
        .objects
        .remove_component::<components::combat::Intent>(&caster_oid);

    position::slide_to(world, caster_oid, dest, server_packets::FlyType::Charge);
    // Java's `revalidateZone(true)`: `slide_to` revalidates unforced, which
    // skips a landing less than 100 units from the last check.
    crate::game_loop::space::zones::revalidate_zone(world, caster_oid, true);
}
