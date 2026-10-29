//! Sayune — the Awakened's jump paths. Port of `SayuneZone`,
//! `FlyMoveStartTask`, `RequestFlyMoveStart`, `RequestFlyMove` and
//! `SayuneRequest`.
//!
//! Standing in a Sayune zone lights the client's jump button
//! (`ExNotifyFlyMoveStart`, re-sent every second). Pressing it opens a
//! [`SayuneRequest`] on the zone's map and makes the first hop; each later
//! `RequestFlyMove` makes the next one, or picks a branch at a junction, until
//! the path runs out and the request closes.

use crate::data::sayune_data::SayuneEntry;
use crate::game_loop::helpers;
use crate::game_loop::space::position::maybe_position;
use crate::model::Player;
use crate::model::components::space::{SayuneRequest, ZoneFlags};
use crate::network::server_packets::{self, sm_ids};
use crate::scheduler::{ScheduledTask, ms_to_ticks};
use crate::world::World;
use tracing::warn;

/// `FlyMoveStartTask`'s re-arm period.
const NOTIFY_PERIOD_MS: u64 = 1000;

/// `Broadcast.toKnownPlayersInRadius(player, …, 1000)` in `SayuneRequest.move`.
const BROADCAST_RADIUS: f64 = 1000.0;

/// `SayuneZone.onEnter` / `onExit`, edge-triggered from `revalidate_zone` off
/// the Sayune zone the player was listed in before and stands in now.
pub(crate) fn on_zone_change(
    world: &mut World,
    object_id: i32,
    old: Option<usize>,
    new: Option<usize>,
) {
    if old == new {
        return;
    }
    if let Some(f) = world.objects.get_component_mut::<ZoneFlags>(&object_id) {
        f.sayune_zone = new;
        // `onExit`: `setInsideZone(ZoneId.SAYUNE, false)`.
        if old.is_some() {
            f.sayune = false;
        }
    }
    let Some(zone) = new else {
        return;
    };
    // `onEnter`: an Awakened (or, with `FreeJumpsForAll`, anyone) who is
    // neither transformed nor mounted.
    let Some(p) = world.objects.get_component::<Player>(&object_id) else {
        return;
    };
    let qualifies = (world
        .data
        .categories
        .contains("SIXTH_CLASS_GROUP", p.class_id)
        || world.cfg.custom_misc.free_jumps_for_all)
        && p.transform_id == 0
        && !p.is_mounted();
    if !qualifies {
        return;
    }
    if let Some(f) = world.objects.get_component_mut::<ZoneFlags>(&object_id) {
        f.sayune = true;
    }
    // `ThreadPool.execute(new FlyMoveStartTask(this, player))`.
    fly_move_start_task(world, object_id, zone);
}

/// `FlyMoveStartTask.run` — while the player is still listed in the zone and
/// has not started a flight, light the jump button and check again in a
/// second.
pub(crate) fn fly_move_start_task(world: &mut World, object_id: i32, zone: usize) {
    let listed = world
        .objects
        .get_component::<ZoneFlags>(&object_id)
        .is_some_and(|f| f.sayune_zone == Some(zone));
    if !listed || world.objects.has_component::<SayuneRequest>(&object_id) {
        return;
    }
    helpers::send_to_player(world, object_id, server_packets::ex_notify_fly_move_start());
    world.scheduler.schedule(
        world.tick + ms_to_ticks(NOTIFY_PERIOD_MS),
        ScheduledTask::FlyMoveStart { object_id, zone },
    );
}

/// Java `isInsideZone(ZoneId.SAYUNE)`.
pub(crate) fn in_sayune_zone(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<ZoneFlags>(&object_id)
        .is_some_and(|f| f.sayune)
}

/// Java `Player.hasRequests()` — any `AbstractRequest` open. Of Java's request
/// classes this port has the enchant window, a party invitation and Sayune
/// itself; the rest belong to systems not ported.
fn has_requests(world: &World, object_id: i32) -> bool {
    world
        .objects
        .has_component::<crate::model::components::commerce::EnchantRequest>(&object_id)
        || world.objects.has_component::<SayuneRequest>(&object_id)
        || world
            .objects
            .get_component::<crate::model::components::social::PendingRequest>(&object_id)
            .is_some_and(|r| {
                matches!(
                    r.kind,
                    crate::model::components::social::RequestKind::PartyInvite { .. }
                )
            })
}

