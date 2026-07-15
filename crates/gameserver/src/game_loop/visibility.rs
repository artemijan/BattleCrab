//! Player↔player visibility, driven by the world-region grid — the port of
//! Java `World.addVisibleObject` / `switchRegion` / `removeVisibleObject` plus
//! `WorldObject.updateWorldRegion`. Two players see each other exactly while
//! their region cells are within each other's 3×3 surrounding block
//! (`world::regions_adjacent`); entering that block exchanges `CharInfo`,
//! leaving it exchanges `DeleteObject`, and every broadcast helper is scoped
//! by the same rule (`helpers::broadcast_to_others`).

use crate::model::components::{Movement, Position, RegionCell, TargetRef};
use crate::model::Player;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::{region_of, regions_adjacent, World};

use super::helpers::client_for_player;

/// `CreatureAI.describeStateToPlayer`, players-only: right after a `CharInfo`
/// introduces `p`, tell the observer about in-flight state — currently just an
/// ongoing move (without it, a mover entering visibility stands still on the
/// observer's screen until the next `MoveToLocation` broadcast).
fn describe_state(observer: &ClientSession, p: &Player, pos: &Position, movement: Option<&Movement>) {
    if let Some(Movement(m)) = movement {
        observer.send(server_packets::move_to_location(
            p.object_id, m.dest_x, m.dest_y, m.dest_z, pos.x, pos.y, pos.z,
        ));
    }
}

/// `CharInfo` + follow-up state for one player, to one observer session.
/// A GM with `//hide` on (`AdminFlags.hidden`) is not described to others —
/// Java's `isInvisible()` filter in `Creature.broadcastInfo`/knownlist.
fn send_char_info(world: &World, observer: &ClientSession, player_id: i32) {
    if world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&player_id)
        .is_some_and(|f| f.hidden)
    {
        return;
    }
    let Some(v) = crate::model::PlayerView::of(&world.objects, player_id) else { return };
    observer.send(server_packets::char_info(&v));
    describe_state(observer, v.p, v.pos, world.objects.get_component::<Movement>(&player_id));
}

/// Send one NPC's `NpcInfo` to a session (skipping NPCs whose template went
/// missing — can't happen with a consistent datapack).
fn send_npc_info(world: &World, session: &ClientSession, npc_id: i32) {
    let Some(v) = crate::model::npc::NpcView::of(&world.objects, npc_id) else { return };
    let Some(t) = v.npc.template(world) else { return };
    session.send(server_packets::npc_info(&v, t));
}

/// The region cell a player is registered in (`None` once they're gone).
fn player_region(world: &World, object_id: i32) -> Option<(i32, i32)> {
    world.objects.get_component::<RegionCell>(&object_id).map(|r| r.0)
}

/// Java `World.addVisibleObject` for a player spawning in (`EnterWorld` →
/// `spawnMe`): mutual `CharInfo` with every player already visible from the
/// spawn region, plus `NpcInfo` for every NPC in the 3×3 block (NPCs are
/// told nothing — they get aggro/AI eyes in G9).
pub(crate) fn on_enter_world(world: &World, client_id: u32, object_id: i32) {
    let Some(my_region) = player_region(world, object_id) else { return };
    let Some(my_session) = world.clients.get(&client_id) else { return };
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let other_id = s.player_object_id();
            if other_id == object_id {
                continue;
            }
            let Some(other_region) = player_region(world, other_id) else { continue };
            if regions_adjacent(my_region, other_region) {
                send_char_info(world, cs, object_id);
                send_char_info(world, my_session, other_id);
            }
        }
    }
    for npc_id in world.npcs_visible_from(my_region) {
        send_npc_info(world, my_session, npc_id);
    }
    // Doors render like NPCs (Java `Door.sendInfo`: StaticObjectInfo +
    // DoorStatusUpdate).
    for door_id in world.doors_visible_from(my_region) {
        super::doors::send_door_info(world, my_session, door_id);
    }
    for so_id in world.statics_visible_from(my_region) {
        send_static_object_info(world, my_session, so_id);
    }
    for item_id in world.ground_items_visible_from(my_region) {
        if let Some(view) = super::ground_items::ground_item_view(world, item_id) {
            my_session.send(server_packets::spawn_item(&view));
        }
    }
}

/// `StaticObject.sendInfo(player)`.
fn send_static_object_info(world: &World, session: &ClientSession, object_id: i32) {
    if let Some(so) = world.objects.get_component::<crate::model::static_object::StaticObj>(&object_id) {
        session.send(server_packets::static_object_info(so.static_id, so.object_id));
    }
}

