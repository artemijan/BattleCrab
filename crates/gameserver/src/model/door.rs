//! The `Door` world object (Java `model/actor/instance/Door`), scoped to
//! G12's static-world slice: every `DoorData.xml` door spawns at boot as an
//! entity in `World.objects`, renders to clients (`StaticObjectInfo` +
//! `DoorStatusUpdate`), blocks LOS/movement while closed (the collision
//! side lives in `geo::doors::DoorGrid`, registered before the engine is
//! shared), and opens/closes through `game_loop/doors.rs` — BY_TIME cycles
//! at boot, everything else when a script/system calls open/close. Not
//! ported: door HP/attackable doors (sieges), click-to-target (`DoorAction`
//! only matters for clan-hall owners), `masterClose` cascades, emitters.

use bevy_ecs::component::Component;

use crate::model::components::{Position, RegionCell};
use crate::world::{region_of, World};

/// The door residual core: identity + task bookkeeping. Open state is *not*
/// here — the `geo::doors::DoorGrid` atomic is the single source of truth
/// (the geo queries on the path worker read it too).
#[derive(Debug, Clone, Component)]
pub struct Door {
    pub object_id: i32,
    /// Template id (`world.data.door_data.get(door_id)`).
    pub door_id: i32,
    /// Generation guard for the auto-close task (bumped by every open/close,
    /// like Java cancelling `_autoCloseTask`).
    pub auto_close_seq: u64,
}

/// Spawn every loaded door template as a world entity (Java `DoorData`'s
/// `spawnDoor` pass at boot) and index them by region for visibility. The
/// collision grid (`world.geo.doors`) was populated at boot in `main.rs`;
/// tests that skip that just get non-blocking doors.
pub fn spawn_doors(world: &mut World) -> usize {
    let mut placed = 0;
    for i in 0..world.data.door_data.doors.len() {
        let t = &world.data.door_data.doors[i];
        let (door_id, x, y, z) = (t.id, t.x, t.y, t.z);
        let object_id = world.next_npc_object_id;
        world.next_npc_object_id += 1;

        let region = region_of(x, y);
        world.objects.spawn(
            object_id,
            (
                Door { object_id, door_id, auto_close_seq: 0 },
                Position { x, y, z, heading: 0 },
                RegionCell(region),
            ),
        );
        world.door_regions.entry(region).or_default().push(object_id);
        placed += 1;
    }
    if placed > 0 {
        tracing::info!("DoorData: Spawned {placed} doors.");
    }
    placed
}

/// Test hook: spawn one synthetic door (template + entity + collision shape).
#[doc(hidden)]
pub fn spawn_door_for_test(world: &mut World, template: crate::data::door_data::DoorTemplate) -> i32 {
    // Collision registration needs a uniquely-held engine (tests only).
    if let Some(geo) = std::sync::Arc::get_mut(&mut world.geo) {
        geo.doors.register(
            template.id,
            template.node_x,
            template.node_y,
            template.z_min(),
            template.z_max(),
            template.open_by_default,
        );
    }
    world.data.door_data.insert_for_test(template);
    let idx = world.data.door_data.doors.len() - 1;
    let t = &world.data.door_data.doors[idx];
    let (door_id, x, y, z) = (t.id, t.x, t.y, t.z);
    let object_id = world.next_npc_object_id;
    world.next_npc_object_id += 1;
    let region = region_of(x, y);
    world.objects.spawn(
        object_id,
        (Door { object_id, door_id, auto_close_seq: 0 }, Position { x, y, z, heading: 0 }, RegionCell(region)),
    );
    world.door_regions.entry(region).or_default().push(object_id);
    object_id
}
