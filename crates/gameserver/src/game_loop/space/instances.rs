//! Instance lifecycle (G27) — create a private world from a template, move
//! players in and out, and tear it down once empty. Java `InstanceManager` +
//! `Instance` (create → spawn groups; addPlayer → enter location; on the last
//! player leaving, the `<time empty>` grace period, then destroy).

use crate::data::instance_data::ExitType;
use crate::game_loop::helpers::instance_of;
use crate::game_loop::net::broadcast;
use crate::game_loop::space::position::pos_of;
use crate::game_loop::space::position::region_cell_of;
use crate::model::components::space::{
    InstanceDoorOpen, InstanceId, InstanceSpawn, Position, RegionCell,
};
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
    // `new Instance`: duration, then doors, then the default spawns.
    set_duration(world, instance_id, template.duration_min);
    spawn_instance_doors(world, instance_id, &template.doors);
    for (group_idx, group) in template.groups.iter().enumerate() {
        if !group.spawn_by_default {
            continue;
        }
        for npc_idx in 0..group.npcs.len() {
            spawn_template_npc(world, instance_id, group_idx, npc_idx);
        }
    }
    Some(instance_id)
}

/// One template spawn line into a live instance, tagged with its line so a
/// `respawnTime` brings it back here. A no-op once the instance is gone.
pub(crate) fn spawn_template_npc(
    world: &mut World,
    instance_id: i32,
    group_idx: usize,
    npc_idx: usize,
) -> Option<i32> {
    let template_id = world.instances.get(instance_id)?.template_id;
    let spawn = world
        .data
        .instance_templates
        .get(template_id)?
        .groups
        .get(group_idx)?
        .npcs
        .get(npc_idx)?
        .clone();
    let oid = crate::game_loop::npc::spawn_npc_at(
        world,
        spawn.npc_id,
        spawn.x,
        spawn.y,
        spawn.z,
        spawn.heading,
    )?;
    world.objects.add_components(&oid, InstanceId(instance_id));
    world
        .objects
        .add_components(&oid, InstanceSpawn { group_idx, npc_idx });
    world.instances.record_npc(instance_id, oid);
    // `Spawn.doSpawn` shows the NPC to whoever is already inside.
    crate::game_loop::npc::introduce_npc(world, oid);
    Some(oid)
}

/// `Spawn.decreaseCount` for an instance spawn line: schedule the respawn
/// back into the same instance when the line has a `respawnTime`.
pub(crate) fn schedule_respawn(world: &mut World, npc_oid: i32) {
    let instance_id = instance_of(world, npc_oid);
    let Some(&InstanceSpawn { group_idx, npc_idx }) =
        world.objects.get_component::<InstanceSpawn>(&npc_oid)
    else {
        return;
    };
    let Some(respawn_secs) = world
        .instances
        .get(instance_id)
        .and_then(|i| world.data.instance_templates.get(i.template_id))
        .and_then(|t| t.groups.get(group_idx))
        .and_then(|g| g.npcs.get(npc_idx))
        .map(|n| n.respawn_secs)
        .filter(|&s| s > 0)
    else {
        return;
    };
    world.scheduler.schedule(
        world.tick + respawn_secs as u64 * 10,
        ScheduledTask::InstanceNpcRespawn {
            instance_id,
            group_idx,
            npc_idx,
        },
    );
}

/// `Instance.setDuration`: a negative duration never ends; under a minute
/// destroys at once; otherwise the end is set, the warning sent when it is at
/// most five minutes off, and the clean-up armed one minute (≤ 5 min) or five
/// minutes before the end.
pub(crate) fn set_duration(world: &mut World, instance_id: i32, minutes: i32) {
    if minutes < 0 {
        if let Some(inst) = world.instances.get_mut(instance_id) {
            inst.end_tick = None;
        }
        return;
    }
    let tick = world.tick;
    let Some(seq) = world.instances.get_mut(instance_id).map(|inst| {
        inst.cleanup_seq += 1;
        inst.end_tick = Some(tick + minutes as u64 * 600);
        inst.cleanup_seq
    }) else {
        return;
    };
    if minutes < 1 {
        destroy(world, instance_id);
        return;
    }
    send_world_destroy_message(world, instance_id, minutes);
    let lead = if minutes <= 5 { 1 } else { 5 };
    world.scheduler.schedule(
        tick + (minutes - lead) as u64 * 600,
        ScheduledTask::InstanceCleanUp { instance_id, seq },
    );
}