/// Java `updateWorldRegion` → `World.switchRegion`: re-derive the region cell
/// from the current position and, when it changed, fire the visibility deltas
/// against every other in-game player — `DeleteObject` both ways for players
/// dropping out of the 3×3 block (clearing dangling targets, as Java's forget
/// event does), `CharInfo` both ways for players entering it. Call after any
/// position mutation (movement tick, `ValidatePosition` snap, future
/// teleports).
pub(crate) fn update_region(world: &mut World, object_id: i32) {
    let Some((pos, region)) = world
        .objects
        .get_many_mut::<(&Position, &mut RegionCell)>(&object_id)
        .map(|(pos, mut region)| {
            let new = region_of(pos.x, pos.y);
            let old = region.0;
            if new != old {
                region.0 = new;
            }
            (new, old)
        })
    else {
        return;
    };
    let (new, old) = (pos, region);
    if new == old {
        return;
    }

    // Visibility deltas vs every other in-game player (client id included so
    // the send phase needs no per-player session scan).
    let mut deltas: Vec<(i32, u32, bool)> = Vec::new(); // (other_id, client_id, appeared)
    for (&cid, cs) in &world.clients {
        if let ClientSession::InGame(s) = cs {
            let other_id = s.player_object_id();
            if other_id == object_id {
                continue;
            }
            let Some(other_region) = player_region(world, other_id) else { continue };
            let was = regions_adjacent(old, other_region);
            let now = regions_adjacent(new, other_region);
            if was != now {
                deltas.push((other_id, cid, now));
            }
        }
    }
    // NPC deltas: NpcInfo for NPCs entering the 3×3 block, DeleteObject (and
    // a dangling-target drop) for NPCs leaving it. The npc_regions index makes
    // this a walk over the (at most) 12 cells whose adjacency changed.
    let my_client = client_for_player(world, object_id);
    if let Some(cs) = my_client.and_then(|cid| world.clients.get(&cid)) {
        for npc_id in world.npcs_visible_from(new) {
            let Some(npc_region) = world.objects.get_component::<RegionCell>(&npc_id) else { continue };
            if !regions_adjacent(old, npc_region.0) {
                send_npc_info(world, cs, npc_id);
            }
        }
        for npc_id in world.npcs_visible_from(old) {
            let Some(npc_region) = world.objects.get_component::<RegionCell>(&npc_id) else { continue };
            if !regions_adjacent(new, npc_region.0) {
                cs.send(server_packets::delete_object(npc_id));
            }
        }
        // Door/static-object deltas, same shape as the NPC ones.
        for door_id in world.doors_visible_from(new) {
            let Some(door_region) = world.objects.get_component::<RegionCell>(&door_id) else { continue };
            if !regions_adjacent(old, door_region.0) {
                super::doors::send_door_info(world, cs, door_id);
            }
        }
        for door_id in world.doors_visible_from(old) {
            let Some(door_region) = world.objects.get_component::<RegionCell>(&door_id) else { continue };
            if !regions_adjacent(new, door_region.0) {
                cs.send(server_packets::delete_object(door_id));
            }
        }
        for so_id in world.statics_visible_from(new) {
            let Some(so_region) = world.objects.get_component::<RegionCell>(&so_id) else { continue };
            if !regions_adjacent(old, so_region.0) {
                send_static_object_info(world, cs, so_id);
            }
        }
        for so_id in world.statics_visible_from(old) {
            let Some(so_region) = world.objects.get_component::<RegionCell>(&so_id) else { continue };
            if !regions_adjacent(new, so_region.0) {
                cs.send(server_packets::delete_object(so_id));
            }
        }
        // Ground-item deltas, same shape (SpawnItem in / DeleteObject out).
        for item_id in world.ground_items_visible_from(new) {
            let Some(r) = world.objects.get_component::<RegionCell>(&item_id) else { continue };
            if !regions_adjacent(old, r.0) {
                if let Some(view) = super::ground_items::ground_item_view(world, item_id) {
                    cs.send(server_packets::spawn_item(&view));
                }
            }
        }
        for item_id in world.ground_items_visible_from(old) {
            let Some(r) = world.objects.get_component::<RegionCell>(&item_id) else { continue };
            if !regions_adjacent(new, r.0) {
                cs.send(server_packets::delete_object(item_id));
            }
        }
    }
    if let Some(TargetRef(Some(target))) = world.objects.get_component::<TargetRef>(&object_id).copied() {
        if let Some(npc_region) = world.objects.get_component::<RegionCell>(&target) {
            if !regions_adjacent(new, npc_region.0) {
                world.objects.get_component_mut::<TargetRef>(&object_id).expect("checked above").0 = None;
            }
        }
    }

    for (other_id, other_client, appeared) in deltas {
        if appeared {
            if let Some(cs) = world.clients.get(&other_client) {
                send_char_info(world, cs, object_id);
            }
            if let Some(cs) = my_client.and_then(|cid| world.clients.get(&cid)) {
                send_char_info(world, cs, other_id);
            }
        } else {
            if let Some(cs) = world.clients.get(&other_client) {
                cs.send(server_packets::delete_object(object_id));
            }
            if let Some(cs) = my_client.and_then(|cid| world.clients.get(&cid)) {
                cs.send(server_packets::delete_object(other_id));
            }
            if let Some(other) = world.objects.get_component_mut::<TargetRef>(&other_id) {
                if other.0 == Some(object_id) {
                    other.0 = None;
                }
            }
            if let Some(me) = world.objects.get_component_mut::<TargetRef>(&object_id) {
                if me.0 == Some(other_id) {
                    me.0 = None;
                }
            }
        }
    }
}

