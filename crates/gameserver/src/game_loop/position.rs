//! Movement/position handlers (`MoveBackwardToLocation`, `RequestStopMove`,
//! `ValidatePosition`) and the path-worker reply handler (`handle_path_result`).

use crate::geo::worker::{PathEvent, PathRequest};
use crate::model::components::{AttackState, Casting, ClientPos, Intent, Movement, PathWait, Position, QueuedAction, Speeds, Vitals};
use crate::model::movement::GeoPath;
use crate::model::Player;
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::{broadcast_including_self, broadcast_to_others};

/// Port of `clientpackets/MoveBackwardToLocation.runImpl` +
/// `Creature.moveToLocation`'s geodata movement checks: the requested
/// destination is clamped to the last walkable cell via
/// `GeoEngine.getValidLocation`, and when the clamp shortens the move by
/// more than 30 units the destination goes to the path worker instead —
/// the move then starts from `handle_path_result` when the route lands
/// (Java runs `CellPathFinding.findPath` synchronously at this point).
/// Door-crossing and teleport-mode switches are skipped as out of scope (no
/// doors/admin-teleport). Java's "remove queued skill upon move request" is
/// covered by the busy branch overwriting the `QueuedAction` slot — outside
/// a cast/swing the slot is always empty, so there is nothing to clear.
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

    // Java `MoveBackwardToLocation`: "Correcting targetZ from floor level to
    // head level." The client sends the destination z at *floor* level, which
    // does not resolve to the right geodata layer (on stacked terrain — bridges
    // — the raw floor z can snap to the surface *under* the deck). Bumping it up
    // by the player's collision height gives head level, matching what
    // `ValidatePosition` reports and what the geodata queries expect. Applied
    // before the intention/geodata logic, exactly like Java (after the
    // origin==target stop check, before `setIntention`).
    let collision_height = world
        .objects
        .get_component::<crate::model::components::Collision>(&object_id)
        .map_or(0.0, |c| c.height);
    let target_z = (pkt.target_z as f64 + collision_height) as i32;

    // Stunned/asleep/paralyzed or rooted players can't move either — the rest
    // of `isMovementDisabled`'s effect-driven terms.
    if super::abnormal::is_movement_disabled(world, object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::stop_move(object_id, cur.x, cur.y, cur.z, cur.heading));
            cs.send(server_packets::action_failed());
        }
        return;
    }
    // Dead players can't move at all (`isMovementDisabled`).
    if world.objects.get_component::<Vitals>(&object_id).is_some_and(|v| v.dead) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    // Java `PlayerAI.onIntentionMoveTo`: a move request while busy (mid-cast
    // or mid-swing, `isCastingNow || isAttackingNow`) is rejected with
    // ActionFailed (the cast/swing is NOT aborted) but saved as the next
    // intention (`saveNextIntention`), replayed when the cast stops
    // (`stop_casting`) or the swing ends (`AttackFinish`). The click also
    // displaces a pending attack loop — afterwards the player moves, not
    // swings.
    let mid_swing = world
        .objects
        .get_component::<AttackState>(&object_id)
        .is_some_and(|st| st.attack_end_tick > world.tick);
    if mid_swing || world.objects.has_component::<Casting>(&object_id) {
        world.objects.remove_component::<Intent>(&object_id);
        world
            .objects
            .add_components(&object_id, QueuedAction::Move { x: pkt.target_x, y: pkt.target_y, z: target_z });
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    // A manual move click replaces an attack loop (MOVE_TO intention).
    if world.objects.has_component::<Intent>(&object_id) {
        world.objects.remove_component::<Intent>(&object_id);
    }

    intention_move_to(world, client_id, object_id, cur, (pkt.target_x, pkt.target_y, target_z));
}

