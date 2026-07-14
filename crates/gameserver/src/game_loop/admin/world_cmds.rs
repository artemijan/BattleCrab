//! World-feature commands — `AdminDoorControl` (open/close doors),
//! `AdminZone`/`AdminZones` (zone inspection), `AdminShop` (`//buy`/`//gmshop`)
//! and `AdminClan`'s `//clan_info`.
//!
//! `AdminFence` needs a spawnable-fence runtime the server has not ported;
//! quest-script reload / clan leadership ops / pledge editing likewise need
//! systems (script engine, clan mutation) that are only partially present, so
//! those siblings stay on the not-implemented path.

use crate::data::zone_data::ZoneKind;
use crate::game_loop::doors;
use crate::model::components::{Position, ZoneFlags};
use crate::model::door::Door;
use crate::model::Player;
use crate::network::trade;
use crate::world::World;

use super::{current_target, send_message, send_sm};

/// `AdminDoorControl`'s `//open`/`//close [doorId]` and `//openall`/`//closeall`
/// — toggle one door (by template id, or the targeted door) or every door.
pub(super) fn admin_door(world: &mut World, client_id: u32, object_id: i32, open: bool, all: bool, args: &[&str]) {
    if all {
        let door_oids: Vec<i32> = world.door_regions.values().flatten().copied().collect();
        for oid in door_oids {
            toggle(world, oid, open);
        }
        send_message(world, client_id, if open { "All doors opened." } else { "All doors closed." });
        return;
    }
    if let Some(door_id) = args.first().and_then(|s| s.parse::<i32>().ok()) {
        doors::open_close_by_door_id(world, door_id, open);
        return;
    }
    // No id → the targeted door.
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Door>(oid)) else {
        send_message(world, client_id, "Incorrect target.");
        return;
    };
    toggle(world, target, open);
}

fn toggle(world: &mut World, door_oid: i32, open: bool) {
    if open {
        doors::open_door(world, door_oid);
    } else {
        doors::close_door(world, door_oid);
    }
}

/// `AdminZone`/`AdminZones`'s `//zones` / `//zone_check` — report which zones the
/// GM currently stands in (Java opens a map-region HTML; text here).
pub(super) fn admin_zones(world: &mut World, client_id: u32, object_id: i32) {
    let mask = world.objects.get_component::<ZoneFlags>(&object_id).map_or(0, |z| z.mask);
    let mut names = Vec::new();
    for (kind, label) in [
        (ZoneKind::Peace, "Peace"),
        (ZoneKind::Water, "Water"),
        (ZoneKind::NoRestart, "NoRestart"),
        (ZoneKind::Pvp, "PvP"),
    ] {
        if mask & kind.bit() != 0 {
            names.push(label);
        }
    }
    send_message(world, client_id, "=== Zones here ===");
    send_message(world, client_id, &if names.is_empty() { "None".to_string() } else { names.join(", ") });
}

/// `AdminShop`'s `//buy <buyListId>` — open a buy window for a merchant buy-list
/// (admin path skips the npc-allowed check Java also bypasses).
pub(super) fn admin_buy(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(list_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Please specify buylist.");
        return;
    };
    let Some(list) = world.data.buy_lists.get(list_id) else {
        send_message(world, client_id, &format!("Buylist {list_id} not found."));
        return;
    };
    let Some(inventory) = world.objects.get_component::<crate::model::inventory::Inventory>(&object_id) else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(trade::buy_list(list, inventory, &world.data));
        cs.send(trade::ex_buy_sell_list_sell(inventory, &world.data, false));
    }
}

/// `AdminClan`'s `//clan_info` — dump the targeted player's clan (name, leader,
/// level, member count).
pub(super) fn admin_clan_info(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)) else {
        super::send_sm(world, client_id, crate::network::server_packets::sm_ids::INVALID_TARGET);
        return;
    };
    let Some(clan_id) = world.objects.get_component::<Player>(&target).map(|p| p.clan_id).filter(|&c| c != 0) else {
        send_message(world, client_id, "Target is not in a clan.");
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        send_message(world, client_id, "Clan not found.");
        return;
    };
    send_message(world, client_id, &format!("=== Clan {} ({}) ===", clan.name, clan.id));
    send_message(world, client_id, &format!("Leader: {}  Level: {}  Members: {}", clan.leader_name(), clan.level, clan.members.len()));
}

