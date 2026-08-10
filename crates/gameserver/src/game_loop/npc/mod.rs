use crate::world::World;

pub(crate) mod ai;
pub(crate) mod cast;
pub mod view;

pub(crate) fn lvl_of_npc(world: &World, object_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&object_id)
        .and_then(|n| world.data.npc_data.get(n.npc_id))
        .map(|t| t.level)
}