/// `Instance.cleanUp`.
pub(crate) fn handle_clean_up(world: &mut World, instance_id: i32, seq: u64) {
    let Some(end) = world
        .instances
        .get(instance_id)
        .filter(|i| i.cleanup_seq == seq)
        .and_then(|i| i.end_tick)
    else {
        return;
    };
    if end.saturating_sub(world.tick) <= 600 {
        send_world_destroy_message(world, instance_id, 1);
        world.scheduler.schedule(
            world.tick + 600,
            ScheduledTask::InstanceDestroy { instance_id, seq },
        );
    } else {
        send_world_destroy_message(world, instance_id, 5);
        world.scheduler.schedule(
            world.tick + 5 * 600,
            ScheduledTask::InstanceCleanUp { instance_id, seq },
        );
    }
}

/// The destroy `cleanUp` armed, unless a later `setDuration` replaced it.
pub(crate) fn handle_timed_destroy(world: &mut World, instance_id: i32, seq: u64) {
    if world
        .instances
        .get(instance_id)
        .is_some_and(|i| i.cleanup_seq == seq)
    {
        destroy(world, instance_id);
    }
}

/// `Instance.sendWorldDestroyMessage`: SM 2107, only at five minutes or less.
fn send_world_destroy_message(world: &World, instance_id: i32, minutes: i32) {
    if minutes > 5 {
        return;
    }
    let packet = server_packets::system_message_with(
        server_packets::sm_ids::THIS_INSTANT_ZONE_WILL_BE_TERMINATED_IN_S1_MIN_YOU_WILL_BE_FORCED_OUT_OF_THE_DUNGEON_WHEN_THE_TIME_EXPIRES,
        &[server_packets::SmParam::Int(minutes)],
    );
    broadcast_to_instance(world, instance_id, &packet);
}

/// `Instance.finishInstance(delay)`. No template this port enters declares a
/// `<reenter>`, so the `ON_FINISH` re-entry stamp has nothing to write.
pub(crate) fn finish_instance(world: &mut World, instance_id: i32, delay_min: i32) {
    set_duration(world, instance_id, delay_min);
}