/// Port of `clientpackets/RequestStopMove.runImpl`:
/// `player.stopMove(player.getLocation())`. Deletes the in-flight move (Java
/// `_move = null`) — and any pending path-worker request, so a still-in-flight
/// reply lands stale in `handle_path_result` rather than restarting the walk —
/// keeps the player at its current (tick-advanced) location, then broadcasts
/// `StopMove` (`Player.broadcastPacket` includes self). The `setXYZ`/
/// `revalidateZone` in Java are no-ops here: the location passed is the
/// player's own current position, so nothing moves and no zone boundary is
/// crossed.
pub(crate) fn handle_request_stop_move(world: &mut World, client_id: u32) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let Some(cur) = world.objects.get_component::<Position>(&object_id).copied() else { return };

    world.objects.remove_component::<Movement>(&object_id);
    world.objects.remove_component::<PathWait>(&object_id);

    broadcast_including_self(
        world,
        object_id,
        &server_packets::stop_move(object_id, cur.x, cur.y, cur.z, cur.heading),
    );
}

/// Port of `clientpackets/ExSendSelectedQuestZoneID.runImpl`: store the quest
/// zone the client selected on `Player` (read later by quest teleports).
pub(crate) fn handle_ex_send_selected_quest_zone_id(world: &mut World, client_id: u32, ex_body: &[u8]) {
    let Some(quest_zone_id) = cp::read_selected_quest_zone_id(ex_body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    if let Some(player) = world.objects.get_component_mut::<Player>(&object_id) {
        player.quest_zone_id = quest_zone_id;
    }
}

/// The movement pipeline behind the intention gates — geodata clamping,
/// path-worker handoff, or a straight move (`Creature.moveToLocation`'s
/// body). Entered from the move packet handler and from the queued-move
/// replay when a cast stops.
pub(crate) fn intention_move_to(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    cur: Position,
    target: (i32, i32, i32),
) {
    let (mut target_x, mut target_y, target_z) = target;
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

    // GEODATA MOVEMENT CHECKS AND PATHFINDING (`Creature.moveToLocation`).
    let (original_x, original_y, original_z) = (target_x, target_y, target_z);
    let original_distance = distance;
    if world.path_finding > 0 {
        // A re-click onto the geo cell we're already pathing to is ignored;
        // a click elsewhere abandons route following on the in-flight move
        // (Java `isOnGeodataPath()` → same gtx/gty return / index = -1).
        let gtx = world.geo.get_geo_x(original_x);
        let gty = world.geo.get_geo_y(original_y);
        if let Some(mv) = world.objects.get_component_mut::<Movement>(&object_id) {
            if let Some(gp) = &mv.0.geo_path {
                if gp.has_next() {
                    if gp.gtx == gtx && gp.gty == gty {
                        return;
                    }
                    mv.0.geo_path = None;
                }
            }
        }
    }

    // Java skips the destination correction for far clicks (> 3000: "should
    // be able to click far away and move") and for intentional falls
    // ((curZ - z) > 300 with distance < 300).
    if world.path_finding > 0 && distance <= 3000.0 && !(cur.z - target_z > 300 && distance < 300.0) {
        let (vx, vy, _vz) = world.geo.get_valid_location(cur.x, cur.y, cur.z, target_x, target_y, target_z);
        // Players keep the client-requested z (Java: `if (!isPlayer()) z = destiny.getZ()`).
        target_x = vx;
        target_y = vy;
        dx = (target_x - cur.x) as f64;
        dy = (target_y - cur.y) as f64;
        distance = (dx * dx + dy * dy).sqrt();
    }

    // The clamp shortened the move by > 30 units — hand the original
    // destination to the path worker; the move starts (or fails with
    // ActionFailed) in `handle_path_result` when the reply lands.
    if world.path_finding > 0 && (original_distance - distance) > 30.0 {
        let seq = world.next_path_seq();
        world.objects.add_components(&object_id, PathWait { seq });
        let _ = world.path.send(PathRequest {
            seq,
            client_id,
            object_id,
            from: (cur.x, cur.y, cur.z),
            to: (original_x, original_y, original_z),
            playable: true,
        });
        return;
    }

    // Java: `(distance < 1) && (Config.PATHFINDING > 0 || isPlayable())` —
    // a fully clamped-away (or degenerate) move is canceled.
    if distance < 1.0 {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    start_move(world, client_id, object_id, cur, (target_x, target_y, target_z), None);
}

/// The path worker's reply (`geo::worker::PathEvent`): start the route move,
/// or tell the player the click leads nowhere. Java reaches the same two
/// outcomes inline in `Creature.moveToLocation` ("if found" / "No path
/// found" + ActionFailed); the extra liveness re-checks cover state changes
/// during the round-trip, which the synchronous Java flow can't see.
pub(crate) fn handle_path_result(world: &mut World, ev: PathEvent) {
    let PathEvent { seq, client_id, object_id, to, path } = ev;
    // Stale reply: the player left, or clicked again (newer seq) — drop it.
    match world.objects.get_component::<PathWait>(&object_id) {
        Some(w) if w.seq == seq => {}
        _ => return,
    }
    world.objects.remove_component::<PathWait>(&object_id);

    // Java `found = (geoPath != null) && (geoPath.size() > 1)`; a player
    // with no path gets ActionFailed (any in-flight move keeps running).
    // NPCs share this reply path as of G21 and have no client, so every
    // client-facing send is gated on the mover actually being a player rather
    // than on `client_id` (which would be a sentinel for an NPC).
    let is_player = world.objects.has_component::<Player>(&object_id);
    let points = match path {
        Some(p) if p.len() > 1 => p,
        _ => {
            if is_player {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::action_failed());
                }
            }
            return;
        }
    };

    // Move gates re-checked after the round-trip (same set as the click).
    let is_dead = world.objects.get_component::<Vitals>(&object_id).is_some_and(|v| v.dead);
    if world.objects.has_component::<Casting>(&object_id) || is_dead {
        if is_player {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::action_failed());
            }
        }
        return;
    }
    let Some(cur) = world.objects.get_component::<Position>(&object_id).copied() else { return };

    let first = points[0];
    let geo_path = GeoPath {
        points,
        index: 0,
        accurate_tx: to.0,
        accurate_ty: to.1,
        gtx: world.geo.get_geo_x(to.0),
        gty: world.geo.get_geo_y(to.1),
    };
    start_move(world, client_id, object_id, cur, first, Some(geo_path));
}

