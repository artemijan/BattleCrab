//! `AdminMobGroup` — GM-controlled mob groups (`//mobgroup_*`). Groups are
//! registered in [`World::mob_groups`](crate::world::World::mob_groups); their
//! members are ordinary runtime-spawned NPCs tagged with
//! [`Controllable`](crate::model::mob_group::Controllable) and steered by the
//! group's [`MobGroupState`] in `npc_ai::controllable_think`.

use crate::game_loop::guard;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::nth_arg;
use crate::game_loop::helpers::set_position;
use crate::model::components::AdminFlags;
use crate::model::mob_group::{Controllable, MobGroup, MobGroupState};
use crate::model::npc::Npc;
use crate::world::World;

use super::send_message;

/// `//mobmenu` — the mob-group admin HTML page.
pub(super) fn admin_mobmenu(world: &mut World, client_id: u32) {
    super::menu::show_admin_html(world, client_id, "mobgroup.htm");
}

/// `//mobgroup_list` — list every group with its size and state.
pub(super) fn admin_mobgroup_list(world: &mut World, client_id: u32) {
    let mut groups: Vec<(i32, i32, usize, MobGroupState)> = world
        .mob_groups
        .values()
        .map(|g| (g.id, g.npc_id, alive(world, g), g.state))
        .collect();
    groups.sort_by_key(|g| g.0);
    send_message(
        world,
        client_id,
        &format!("=== Mob groups ({}) ===", groups.len()),
    );
    for (id, npc_id, count, state) in groups {
        send_message(
            world,
            client_id,
            &format!(
                "  #{id}: npc {npc_id}, {count} alive, {}",
                state_name(state)
            ),
        );
    }
}

/// `//mobgroup_create <group> <npcId> <count>` — register a new (unspawned)
/// group.
pub(super) fn admin_mobgroup_create(world: &mut World, client_id: u32, args: &[&str]) {
    let (Some(group_id), Some(npc_id), Some(count)) = (
        nth_arg::<i32>(args, 0),
        nth_arg::<i32>(args, 1),
        nth_arg::<i32>(args, 2),
    ) else {
        send_message(
            world,
            client_id,
            "Usage: //mobgroup_create <group> <npcid> <count>",
        );
        return;
    };
    if world.mob_groups.contains_key(&group_id) {
        send_message(
            world,
            client_id,
            &format!("Mob group {group_id} already exists."),
        );
        return;
    }
    if world.data.npc_data.get(npc_id).is_none() {
        send_message(world, client_id, "Invalid NPC ID specified.");
        return;
    }
    world.mob_groups.insert(
        group_id,
        MobGroup::new(group_id, npc_id, count.clamp(1, 100)),
    );
    send_message(world, client_id, &format!("Mob group {group_id} created."));
}

/// `//mobgroup_remove`/`//mobgroup_delete <group>` — unspawn and drop the group.
pub(super) fn admin_mobgroup_remove(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let Some(group_id) = group_arg(world, client_id, args) else {
        return;
    };
    let _ = object_id;
    despawn_members(world, group_id);
    world.mob_groups.remove(&group_id);
    send_message(
        world,
        client_id,
        &format!("Mob group {group_id} unspawned and removed."),
    );
}

/// `//mobgroup_spawn <group> [x y z]` — spawn the group's members at the GM
/// (or explicit coords).
pub(super) fn admin_mobgroup_spawn(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let Some(group_id) = group_arg(world, client_id, args) else {
        return;
    };
    let (npc_id, max_count, already) = {
        let g = world.mob_groups.get(&group_id).expect("checked");
        (g.npc_id, g.max_count, alive(world, g))
    };
    if already > 0 {
        send_message(world, client_id, "Group is already spawned.");
        return;
    }
    // Position: explicit `x y z` (args[1..4]) or the GM's own.
    let pos = match (
        nth_arg::<i32>(args, 1),
        nth_arg::<i32>(args, 2),
        nth_arg::<i32>(args, 3),
    ) {
        (Some(x), Some(y), Some(z)) => (x, y, z, 0),
        _ => {
            let Some(p) = maybe_position(world, object_id) else {
                return;
            };
            (p.x, p.y, p.z, p.heading)
        }
    };
    let mut members = Vec::new();
    for _ in 0..max_count {
        if let Some(oid) =
            crate::model::npc::spawn_npc_at(world, npc_id, pos.0, pos.1, pos.2, pos.3)
        {
            world
                .objects
                .add_components(&oid, Controllable { group_id });
            super::death::introduce_npc(world, oid);
            members.push(oid);
        }
    }
    let n = members.len();
    if let Some(g) = world.mob_groups.get_mut(&group_id) {
        g.members = members;
        g.state = MobGroupState::Idle;
    }
    send_message(
        world,
        client_id,
        &format!("Spawned {n} mob(s) in group {group_id}."),
    );
}

/// `//mobgroup_unspawn <group>` — despawn the members but keep the group.
pub(super) fn admin_mobgroup_unspawn(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(group_id) = group_arg(world, client_id, args) else {
        return;
    };
    despawn_members(world, group_id);
    send_message(
        world,
        client_id,
        &format!("Mob group {group_id} unspawned."),
    );
}

/// `//mobgroup_kill <group>` — kill every member (GM as killer).
pub(super) fn admin_mobgroup_kill(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let Some(group_id) = group_arg(world, client_id, args) else {
        return;
    };
    for oid in members(world, group_id) {
        if world
            .objects
            .get_component::<crate::model::components::Vitals>(&oid)
            .is_some_and(|v| !v.dead)
        {
            super::death::npc_do_die(world, oid, object_id);
        }
    }
    send_message(world, client_id, &format!("Mob group {group_id} killed."));
}