/// Java `World.removeVisibleObject` for a player leaving the world (logout /
/// restart / disconnect): `DeleteObject` to every player that could see them,
/// dropping dangling targets. The Java side also deletes every visible object
/// from the *leaver's* screen; their client is leaving the game scene anyway
/// (and the session may already be gone on the restart path), so that
/// direction is skipped. Call *before* removing the player from
/// `world.objects`.
pub(crate) fn on_leave_world(world: &mut World, object_id: i32) {
    let Some(region) = player_region(world, object_id) else { return };
    let mut observers: Vec<i32> = Vec::new();
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let other_id = s.player_object_id();
            if other_id == object_id {
                continue;
            }
            let Some(other_region) = player_region(world, other_id) else { continue };
            if regions_adjacent(region, other_region) {
                cs.send(server_packets::delete_object(object_id));
                observers.push(other_id);
            }
        }
    }
    for other_id in observers {
        if let Some(other) = world.objects.get_component_mut::<TargetRef>(&other_id) {
            if other.0 == Some(object_id) {
                other.0 = None;
            }
        }
    }
}

/// Java `updateWorldRegion`/`switchRegion` for a *moving NPC* (G9): re-derive
/// its region cell, re-index `World.npc_regions`, and send `NpcInfo` /
/// `DeleteObject` deltas to players whose 3×3 adjacency changed (dropping
/// dangling targets, as the player-side path does).
pub(crate) fn update_npc_region(world: &mut World, npc_object_id: i32) {
    let Some((new, old)) = world
        .objects
        .get_many_mut::<(&Position, &mut RegionCell)>(&npc_object_id)
        .map(|(pos, mut region)| {
            let new = region_of(pos.x, pos.y);
            let old = region.0;
            if new != old {
                region.0 = new;
            }
            (new, old)
        })
    else {
        return;
    };
    if new == old {
        return;
    }
    if let Some(ids) = world.npc_regions.get_mut(&old) {
        ids.retain(|&id| id != npc_object_id);
    }
    world.npc_regions.entry(new).or_default().push(npc_object_id);

    let mut lost_watchers: Vec<i32> = Vec::new();
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let player_id = s.player_object_id();
            let Some(player_region) = player_region(world, player_id) else { continue };
            let was = regions_adjacent(old, player_region);
            let now = regions_adjacent(new, player_region);
            if !was && now {
                send_npc_info(world, cs, npc_object_id);
            } else if was && !now {
                cs.send(server_packets::delete_object(npc_object_id));
                lost_watchers.push(player_id);
            }
        }
    }
    for player_id in lost_watchers {
        if let Some(p) = world.objects.get_component_mut::<TargetRef>(&player_id) {
            if p.0 == Some(npc_object_id) {
                p.0 = None;
            }
        }
    }
}

/// The per-tick movement system plus Java's `setXYZ` → `updateWorldRegion`
/// coupling: advance every mover (`movement::tick`), then fire region switches
/// for anyone whose cell changed. `update_region` early-outs on an unchanged
/// cell, so the sweep is a cheap comparison per player on quiet ticks.
pub(crate) fn movement_tick(world: &mut World) {
    let outcome = crate::model::movement::tick(world);
    let mut ids: Vec<i32> = Vec::new();
    world.objects.for_each_mut::<&Player>(|p| ids.push(p.object_id));
    for id in ids {
        update_region(world, id);
        // Java couples `updatePosition` with `revalidateZone(false)` — the
        // 100-unit `last_validate` filter inside makes this cheap per tick.
        super::zones::revalidate_zone(world, id, false);
    }
    for id in outcome.moved_npcs {
        update_npc_region(world, id);
    }
    // Route advances need a `MoveToLocation` for the new segment — unlike
    // plain moves, the client only knows the previous segment's endpoint
    // (Java `moveToNextRoutePoint` → `broadcastMoveToLocation`).
    for id in outcome.route_advanced {
        let Some(Movement(m)) = world.objects.get_component::<Movement>(&id) else { continue };
        let pkt = server_packets::move_to_location(id, m.dest_x, m.dest_y, m.dest_z, m.start_x, m.start_y, m.start_z);
        super::helpers::broadcast_including_self(world, id, &pkt);
    }
}
