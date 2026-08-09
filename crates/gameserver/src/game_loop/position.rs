//! Movement/position handlers (`MoveBackwardToLocation`, `RequestStopMove`,
//! `ValidatePosition`) and the path-worker reply handler (`handle_path_result`).

use crate::game_loop::guard::position;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::send_action_failed;
use crate::game_loop::helpers::set_position;
use crate::game_loop::helpers::set_position_heading;
use crate::geo::worker::{PathEvent, PathRequest};
use crate::model::Player;
use crate::model::components::{
    AttackState, Casting, ClientPos, Intent, Movement, PathWait, Position, QueuedAction, Speeds,
};
use crate::model::movement::GeoPath;
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
/// Door-crossing is skipped as out of scope; the GM teleport-mode switch is
/// ported (see [`take_admin_tele_mode`]). Java's "remove queued skill upon move request" is
/// covered by the busy branch overwriting the `QueuedAction` slot — outside
/// a cast/swing the slot is always empty, so there is nothing to clear.
pub(crate) fn handle_move_backward_to_location(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::MoveBackwardToLocation::read(body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    if world.objects.get_component::<Player>(&object_id).is_none() {
        return;
    }
    let Some(cur) = position(world, object_id) else {
        return;
    };

    if pkt.target_x == pkt.origin_x && pkt.target_y == pkt.origin_y && pkt.target_z == pkt.origin_z
    {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::stop_move(
                object_id,
                cur.x,
                cur.y,
                cur.z,
                cur.heading,
            ));
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

    // Java `MoveBackwardToLocation`'s `switch (player.getTeleMode())`, armed
    // from the GM "Additional Movement Options" window (`//instant_move`,
    // `//teleto sayune|charge`). It sits here — *before* the movement-disabled
    // / rest / dead gates, which live inside `PlayerAI.onIntentionMoveTo`, i.e.
    // inside the `default:` arm — so an armed GM warps even while stunned or
    // seated. `NORMAL` falls through to the ordinary walk below.
    if take_admin_tele_mode(
        world,
        object_id,
        (pkt.target_x, pkt.target_y, target_z),
        client_id,
    ) {
        return;
    }

    // Stunned/asleep/paralyzed or rooted players can't move either — the rest
    // of `isMovementDisabled`'s effect-driven terms.
    if super::abnormal::is_movement_disabled(world, object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::stop_move(
                object_id,
                cur.x,
                cur.y,
                cur.z,
                cur.heading,
            ));
            cs.send(server_packets::action_failed());
        }
        return;
    }
    // `PlayerAI.onIntentionMoveTo`'s first branch: `if (getIntention() ==
    // AI_INTENTION_REST) { clientActionFailed(); return; }` — a seated player
    // stays put, and stays seated: Java neither stands them up nor moves them,
    // it just drops the click. The refusal outlives the 2.5 s sit animation and
    // covers the 2.5 s stand-up animation too, since REST is only released by
    // `StandUpTask` (which clears the seated flag in the same breath).
    if super::sit_stand::is_resting(world, object_id) {
        send_action_failed(world, client_id);
        return;
    }
    // Dead players can't move at all (`isMovementDisabled`).
    if is_dead(world, object_id) {
        send_action_failed(world, client_id);
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
        world.objects.add_components(
            &object_id,
            QueuedAction::Move {
                x: pkt.target_x,
                y: pkt.target_y,
                z: target_z,
            },
        );
        send_action_failed(world, client_id);
        return;
    }
    // A manual move click replaces an attack loop (MOVE_TO intention).
    if world.objects.has_component::<Intent>(&object_id) {
        world.objects.remove_component::<Intent>(&object_id);
    }

    intention_move_to(
        world,
        client_id,
        object_id,
        cur,
        (pkt.target_x, pkt.target_y, target_z),
    );
}