/// The tail of `Creature.moveToLocation`: store the `MoveData` (heading,
/// speed-derived tick count, optional geodata route) and broadcast
/// `MoveToLocation` — including the mover, who does not self-predict and
/// only starts walking on the server's confirmation (Java `broadcastPacket`,
/// which `Player` overrides with `includeSelf == true`).
pub(crate) fn start_move(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    cur: Position,
    dest: (i32, i32, i32),
    geo_path: Option<GeoPath>,
) {
    let (target_x, target_y, target_z) = dest;
    let dx = (target_x - cur.x) as f64;
    let dy = (target_y - cur.y) as f64;
    let distance = (dx * dx + dy * dy).sqrt();
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
            geo_path,
        }),
    );

    let move_pkt =
        server_packets::move_to_location(object_id, target_x, target_y, target_z, start_x, start_y, start_z);
    // The mover's own copy (Java's `includeSelf` override on `Player`); an NPC
    // has no client, and `broadcast_to_others` covers the onlookers either way.
    if world.objects.has_component::<Player>(&object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(move_pkt.clone());
        }
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
    // Java bails while casting, teleporting, or in observer mode (no observer
    // mode yet). The teleporting bail is load-bearing: during a far teleport
    // the client keeps reporting its OLD position until it finishes loading
    // and sends Appearing — without the bail, the out-of-sync snap below
    // reverts the server position to the pre-teleport spot and the client
    // hangs on the black loading screen.
    if objects.has_component::<Casting>(&object_id)
        || objects.get_component::<Player>(&object_id).is_none_or(|p| p.teleporting)
    {
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
    // boundary (Java `setXYZ` → `updateWorldRegion`), and Java's
    // `ValidatePosition` ends with `player.revalidateZone(false)`.
    super::visibility::update_region(world, object_id);
    super::zones::revalidate_zone(world, object_id, false);
}