/// `//mobgroup_teleport <group>` — relocate the live members to the GM.
pub(super) fn admin_mobgroup_teleport(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let Some(group_id) = group_arg(world, client_id, args) else {
        return;
    };
    let Some(gm) = maybe_position(world, object_id) else {
        return;
    };
    for oid in members(world, group_id) {
        world
            .objects
            .remove_component::<crate::model::components::Movement>(&oid);
        set_position(world, oid, (gm.x, gm.y, gm.z));
        super::visibility::update_npc_region(world, oid);
    }
    send_message(
        world,
        client_id,
        &format!("Mob group {group_id} teleported."),
    );
}

/// `//mobgroup_invul <group> on|off` — toggle member invulnerability.
pub(super) fn admin_mobgroup_invul(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(group_id) = group_arg(world, client_id, args) else {
        return;
    };
    let Some(on) = on_off(args.get(1)) else {
        send_message(world, client_id, "Usage: //mobgroup_invul <group> on|off");
        return;
    };
    for oid in members(world, group_id) {
        let mut flags = world
            .objects
            .get_component::<AdminFlags>(&oid)
            .copied()
            .unwrap_or_default();
        flags.invul = on;
        world.objects.add_components(&oid, flags);
    }
    if let Some(g) = world.mob_groups.get_mut(&group_id) {
        g.invul = on;
    }
    send_message(
        world,
        client_id,
        &format!(
            "Mob group {group_id} invul {}.",
            if on { "on" } else { "off" }
        ),
    );
}

/// The state-setting commands (`idle`/`rnd`/`nomove`/`attack`/`attackgrp`/
/// `follow`/`return`/`casting`) — resolve the target/mode and store it on the
/// group; the AI tick acts on it.
pub(super) fn admin_mobgroup_state(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    kind: &str,
    args: &[&str],
) {
    let Some(group_id) = group_arg(world, client_id, args) else {
        return;
    };
    let state = match kind {
        "idle" => MobGroupState::Idle,
        "rnd" => MobGroupState::Random,
        "follow" => MobGroupState::Follow(object_id),
        "return" => MobGroupState::Return(object_id),
        "nomove" => match on_off(args.get(1)) {
            Some(true) => MobGroupState::NoMove,
            Some(false) => MobGroupState::Idle,
            None => {
                send_message(world, client_id, "Usage: //mobgroup_nomove <group> on|off");
                return;
            }
        },
        "attack" | "casting" => {
            let Some(target) = guard::target(world, object_id) else {
                send_message(world, client_id, "Select a target first.");
                return;
            };
            if kind == "casting" {
                MobGroupState::Cast(target)
            } else {
                MobGroupState::Attack(target)
            }
        }
        "attackgrp" => {
            let Some(other) = nth_arg::<i32>(args, 1) else {
                send_message(
                    world,
                    client_id,
                    "Usage: //mobgroup_attackgrp <group> <otherGroup>",
                );
                return;
            };
            if !world.mob_groups.contains_key(&other) {
                send_message(world, client_id, "Invalid group specified.");
                return;
            }
            MobGroupState::AttackGroup(other)
        }
        _ => return,
    };
    if let Some(g) = world.mob_groups.get_mut(&group_id) {
        g.state = state;
    }
    send_message(
        world,
        client_id,
        &format!("Mob group {group_id} → {}.", state_name(state)),
    );
}

// --- helpers ---

/// Parse `args[0]` as a group id and confirm the group exists.
fn group_arg(world: &World, client_id: u32, args: &[&str]) -> Option<i32> {
    let Some(id) = nth_arg::<i32>(args, 0) else {
        send_message(world, client_id, "Incorrect command arguments.");
        return None;
    };
    if !world.mob_groups.contains_key(&id) {
        send_message(world, client_id, "Invalid group specified.");
        return None;
    }
    Some(id)
}

fn on_off(arg: Option<&&str>) -> Option<bool> {
    match arg.map(|s| s.to_lowercase()) {
        Some(s) if s == "on" || s == "true" => Some(true),
        Some(s) if s == "off" || s == "false" => Some(false),
        _ => None,
    }
}

/// The group's member object ids (empty if the group is gone).
fn members(world: &World, group_id: i32) -> Vec<i32> {
    world
        .mob_groups
        .get(&group_id)
        .map(|g| g.members.clone())
        .unwrap_or_default()
}

/// Count members that still exist and are alive.
fn alive(world: &World, group: &MobGroup) -> usize {
    group
        .members
        .iter()
        .filter(|&&m| {
            world
                .objects
                .get_component::<crate::model::components::Vitals>(&m)
                .is_some_and(|v| !v.dead)
        })
        .count()
}

/// Despawn every live member and clear the roster.
fn despawn_members(world: &mut World, group_id: i32) {
    for oid in members(world, group_id) {
        if world.objects.has_component::<Npc>(&oid) {
            super::death::despawn_npc_by_oid(world, oid);
        }
    }
    if let Some(g) = world.mob_groups.get_mut(&group_id) {
        g.members.clear();
    }
}

fn state_name(state: MobGroupState) -> &'static str {
    match state {
        MobGroupState::Idle => "idle",
        MobGroupState::NoMove => "no-move",
        MobGroupState::Random => "random",
        MobGroupState::Attack(_) => "attack",
        MobGroupState::AttackGroup(_) => "attack-group",
        MobGroupState::Follow(_) => "follow",
        MobGroupState::Return(_) => "return",
        MobGroupState::Cast(_) => "cast",
    }
}
