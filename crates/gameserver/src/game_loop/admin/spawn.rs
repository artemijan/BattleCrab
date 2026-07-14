//! NPC spawn commands — `AdminSpawn`'s `//spawn` and `AdminDelete`'s `//delete`.

use crate::model::components::{Position, RegionCell};
use crate::world::World;

use super::{current_target, send_message};

/// `AdminSpawn`'s `//spawn <npcId>` — spawn one NPC at the GM's location with
/// no respawn (the permanent/respawn forms are TODO).
pub(super) fn admin_spawn(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(npc_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //spawn <npcId>");
        return;
    };
    if world.data.npc_data.get(npc_id).is_none() {
        send_message(world, client_id, &format!("NPC id {npc_id} does not exist."));
        return;
    }
    let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    if let Some(spawned) = crate::model::npc::spawn_npc_at(world, npc_id, pos.x, pos.y, pos.z, pos.heading) {
        super::death::introduce_npc(world, spawned);
    }
}

/// `AdminDelete`'s `//delete` — despawn the targeted NPC.
pub(super) fn admin_delete(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id) else {
        send_message(world, client_id, "Select an NPC first.");
        return;
    };
    if !world.objects.has_component::<crate::model::npc::Npc>(&target) {
        send_message(world, client_id, "Target is not an NPC.");
        return;
    }
    let Some(region) = world.objects.get_component::<RegionCell>(&target).map(|r| r.0)
    else {
        return;
    };
    super::death::despawn_npc(world, target, region);
}
