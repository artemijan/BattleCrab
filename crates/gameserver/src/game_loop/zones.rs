//! Zone revalidation — the port of `Creature.revalidateZone(force)` +
//! `ZoneRegion.revalidateZones` + the `Player.revalidateZone` override's
//! compass-code push, driven by the `ZoneData` point queries instead of
//! per-zone character lists (each zone type's enter/exit effect is a diff
//! branch here; with three zone kinds and one consumer each, materializing
//! Java's `ZoneType._characterList` buys nothing).
//!
//! Call sites mirror Java: every movement tick (self-filtered by the
//! 100-unit `last_validate` distance), enter world / teleport / position
//! snaps with `force = true`.

use crate::model::components::{Speeds, ZoneFlags};
use crate::network::server_packets::{self, compass_zone};
use crate::world::World;

use super::helpers::client_for_player;

/// Java `Creature.revalidateZone(force)` for a player. Recomputes the
/// membership mask at the current position and applies the enter/exit
/// effects of every bit that flipped.
pub(crate) fn revalidate_zone(world: &mut World, object_id: i32, force: bool) {
    let Some(pos) = world.objects.get_component::<crate::model::components::Position>(&object_id).copied() else {
        return;
    };
    let Some(flags) = world.objects.get_component::<ZoneFlags>(&object_id).copied() else { return };

    // "This function is called too often from movement code."
    if !force {
        let (lx, ly, lz) = flags.last_validate;
        // f64: the fresh-player sentinel (`i32::MIN`) would overflow any
        // integer square.
        let (dx, dy, dz) =
            (pos.x as f64 - lx as f64, pos.y as f64 - ly as f64, pos.z as f64 - lz as f64);
        if dx * dx + dy * dy + dz * dz < 100.0 * 100.0 {
            return;
        }
    }

    let new_mask = world.data.zone_data.mask_at(pos.x, pos.y, pos.z);

    // Compass indicator (`Player.revalidateZone`'s tail): peace icon vs
    // general, only pushed when the code changes. Siege/PvP/altered codes
    // wait for their zone types.
    let compass = if new_mask & crate::data::zone_data::ZoneKind::Peace.bit() != 0 {
        compass_zone::PEACE
    } else {
        compass_zone::GENERAL
    };
    let compass_changed = compass != flags.last_compass;

    let old_mask = flags.mask;
    if let Some(f) = world.objects.get_component_mut::<ZoneFlags>(&object_id) {
        f.mask = new_mask;
        f.last_validate = (pos.x, pos.y, pos.z);
        f.last_compass = compass;
    }

    if compass_changed {
        if let Some(cs) = client_for_player(world, object_id).and_then(|cid| world.clients.get(&cid)) {
            cs.send(server_packets::ex_set_compass_zone_code(compass));
        }
    }

    // WaterZone.onEnter/onExit: flip the swim-speed branch and let everyone
    // (self included) re-read the speeds (Java `broadcastUserInfo`).
    let water_bit = crate::data::zone_data::ZoneKind::Water.bit();
    if (old_mask ^ new_mask) & water_bit != 0 {
        let swimming = new_mask & water_bit != 0;
        if let Some(speeds) = world.objects.get_component_mut::<Speeds>(&object_id) {
            speeds.swimming = swimming;
        }
        super::party::broadcast_user_info(world, object_id);
    }

    // Peace/NoRestart have no enter/exit side effects — membership itself is
    // the state their consumers check (`is_inside_peace_zone`; NO_RESTART has
    // no reader in this Mobius version beyond the login-inside teleport).
}

/// Java `Creature.isInsidePeaceZone(attacker, target)` narrowed to the
/// states that exist: both sides must be players (playable), no karma/
/// GM-override branches (no reputation-based PvP or access levels yet).
/// True ⇒ hostile actions between them are refused.
pub(crate) fn is_inside_peace_zone(world: &World, attacker_oid: i32, target_oid: i32) -> bool {
    let attacker_player = world.objects.has_component::<crate::model::Player>(&attacker_oid);
    let target_player = world.objects.has_component::<crate::model::Player>(&target_oid);
    if !attacker_player || !target_player {
        return false;
    }
    let in_peace = |oid: i32| {
        world
            .objects
            .get_component::<ZoneFlags>(&oid)
            .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Peace))
    };
    in_peace(attacker_oid) || in_peace(target_oid)
}
