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
fn describe_state(
    observer: &ClientSession,
    p: &Player,
    pos: &Position,
    movement: Option<&Movement>,
) {
    if let Some(Movement(m)) = movement {
        observer.send(server_packets::move_to_location(
            p.object_id,
            m.dest_x,
            m.dest_y,
            m.dest_z,
            pos.x,
            pos.y,
            pos.z,
        ));
    }
}

/// Re-send a player's `CharInfo` to everyone who can currently see them.
///
/// Used when a field that only `CharInfo` carries changes — the cubic id list
/// has no incremental packet on this chronicle, so the whole record goes again.
pub(crate) fn refresh_char_info(world: &World, player_id: i32) {
    use crate::model::components::RegionCell;
    let Some(from) = world
        .objects
        .get_component::<RegionCell>(&player_id)
        .copied()
    else {
        return;
    };
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let other_id = s.player_object_id();
            if other_id == player_id {
                continue;
            }
            let Some(other) = world.objects.get_component::<RegionCell>(&other_id) else {
                continue;
            };
            if crate::world::regions_adjacent(from.0, other.0) {
                send_char_info(world, cs, player_id);
            }
        }
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
    let Some(v) = crate::model::PlayerView::of(&world.objects, player_id) else {
        return;
    };
    let cubics = world
        .objects
        .get_component::<super::cubic::Cubics>(&player_id)
        .map(|c| c.ids())
        .unwrap_or_default();
    observer.send(server_packets::char_info(
        &v,
        &super::abnormal::visual_effects(world, player_id),
        &cubics,
    ));
    // Java `Player.sendInfo` pairs each CharInfo with a RelationChanged, so the
    // viewer learns the relation bits CharInfo can't carry — notably the
    // clan-leader crown (`RELATION_LEADER`). Without it a leader shows no crown
    // to others outside a siege.
    if let ClientSession::InGame(s) = observer {
        observer.send(super::pvp::sendinfo_relation_changed(
            world,
            player_id,
            s.player_object_id(),
        ));
    }
    describe_state(
        observer,
        v.p,
        v.pos,
        world.objects.get_component::<Movement>(&player_id),
    );
}

/// Send one NPC's `NpcInfo` to a session (skipping NPCs whose template went
/// missing — can't happen with a consistent datapack).
/// A servitor is introduced with `SummonInfo` rather than `NpcInfo` — the
/// client needs the summon packet to render the owner's name under it and to
/// treat it as a summon rather than a monster.
///
/// The **owner** is excluded here: they already got `PetSummonInfo` when it was
/// summoned, and Java likewise splits the two in `Summon.sendInfo`.
fn send_summon_info(
    world: &World,
    session: &ClientSession,
    servitor_oid: i32,
    viewer_oid: i32,
) -> bool {
    use crate::model::components::{CombatStats, Position, ServitorOf, Speeds, Vitals};
    let Some(link) = world
        .objects
        .get_component::<ServitorOf>(&servitor_oid)
        .copied()
    else {
        return false;
    };
    if link.owner_object_id == viewer_oid {
        return true; // the owner's view is the PetInfo one
    }
    let (Some(npc), Some(pos), Some(vitals), Some(speeds), Some(combat)) = (
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&servitor_oid),
        world.objects.get_component::<Position>(&servitor_oid),
        world.objects.get_component::<Vitals>(&servitor_oid),
        world.objects.get_component::<Speeds>(&servitor_oid),
        world.objects.get_component::<CombatStats>(&servitor_oid),
    ) else {
        return true;
    };
    let Some(t) = npc.template(world) else {
        return true;
    };
    let owner_name = world
        .objects
        .get_component::<crate::model::Player>(&link.owner_object_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    session.send(server_packets::summon_info(
        servitor_oid,
        t,
        pos,
        vitals,
        speeds,
        combat,
        &owner_name,
        0, // relation — the per-viewer PvP relation isn't resolved here yet
        false,
    ));
    true
}

fn send_npc_info(world: &World, session: &ClientSession, npc_id: i32) {
    let Some(v) = crate::model::npc::NpcView::of(&world.objects, npc_id) else {
        return;
    };
    let Some(t) = v.npc.template(world) else {
        return;
    };
    session.send(server_packets::npc_info(&v, t, &world.cfg.npc));
}

/// The region cell a player is registered in (`None` once they're gone).
fn player_region(world: &World, object_id: i32) -> Option<(i32, i32)> {
    world
        .objects
        .get_component::<RegionCell>(&object_id)
        .map(|r| r.0)
}

