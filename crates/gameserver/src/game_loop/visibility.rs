//! Player↔player visibility, driven by the world-region grid — the port of
//! Java `World.addVisibleObject` / `switchRegion` / `removeVisibleObject` plus
//! `WorldObject.updateWorldRegion`. Two players see each other exactly while
//! their region cells are within each other's 3×3 surrounding block
//! (`world::regions_adjacent`); entering that block exchanges `CharInfo`,
//! leaving it exchanges `DeleteObject`, and every broadcast helper is scoped
//! by the same rule (`helpers::broadcast_to_others`).

use crate::model::Player;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::{region_of, regions_adjacent, World};

use super::helpers::client_for_player;

/// `CreatureAI.describeStateToPlayer`, players-only: right after a `CharInfo`
/// introduces `p`, tell the observer about in-flight state — currently just an
/// ongoing move (without it, a mover entering visibility stands still on the
/// observer's screen until the next `MoveToLocation` broadcast).
fn describe_state(observer: &ClientSession, p: &Player) {
    if let Some(m) = &p.move_data {
        observer.send(server_packets::move_to_location(
            p.object_id, m.dest_x, m.dest_y, m.dest_z, p.x, p.y, p.z,
        ));
    }
}

/// Send one NPC's `NpcInfo` to a session (skipping NPCs whose template went
/// missing — can't happen with a consistent datapack).
fn send_npc_info(world: &World, session: &ClientSession, npc_id: i32) {
    let Some(npc) = world.npcs.get(&npc_id) else { return };
    let Some(t) = npc.template(world) else { return };
    session.send(server_packets::npc_info(npc, t));
}