/// The GM click-to-move latch (`Player.getTeleMode()`) armed from the
/// "Additional Movement Options" window — `MoveBackwardToLocation`'s
/// `switch (teleMode)`. Returns `true` when the click was consumed by a mode
/// and must not start an ordinary walk.
///
/// The three armed modes differ in how the GM travels and in whether the latch
/// survives the click:
///
/// | mode | travel | latch after |
/// |---|---|---|
/// | `DEMONIC` | `teleToLocation` — full teleport, loading screen | cleared |
/// | `SAYUNE` | `setXYZ` — silent slide, no loading screen | cleared |
/// | `CHARGE` | `setXYZ` + charge animation (skill 30012) | **kept** |
///
/// `CHARGE` keeping the latch is Java's, not an oversight here: its arm is the
/// only one of the three that never calls `setTeleMode(NORMAL)`, so charge mode
/// stays on until "Normal mode" (`//teleto end`) turns it off.
fn take_admin_tele_mode(
    world: &mut World,
    object_id: i32,
    dest: (i32, i32, i32),
    client_id: u32,
) -> bool {
    let mode = world
        .objects
        .get_component::<Player>(&object_id)
        .map_or(crate::enums::AdminTeleportType::Normal, |p| p.tele_mode);
    let (x, y, z) = dest;
    match mode {
        crate::enums::AdminTeleportType::Normal => return false,
        crate::enums::AdminTeleportType::Demonic => {
            send_action_failed(world, client_id);
            super::death::teleport_player(world, object_id, x, y, z);
            set_tele_mode(world, object_id, crate::enums::AdminTeleportType::Normal);
        }
        crate::enums::AdminTeleportType::Sayune => {
            // Java sends `ExFlyMove` to the GM and `ExFlyMoveBroadcast` to
            // everyone else around the Sayune hop, then `setXYZ`.
            //
            // SKIP(protocol): those two are `0xFE:0xE8` / `0xFE:0x108`,
            // Ertheia-era ex-opcodes with no counterpart in the Interlude
            // protocol this server speaks. This is not deferred work — the
            // client on the other end has no handler for them, so there is
            // nothing to send and nothing a later milestone could change. The
            // hop itself (the `setXYZ` half) is ported; the port substitutes a
            // `FlyToLocation(DUMMY)` — the Interlude "slide, no animation"
            // packet — so the client actually follows the server instead of
            // staying put and being snapped back.
            slide_to(world, object_id, dest, server_packets::FlyType::Dummy);
            set_tele_mode(world, object_id, crate::enums::AdminTeleportType::Normal);
        }
        crate::enums::AdminTeleportType::Charge => {
            // Java: `setXYZ` first, then MagicSkillUse(30012, lvl 10, 500ms) →
            // FlyToLocation(CHARGE) → MagicSkillLaunched, all to self and known
            // players, then ActionFailed. Because `setXYZ` runs first, Java's
            // `FlyToLocation` constructor reads the *destination* as its origin
            // — the client flies to `dest` regardless, so the port keeps that
            // ordering rather than "fixing" the origin.
            let Some(pos) = position(world, object_id) else {
                return true;
            };
            let skill_use = world
                .objects
                .get_component::<Player>(&object_id)
                .map(|p| {
                    let mut at = pos;
                    at.x = x;
                    at.y = y;
                    at.z = z;
                    server_packets::magic_skill_use(
                        p,
                        &at,
                        (object_id, x, y, z),
                        CHARGE_SKILL_ID,
                        CHARGE_SKILL_LEVEL,
                        500,
                        -1,
                        0,
                    )
                })
                .unwrap_or_default();
            slide_to(world, object_id, dest, server_packets::FlyType::Charge);
            // The two skill packets bracket the fly in Java; the fly itself is
            // sent by `slide_to`, which also moves the server position.
            broadcast_including_self(world, object_id, &skill_use);
            let launched = server_packets::magic_skill_launched(
                object_id,
                CHARGE_SKILL_ID,
                CHARGE_SKILL_LEVEL,
                &[object_id],
            );
            broadcast_including_self(world, object_id, &launched);
            send_action_failed(world, client_id);
        }
    }
    true
}

/// Java `AdminTeleport`'s charge animation — "Rush Impact"-style flourish the
/// CHARGE tele mode plays on arrival (`new MagicSkillUse(player, 30012, 10,
/// 500, 0)`).
const CHARGE_SKILL_ID: i32 = 30012;
const CHARGE_SKILL_LEVEL: i32 = 10;

fn set_tele_mode(world: &mut World, object_id: i32, mode: crate::enums::AdminTeleportType) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.tele_mode = mode;
    }
}