/// `AdminGeodata`'s read-only queries: `//geo_pos` / `//geo_spawn_pos` (report
/// the GM's geo coordinates + height) and `//geo_can_move` / `//geo_can_see`
/// (line-of-sight from the GM to the current target). The geo-editor / grid /
/// save commands mutate geodata and stay on the not-implemented path.
pub(super) fn admin_geo_pos(world: &mut World, client_id: u32, object_id: i32, spawn: bool) {
    let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    let geo = &world.geo;
    let (gx, gy) = (geo.get_geo_x(pos.x), geo.get_geo_y(pos.y));
    if !geo.has_geo_pos(gx, gy) {
        send_message(world, client_id, "There is no geodata at this position.");
        return;
    }
    let gz = if spawn { geo.get_spawn_height(pos.x, pos.y, pos.z) } else { geo.get_height(pos.x, pos.y, pos.z) };
    send_message(
        world,
        client_id,
        &format!("WorldX: {}, WorldY: {}, WorldZ: {}, GeoX: {gx}, GeoY: {gy}, GeoZ: {gz}", pos.x, pos.y, pos.z),
    );
}

pub(super) fn admin_geo_can_see(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id) else {
        send_sm(world, client_id, crate::network::server_packets::sm_ids::INVALID_TARGET);
        return;
    };
    let (Some(a), Some(b)) = (
        world.objects.get_component::<Position>(&object_id).copied(),
        world.objects.get_component::<Position>(&target).copied(),
    ) else {
        return;
    };
    let visible = world.geo.can_see_target(a.x, a.y, a.z, b.x, b.y, b.z);
    send_message(world, client_id, if visible { "Can see target." } else { "Cannot see target." });
}

/// `AdminGeodata`'s `//geomap` — the geodata tile (region file) the GM stands in
/// plus that tile's world bounds.
pub(super) fn admin_geomap(world: &mut World, client_id: u32, object_id: i32) {
    let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    let ((tx, ty), (min_x, min_y), (max_x, max_y)) = world.geo.geomap_tile(pos.x, pos.y);
    send_message(world, client_id, &format!("GeoMap: {tx}_{ty} ({min_x},{min_y} to {max_x},{max_y})"));
}

/// `AdminGeodata`'s `//geocell` — the geo cell (geoX/geoY), its nearest Z and
/// the cell-center world coords at the GM's position.
pub(super) fn admin_geocell(world: &mut World, client_id: u32, object_id: i32) {
    let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    let geo = &world.geo;
    let (gx, gy) = (geo.get_geo_x(pos.x), geo.get_geo_y(pos.y));
    let gz = geo.get_nearest_z(gx, gy, pos.z);
    let (wx, wy) = (geo.get_world_x(gx), geo.get_world_y(gy));
    send_message(world, client_id, &format!("GeoCell: {gx}, {gy}. XYZ ({wx}, {wy}, {gz})"));
}

/// `AdminGeodata`'s `//geoenable<dir>` / `//geodisable<dir>` — set or clear one
/// NSWE passability bit on the GM's nearest cell (Java `setNearestNswe` /
/// `unsetNearestNswe`). The edit layers over the immutable base geodata and
/// takes effect immediately for movement/pathfinding.
pub(super) fn admin_geo_nswe(world: &mut World, client_id: u32, object_id: i32, nswe: u8, enable: bool) {
    let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    let (gx, gy) = (world.geo.get_geo_x(pos.x), world.geo.get_geo_y(pos.y));
    if !world.geo.has_geo_pos(gx, gy) {
        send_message(world, client_id, "There is no geodata at this position.");
        return;
    }
    if enable {
        world.geo.set_nearest_nswe(gx, gy, pos.z, nswe);
    } else {
        world.geo.unset_nearest_nswe(gx, gy, pos.z, nswe);
    }
    send_message(world, client_id, &format!("Cell {gx},{gy}: {} {}.", dir_name(nswe), if enable { "enabled" } else { "disabled" }));
}

fn dir_name(nswe: u8) -> &'static str {
    match nswe {
        crate::geo::NSWE_NORTH => "north",
        crate::geo::NSWE_SOUTH => "south",
        crate::geo::NSWE_EAST => "east",
        crate::geo::NSWE_WEST => "west",
        _ => "direction",
    }
}

/// `AdminGeodata`'s `//geosave` / `//geosaveall` — Java writes the edited region
/// back to the geoedit output dir in the L2 binary format. That serializer is
/// not ported; runtime edits live in memory (`GeoEngine` override map) and
/// apply until restart. Reports how many edits are pending.
pub(super) fn admin_geosave(world: &mut World, client_id: u32) {
    let n = world.geo.override_count();
    send_message(world, client_id, &format!("{n} runtime geo edit(s) active (in memory; binary region save is not ported)."));
}

/// `AdminGeodata`'s `//geoedit` / `//geogrid` — Java toggles a client-side
/// geo-edit mode / debug grid overlay (`GeoUtils.debugGrid`, `ExServerPrimitive`
/// packets). That client visualization is not ported; the NSWE edit commands
/// work directly.
pub(super) fn admin_geo_clientviz(world: &mut World, client_id: u32) {
    send_message(world, client_id, "The geo grid overlay / edit mode is not available; use //geoenable*/geodisable* directly.");
}
