//! The `StaticObject` world object (Java `model/actor/instance/
//! StaticObject`) — G12's decoration slice: town map panels and castle
//! thrones spawn at boot and render via `StaticObjectInfo`. No click
//! behavior (the town-map dialog and throne sitting are gated on community
//! board / castle systems).

use bevy_ecs::component::Component;

use crate::model::components::space::{Position, RegionCell};
use crate::world::{World, region_of};

#[derive(Debug, Clone, Component)]
pub struct StaticObj {
    pub object_id: i32,
    /// Template id (`StaticObjects.xml` `id`), the packet's staticObjectId.
    pub static_id: i32,
}

/// Spawn every `StaticObjects.xml` entry as a world entity (Java
/// `StaticObjectData.load` → `initObject`).
pub fn spawn_static_objects(world: &mut World) -> usize {
    let mut placed = 0;
    for i in 0..world.data.static_object_data.objects.len() {
        let t = &world.data.static_object_data.objects[i];
        let (static_id, x, y, z) = (t.id, t.x, t.y, t.z);
        let object_id = world.next_npc_object_id;
        world.next_npc_object_id += 1;
        let region = region_of(x, y);
        world.objects.spawn(
            object_id,
            (
                StaticObj {
                    object_id,
                    static_id,
                },
                Position {
                    x,
                    y,
                    z,
                    heading: 0,
                },
                RegionCell(region),
            ),
        );
        world
            .static_regions
            .entry(region)
            .or_default()
            .push(object_id);
        placed += 1;
    }
    if placed > 0 {
        tracing::info!("StaticObjectData: Spawned {placed} static objects.");
    }
    placed
}