/// Java `World.addVisibleObject` for a player spawning in (`EnterWorld` →
/// `spawnMe`): mutual `CharInfo` with every player already visible from the
/// spawn region, plus `NpcInfo` for every NPC in the 3×3 block (NPCs are
/// told nothing — they get aggro/AI eyes in G9).
pub(crate) fn on_enter_world(world: &World, client_id: u32, object_id: i32) {
    let Some(my_region) = player_region(world, object_id) else {
        return;
    };
    let Some(my_session) = world.clients.get(&client_id) else {
        return;
    };
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let other_id = s.player_object_id();
            if other_id == object_id {
                continue;
            }
            let Some(other_region) = player_region(world, other_id) else {
                continue;
            };
            if regions_adjacent(my_region, other_region) {
                send_char_info(world, cs, object_id);
                send_char_info(world, my_session, other_id);
            }
        }
    }
    for npc_id in world.npcs_visible_from(my_region) {
        // A servitor takes the summon packet; everything else the NPC one.
        if send_summon_info(world, my_session, npc_id, object_id) {
            continue;
        }
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
    if let Some(so) = world
        .objects
        .get_component::<crate::model::static_object::StaticObj>(&object_id)
    {
        session.send(server_packets::static_object_info(
            so.static_id,
            so.object_id,
        ));
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
            let Some(other_region) = player_region(world, other_id) else {
                continue;
            };
            let was = regions_adjacent(old, other_region);
            let now = regions_adjacent(new, other_region);
            if was != now {
                deltas.push((other_id, cid, now));
            }
        }
    }
    // Dangling-target drop first: Java `switchRegion` runs `setTarget(null)`
    // *before* the target's `DeleteObject`, and `Player.setTarget(null)`
    // broadcasts `TargetUnselected` including the owner's own client — without
    // that packet our client keeps the object id selected and the ground ring
    // re-attaches when the same id re-enters via `NpcInfo`/`CharInfo`.
    // TODO(G9): Java also fires `EVT_FORGET_OBJECT` at both sides' AI here so
    // NPCs drop aggro/follow on objects leaving their block — wire up when NPC
    // AI can hold references.
    if let Some(TargetRef(Some(target))) = world
        .objects
        .get_component::<TargetRef>(&object_id)
        .copied()
    {
        let target_region = world
            .objects
            .get_component::<RegionCell>(&target)
            .map(|r| r.0);
        if target_region.is_some_and(|r| !regions_adjacent(new, r)) {
            super::target::drop_target_notify(world, object_id);
        }
    }

    // NPC deltas: NpcInfo for NPCs entering the 3×3 block, DeleteObject for
    // NPCs leaving it. The npc_regions index makes this a walk over the (at
    // most) 12 cells whose adjacency changed.
    let my_client = client_for_player(world, object_id);
    if let Some(cs) = my_client.and_then(|cid| world.clients.get(&cid)) {
        for npc_id in world.npcs_visible_from(new) {
            let Some(npc_region) = world.objects.get_component::<RegionCell>(&npc_id) else {
                continue;
            };
            if !regions_adjacent(old, npc_region.0) {
                if !send_summon_info(world, cs, npc_id, object_id) {
                    send_npc_info(world, cs, npc_id);
                }
            }
        }
        for npc_id in world.npcs_visible_from(old) {
            let Some(npc_region) = world.objects.get_component::<RegionCell>(&npc_id) else {
                continue;
            };
            if !regions_adjacent(new, npc_region.0) {
                cs.send(server_packets::delete_object(npc_id));
            }
        }
        // Door/static-object deltas, same shape as the NPC ones.
        for door_id in world.doors_visible_from(new) {
            let Some(door_region) = world.objects.get_component::<RegionCell>(&door_id) else {
                continue;
            };
            if !regions_adjacent(old, door_region.0) {
                super::doors::send_door_info(world, cs, door_id);
            }
        }
        for door_id in world.doors_visible_from(old) {
            let Some(door_region) = world.objects.get_component::<RegionCell>(&door_id) else {
                continue;
            };
            if !regions_adjacent(new, door_region.0) {
                cs.send(server_packets::delete_object(door_id));
            }
        }
        for so_id in world.statics_visible_from(new) {
            let Some(so_region) = world.objects.get_component::<RegionCell>(&so_id) else {
                continue;
            };
            if !regions_adjacent(old, so_region.0) {
                send_static_object_info(world, cs, so_id);
            }
        }
        for so_id in world.statics_visible_from(old) {
            let Some(so_region) = world.objects.get_component::<RegionCell>(&so_id) else {
                continue;
            };
            if !regions_adjacent(new, so_region.0) {
                cs.send(server_packets::delete_object(so_id));
            }
        }
        // Ground-item deltas, same shape (SpawnItem in / DeleteObject out).
        for item_id in world.ground_items_visible_from(new) {
            let Some(r) = world.objects.get_component::<RegionCell>(&item_id) else {
                continue;
            };
            if !regions_adjacent(old, r.0) {
                if let Some(view) = super::ground_items::ground_item_view(world, item_id) {
                    cs.send(server_packets::spawn_item(&view));
                }
            }
        }
        for item_id in world.ground_items_visible_from(old) {
            let Some(r) = world.objects.get_component::<RegionCell>(&item_id) else {
                continue;
            };
            if !regions_adjacent(new, r.0) {
                cs.send(server_packets::delete_object(item_id));
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
            // Target drops (with their TargetUnselected) precede the
            // DeleteObject, as in Java. The mover's own target was already
            // handled above; the guard makes a second call a no-op anyway.
            if world
                .objects
                .get_component::<TargetRef>(&other_id)
                .copied()
                .is_some_and(|t| t.0 == Some(object_id))
            {
                super::target::drop_target_notify(world, other_id);
            }
            if world
                .objects
                .get_component::<TargetRef>(&object_id)
                .copied()
                .is_some_and(|t| t.0 == Some(other_id))
            {
                super::target::drop_target_notify(world, object_id);
            }
            if let Some(cs) = world.clients.get(&other_client) {
                cs.send(server_packets::delete_object(object_id));
            }
            if let Some(cs) = my_client.and_then(|cid| world.clients.get(&cid)) {
                cs.send(server_packets::delete_object(other_id));
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
    let Some(region) = player_region(world, object_id) else {
        return;
    };
    let mut observers: Vec<(u32, i32)> = Vec::new();
    for (&cid, cs) in &world.clients {
        if let ClientSession::InGame(s) = cs {
            let other_id = s.player_object_id();
            if other_id == object_id {
                continue;
            }
            let Some(other_region) = player_region(world, other_id) else {
                continue;
            };
            if regions_adjacent(region, other_region) {
                observers.push((cid, other_id));
            }
        }
    }
    for (cid, other_id) in observers {
        // TargetUnselected before DeleteObject (Java `removeVisibleObject`
        // runs `setTarget(null)` first) — see `drop_target_notify`.
        if world
            .objects
            .get_component::<TargetRef>(&other_id)
            .copied()
            .is_some_and(|t| t.0 == Some(object_id))
        {
            super::target::drop_target_notify(world, other_id);
        }
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(server_packets::delete_object(object_id));
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
    world
        .npc_regions
        .entry(new)
        .or_default()
        .push(npc_object_id);

    // Deltas are collected first so the target drop (which needs `&mut World`
    // for its TargetUnselected broadcast) can run before the DeleteObject
    // send, matching Java `switchRegion` order.
    let mut deltas: Vec<(u32, i32, bool)> = Vec::new(); // (client_id, player_id, appeared)
    for (&cid, cs) in &world.clients {
        if let ClientSession::InGame(s) = cs {
            let player_id = s.player_object_id();
            let Some(player_region) = player_region(world, player_id) else {
                continue;
            };
            let was = regions_adjacent(old, player_region);
            let now = regions_adjacent(new, player_region);
            if was != now {
                deltas.push((cid, player_id, now));
            }
        }
    }
    for (cid, player_id, appeared) in deltas {
        if appeared {
            if let Some(cs) = world.clients.get(&cid) {
                send_npc_info(world, cs, npc_object_id);
            }
        } else {
            if world
                .objects
                .get_component::<TargetRef>(&player_id)
                .copied()
                .is_some_and(|t| t.0 == Some(npc_object_id))
            {
                super::target::drop_target_notify(world, player_id);
            }
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(server_packets::delete_object(npc_object_id));
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
    world
        .objects
        .for_each_mut::<&Player>(|p| ids.push(p.object_id));
    for id in ids {
        update_region(world, id);
        // Java couples `updatePosition` with `revalidateZone(false)` — the
        // 100-unit `last_validate` filter inside makes this cheap per tick.
        super::zones::revalidate_zone(world, id, false);
    }
    for id in outcome.moved_npcs {
        update_npc_region(world, id);
    }
    // `CreatureAI.onEvtArrived`: "if the Intention was AI_INTENTION_MOVE_TO,
    // set the Intention to AI_INTENTION_ACTIVE" — a mob that finishes its flee
    // leg starts thinking again (and re-acquires a target if the fear has since
    // worn off; if it hasn't, the next fear tick shoves it onward).
    for id in &outcome.arrived {
        if let Some(ai) = world
            .objects
            .get_component_mut::<crate::model::npc::NpcAi>(id)
        {
            if ai.intention == crate::model::npc::NpcIntention::MoveTo {
                ai.intention = crate::model::npc::NpcIntention::Active;
            }
        }
    }
    // Route advances need a `MoveToLocation` for the new segment — unlike
    // plain moves, the client only knows the previous segment's endpoint
    // (Java `moveToNextRoutePoint` → `broadcastMoveToLocation`).
    for id in outcome.route_advanced {
        let Some(Movement(m)) = world.objects.get_component::<Movement>(&id) else {
            continue;
        };
        let pkt = server_packets::move_to_location(
            id, m.dest_x, m.dest_y, m.dest_z, m.start_x, m.start_y, m.start_z,
        );
        super::helpers::broadcast_including_self(world, id, &pkt);
    }
}
