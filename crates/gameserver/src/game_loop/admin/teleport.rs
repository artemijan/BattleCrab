//! Movement commands — `AdminGmSpeed` and the `AdminTeleport` family
//! (`//teleport`, `//recall`, `//teleto`).

use crate::model::components::{Position, Speeds};
use crate::model::Player;
use crate::world::World;

use super::{current_target, find_online_player, send_message};

/// `AdminGmSpeed` — scale the target player's (or self's) movement speed. Java
/// adds `baseSpeed * boost` as a fixed value to each speed stat, i.e. total =
/// `baseSpeed * (1 + boost)`; the Rust move model already carries a
/// `move_multiplier`, so `1 + boost` is the exact equivalent (boost 0 resets).
/// Range 0..=10, matching Java's custom clamp. NPC targets are TODO.
pub(super) fn admin_gmspeed(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(boost) = args.first().and_then(|s| s.parse::<f64>().ok()).filter(|b| (0.0..=10.0).contains(b))
    else {
        send_message(world, client_id, "//gmspeed [0...10]");
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    if let Some(speeds) = world.objects.get_component_mut::<Speeds>(&target) {
        speeds.move_multiplier = 1.0 + boost;
    }
    super::party::broadcast_user_info(world, target);
}

/// `AdminTeleport`'s coordinate form (`//teleport x y z`) — send the GM to an
/// explicit location. The menu/target-teleport variants are TODO.
pub(super) fn admin_teleport_coords(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let coords = (
        args.first().and_then(|s| s.parse::<i32>().ok()),
        args.get(1).and_then(|s| s.parse::<i32>().ok()),
        args.get(2).and_then(|s| s.parse::<i32>().ok()),
    );
    let (Some(x), Some(y), Some(z)) = coords else {
        send_message(world, client_id, "Usage: //teleport <x> <y> <z>");
        return;
    };
    super::death::teleport_player(world, object_id, x, y, z);
}

/// `AdminTeleport`'s `//recall <name>` — bring an online player to the GM's
/// location (or, with no name, the currently targeted player).
pub(super) fn admin_recall(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let target = match args.first() {
        Some(name) => find_online_player(world, name),
        None => current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)),
    };
    let Some(target) = target else {
        send_message(world, client_id, "Usage: //recall <player name>");
        return;
    };
    let Some(&pos) = world.objects.get_component::<Position>(&object_id) else { return };
    super::death::teleport_player(world, target, pos.x, pos.y, pos.z);
}

/// `AdminTeleport`'s `//teleto` — send the GM to the current target's position.
pub(super) fn admin_teleto(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id) else {
        send_message(world, client_id, "Select a target first.");
        return;
    };
    let Some(&pos) = world.objects.get_component::<Position>(&target) else { return };
    super::death::teleport_player(world, object_id, pos.x, pos.y, pos.z);
}