/// Java `Creature.setXYZ` + a `FlyToLocation` — move the player without a
/// teleport (no fade, no decay/respawn round trip) and let the clients animate
/// the slide. `FlyToLocation`'s constructor arms `blinkActive`, which makes the
/// next `ValidatePosition` skip its out-of-sync snap so the slide survives the
/// client's stale position report.
fn slide_to(
    world: &mut World,
    object_id: i32,
    dest: (i32, i32, i32),
    fly_type: server_packets::FlyType,
) {
    let Some(from) = position(world, object_id) else {
        return;
    };
    let (x, y, z) = dest;
    set_position(world, object_id, (x, y, z));
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.blink_active = true;
    }
    // A slide leaves any in-flight walk behind: without dropping it the mover
    // keeps interpolating from the new point toward the old destination.
    world.objects.remove_component::<Movement>(&object_id);
    world.objects.remove_component::<PathWait>(&object_id);
    broadcast_including_self(
        world,
        object_id,
        &server_packets::fly_to_location(object_id, (from.x, from.y, from.z), dest, fly_type),
    );
    super::visibility::update_region(world, object_id);
    super::zones::revalidate_zone(world, object_id, false);
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
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    let Some(cur) = position(world, object_id) else {
        return;
    };

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
pub(crate) fn handle_ex_send_selected_quest_zone_id(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(quest_zone_id) = cp::read_selected_quest_zone_id(ex_body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    if let Some(player) = world.objects.get_component_mut::<Player>(&object_id) {
        player.quest_zone_id = quest_zone_id;
    }
}

/// Java `Creature.moveToLocation`'s `isInWater` local:
/// `isInsideZone(ZoneId.WATER) && !isInsideZone(ZoneId.CASTLE)`. **Not** the
/// same predicate as [`Speeds::swimming`] — the speed branch has no castle
/// exception, so a player in a castle moat swims at swim speed but still moves
/// under geodata. Nor is it `Player.isInWater()`, which in Java means "the
/// drowning task is running" (see [`super::water`]).
pub(crate) fn is_in_water(world: &World, object_id: i32) -> bool {
    if !world
        .objects
        .get_component::<Speeds>(&object_id)
        .is_some_and(|s| s.swimming)
    {
        return false;
    }
    let Some(pos) = world.objects.get_component::<Position>(&object_id) else {
        return false;
    };
    !world.data.zone_data.in_castle_zone(pos.x, pos.y, pos.z)
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
    let (mut target_x, mut target_y, mut target_z) = target;
    let mut dx = (target_x - cur.x) as f64;
    let mut dy = (target_y - cur.y) as f64;
    if dx * dx + dy * dy > 98_010_000.0 {
        // 9900² — Java's max single-click move distance.
        send_action_failed(world, client_id);
        return;
    }
    let mut distance = (dx * dx + dy * dy).sqrt();

    // Java `Creature.moveToLocation` gates the whole geodata section on
    // `!_isFlying && !isInWater` (`isInWater` = WATER zone, minus castle
    // moats): a wyvern rider or swimmer moves in a straight 3D line — no
    // destination clamp, no pathfinder, no "no path found" abort. Without
    // this exemption every flight click was snapped back to the terrain.
    let is_flying = world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(Player::is_flying);
    let in_water = is_in_water(world, object_id);
    let floating = is_flying || in_water;

    // "Make water move short and use no geodata checks for swimming chars
    // distance in a click can easily be over 3000." A swim click is cut to the
    // first 700 units of the ray (all three axes scaled), so a long-range click
    // underwater becomes a series of short legs the client can keep up with
    // instead of one unverified 3000-unit glide.
    if in_water && distance > 700.0 {
        let divider = 700.0 / distance;
        let dz = (target_z - cur.z) as f64;
        target_x = cur.x + (divider * dx) as i32;
        target_y = cur.y + (divider * dy) as i32;
        target_z = cur.z + (divider * dz) as i32;
        dx = (target_x - cur.x) as f64;
        dy = (target_y - cur.y) as f64;
        distance = (dx * dx + dy * dy).sqrt();
    }

    // GEODATA MOVEMENT CHECKS AND PATHFINDING (`Creature.moveToLocation`).
    let (original_x, original_y, original_z) = (target_x, target_y, target_z);
    let original_distance = distance;
    if world.path_finding > 0 && !floating {
        // A re-click onto the geo cell we're already pathing to is ignored;
        // a click elsewhere abandons route following on the in-flight move
        // (Java `isOnGeodataPath()` → same gtx/gty return / index = -1).
        let gtx = world.geo.get_geo_x(original_x);
        let gty = world.geo.get_geo_y(original_y);
        if let Some(mv) = world.objects.get_component_mut::<Movement>(&object_id)
            && let Some(gp) = &mv.0.geo_path
            && gp.has_next()
        {
            if gp.gtx == gtx && gp.gty == gty {
                return;
            }
            mv.0.geo_path = None;
        }
    }

    // Java skips the destination correction for far clicks (> 3000: "should
    // be able to click far away and move") and for intentional falls
    // ((curZ - z) > 300 with distance < 300).
    if world.path_finding > 0
        && !floating
        && distance <= 3000.0
        && !(cur.z - target_z > 300 && distance < 300.0)
    {
        let (vx, vy, _vz) = world
            .geo
            .get_valid_location(cur.x, cur.y, cur.z, target_x, target_y, target_z);
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
    if world.path_finding > 0 && !floating && (original_distance - distance) > 30.0 {
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
    // a fully clamped-away (or degenerate) move is canceled. Exception:
    // `verticalMovementOnly` (flying, dx=dy=0, dz≠0) sets `distance = |dz|`
    // first, so a straight up/down flight click goes through.
    if distance < 1.0 && !(is_flying && target_z != cur.z) {
        send_action_failed(world, client_id);
        return;
    }

    start_move(
        world,
        client_id,
        object_id,
        cur,
        (target_x, target_y, target_z),
        None,
    );
}

/// The path worker's reply (`geo::worker::PathEvent`): start the route move,
/// or tell the player the click leads nowhere. Java reaches the same two
/// outcomes inline in `Creature.moveToLocation` ("if found" / "No path
/// found" + ActionFailed); the extra liveness re-checks cover state changes
/// during the round-trip, which the synchronous Java flow can't see.
pub(crate) fn handle_path_result(world: &mut World, ev: PathEvent) {
    let PathEvent {
        seq,
        client_id,
        object_id,
        to,
        path,
    } = ev;
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
            if is_player && let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::action_failed());
            }
            return;
        }
    };

    // Move gates re-checked after the round-trip (same set as the click).
    let is_dead = is_dead(world, object_id);
    if world.objects.has_component::<Casting>(&object_id) || is_dead {
        if is_player && let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    let Some(cur) = position(world, object_id) else {
        return;
    };

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
    let mut distance = (dx * dx + dy * dy).sqrt();
    // Java: when floating (flying / swimming) the Z leg is real travel, so it
    // counts toward the move duration (`distance = Math.hypot(distance, dz)`).
    // Same `_isFlying || isInWater` pair as the geodata gate — castle moats
    // included, so a moat crossing is timed as flat ground like Java does.
    let floating = world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(Player::is_flying)
        || is_in_water(world, object_id);
    if floating {
        let dz = (target_z - cur.z) as f64;
        distance = (distance * distance + dz * dz).sqrt();
    }
    let (start_x, start_y, start_z) = (cur.x, cur.y, cur.z);
    let heading = crate::model::movement::calculate_heading(dx, dy);
    let Some(speed) = world
        .objects
        .get_component::<Speeds>(&object_id)
        .map(Speeds::move_speed)
    else {
        return;
    };
    let total_ticks = if speed > 0.0 {
        ((10.0 * distance / speed).round() as u64).max(1)
    } else {
        1
    };
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

    let move_pkt = server_packets::move_to_location(
        object_id, target_x, target_y, target_z, start_x, start_y, start_z,
    );
    // The mover's own copy (Java's `includeSelf` override on `Player`); an NPC
    // has no client, and `broadcast_to_others` covers the onlookers either way.
    if world.objects.has_component::<Player>(&object_id)
        && let Some(cs) = world.clients.get(&client_id)
    {
        cs.send(move_pkt.clone());
    }
    broadcast_to_others(world, object_id, &move_pkt);
}

