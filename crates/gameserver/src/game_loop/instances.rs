//! Instance lifecycle (G27) — create a private world from a template, move
//! players in and out, and tear it down once empty. Java `InstanceManager` +
//! `Instance` (create → spawn groups; addPlayer → enter location; on the last
//! player leaving, the `<time empty>` grace period, then destroy).

use crate::data::instance_data::ExitType;
use crate::game_loop::helpers::instance_of;
use crate::model::components::{InstanceDoorOpen, InstanceId, Position, RegionCell};
use crate::model::door::Door;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::{World, region_of};

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
    spawn_instance_doors(world, instance_id, &template.doors);
    Some(instance_id)
}

/// Spawn this instance's private door copies from the template's doorlist (Java
/// the instance clones its own door instances). Each copy carries its own
/// [`InstanceDoorOpen`] state — starting at the door template's default — so
/// concurrent instances toggle independently of the shared collision grid.
fn spawn_instance_doors(world: &mut World, instance_id: i32, door_ids: &[i32]) {
    for &door_id in door_ids {
        let Some(t) = world.data.door_data.get(door_id) else {
            continue;
        };
        let (x, y, z, hp, default_open) = (t.x, t.y, t.z, t.hp_max, t.open_by_default);
        let object_id = world.next_npc_object_id;
        world.next_npc_object_id += 1;
        let region = region_of(x, y);
        world.objects.spawn(
            object_id,
            (
                Door {
                    object_id,
                    door_id,
                    auto_close_seq: 0,
                    current_hp: hp,
                },
                Position {
                    x,
                    y,
                    z,
                    heading: 0,
                },
                RegionCell(region),
                InstanceId(instance_id),
                InstanceDoorOpen(default_open),
            ),
        );
        world
            .door_regions
            .entry(region)
            .or_default()
            .push(object_id);
        world.instances.record_door(instance_id, object_id);
    }
}

/// Open or close one of an instance's private doors (Java
/// `Instance.openCloseDoor`): flip the copy's own state and broadcast the new
/// look to the instance's players.
pub(crate) fn open_close_door(world: &mut World, instance_id: i32, door_id: i32, open: bool) {
    let Some(&door_oid) = world
        .instances
        .get(instance_id)
        .map(|i| &i.doors)
        .and_then(|doors| {
            doors.iter().find(|&&oid| {
                world
                    .objects
                    .get_component::<Door>(&oid)
                    .is_some_and(|d| d.door_id == door_id)
            })
        })
    else {
        return;
    };
    if let Some(state) = world
        .objects
        .get_component_mut::<InstanceDoorOpen>(&door_oid)
    {
        state.0 = open;
    }
    broadcast_instance_door(world, instance_id, door_oid);
}

/// Push a door copy's `StaticObjectInfo` + `DoorStatusUpdate` to the instance.
fn broadcast_instance_door(world: &World, instance_id: i32, door_oid: i32) {
    let Some(door) = world.objects.get_component::<Door>(&door_oid) else {
        return;
    };
    let Some(t) = world.data.door_data.get(door.door_id) else {
        return;
    };
    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&door_oid)
        .map(|r| r.0)
    else {
        return;
    };
    let open = crate::game_loop::doors::door_open_state(world, door_oid, door.door_id);
    crate::game_loop::helpers::broadcast_near_region_in(
        world,
        region,
        instance_id,
        &server_packets::static_object_info_door(door, t, open),
    );
    crate::game_loop::helpers::broadcast_near_region_in(
        world,
        region,
        instance_id,
        &server_packets::door_status_update(door, t, open),
    );
}

/// Spawn a named (non-`spawnByDefault`) group into a live instance and return
/// the spawned NPC object ids (Java `Instance.spawnGroup`). Each NPC is tagged
/// into the instance and recorded for teardown.
pub(crate) fn spawn_group(world: &mut World, instance_id: i32, group_name: &str) -> Vec<i32> {
    let Some(template_id) = world.instances.get(instance_id).map(|i| i.template_id) else {
        return Vec::new();
    };
    let Some(template) = world.data.instance_templates.get(template_id).cloned() else {
        return Vec::new();
    };
    let mut spawned = Vec::new();
    for group in &template.groups {
        if group.name != group_name {
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
                spawned.push(oid);
            }
        }
    }
    spawned
}

/// Spawn a single NPC into a live instance (Java `addSpawn(..., instanceId)`),
/// returning its object id. Tagged into the instance and recorded for teardown.
pub(crate) fn spawn_npc(
    world: &mut World,
    instance_id: i32,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
) -> Option<i32> {
    let oid = crate::model::npc::spawn_npc_at(world, npc_id, x, y, z, heading)?;
    world.objects.add_components(&oid, InstanceId(instance_id));
    world.instances.record_npc(instance_id, oid);
    Some(oid)
}

/// Send a packet to every player currently inside an instance (Java
/// `broadcastPacket(Instance, packet)` — all members, not region-scoped).
pub(crate) fn broadcast_to_instance(world: &World, instance_id: i32, packet: &[u8]) {
    let Some(inst) = world.instances.get(instance_id) else {
        return;
    };
    for &member in inst.members.keys() {
        if let Some(cid) = crate::game_loop::helpers::client_for_player(world, member)
            && let Some(cs) = world.clients.get(&cid)
        {
            cs.send(packet.to_vec());
        }
    }
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
            .get_component::<RegionCell>(&npc_oid)
            .map(|r| r.0)
            .unwrap_or((0, 0));
        crate::game_loop::death::despawn_npc(world, npc_oid, region);
    }
    let doors = world
        .instances
        .get(instance_id)
        .map(|i| i.doors.clone())
        .unwrap_or_default();
    for door_oid in doors {
        despawn_instance_door(world, instance_id, door_oid);
    }
    world.instances.destroy(instance_id);
}

/// Remove one instance door copy: DeleteObject to the instance, drop it from the
/// region index, and despawn the entity.
fn despawn_instance_door(world: &mut World, instance_id: i32, door_oid: i32) {
    let region = world
        .objects
        .get_component::<RegionCell>(&door_oid)
        .map(|r| r.0)
        .unwrap_or((0, 0));
    crate::game_loop::helpers::broadcast_near_region_in(
        world,
        region,
        instance_id,
        &server_packets::delete_object(door_oid),
    );
    if let Some(ids) = world.door_regions.get_mut(&region) {
        ids.retain(|&id| id != door_oid);
    }
    world.objects.despawn(&door_oid);
}

fn position_of(world: &World, object_id: i32) -> (i32, i32, i32) {
    world
        .objects
        .get_component::<Position>(&object_id)
        .map(|p| (p.x, p.y, p.z))
        .unwrap_or((0, 0, 0))
}