/// `RequestFlyMoveStart.runImpl` — the jump button.
pub(crate) fn handle_request_fly_move_start(world: &mut World, client_id: u32) {
    let Some(object_id) = helpers::player_of(world, client_id) else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&object_id) else {
        return;
    };
    if !in_sayune_zone(world, object_id)
        || world.objects.has_component::<SayuneRequest>(&object_id)
        || (!world
            .data
            .categories
            .contains("SIXTH_CLASS_GROUP", p.class_id)
            && !world.cfg.custom_misc.free_jumps_for_all)
    {
        return;
    }
    let reputation = p.reputation;
    let has_summon = crate::game_loop::servitor::pet_of(world, object_id).is_some()
        || crate::game_loop::servitor::servitor_of(world, object_id).is_some();
    if has_summon {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOU_MAY_NOT_USE_SAYUNE_WHILE_PET_OR_SUMMONED_PET_IS_OUT,
        );
        return;
    }
    if reputation < 0 {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOU_CANNOT_USE_SAYUNE_WHILE_IN_A_CHAOTIC_STATE,
        );
        return;
    }
    if has_requests(world, object_id) {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::SAYUNE_CANNOT_BE_USED_WHILE_TAKING_OTHER_ACTIONS,
        );
        return;
    }
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    // `ZoneManager.getZone(player, SayuneZone.class)` — geometry, as Java's is.
    let Some(zone) = world.data.zone_data.sayune_zone_at(pos.x, pos.y, pos.z) else {
        return;
    };
    let zone = &world.data.zone_data.zones[zone];
    if zone.sayune_map_id == -1 {
        helpers::send_message(world, client_id, "That zone is not supported yet!");
        warn!(
            "RequestFlyMoveStart: player {object_id} Requested sayune on zone with no map id set"
        );
        return;
    }
    let Some(map) = world.data.sayune.map(zone.sayune_map_id) else {
        helpers::send_message(world, client_id, "This zone is not handled yet!!");
        warn!(
            "RequestFlyMoveStart: player {object_id} Requested sayune on unhandled map zone {}",
            zone.name
        );
        return;
    };
    let request = SayuneRequest {
        map_id: map.id,
        selecting: false,
        possible: map.inner.iter().cloned().collect(),
    };
    world.objects.add_components(&object_id, request);
    fly_move(world, object_id, 0);
}

/// `RequestFlyMove.runImpl` — the next hop, or the branch picked at a junction.
pub(crate) fn handle_request_fly_move(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(object_id) = helpers::player_of(world, client_id) else {
        return;
    };
    let mut r = commons::network::PacketReader::new(body);
    let Some(location_id) = r.read_i32() else {
        return;
    };
    if !world.objects.has_component::<SayuneRequest>(&object_id) {
        return;
    }
    fly_move(world, object_id, location_id);
}

/// `SayuneRequest.findEntry(pos)`: at a junction, the choice with that id;
/// otherwise the next point on the path, taken off it.
fn find_entry(request: &mut SayuneRequest, pos: i32) -> Option<SayuneEntry> {
    if request.possible.is_empty() {
        None
    } else if request.selecting {
        request.possible.iter().find(|e| e.id == pos).cloned()
    } else {
        request.possible.pop_front()
    }
}

/// `SayuneRequest.move(player, pos)`.
fn fly_move(world: &mut World, object_id: i32, pos: i32) {
    let Some(mut request) = world
        .objects
        .get_component::<SayuneRequest>(&object_id)
        .cloned()
    else {
        return;
    };
    let Some(map) = world.data.sayune.map(request.map_id).cloned() else {
        not_found(world, object_id, request.map_id);
        return;
    };
    if map.inner.is_empty() {
        not_found(world, object_id, request.map_id);
        return;
    }
    let Some(next) = find_entry(&mut request, pos) else {
        world.objects.remove_component::<SayuneRequest>(&object_id);
        return;
    };
    // A branch was picked: the path ahead is what follows it.
    if request.selecting {
        request.selecting = false;
        if !next.selector {
            request.possible = next.inner.iter().cloned().collect();
        }
    }
    let sayune_type = if pos == 0 && next.selector {
        server_packets::SAYUNE_START_LOC
    } else if next.selector {
        server_packets::SAYUNE_MULTI_WAY_LOC
    } else {
        server_packets::SAYUNE_ONE_WAY_LOC
    };
    let locations = if next.selector {
        next.inner.clone()
    } else {
        vec![next.clone()]
    };
    if next.selector {
        request.possible = locations.iter().cloned().collect();
        request.selecting = true;
    }
    if let Some(r) = world.objects.get_component_mut::<SayuneRequest>(&object_id) {
        *r = request.clone();
    }

    let points: Vec<(i32, i32, i32, i32)> =
        locations.iter().map(|e| (e.id, e.x, e.y, e.z)).collect();
    helpers::send_to_player(
        world,
        object_id,
        server_packets::ex_fly_move(object_id, sayune_type, request.map_id, &points),
    );

    // `locations.get(0)` — a junction always has choices in this datapack.
    let Some(active) = locations.first() else {
        return;
    };
    if let Some(from) = maybe_position(world, object_id) {
        let pkt = server_packets::ex_fly_move_broadcast(
            object_id,
            sayune_type,
            map.id,
            active.xyz(),
            (from.x, from.y, from.z),
        );
        broadcast_in_radius(world, object_id, &pkt);
    }
    super::position::set_xyz(world, object_id, active.xyz());
}

/// `player.sendMessage("MapId: " + _mapId + " was not found in the map!")`.
fn not_found(world: &World, object_id: i32, map_id: i32) {
    if let Some(client_id) = helpers::client_for_player(world, object_id) {
        helpers::send_message(
            world,
            client_id,
            &format!("MapId: {map_id} was not found in the map!"),
        );
    }
}

/// `Broadcast.toKnownPlayersInRadius(player, packet, 1000)`: the known players
/// within the radius, not the jumper.
fn broadcast_in_radius(world: &World, object_id: i32, packet: &[u8]) {
    let Some(region) = super::position::region_cell_of(world, object_id) else {
        return;
    };
    let instance = helpers::instance_of(world, object_id);
    for other in world.players_visible_from(region) {
        if other == object_id || helpers::instance_of(world, other) != instance {
            continue;
        }
        if crate::geo::distance::distance_3d(world, object_id, other)
            .is_some_and(|d| d <= BROADCAST_RADIUS)
            && let Some(cs) = world.clients.session_of_player(other)
        {
            cs.send(bytes::Bytes::copy_from_slice(packet));
        }
    }
}
