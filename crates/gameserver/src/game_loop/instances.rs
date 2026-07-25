//! Instance lifecycle (G27) — create a private world from a template, move
//! players in and out, and tear it down once empty. Java `InstanceManager` +
//! `Instance` (create → spawn groups; addPlayer → enter location; on the last
//! player leaving, the `<time empty>` grace period, then destroy).

use crate::data::instance_data::ExitType;
use crate::game_loop::helpers::instance_of;
use crate::model::components::{InstanceId, Position};
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// `InstanceManager.createInstance` from a template: allocate the instance and
/// spawn its default groups into it. Returns the new instance id, or `None` if
/// the template is unknown.
pub(crate) fn create_from_template(world: &mut World, template_id: i32) -> Option<i32> {
    let template = world.data.instance_templates.get(template_id)?.clone();
    let instance_id = world.instances.create(template_id);
    for group in &template.groups {
        if !group.spawn_by_default {
            continue;
        }
        for spawn in &group.npcs {
            if let Some(oid) = crate::model::npc::spawn_npc_at(
                world,
                spawn.npc_id,
                spawn.x,
                spawn.y,
                spawn.z,
                spawn.heading,
            ) {
                world.objects.add_components(&oid, InstanceId(instance_id));
                world.instances.record_npc(instance_id, oid);
            }
        }
    }
    // TODO(G27): open the instance's doors (needs per-instance door state).
    Some(instance_id)
}

/// Move a player into `instance_id` (Java `Instance.addPlayer` + the enter
/// location), remembering where they came from for an `ORIGIN` exit.
pub(crate) fn enter(world: &mut World, player: i32, instance_id: i32) {
    let Some(template_id) = world.instances.get(instance_id).map(|i| i.template_id) else {
        return;
    };
    let ret = position_of(world, player);
    world.instances.add_member(instance_id, player, ret);
    world
        .objects
        .add_components(&player, InstanceId(instance_id));
    if let Some(loc) = world
        .data
        .instance_templates
        .get(template_id)
        .and_then(|t| t.enter)
    {
        crate::game_loop::death::teleport_player(world, player, loc.0, loc.1, loc.2);
    }
}

/// Send a player out of their instance (Java the exit location: `ORIGIN` sends
/// them back to where they entered, a fixed exit to that spot). Arms the
/// empty-destroy timer when the last member leaves.
pub(crate) fn exit(world: &mut World, player: i32) {
    let instance_id = instance_of(world, player);
    if instance_id == 0 {
        return;
    }
    let template_id = world
        .instances
        .get(instance_id)
        .map_or(0, |i| i.template_id);
    let ret = world
        .instances
        .remove_member(instance_id, player, world.tick);
    world.objects.remove_component::<InstanceId>(&player);

    let dest = match world
        .data
        .instance_templates
        .get(template_id)
        .map(|t| t.exit)
    {
        Some(ExitType::Fixed(x, y, z)) => Some((x, y, z)),
        _ => ret, // ORIGIN, or a bare instance → back where they came from
    };
    if let Some((x, y, z)) = dest {
        crate::game_loop::death::teleport_player(world, player, x, y, z);
    }

    if world.instances.member_count(instance_id) == 0 {
        let empty_min = world
            .data
            .instance_templates
            .get(template_id)
            .map_or(0, |t| t.empty_destroy_min);
        // minutes → ticks (10 ticks/s). A zero grace tears down next tick.
        let delay = (empty_min.max(0) as u64) * 600;
        world.scheduler.schedule(
            world.tick + delay.max(1),
            ScheduledTask::InstanceEmptyCheck { instance_id },
        );
    }
}

/// `InstanceEmptyCheck`: destroy the instance if it is still empty (a member may
/// have re-entered during the grace period).
pub(crate) fn handle_empty_check(world: &mut World, instance_id: i32) {
    if world.instances.member_count(instance_id) == 0 {
        destroy(world, instance_id);
    }
}

/// Tear an instance down: oust any members still inside, despawn its NPCs, and
/// drop its bookkeeping (Java `Instance.destroy` teleports players out first).
pub(crate) fn destroy(world: &mut World, instance_id: i32) {
    let members: Vec<i32> = world
        .instances
        .get(instance_id)
        .map(|i| i.members.keys().copied().collect())
        .unwrap_or_default();
    for member in members {
        exit(world, member);
    }
    let npcs = world
        .instances
        .get(instance_id)
        .map(|i| i.npcs.clone())
        .unwrap_or_default();
    for npc_oid in npcs {
        let region = world
            .objects
            .get_component::<crate::model::components::RegionCell>(&npc_oid)
            .map(|r| r.0)
            .unwrap_or((0, 0));
        crate::game_loop::death::despawn_npc(world, npc_oid, region);
    }
    world.instances.destroy(instance_id);
}

fn position_of(world: &World, object_id: i32) -> (i32, i32, i32) {
    world
        .objects
        .get_component::<Position>(&object_id)
        .map(|p| (p.x, p.y, p.z))
        .unwrap_or((0, 0, 0))
}
