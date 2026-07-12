//! Movement/position handlers (`MoveBackwardToLocation`, `ValidatePosition`).

use crate::model::components::{Casting, ClientPos, Intent, Movement, Position, Speeds, Vitals};
use crate::model::Player;
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::broadcast_to_others;

/// Port of `clientpackets/MoveBackwardToLocation.runImpl` +
/// `Creature.moveToLocation`'s geodata movement checks: the requested
/// destination is clamped to the last walkable cell via
/// `GeoEngine.getValidLocation`. The pathfinding fallback (Java runs
/// `CellPathFinding` when the clamp shortens the path by > 30 units) is not
/// ported yet — where Java would walk around an obstacle, the player walks up
/// to it and stops. Door-crossing, teleport-mode switches, and queued-skill
/// clearing are all skipped as out of scope (no doors/admin-teleport/
/// queued-skills yet).
pub(crate) fn handle_move_backward_to_location(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::MoveBackwardToLocation::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    if world.objects.get_component::<crate::model::Player>(&object_id).is_none() {
        return;
    }
    let Some(cur) = world.objects.get_component::<Position>(&object_id).copied() else { return };

    if pkt.target_x == pkt.origin_x && pkt.target_y == pkt.origin_y && pkt.target_z == pkt.origin_z {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::stop_move(object_id, cur.x, cur.y, cur.z, cur.heading));
            cs.send(server_packets::action_failed());
        }
        return;
    }

    // Java `PlayerAI.onIntentionMoveTo`: a move request during a cast is
    // rejected with ActionFailed (the cast is NOT aborted); the queued
    // next-intention move is not ported. Dead players can't move at all
    // (`isMovementDisabled`).
    let is_dead = world.objects.get_component::<Vitals>(&object_id).is_some_and(|v| v.dead);
    if world.objects.has_component::<Casting>(&object_id) || is_dead {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    // A manual move click replaces an attack loop (MOVE_TO intention).
    world.objects.remove_component::<Intent>(&object_id);

    let mut target_x = pkt.target_x;
    let mut target_y = pkt.target_y;
    let target_z = pkt.target_z;
    let mut dx = (target_x - cur.x) as f64;
    let mut dy = (target_y - cur.y) as f64;
    if dx * dx + dy * dy > 98_010_000.0 {
        // 9900² — Java's max single-click move distance.
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    let mut distance = (dx * dx + dy * dy).sqrt();

    // GEODATA MOVEMENT CHECKS (`Creature.moveToLocation`). Java skips the
    // destination correction for far clicks (> 3000: "should be able to
    // click far away and move" — pathfinding would take over) and for
    // intentional falls ((curZ - z) > 300 with distance < 300).
    if world.path_finding > 0 && distance <= 3000.0 && !(cur.z - target_z > 300 && distance < 300.0) {
        let (vx, vy, _vz) = world.geo.get_valid_location(cur.x, cur.y, cur.z, target_x, target_y, target_z);
        // Players keep the client-requested z (Java: `if (!isPlayer()) z = destiny.getZ()`).
        target_x = vx;
        target_y = vy;
        dx = (target_x - cur.x) as f64;
        dy = (target_y - cur.y) as f64;
        distance = (dx * dx + dy * dy).sqrt();
    }

    // Java: `(distance < 1) && (Config.PATHFINDING > 0 || isPlayable())` —
    // a fully clamped-away (or degenerate) move is canceled.
    if distance < 1.0 {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    let (start_x, start_y, start_z) = (cur.x, cur.y, cur.z);
    let heading = crate::model::movement::calculate_heading(dx, dy);
    let Some(speed) = world.objects.get_component::<Speeds>(&object_id).map(Speeds::move_speed) else { return };
    let total_ticks = if speed > 0.0 { ((10.0 * distance / speed).round() as u64).max(1) } else { 1 };
    let start_tick = world.tick;

    if let Some(pos) = world.objects.get_component_mut::<Position>(&object_id) {
        pos.heading = heading;
    }
    world.objects.add_components(
        &object_id,
        Movement(crate::model::movement::MoveData {
            start_x,
            start_y,
            start_z,
            dest_x: target_x,
            dest_y: target_y,
            dest_z: target_z,
            start_tick,
            total_ticks,
        }),
    );

    // Broadcast once at move start, including the mover — the client does
    // not self-predict; it only starts walking once the server confirms with
    // `MoveToLocation` (Java: `Creature.moveToLocation` → `broadcastPacket`,
    // which `Player` overrides with `includeSelf == true`).
    let move_pkt =
        server_packets::move_to_location(object_id, target_x, target_y, target_z, start_x, start_y, start_z);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(move_pkt.clone());
    }
    broadcast_to_others(world, object_id, &move_pkt);
}

/// Port of `clientpackets/ValidatePosition.runImpl` — reconcile the client's
/// periodic position report with the server's authoritative position.
/// Narrowing: no vehicles, falling state, flying/water zones, observer mode,
/// or Blink, and the trailing door-exploit check is skipped (no doors) —
/// those branches simply can't trigger yet.
pub(crate) fn handle_validate_position(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::ValidatePosition::read(body) else { return };
    // Field-level split borrow: `player`+`pos` (mut) + `geo`/`clients` (shared).
    let World { clients, objects, geo, .. } = world;
    let Some(ClientSession::InGame(session)) = clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    // Java: also bails while teleporting / in observer mode (states we lack).
    if objects.has_component::<Casting>(&object_id) {
        return;
    }
    let Some((player, mut pos, speeds, mut client)) =
        objects.get_many_mut::<(&mut Player, &mut Position, &Speeds, &mut ClientPos)>(&object_id)
    else {
        return;
    };
    let _ = player;

    if pkt.x == 0 && pkt.y == 0 && pos.x != 0 {
        return;
    }

    let dx = (pkt.x - pos.x) as f64;
    let dy = (pkt.y - pos.y) as f64;
    let dz = (pkt.z - pos.z) as f64;
    let diff_sq = dx * dx + dy * dy;

    // "If too large, messes observation" — moderate drift only.
    let mut correction: Option<Vec<u8>> = None;
    if diff_sq < 360_000.0 && (diff_sq > 250_000.0 || dz.abs() > 200.0) {
        if dz.abs() > 200.0 && dz.abs() < 1500.0 && (pkt.z - client.z).abs() < 800 {
            // Plausible stairs/slope climb: trust the client's z.
            pos.z = pkt.z;
        } else {
            // Push the server position back to the client (built pre-snap,
            // exactly where Java builds the packet).
            correction = Some(server_packets::validate_location(object_id, pos.x, pos.y, pos.z, pos.heading));
        }
    }

    // Out-of-sync check: a jump larger than one second of movement snaps the
    // server to the client position, geodata-correcting z when the server
    // was above the client (falling through a floor edge).
    let sdx = (pkt.x - pos.x) as f64;
    let sdy = (pkt.y - pos.y) as f64;
    let sdz = (pkt.z - pos.z) as f64;
    let move_speed = speeds.move_speed();
    if (sdx * sdx + sdy * sdy + sdz * sdz).sqrt() > move_speed {
        let z = if pos.z > pkt.z { geo.get_height(pkt.x, pkt.y, pos.z) } else { pkt.z };
        pos.x = pkt.x;
        pos.y = pkt.y;
        pos.z = z;
    }

    client.x = pkt.x;
    client.y = pkt.y;
    client.z = pkt.z;
    client.heading = pkt.heading;

    if let (Some(pkt_bytes), Some(cs)) = (correction, clients.get(&client_id)) {
        cs.send(pkt_bytes);
    }

    // The out-of-sync snap above may have moved the player across a region
    // boundary (Java `setXYZ` → `updateWorldRegion`).
    super::visibility::update_region(world, object_id);
}
