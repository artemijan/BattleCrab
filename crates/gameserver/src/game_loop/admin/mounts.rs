//! `AdminRide` mount commands — `//ride_strider`/`//ride_wolf`/`//ride_wyvern`
//! and `//unride`. Java's `//ride_horse`/`//ride_bike` are *transformations*
//! (not mounts) and stay on the deferred transform subsystem, as does
//! `AdminTransform`.
//!
//! A mount is durable state on the `Player` (`mount_type` + `mount_npc_id`) that
//! the UserInfo/CharInfo builders serialize, so it renders on every client that
//! later sees the rider — not just a one-off broadcast. Java also swaps the
//! rider's speed/collision to the mount's; that stat change needs mount stat
//! data and is left as a documented TODO (the visual mount is complete).

use crate::model::components::Position;
use crate::model::Player;
use crate::network::server_packets;
use crate::world::World;

use super::{current_target, send_message};

/// The fixed npc ids `AdminRide` mounts (Java `petRideId`), with their
/// `MountType` ordinal (1 strider, 2 wyvern, 3 wolf).
pub(super) enum Mount {
    Strider,
    Wolf,
    Wyvern,
}

impl Mount {
    fn npc_id(&self) -> i32 {
        match self {
            Mount::Strider => 12526,
            Mount::Wolf => 16041,
            Mount::Wyvern => 12621,
        }
    }

    fn mount_type(&self) -> u8 {
        match self {
            Mount::Strider => 1,
            Mount::Wyvern => 2,
            Mount::Wolf => 3,
        }
    }
}

/// Java `AdminRide.getRideTarget` — the current target if it's a *different*
/// player, else the GM.
fn ride_target(world: &World, object_id: i32) -> i32 {
    current_target(world, object_id)
        .filter(|&oid| oid != object_id && world.objects.has_component::<Player>(&oid))
        .unwrap_or(object_id)
}

/// `AdminRide`'s `//ride_strider|ride_wolf|ride_wyvern` — mount the ride target
/// on the fixed creature. Refused if already mounted.
pub(super) fn admin_ride(world: &mut World, client_id: u32, object_id: i32, mount: Mount) {
    let target = ride_target(world, object_id);
    if world.objects.get_component::<Player>(&target).is_some_and(|p| p.mount_type != 0) {
        send_message(world, client_id, "Target already have a summon.");
        return;
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.mount_type = mount.mount_type();
        p.mount_npc_id = mount.npc_id();
    }
    broadcast_ride(world, target, true);
    super::party::broadcast_user_info(world, target);
}

/// Clear the mount on `target` (Java `Player.dismount`) and broadcast. No-op if
/// not mounted. The `//unride*` commands route here through the transform
/// module's combined dismount-or-untransform path.
pub(super) fn dismount(world: &mut World, target: i32) {
    if !world.objects.get_component::<Player>(&target).is_some_and(|p| p.mount_type != 0) {
        return;
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.mount_type = 0;
        p.mount_npc_id = 0;
    }
    broadcast_ride(world, target, false);
    super::party::broadcast_user_info(world, target);
}

/// Broadcast the `Ride` packet (mount/dismount) to the rider and everyone
/// nearby.
fn broadcast_ride(world: &World, target: i32, mounted: bool) {
    let (Some(p), Some(pos)) = (
        world.objects.get_component::<Player>(&target),
        world.objects.get_component::<Position>(&target).copied(),
    ) else {
        return;
    };
    let packet = server_packets::ride(target, mounted, p.mount_type, p.mount_npc_id, pos.x, pos.y, pos.z);
    super::helpers::broadcast_including_self(world, target, &packet);
}
