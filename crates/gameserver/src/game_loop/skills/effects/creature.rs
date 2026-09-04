//! Two lookups every effect module needs: a creature's level and its display
//! name, whichever side of the player/NPC split it is on.

use crate::game_loop::{helpers, npc};
use crate::world::World;

/// A target creature's level (Java `Creature.getLevel()`) for the debuff
/// landing-rate math — an NPC reads its template, a player its record. Defaults
/// to 1, matching the Spoil landing-level fallback.
pub(crate) fn creature_level(world: &World, oid: i32) -> i32 {
    // Java `Cubic.getLevel()` → `_owner.getLevel()`. Checked before the NPC/
    // player split because a cubic's caster entity is neither.
    if let Some(c) = world
        .objects
        .get_component::<crate::model::components::summons::CubicOf>(&oid)
    {
        return world
            .objects
            .get_component::<crate::model::Player>(&c.owner_object_id)
            .map(|p| p.level)
            .unwrap_or(1);
    }
    if crate::game_loop::combat::is_npc_oid(oid) {
        npc::npc_template(world, oid).map(|t| t.level).unwrap_or(1)
    } else {
        world
            .objects
            .get_component::<crate::model::Player>(&oid)
            .map(|p| p.level)
            .unwrap_or(1)
    }
}

/// Test hook for [`creature_level`], which is private to this module.
#[cfg(test)]
pub(crate) fn creature_level_for_test(world: &World, oid: i32) -> i32 {
    creature_level(world, oid)
}

/// A target creature's display name (Java `Creature.getName()`) for the debuff
/// landed/resisted caster line — an NPC's template name or the player's name.
pub(crate) fn creature_name(world: &World, oid: i32) -> String {
    if crate::game_loop::combat::is_npc_oid(oid) {
        npc::npc_name_or_empty(world, oid)
    } else {
        helpers::player_name_or_empty(world, oid)
    }
}
