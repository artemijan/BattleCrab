//! `AdminRide` mount commands — `//ride_strider`/`//ride_wolf`/`//ride_wyvern`
//! and `//unride`. Java's `//ride_horse`/`//ride_bike` are *transformations*
//! (not mounts) and stay on the deferred transform subsystem, as does
//! `AdminTransform`.
//!
//! A mount is durable state on the `Player` (`mount_type` + `mount_npc_id` +
//! `mount_level`) that the UserInfo/CharInfo builders serialize, so it renders
//! on every client that later sees the rider. Mounting swaps the rider's
//! speeds to the mount's `speed_on_ride` row (`recalculate_stats`) and its
//! collision cylinder to the mount NPC template's (Java
//! `Player.getCollisionRadius/Height` read the mount template while mounted);
//! a wyvern (`mount_type == 2`) additionally flies — `Player::is_flying`
//! feeds the movement pipeline's geodata exemptions and the packet fly
//! fields.

use crate::model::components::{Collision, Position, Speeds};
use crate::model::Player;
use crate::network::server_packets::{self, sm_ids};
use crate::world::World;

use super::{current_target, send_message, send_sm};

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
    if world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(Player::is_mounted)
    {
        send_message(world, client_id, "Target already have a summon.");
        return;
    }
    mount_player(world, target, mount.npc_id(), mount.mount_type());
}

/// The state half of Java `Player.mount(npcId, controlItemObjId, useFood)` +
/// `setMount(npcId, getLevel())`: set the mount fields (mount level = the
/// *rider's* level on this path), swap collision and speeds to the mount's,
/// and broadcast `Ride` + UserInfo/CharInfo.
///
/// TODO(G29): Java `mount()` also disarms the weapon and shield (refusing the
/// mount if it can't), stops all toggles, and starts the mount feed task
/// (`startFeed` — the hunger that halves speed and force-dismounts at 0);
/// those land with rideable pets / mount feeding.
pub(crate) fn mount_player(world: &mut World, target: i32, npc_id: i32, mount_type: u8) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.mount_type = mount_type;
        p.mount_npc_id = npc_id;
        p.mount_level = p.level;
    }
    // Java `Player.getCollisionRadius/Height`: while mounted, the mount NPC
    // template's cylinder replaces the class template's.
    if let Some(t) = world.data.npc_data.get(npc_id) {
        let (radius, height) = (t.collision_radius, t.collision_height);
        world
            .objects
            .add_components(&target, Collision { radius, height });
    }
    super::transforms::recompute_speeds(world, target);
    broadcast_ride(world, target, true);
    super::party::broadcast_user_info(world, target);
}

/// Java `Player.dismount()` — refuse mid-air/over-water dismounts, then clear
/// the mount, restore the class collision/speeds, and broadcast. No-op if not
/// mounted. The `//unride*` commands route here through the transform module's
/// combined dismount-or-untransform path.
///
/// TODO(G29): Java also clears the feed gauge (`SetupGauge(3, 0, 0)`), stops
/// the feed task, persists the leftover feed (`storePetFood`), and removes the
/// Wyvern Breath skill (4289) — feed lands with mount feeding, and Wyvern
/// Breath is never granted yet (Java's own `setMount` only grants the strider
/// skill, so 4289 exists solely to be removed here).
pub(crate) fn dismount(world: &mut World, target: i32) {
    if !world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(Player::is_mounted)
    {
        return;
    }
    let Some(pos) = world.objects.get_component::<Position>(&target).copied() else {
        return;
    };
    // Java: with no water 300 below, a dismount hanging in the sky (z > 10000
    // outside water) or more than 300 above the geodata floor is refused —
    // land the wyvern first.
    let water_below = world
        .data
        .zone_data
        .zones_at(pos.x, pos.y, pos.z - 300)
        .any(|z| z.kind == crate::data::zone_data::ZoneKind::Water);
    if !water_below {
        let swimming = world
            .objects
            .get_component::<Speeds>(&target)
            .is_some_and(|s| s.swimming);
        let client = super::helpers::client_for_player(world, target);
        if !swimming && pos.z > 10000 {
            if let Some(cid) = client {
                send_sm(
                    world,
                    cid,
                    sm_ids::YOU_ARE_NOT_ALLOWED_TO_DISMOUNT_IN_THIS_LOCATION,
                );
            }
            return;
        }
        if world.geo.get_height(pos.x, pos.y, pos.z) + 300 < pos.z {
            if let Some(cid) = client {
                send_sm(world, cid, sm_ids::YOU_CANNOT_DISMOUNT_FROM_THIS_ELEVATION);
            }
            return;
        }
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.mount_type = 0;
        p.mount_npc_id = 0;
        p.mount_level = 0;
    }
    // Restore the class-template collision (same shape as untransform).
    let (class_id, base_class_id) = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| (p.class_id, p.base_class_id))
        .unwrap_or((0, 0));
    if let Some(t) = world
        .data
        .player_templates
        .get(class_id)
        .or_else(|| world.data.player_templates.get(base_class_id))
    {
        world.objects.add_components(
            &target,
            Collision {
                radius: t.collision_radius,
                height: t.collision_height,
            },
        );
    }
    super::transforms::recompute_speeds(world, target);
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
    let packet = server_packets::ride(
        target,
        mounted,
        p.mount_type,
        p.mount_npc_id,
        pos.x,
        pos.y,
        pos.z,
    );
    super::helpers::broadcast_including_self(world, target, &packet);
}