/// Java `World.addVisibleObject` for a player spawning in (`EnterWorld` →
/// `spawnMe`): mutual `CharInfo` with every player already visible from the
/// spawn region, plus `NpcInfo` for every NPC in the 3×3 block (NPCs are
/// told nothing — they get aggro/AI eyes in G9).
pub(crate) fn on_enter_world(world: &World, client_id: u32, object_id: i32) {
    let Some(me) = world.players.get(&object_id) else { return };
    let Some(my_session) = world.clients.get(&client_id) else { return };
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let other_id = s.player_object_id();
            if other_id == object_id {
                continue;
            }
            let Some(other) = world.players.get(&other_id) else { continue };
            if regions_adjacent(me.region, other.region) {
                cs.send(server_packets::char_info(me));
                my_session.send(server_packets::char_info(other));
                describe_state(my_session, other);
            }
        }
    }
    for npc_id in world.npcs_visible_from(me.region) {
        send_npc_info(world, my_session, npc_id);
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
    let Some(p) = world.players.get(&object_id) else { return };
    let new = region_of(p.x, p.y);
    let old = p.region;
    if new == old {
        return;
    }
    world.players.get_mut(&object_id).expect("checked above").region = new;

    // Visibility deltas vs every other in-game player (client id included so
    // the send phase needs no per-player session scan).
    let mut deltas: Vec<(i32, u32, bool)> = Vec::new(); // (other_id, client_id, appeared)
    for (&cid, cs) in &world.clients {
        if let ClientSession::InGame(s) = cs {
            let other_id = s.player_object_id();
            if other_id == object_id {
                continue;
            }
            let Some(other) = world.players.get(&other_id) else { continue };
            let was = regions_adjacent(old, other.region);
            let now = regions_adjacent(new, other.region);
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
            let npc_region = world.npcs[&npc_id].region;
            if !regions_adjacent(old, npc_region) {
                send_npc_info(world, cs, npc_id);
            }
        }
        for npc_id in world.npcs_visible_from(old) {
            let npc_region = world.npcs[&npc_id].region;
            if !regions_adjacent(new, npc_region) {
                cs.send(server_packets::delete_object(npc_id));
            }
        }
    }
    if let Some(me) = world.players.get(&object_id) {
        if let Some(target) = me.target {
            if let Some(npc) = world.npcs.get(&target) {
                if !regions_adjacent(new, npc.region) {
                    world.players.get_mut(&object_id).expect("checked above").target = None;
                }
            }
        }
    }

    for (other_id, other_client, appeared) in deltas {
        if appeared {
            if let (Some(me), Some(other)) = (world.players.get(&object_id), world.players.get(&other_id)) {
                if let Some(cs) = world.clients.get(&other_client) {
                    cs.send(server_packets::char_info(me));
                    describe_state(cs, me);
                }
                if let Some(cs) = my_client.and_then(|cid| world.clients.get(&cid)) {
                    cs.send(server_packets::char_info(other));
                    describe_state(cs, other);
                }
            }
        } else {
            if let Some(cs) = world.clients.get(&other_client) {
                cs.send(server_packets::delete_object(object_id));
            }
            if let Some(cs) = my_client.and_then(|cid| world.clients.get(&cid)) {
                cs.send(server_packets::delete_object(other_id));
            }
            if let Some(other) = world.players.get_mut(&other_id) {
                if other.target == Some(object_id) {
                    other.target = None;
                }
            }
            if let Some(me) = world.players.get_mut(&object_id) {
                if me.target == Some(other_id) {
                    me.target = None;
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
/// `world.players`.
pub(crate) fn on_leave_world(world: &mut World, object_id: i32) {
    let Some(p) = world.players.get(&object_id) else { return };
    let region = p.region;
    let mut observers: Vec<i32> = Vec::new();
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let other_id = s.player_object_id();
            if other_id == object_id {
                continue;
            }
            let Some(other) = world.players.get(&other_id) else { continue };
            if regions_adjacent(region, other.region) {
                cs.send(server_packets::delete_object(object_id));
                observers.push(other_id);
            }
        }
    }
    for other_id in observers {
        if let Some(other) = world.players.get_mut(&other_id) {
            if other.target == Some(object_id) {
                other.target = None;
            }
        }
    }
}

/// Java `updateWorldRegion`/`switchRegion` for a *moving NPC* (G9): re-derive
/// its region cell, re-index `World.npc_regions`, and send `NpcInfo` /
/// `DeleteObject` deltas to players whose 3×3 adjacency changed (dropping
/// dangling targets, as the player-side path does).
pub(crate) fn update_npc_region(world: &mut World, npc_object_id: i32) {
    let Some(npc) = world.npcs.get(&npc_object_id) else { return };
    let new = region_of(npc.x, npc.y);
    let old = npc.region;
    if new == old {
        return;
    }
    world.npcs.get_mut(&npc_object_id).expect("checked above").region = new;
    if let Some(ids) = world.npc_regions.get_mut(&old) {
        ids.retain(|&id| id != npc_object_id);
    }
    world.npc_regions.entry(new).or_default().push(npc_object_id);

    let mut lost_watchers: Vec<i32> = Vec::new();
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let player_id = s.player_object_id();
            let Some(player) = world.players.get(&player_id) else { continue };
            let was = regions_adjacent(old, player.region);
            let now = regions_adjacent(new, player.region);
            if !was && now {
                send_npc_info(world, cs, npc_object_id);
            } else if was && !now {
                cs.send(server_packets::delete_object(npc_object_id));
                lost_watchers.push(player_id);
            }
        }
    }
    for player_id in lost_watchers {
        if let Some(p) = world.players.get_mut(&player_id) {
            if p.target == Some(npc_object_id) {
                p.target = None;
            }
        }
    }
}

/// The per-tick movement system plus Java's `setXYZ` → `updateWorldRegion`
/// coupling: advance every mover (`movement::tick`), then fire region switches
/// for anyone whose cell changed. `update_region` early-outs on an unchanged
/// cell, so the sweep is a cheap comparison per player on quiet ticks.
pub(crate) fn movement_tick(world: &mut World) {
    crate::model::movement::tick(world);
    let ids: Vec<i32> = world.players.keys().copied().collect();
    for id in ids {
        update_region(world, id);
    }
    for id in crate::model::movement::tick_npcs(world) {
        update_npc_region(world, id);
    }
}
