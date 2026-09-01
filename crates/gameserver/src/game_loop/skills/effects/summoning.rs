//! Summoning-flavored instant effects: NPC/totem summons and the servitor
//! betrayal flip, extracted from the `apply_skill_effects` match.

use super::servitor_owner_of;
use crate::game_loop::space::position::pos_of;
use crate::model::components::Vitals;
use crate::model::skill::Skill;
use crate::world::World;
/// `SummonNpc.instant` — the `EffectPoint` branch drops the symbol totems
/// (PLAN_G19_SYMBOLS.md); every other template type takes Java's **default**
/// plain-spawn branch (the Holiday Trees and Squash/Watermelon seeds —
/// item-cast carriers, so "learnable" was never the right reachability test
/// here). SKIP(G19): the `Decoy` branch — no reachable skill on this dist
/// summons a template of type `Decoy` (Decoy 525 has no tree row or item
/// grant; item 13769's "Life-size Decoy" 32544 is type `Folk`; verified
/// 2026-08-06).
pub(crate) fn summon_npc(
    world: &mut World,
    target_oid: i32,
    skill: &Skill,
    npc_id: i32,
    npc_count: i32,
    despawn_delay: i32,
) {
    // Java: effected must be a live player (dead/observer gated).
    let effected_alive_player = world
        .objects
        .has_component::<crate::model::Player>(&target_oid)
        && world
            .objects
            .get_component::<Vitals>(&target_oid)
            .is_some_and(|v| !v.dead);
    if !effected_alive_player {
        return;
    }
    // `if (player.isMounted()) return;`
    if world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
        .is_some_and(|p| p.is_mounted())
    {
        return;
    }
    // GROUND skills spawn at the stored world position; everything else at
    // the effected creature (`SummonNpc.instant`).
    let fallback = pos_of(world, target_oid).unwrap_or((0, 0, 0));
    let (x, y, z) = if skill.target_type == crate::model::skill::TargetType::Ground {
        world
            .objects
            .get_component::<crate::model::components::GroundSkillTarget>(&target_oid)
            .map(|g| (g.x, g.y, g.z))
            .unwrap_or(fallback)
    } else {
        fallback
    };
    let is_effect_point = world
        .data
        .npc_data
        .get(npc_id)
        .is_some_and(|t| t.type_name == "EffectPoint");
    for _ in 0..npc_count.max(1) {
        if is_effect_point {
            crate::game_loop::skills::effect_point::spawn_effect_point(
                world,
                target_oid,
                npc_id,
                x,
                y,
                z,
                despawn_delay,
            );
        } else {
            crate::game_loop::skills::effect_point::spawn_plain_summon(
                world,
                target_oid,
                npc_id,
                x,
                y,
                z,
                despawn_delay,
            );
        }
    }
}

/// `Betray.onStart` — the servitor turns on its owner. `canStart` requires a
/// player effector and a summon effected, so this is aimed at somebody
/// *else's* pet. The `BETRAYED` flag (which stops it obeying and makes it
/// auto-attackable) rides the landed buff; what happens here is the AI flip.
pub(crate) fn betray(world: &mut World, caster_oid: i32, target_oid: i32) {
    let Some(owner) = servitor_owner_of(world, target_oid) else {
        return; // not a summon — Java's `canStart` refuses
    };
    if !world
        .objects
        .has_component::<crate::model::Player>(&caster_oid)
    {
        return;
    }
    // `getAI().setIntention(ATTACK, getActingPlayer())` — the servitor's own
    // owner becomes its target. Routed through the ordinary attack order so it
    // stops following, takes the top hate slot and arms the attack timeout
    // exactly like a commanded attack would.
    crate::game_loop::servitor::servitor_attack(world, owner, owner);
}