/// Port of `clientpackets/ValidatePosition.runImpl` — reconcile the client's
/// periodic position report with the server's authoritative position.
/// Narrowing: no vehicles, falling state, observer mode, or Blink, and the
/// trailing door-exploit check is skipped (no doors) — those branches simply
/// can't trigger yet. Flying (wyvern) and swimming take Java's trust-the-
/// client-Z branch below.
pub(crate) fn handle_validate_position(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::ValidatePosition::read(body) else {
        return;
    };
    // Field-level split borrow: `player`+`pos` (mut) + `geo`/`clients` (shared).
    let World {
        clients,
        objects,
        geo,
        ..
    } = world;
    let Some(ClientSession::InGame(session)) = clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    // Java bails while casting, teleporting, or in observer mode (no observer
    // mode yet). The teleporting bail is load-bearing: during a far teleport
    // the client keeps reporting its OLD position until it finishes loading
    // and sends Appearing — without the bail, the out-of-sync snap below
    // reverts the server position to the pre-teleport spot and the client
    // hangs on the black loading screen.
    if objects.has_component::<Casting>(&object_id)
        || objects
            .get_component::<Player>(&object_id)
            .is_none_or(|p| p.teleporting)
    {
        return;
    }
    let Some((mut player, mut pos, speeds, mut client)) =
        objects.get_many_mut::<(&mut Player, &mut Position, &Speeds, &mut ClientPos)>(&object_id)
    else {
        return;
    };

    if pkt.x == 0 && pkt.y == 0 && pos.x != 0 {
        return;
    }

    let dx = (pkt.x - pos.x) as f64;
    let dy = (pkt.y - pos.y) as f64;
    let dz = (pkt.z - pos.z) as f64;
    let diff_sq = dx * dx + dy * dy;

    let mut correction: Option<Vec<u8>> = None;
    if player.is_flying() || speeds.swimming {
        // Java: flying/swimming trusts the client's Z outright (`setXYZ(realX,
        // realY, _z)`) — there is no floor to re-ground against — and only a
        // large *horizontal* drift (> 300 units) gets pushed back. Without
        // this branch the geo snap below kept yanking a climbing wyvern down
        // to terrain height.
        pos.z = pkt.z;
        if diff_sq > 90_000.0 {
            correction = Some(server_packets::validate_location(
                object_id,
                pos.x,
                pos.y,
                pos.z,
                pos.heading,
            ));
        }
    } else if diff_sq < 360_000.0 && (diff_sq > 250_000.0 || dz.abs() > 200.0) {
        // "If too large, messes observation" — moderate drift only.
        if dz.abs() > 200.0 && dz.abs() < 1500.0 && (pkt.z - client.z).abs() < 800 {
            // Plausible stairs/slope climb: trust the client's z.
            pos.z = pkt.z;
        } else {
            // Push the server position back to the client (built pre-snap,
            // exactly where Java builds the packet).
            correction = Some(server_packets::validate_location(
                object_id,
                pos.x,
                pos.y,
                pos.z,
                pos.heading,
            ));
        }
    }

    // Out-of-sync check: a jump larger than one second of movement snaps the
    // server to the client position, geodata-correcting z when the server
    // was above the client (falling through a floor edge). Java guards this
    // with `isBlinkActive()`: right after a `FlyToLocation` the client is still
    // reporting its pre-fly position, and adopting it would undo the slide the
    // server just performed — so the first such report is swallowed and only
    // clears the flag.
    let sdx = (pkt.x - pos.x) as f64;
    let sdy = (pkt.y - pos.y) as f64;
    let sdz = (pkt.z - pos.z) as f64;
    let move_speed = speeds.move_speed();
    if (sdx * sdx + sdy * sdy + sdz * sdz).sqrt() > move_speed {
        if player.blink_active {
            player.blink_active = false;
        } else {
            let z = if pos.z > pkt.z {
                geo.get_height(pkt.x, pkt.y, pos.z)
            } else {
                pkt.z
            };
            pos.x = pkt.x;
            pos.y = pkt.y;
            pos.z = z;
        }
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

/// Port of `clientpackets/CannotMoveAnymore.runImpl` → the AI's
/// `EVT_ARRIVED_BLOCKED`: the client reports that the move it was walking is
/// blocked, at the location it actually reached.
///
/// Java's `CreatureAI.onEvtArrivedBlocked` drops a `MOVE_TO`/`CAST` intention
/// back to `ACTIVE`, stops the movement server- and client-side at that
/// location, and re-thinks. The port does the same: the in-flight move (and any
/// pending path request) is dropped, the intent is cleared for those two kinds,
/// the player is placed where the client says it stopped, and `StopMove` goes
/// out to everyone including the mover.
pub(crate) fn handle_cannot_move_anymore(world: &mut World, client_id: u32, body: &[u8]) {
    let mut r = commons::network::PacketReader::new(body);
    let (Some(x), Some(y), Some(z), Some(heading)) =
        (r.read_i32(), r.read_i32(), r.read_i32(), r.read_i32())
    else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };

    world.objects.remove_component::<Movement>(&object_id);
    world.objects.remove_component::<PathWait>(&object_id);
    // `if (getIntention() == MOVE_TO || getIntention() == CAST) setIntention(ACTIVE)`
    // — an attack or interact intention survives, and its own think re-issues
    // the walk.
    let clear = matches!(
        world
            .objects
            .get_component::<Intent>(&object_id)
            .map(|i| i.0),
        Some(crate::model::PlayerIntent::Cast { .. })
    );
    if clear {
        world.objects.remove_component::<Intent>(&object_id);
    }

    // `clientStopMoving(location)`: land where the client says it stopped.
    set_position_heading(world, object_id, (x, y, z), heading);
    super::zones::revalidate_zone(world, object_id, true);
    broadcast_including_self(
        world,
        object_id,
        &server_packets::stop_move(object_id, x, y, z, heading),
    );
}