/// Spawn this instance's private door copies from the template's doorlist (Java
/// the instance clones its own door instances). Each copy carries its own
/// [`InstanceDoorOpen`] state — starting at the door template's default — so
/// concurrent instances toggle independently of the shared collision grid.
fn spawn_instance_doors(
    world: &mut World,
    instance_id: i32,
    doors: &[crate::data::instance_data::TemplateDoor],
) {
    for door in doors {
        let door_id = door.id;
        let Some(t) = world.data.door_data.get(door_id) else {
            continue;
        };
        // The instance node's `<openStatus>` merges over the door template's.
        let default_open = door.open_by_default.unwrap_or(t.open_by_default);
        let (x, y, z, hp) = (t.x, t.y, t.z, t.hp_max);
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
    let Some(region) = region_cell_of(world, door_oid) else {
        return;
    };
    let open = crate::game_loop::npc::doors::door_open_state(world, door_oid, door.door_id);
    broadcast::broadcast_near_region_in(
        world,
        region,
        instance_id,
        &server_packets::static_object_info_door(door, t, open),
    );
    broadcast::broadcast_near_region_in(
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
    for (group_idx, group) in template.groups.iter().enumerate() {
        if group.name != group_name {
            continue;
        }
        for npc_idx in 0..group.npcs.len() {
            spawned.extend(spawn_template_npc(world, instance_id, group_idx, npc_idx));
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
    let oid = crate::game_loop::npc::spawn_npc_at(world, npc_id, x, y, z, heading)?;
    world.objects.add_components(&oid, InstanceId(instance_id));
    world.instances.record_npc(instance_id, oid);
    crate::game_loop::npc::introduce_npc(world, oid);
    Some(oid)
}

/// Send a packet to every player currently inside an instance (Java
/// `broadcastPacket(Instance, packet)` — all members, not region-scoped).
pub(crate) fn broadcast_to_instance(world: &World, instance_id: i32, packet: &[u8]) {
    let Some(inst) = world.instances.get(instance_id) else {
        return;
    };
    for &member in inst.members.keys() {
        crate::game_loop::helpers::send_to_player(world, member, packet.to_vec());
    }
}

/// Move a player into `instance_id` (Java `Instance.addPlayer` + the enter
/// location), remembering where they came from for an `ORIGIN` exit.
pub(crate) fn enter(world: &mut World, player: i32, instance_id: i32) {
    let Some(template_id) = world.instances.get(instance_id).map(|i| i.template_id) else {
        return;
    };
    // A player with no Position cannot have entered; the origin fallback is
    // unreachable in practice and only keeps the return location total.
    let ret = pos_of(world, player).unwrap_or((0, 0, 0));
    world.instances.add_member(instance_id, player, ret);
    world
        .objects
        .add_components(&player, InstanceId(instance_id));
    let loc = world
        .data
        .instance_templates
        .get(template_id)
        .cloned()
        .and_then(|t| t.enter_location(|n| world.roll(n)));
    if let Some(loc) = loc {
        crate::game_loop::death::teleport_player(world, player, loc.0, loc.1, loc.2);
    }
}

/// Drop `player` out of `instance_id` and answer where they belong now:
/// a `Fixed` exit's spot, or — for `ORIGIN`, and for a bare instance with no
/// template — wherever they entered from. Returns the template id alongside it,
/// since the caller that arms the empty-destroy timer needs it too.
///
/// Both exit paths do exactly this; only what they do with the answer differs.
/// [`exit`] teleports, because there is a client to tell. The logout path just
/// rewrites the stored position, because there is not.
fn leave_instance(
    world: &mut World,
    player: i32,
    instance_id: i32,
) -> (i32, Option<(i32, i32, i32)>) {
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
        _ => ret,
    };
    (template_id, dest)
}

/// Send a player out of their instance (Java the exit location: `ORIGIN` sends
/// them back to where they entered, a fixed exit to that spot). Arms the
/// empty-destroy timer when the last member leaves.
pub(crate) fn exit(world: &mut World, player: i32) {
    let instance_id = instance_of(world, player);
    if instance_id == 0 {
        return;
    }
    let (template_id, dest) = leave_instance(world, player, instance_id);
    if let Some((x, y, z)) = dest {
        crate::game_loop::death::teleport_player(world, player, x, y, z);
    }

    if world.instances.member_count(instance_id) == 0 {
        // `Instance.removePlayer`: a zero duration or zero empty time tears
        // down at once (next tick here), a positive empty time after that
        // grace, and -1 never. A bare instance has no template and goes at once.
        let (duration_min, empty_min) = world
            .data
            .instance_templates
            .get(template_id)
            .map_or((0, 0), |t| (t.duration_min, t.empty_destroy_min));
        let delay = if duration_min == 0 || empty_min == 0 {
            Some(0)
        } else if empty_min > 0 {
            Some(empty_min as u64 * 600)
        } else {
            None
        };
        if let Some(delay) = delay {
            world.scheduler.schedule(
                world.tick + delay.max(1),
                ScheduledTask::InstanceEmptyCheck { instance_id },
            );
        }
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
        crate::game_loop::npc::despawn_npc_by_oid(world, npc_oid);
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

/// `Instance.removeNpcs`: despawn every NPC of the instance and forget them.
pub(crate) fn remove_npcs(world: &mut World, instance_id: i32) {
    let npcs = world
        .instances
        .get_mut(instance_id)
        .map(|i| std::mem::take(&mut i.npcs))
        .unwrap_or_default();
    for npc_oid in npcs {
        crate::game_loop::npc::despawn_npc_by_oid(world, npc_oid);
    }
}

/// Remove one instance door copy: DeleteObject to the instance, drop it from the
/// region index, and despawn the entity.
fn despawn_instance_door(world: &mut World, instance_id: i32, door_oid: i32) {
    let region = region_cell_of(world, door_oid).unwrap_or((0, 0));
    broadcast::broadcast_near_region_in(
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

/// `Instance.onDeath`: start the eject clock for a player who died inside an
/// instance.
///
/// Java warns first (`IF_YOU_ARE_NOT_RESURRECTED_WITHIN_S1_MINUTE_S…`) and
/// schedules a task that re-checks `isDead()` when it fires — so a resurrection
/// cancels the eject implicitly rather than by cancelling the task. The port
/// keeps that shape: the scheduled task re-reads the death state, which is also
/// why nothing has to be cancelled when the player is raised, teleported out,
/// or logs off.
///
/// `EjectDeadPlayerTime` is the **default** for `InstanceTemplate.ejectTime`;
/// Java lets a template override it with `<ejectTime>`, and no instance on this
/// dist does, so the configured value is the only one in play. `0` disables the
/// eject, matching Java's `> 0` guard.
pub(crate) fn arm_eject_on_death(world: &mut World, player: i32) {
    let instance_id = instance_of(world, player);
    if instance_id == 0 {
        return;
    }
    let minutes = world.cfg.general.eject_dead_player_time_min;
    if minutes <= 0 {
        return;
    }
    crate::game_loop::helpers::send_sm_to_player(
        world,
        player,
        crate::network::server_packets::sm_ids::IF_YOU_ARE_NOT_RESURRECTED_WITHIN_S1_MINUTES_YOU_WILL_BE_EXPELLED,
        &[crate::network::server_packets::SmParam::Int(minutes)],
    );
    world.scheduler.schedule(
        world.tick + (minutes as u64) * 600,
        ScheduledTask::InstanceEjectDead {
            player_object_id: player,
        },
    );
}

/// `Instance.onDeath`'s scheduled body: expel the player **if still dead**.
///
/// The liveness re-check is the whole cancellation mechanism — Java never
/// cancels this task either, it just finds `isDead()` false and does nothing.
pub(crate) fn handle_eject_dead(world: &mut World, player: i32) {
    if !crate::game_loop::helpers::is_dead(world, player) {
        return;
    }
    if instance_of(world, player) == 0 {
        return;
    }
    exit(world, player);
}

/// Java `PlayerVariables.INSTANCE_RESTORE`.
const INSTANCE_RESTORE_VAR: &str = "INSTANCE_RESTORE";

/// `Instance.onPlayerLogout` — the half that decides where a player who logs
/// out inside an instance wakes up.
///
/// `RestorePlayerInstance` **on** (this dist): record the instance id in a
/// player variable and leave their position alone, so the next login can put
/// them back. **Off**: Java moves them to the instance's exit location, because
/// the instance will not be there when they return and the stored coordinates
/// would otherwise drop them into empty geometry.
///
/// Note this is *not* `exit()` — Java calls `removePlayer` and sets the
/// location directly, without the teleport packets or the empty-destroy arming
/// that a live exit does. The player is leaving the world in the same breath.
pub(crate) fn on_player_logout(world: &mut World, player: i32) {
    let instance_id = instance_of(world, player);
    if instance_id == 0 {
        return;
    }
    if world.cfg.general.restore_player_instance {
        if let Some(vars) = world
            .objects
            .get_component_mut::<crate::model::components::player::PlayerVariables>(&player)
        {
            vars.0
                .insert(INSTANCE_RESTORE_VAR.to_string(), instance_id.to_string());
        }
        return;
    }
    // Off: park them at the exit location so the stored position is somewhere
    // that still exists next time.
    let (_, dest) = leave_instance(world, player, instance_id);
    if let Some((x, y, z)) = dest
        && let Some(pos) = world
            .objects
            .get_component_mut::<crate::model::components::space::Position>(&player)
    {
        pos.x = x;
        pos.y = y;
        pos.z = z;
    }
}

/// `EnterWorld`'s restore half: put the player back into the instance they
/// logged out of, **if it is still running**.
///
/// Java guards on `instance.getId() == vars.getInt(INSTANCE_RESTORE, 0)` and
/// clears the variable either way — so a stale id from an instance that has
/// since been destroyed is consumed and discarded rather than retried forever.
pub(crate) fn restore_on_login(world: &mut World, player: i32) {
    let stored = world
        .objects
        .get_component::<crate::model::components::player::PlayerVariables>(&player)
        .and_then(|v| v.0.get(INSTANCE_RESTORE_VAR).cloned());
    let Some(raw) = stored else { return };
    // Consumed whatever happens, matching Java's unconditional `vars.remove`.
    if let Some(vars) = world
        .objects
        .get_component_mut::<crate::model::components::player::PlayerVariables>(&player)
    {
        vars.0.remove(INSTANCE_RESTORE_VAR);
    }
    if !world.cfg.general.restore_player_instance {
        return;
    }
    let Ok(instance_id) = raw.parse::<i32>() else {
        return;
    };
    if world.instances.get(instance_id).is_none() {
        return;
    }
    let ret = pos_of(world, player).unwrap_or((0, 0, 0));
    world.instances.add_member(instance_id, player, ret);
    world
        .objects
        .add_components(&player, InstanceId(instance_id));
}

/// [`instance_of`] for the config tests, which assert membership directly.
#[cfg(test)]
pub(crate) fn instance_of_for_test(world: &World, player: i32) -> i32 {
    instance_of(world, player)
}

/// Java `GroupType` masks.
const GROUP_NONE: i32 = 1;
const GROUP_PARTY: i32 = 2;
const GROUP_COMMAND_CHANNEL: i32 = 4;

/// `InstanceTemplate.setConditions`' group mask: who may enter together,
/// from the `GroupMin`/`GroupMax`/`CommandChannel` conditions (1 and 1 when
/// there are none — solo).
fn group_mask(template: &crate::data::instance_data::InstanceTemplate, party_size: i32) -> i32 {
    let mut only_cc = false;
    let (mut min, mut max) = (1, 1);
    for c in &template.conditions {
        match c.kind.as_str() {
            "CommandChannel" => only_cc = true,
            "GroupMin" => min = c.int_param("limit").unwrap_or(min),
            "GroupMax" => max = c.int_param("limit").unwrap_or(max),
            _ => {}
        }
    }
    let mut mask = 0;
    if !only_cc {
        if min == 1 {
            mask |= GROUP_NONE;
        }
        if (max > 1 && max <= party_size) || (min <= party_size && max > party_size) {
            mask |= GROUP_PARTY;
        }
    }
    if only_cc || max > 7 {
        mask |= GROUP_COMMAND_CHANNEL;
    }
    mask
}

/// `InstanceTemplate.getEnterGroup`: the requester first, then the rest of
/// the party or command channel the mask lets in. `None` when no group type
/// fits.
fn enter_group(
    world: &World,
    template: &crate::data::instance_data::InstanceTemplate,
    player: i32,
) -> Option<Vec<i32>> {
    let mask = group_mask(template, world.cfg.character.alt_party_max_members as i32);
    if mask == 0 {
        return None;
    }
    let party = world
        .objects
        .get_component::<crate::model::components::social::PartyRef>(&player)
        .map(|r| r.0);
    let cc = party.and_then(|p| crate::game_loop::party::command_channel::cc_id_of_party(world, p));
    // `Player.getGroupType`.
    let own = match (party, cc) {
        (Some(_), Some(_)) => GROUP_COMMAND_CHANNEL,
        (Some(_), None) => GROUP_PARTY,
        _ => GROUP_NONE,
    };
    // `getEnterGroupType`: a GM enters alone; else the player's own group if
    // allowed, the mask's only group, or the nearest allowed one.
    let kind = if crate::game_loop::helpers::is_gm(world, player) {
        GROUP_NONE
    } else if mask & own != 0 {
        own
    } else if [GROUP_NONE, GROUP_PARTY, GROUP_COMMAND_CHANNEL].contains(&mask) {
        mask
    } else {
        [GROUP_NONE, GROUP_PARTY, GROUP_COMMAND_CHANNEL]
            .into_iter()
            .find(|&t| t != own && mask & t != 0)?
    };
    let mut group = vec![player];
    let others = match kind {
        GROUP_PARTY => party
            .and_then(|_| crate::game_loop::party::party_members(world, player))
            .unwrap_or_default(),
        GROUP_COMMAND_CHANNEL => cc
            .map(|cc| crate::game_loop::party::command_channel::cc_members(world, cc))
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    group.extend(others.into_iter().filter(|&m| m != player));
    Some(group)
}

/// `InstanceTemplate.validateConditions` for the condition types the ported
/// instances declare. `Level` sends SM 2097 to the whole group naming the
/// member who failed. Any other type refuses entry: its Java test is not
/// ported, and letting the group through would skip it.
fn validate_conditions(
    world: &World,
    template: &crate::data::instance_data::InstanceTemplate,
    group: &[i32],
) -> bool {
    for c in &template.conditions {
        match c.kind.as_str() {
            "Level" => {
                let max_level = world.cfg.character.maximum_player_level;
                let min = c.int_param("min").unwrap_or(1).min(max_level);
                let max = c.int_param("max").unwrap_or(i32::MAX).min(max_level);
                let members: &[i32] = if c.only_leader { &group[..1] } else { group };
                for &member in members {
                    let level = crate::game_loop::helpers::level_of(world, member).unwrap_or(0);
                    if level >= min && level <= max {
                        continue;
                    }
                    let name =
                        crate::game_loop::helpers::player_name(world, member).unwrap_or_default();
                    let to: &[i32] = if c.only_leader { &[member] } else { group };
                    for &p in to {
                        crate::game_loop::helpers::send_sm_to_player(
                            world,
                            p,
                            server_packets::sm_ids::C1_S_LEVEL_DOES_NOT_CORRESPOND_TO_THE_REQUIREMENTS_FOR_ENTRY,
                            &[server_packets::SmParam::Text(name.clone())],
                        );
                    }
                    return false;
                }
            }
            // Read by `group_mask`; their own tests are not ported.
            other => {
                tracing::warn!(
                    "Instance {}: entry condition {other} is not ported; entry refused.",
                    template.id
                );
                return false;
            }
        }
    }
    true
}

/// `InstanceScript.enterInstance`: re-enter the instance that already lets the
/// player in, or create one from `template_id` for the player's enter group
/// and move them all in. Returns the instance entered.
///
/// No template a ported script enters declares `<reenter>`, so the
/// `C1_MAY_NOT_RE_ENTER_YET` check has no stored times to read, and none of
/// their conditions has a success effect to apply.
pub(crate) fn enter_instance(world: &mut World, player: i32, template_id: i32) -> Option<i32> {
    use server_packets::sm_ids;
    if let Some(instance_id) = world.instances.allowed_instance_of(player) {
        if world.instances.get(instance_id).map(|i| i.template_id) != Some(template_id) {
            crate::game_loop::helpers::send_sm_to_player(
                world,
                player,
                sm_ids::YOU_HAVE_ENTERED_ANOTHER_INSTANT_ZONE_THEREFORE_YOU_CANNOT_ENTER_CORRESPONDING_DUNGEON,
                &[],
            );
            return None;
        }
        teleport_in(world, player, instance_id);
        return Some(instance_id);
    }
    let Some(template) = world.data.instance_templates.get(template_id).cloned() else {
        tracing::warn!(
            "Player {player} wants to create instance with unknown template id {template_id}!"
        );
        return None;
    };
    let Some(group) = enter_group(world, &template, player) else {
        tracing::warn!("Instance {template_id} has invalid group size limits!");
        return None;
    };
    if !crate::game_loop::helpers::is_gm(world, player)
        && !validate_conditions(world, &template, &group)
    {
        return None;
    }
    if template.max_worlds != -1
        && world.instances.world_count(template_id) >= template.max_worlds as usize
    {
        crate::game_loop::helpers::send_sm_to_player(
            world,
            player,
            sm_ids::THE_NUMBER_OF_INSTANT_ZONES_THAT_CAN_BE_CREATED_HAS_BEEN_EXCEEDED_PLEASE_TRY_AGAIN_LATER,
            &[],
        );
        return None;
    }
    if group
        .iter()
        .any(|&m| world.instances.allowed_instance_of(m).is_some())
    {
        for &m in &group {
            crate::game_loop::helpers::send_sm_to_player(
                world,
                m,
                sm_ids::YOU_HAVE_ENTERED_ANOTHER_INSTANT_ZONE_THEREFORE_YOU_CANNOT_ENTER_CORRESPONDING_DUNGEON,
                &[],
            );
        }
        return None;
    }
    let instance_id = create_from_template(world, template_id)?;
    for &member in &group {
        world.instances.add_allowed(instance_id, member);
        teleport_in(world, member, instance_id);
    }
    Some(instance_id)
}

/// `InstanceScript.onEnter` → `teleportPlayerIn`. A player already inside
/// keeps the origin they came from; they are only moved to the entrance.
fn teleport_in(world: &mut World, player: i32, instance_id: i32) {
    if instance_of(world, player) != instance_id {
        enter(world, player, instance_id);
        return;
    }
    let template_id = world
        .instances
        .get(instance_id)
        .map_or(0, |i| i.template_id);
    let loc = world
        .data
        .instance_templates
        .get(template_id)
        .cloned()
        .and_then(|t| t.enter_location(|n| world.roll(n)));
    if let Some((x, y, z)) = loc {
        crate::game_loop::death::teleport_player(world, player, x, y, z);
    }
}
