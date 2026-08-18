//! Position, movement-stop and region-cell helpers.

use super::*;

/// The world coordinates of any object carrying a [`Position`], or `None` if
/// it has despawned.
///
/// Delegates to the geo layer's own accessor so there is exactly one
/// implementation; `crate::geo` cannot depend on `game_loop`, so the
/// definition has to live down there.
pub(crate) fn pos_of(world: &World, object_id: i32) -> Option<(i32, i32, i32)> {
    crate::geo::distance::position_of(world, object_id)
}

/// Java `Creature.setXYZ` — put an object at `(x, y, z)` by writing its
/// [`Position`] outright.
///
/// **Teleport semantics, not movement.** This changes where the object *is* and
/// nothing else: no region re-index, no knownlist update, no packet. Every
/// caller pairs it with the rest — `set_player_region` /
/// `visibility::update_npc_region`, a `TeleportToLocation` or `FlyToLocation`
/// broadcast, sometimes an instance change — and dropping one of those is how
/// an object ends up visible to the wrong people or invisible to everyone.
/// [`crate::game_loop::position`] is where movement the world watches happen
/// lives.
///
/// A no-op for an object that has left the world.
pub(crate) fn set_position(world: &mut World, object_id: i32, (x, y, z): (i32, i32, i32)) {
    if let Some(p) = world
        .objects
        .get_component_mut::<model::components::Position>(&object_id)
    {
        p.x = x;
        p.y = y;
        p.z = z;
    }
}

/// [`set_position`] that also faces the object — Java's
/// `setXYZ` + `setHeading` pair, which the respawn and summon paths do
/// together because a creature placed without a heading faces due east.
pub(crate) fn set_position_heading(
    world: &mut World,
    object_id: i32,
    (x, y, z): (i32, i32, i32),
    heading: i32,
) {
    if let Some(p) = world
        .objects
        .get_component_mut::<model::components::Position>(&object_id)
    {
        p.x = x;
        p.y = y;
        p.z = z;
        p.heading = heading;
    }
}

/// Halt a creature mid-path and tell everyone where it stopped — Java
/// `Creature.stopMove` followed by the `StopMove` broadcast.
///
/// A no-op for anything that isn't currently moving. Every intent that
/// interrupts a walk (attack, cast, sit, target change) opens with this.
pub(crate) fn stop_movement(world: &mut World, object_id: i32) {
    if !world.objects.has_component::<Movement>(&object_id) {
        return;
    }
    world.objects.remove_component::<Movement>(&object_id);
    if let Some(pos) = maybe_position(world, object_id) {
        broadcast_including_self(
            world,
            object_id,
            &server_packets::stop_move(object_id, pos.x, pos.y, pos.z, pos.heading),
        );
    }
}

/// The region cell an object is binned into, or `None` once it has left the
/// world.
///
/// The key for [`broadcast_near_region`] and the visibility grids — almost
/// every caller feeds the answer straight to one of those.
///
/// Distinct from [`crate::world::region_of`], which derives a region from raw
/// coordinates; this reads the cell the object is actually registered in.
pub(crate) fn region_cell_of(world: &World, object_id: i32) -> Option<(i32, i32)> {
    world
        .objects
        .get_component::<RegionCell>(&object_id)
        .map(|r| r.0)
}
