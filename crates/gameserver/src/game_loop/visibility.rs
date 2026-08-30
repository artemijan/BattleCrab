//! Player↔player visibility, driven by the world-region grid — the port of
//! Java `World.addVisibleObject` / `switchRegion` / `removeVisibleObject` plus
//! `WorldObject.updateWorldRegion`. Two players see each other exactly while
//! their region cells are within each other's 3×3 surrounding block
//! (`world::regions_adjacent`); entering that block exchanges `CharInfo`,
//! leaving it exchanges `DeleteObject`, and every broadcast helper is scoped
//! by the same rule (`helpers::broadcast_to_others`).

use crate::game_loop::helpers::maybe_position;
use crate::game_loop::helpers::send_to_client;
use crate::model::components::{Movement, Position, RegionCell, TargetRef};
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::{World, region_of, regions_adjacent};

use super::helpers::client_for_player;
use crate::game_loop::helpers::region_cell_of;

/// `CreatureAI.describeStateToPlayer`, players-only: right after a `CharInfo`
/// introduces `p`, tell the observer about in-flight state — currently just an
/// ongoing move (without it, a mover entering visibility stands still on the
/// observer's screen until the next `MoveToLocation` broadcast).
fn describe_state(
    observer: &ClientSession,
    object_id: i32,
    pos: &Position,
    movement: Option<&Movement>,
) {
    if let Some(Movement(m)) = movement {
        observer.send(server_packets::move_to_location(
            object_id, m.dest_x, m.dest_y, m.dest_z, pos.x, pos.y, pos.z,
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
    // One player, many observers — build the packet once (see
    // `char_info_bytes`) and refcount it out.
    let Some(packet) = char_info_bytes(world, player_id) else {
        return;
    };
    for other_id in world.in_game_players_visible_from(from.0) {
        if other_id == player_id {
            continue;
        }
        let Some(cs) = world.clients.session_of_player(other_id) else {
            continue;
        };
        send_char_info_with(world, cs, player_id, &packet);
    }
}

/// `CharInfo` + follow-up state for one player, to one observer session.
/// A GM with `//hide` on (`AdminFlags.hidden`) is not described to others —
/// Java's `isInvisible()` filter in `Creature.broadcastInfo`/knownlist.
fn send_char_info(world: &World, observer: &ClientSession, player_id: i32) {
    let Some(packet) = char_info_bytes(world, player_id) else {
        return;
    };
    send_char_info_with(world, observer, player_id, &packet);
}

/// The `CharInfo` bytes describing one player.
///
/// These are the same for every observer, so a loop introducing one player to
/// several of them builds this **once** — the packet is the biggest the server
/// sends, and rebuilding it per observer meant re-reading the whole component
/// set, the abnormal visuals and the party state each time. The observer-
/// dependent parts (the hidden-GM filter, `RelationChanged`, the in-flight move)
/// live in [`send_char_info_with`].
fn char_info_bytes(world: &World, player_id: i32) -> Option<bytes::Bytes> {
    let v = crate::model::PlayerView::of(&world.objects, player_id)?;
    let cubics = world
        .objects
        .get_component::<super::cubic::Cubics>(&player_id)
        .map(|c| c.ids())
        .unwrap_or_default();
    Some(bytes::Bytes::from(server_packets::char_info(
        &v,
        &super::abnormal::visual_effects(world, player_id),
        &cubics,
        &super::player_info::char_info_state(world, player_id),
    )))
}

/// Deliver a prebuilt [`char_info_bytes`] to one observer, with the parts that
/// genuinely differ per observer.
fn send_char_info_with(
    world: &World,
    observer: &ClientSession,
    player_id: i32,
    packet: &bytes::Bytes,
) {
    if world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&player_id)
        .is_some_and(|f| f.hidden)
    {
        // `isVisibleFor`: a hidden player is still described to observers
        // holding the SEE_ALL_PLAYERS cond-override (`//exceptions`).
        let sees_all = match observer {
            ClientSession::InGame(s) => world
                .objects
                .get_component::<crate::model::Player>(&s.player_object_id())
                .is_some_and(|p| {
                    p.can_override_cond(crate::game_loop::admin::SEE_ALL_PLAYERS_ORDINAL)
                }),
            _ => false,
        };
        if !sees_all {
            return;
        }
    }
    observer.send(packet.clone());
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
    let Some(pos) = world.objects.get_component::<Position>(&player_id) else {
        return;
    };
    describe_state(
        observer,
        player_id,
        pos,
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
        crate::game_loop::abnormal::flags_of(world, servitor_oid)
            & crate::model::skill::effect_flag::BETRAYED
            != 0,
    ));
    true
}

fn send_npc_info(world: &World, session: &ClientSession, npc_id: i32) {
    if let Some(pkt) = npc_info_bytes(world, npc_id) {
        session.send(pkt);
    }
}

/// One `NpcInfo` payload as refcountable `Bytes` — build once, send to any
/// number of observers.
pub(crate) fn npc_info_bytes(world: &World, npc_id: i32) -> Option<bytes::Bytes> {
    let v = crate::model::npc::NpcView::of(&world.objects, npc_id)?;
    let t = v.npc.template(world)?;
    let visuals = super::abnormal::visual_effects(world, npc_id);
    let clan = npc_clan_block(world, npc_id);
    Some(bytes::Bytes::from(server_packets::npc_info(
        &v,
        t,
        &world.cfg.npc,
        &world.cfg.champion,
        &visuals,
        clan,
    )))
}

/// `NpcInfo`'s CLAN gate — the spawn-time `crest_clan_id` (a TAX-zone NPC of
/// an owned castle with the crest display on), resolved to the owner clan's
/// crest ids at send time. Java shows it only for a non-monster standing in a
/// peace zone (`!npc.isMonster() && npc.isInsideZone(ZoneId.PEACE)`).
pub(crate) fn npc_clan_block(world: &World, npc_oid: i32) -> Option<[i32; 5]> {
    let npc = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)?;
    if npc.crest_clan_id == 0 {
        return None;
    }
    let t = npc.template(world)?;
    if t.is_monster() {
        return None;
    }
    // Positional (`zones_at`), not `ZoneFlags` — the flag component is
    // maintained by the player zone tick and never exists on an NPC.
    let pos = world.objects.get_component::<Position>(&npc_oid)?;
    let in_peace = world
        .data
        .zone_data
        .zones_at(pos.x, pos.y, pos.z)
        .any(|z| z.kind == crate::data::zone_data::ZoneKind::Peace);
    if !in_peace {
        return None;
    }
    let clan = world.clans.get(&npc.crest_clan_id)?;
    Some([
        npc.crest_clan_id,
        clan.crest_id,
        clan.crest_large_id,
        clan.ally_id,
        clan.ally_crest_id,
    ])
}

/// The region cell a player is registered in (`None` once they're gone).
fn player_region(world: &World, object_id: i32) -> Option<(i32, i32)> {
    region_cell_of(world, object_id)
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
    let my_instance = super::helpers::instance_of(world, object_id);
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let other_id = s.player_object_id();
            if other_id == object_id {
                continue;
            }
            let Some(other_region) = player_region(world, other_id) else {
                continue;
            };
            if regions_adjacent(my_region, other_region)
                && super::helpers::instance_of(world, object_id)
                    == super::helpers::instance_of(world, other_id)
            {
                send_char_info(world, cs, object_id);
                send_char_info(world, my_session, other_id);
            }
        }
    }
    // The unattended shops: no session, so the loop above never reaches them,
    // but they are standing right there and must be drawn (Java's knownlist is
    // registry-driven and does not care whether a client is attached).
    for &trader_id in world.offline_traders.keys() {
        if trader_id == object_id {
            continue;
        }
        let Some(trader_region) = player_region(world, trader_id) else {
            continue;
        };
        if regions_adjacent(my_region, trader_region)
            && super::helpers::instance_of(world, trader_id) == my_instance
        {
            send_char_info(world, my_session, trader_id);
        }
    }
    for npc_id in world.npcs_visible_from(my_region) {
        // Only content sharing the player's instance is visible.
        if super::helpers::instance_of(world, npc_id) != my_instance {
            continue;
        }
        // A servitor takes the summon packet; everything else the NPC one.
        if send_summon_info(world, my_session, npc_id, object_id) {
            continue;
        }
        send_npc_info(world, my_session, npc_id);
    }
    // Doors render like NPCs (Java `Door.sendInfo`: StaticObjectInfo +
    // DoorStatusUpdate).
    for door_id in world.doors_visible_from(my_region) {
        if super::helpers::instance_of(world, door_id) != my_instance {
            continue;
        }
        super::doors::send_door_info(world, my_session, door_id);
    }
    for so_id in world.statics_visible_from(my_region) {
        if super::helpers::instance_of(world, so_id) != my_instance {
            continue;
        }
        send_static_object_info(world, my_session, so_id);
    }
    for item_id in world.ground_items_visible_from(my_region) {
        if super::helpers::instance_of(world, item_id) != my_instance {
            continue;
        }
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
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    let new = region_of(pos.x, pos.y);
    // Moves the `RegionCell` and `World.player_regions` together; `None` means
    // the cell did not change, which is this function's early-out.
    let Some(old) = world.set_player_region(object_id, new) else {
        return;
    };

    // Visibility deltas vs every other in-game player (client id included so
    // the send phase needs no per-player session scan).
    let mut deltas: Vec<(i32, u32, bool)> = Vec::new(); // (other_id, client_id, appeared)
    // Only players in the union of the old and new 3×3 blocks can have a
    // delta — everyone else was invisible before and stays invisible. Walking
    // both blocks costs at most 18 cells; the client scan this replaced cost
    // one adjacency test per player online.
    let mut seen: std::collections::HashSet<i32> = std::collections::HashSet::new();
    for other_id in world
        .in_game_players_visible_from(old)
        .chain(world.in_game_players_visible_from(new))
        .collect::<Vec<_>>()
    {
        if other_id == object_id || !seen.insert(other_id) {
            continue;
        }
        let Some(cid) = world.clients.client_of_player(other_id) else {
            continue;
        };
        let Some(other_region) = player_region(world, other_id) else {
            continue;
        };
        // Different instances never see each other, so the only delta a
        // region change can produce across instances is "still hidden".
        let same_instance = super::helpers::instance_of(world, object_id)
            == super::helpers::instance_of(world, other_id);
        let was = regions_adjacent(old, other_region) && same_instance;
        let now = regions_adjacent(new, other_region) && same_instance;
        if was != now {
            deltas.push((other_id, cid, now));
        }
    }
    // Dangling-target drop first: Java `switchRegion` runs `setTarget(null)`
    // *before* the target's `DeleteObject`, and `Player.setTarget(null)`
    // broadcasts `TargetUnselected` including the owner's own client — without
    // that packet our client keeps the object id selected and the ground ring
    // re-attaches when the same id re-enters via `NpcInfo`/`CharInfo`.
    if let Some(TargetRef(Some(target))) = world
        .objects
        .get_component::<TargetRef>(&object_id)
        .copied()
    {
        let target_region = region_cell_of(world, target);
        if target_region.is_some_and(|r| !regions_adjacent(new, r)) {
            super::target::drop_target_notify(world, object_id);
        }
    }

    // Java also fires `EVT_FORGET_OBJECT` at the AI of everything that just
    // dropped out of the mover's block. For an `Attackable` mid-fight that
    // lands on `Attackable.setTarget(null)` — the mover's aggro entry is
    // removed and, if it was the last one, the mob stands down for ~25 s
    // instead of instantly re-aggroing whoever is still standing there.
    // (Adjacency is symmetric: the NPCs the mover can no longer see are
    // exactly the ones that can no longer see the mover.)
    let departed: Vec<i32> = world
        .npcs_visible_from(old)
        .into_iter()
        .filter(|npc_id| {
            world
                .objects
                .get_component::<RegionCell>(npc_id)
                .is_some_and(|r| !regions_adjacent(new, r.0))
        })
        .collect();
    for npc_oid in departed {
        super::ai::on_forget_object(world, npc_oid, object_id);
    }

    // NPC deltas: NpcInfo for NPCs entering the 3×3 block, DeleteObject for
    // NPCs leaving it. The npc_regions index makes this a walk over the (at
    // most) 12 cells whose adjacency changed.
    let my_client = client_for_player(world, object_id);
    // The mover's instance is constant across a region change (instance moves go
    // through teleport, not here); so only its own instance's content appears.
    let my_instance = super::helpers::instance_of(world, object_id);
    if let Some(cs) = my_client.and_then(|cid| world.clients.get(&cid)) {
        for npc_id in world.npcs_visible_from(new) {
            let Some(npc_region) = world.objects.get_component::<RegionCell>(&npc_id) else {
                continue;
            };
            if !regions_adjacent(old, npc_region.0)
                && super::helpers::instance_of(world, npc_id) == my_instance
                && !send_summon_info(world, cs, npc_id, object_id)
            {
                send_npc_info(world, cs, npc_id);
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
            if !regions_adjacent(old, door_region.0)
                && super::helpers::instance_of(world, door_id) == my_instance
            {
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
            if !regions_adjacent(old, so_region.0)
                && super::helpers::instance_of(world, so_id) == my_instance
            {
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
            if !regions_adjacent(old, r.0)
                && super::helpers::instance_of(world, item_id) == my_instance
                && let Some(view) = super::ground_items::ground_item_view(world, item_id)
            {
                cs.send(server_packets::spawn_item(&view));
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
        // Unattended shops, same shape: they have no session so the `deltas`
        // scan above never sees them, and they never move — only the mover's
        // own screen changes. They *are* `Player` entities in the region
        // index, so the two blocks are inspected instead of every shop on
        // the server. A shop from the old block always has `was == true`
        // (its cell is in that 3×3), so only the leave test remains — and
        // symmetrically for the new block.
        for trader_id in world.players_visible_from(old) {
            if trader_id == object_id || !world.offline_traders.contains_key(&trader_id) {
                continue;
            }
            let Some(r) = world.objects.get_component::<RegionCell>(&trader_id) else {
                continue;
            };
            if !regions_adjacent(new, r.0) {
                cs.send(server_packets::delete_object(trader_id));
            }
        }
        for trader_id in world.players_visible_from(new) {
            if trader_id == object_id || !world.offline_traders.contains_key(&trader_id) {
                continue;
            }
            let Some(r) = world.objects.get_component::<RegionCell>(&trader_id) else {
                continue;
            };
            if !regions_adjacent(old, r.0)
                && super::helpers::instance_of(world, trader_id) == my_instance
            {
                send_char_info(world, cs, trader_id);
            }
        }
    }
    // The mover is described to every observer that just gained sight of them,
    // so their CharInfo is built once for the whole delta set. The reverse
    // direction is one packet per newly-visible *other* player and cannot be
    // shared. Built lazily: most region crossings produce no `appeared` delta.
    let mut mover_info: Option<bytes::Bytes> = None;
    for (other_id, other_client, appeared) in deltas {
        if appeared {
            if let Some(cs) = world.clients.get(&other_client) {
                let packet = match &mover_info {
                    Some(p) => Some(p),
                    None => {
                        mover_info = char_info_bytes(world, object_id);
                        mover_info.as_ref()
                    }
                };
                if let Some(packet) = packet {
                    send_char_info_with(world, cs, object_id, packet);
                }
            }
            if let Some(cs) = my_client.and_then(|cid| world.clients.get(&cid)) {
                send_char_info(world, cs, other_id);
            }
        } else {
            // Target drops (with their TargetUnselected) precede the
            // DeleteObject, as in Java. The mover's own target was already
            // handled above; the guard makes a second call a no-op anyway.
            drop_target_if_pointing_at(world, other_id, object_id);
            drop_target_if_pointing_at(world, object_id, other_id);
            send_to_client(
                world,
                other_client,
                server_packets::delete_object(object_id),
            );
            if let Some(cid) = my_client {
                send_to_client(world, cid, server_packets::delete_object(other_id));
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
        drop_target_if_pointing_at(world, other_id, object_id);
        send_to_client(world, cid, server_packets::delete_object(object_id));
    }

    // `EVT_FORGET_OBJECT` at the surrounding NPCs' AI — logging out mid-fight
    // is the other way a mob loses its target. See `npc_ai::on_forget_object`.
    for npc_oid in world.npcs_visible_from(region) {
        super::ai::on_forget_object(world, npc_oid, object_id);
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
    let npc_instance = super::helpers::instance_of(world, npc_object_id);
    let mut deltas: Vec<(u32, i32, bool)> = Vec::new(); // (client_id, player_id, appeared)
    // Same argument as the player-side delta: only the old and new 3×3 blocks
    // can contain a player whose view of this NPC changed. This runs for every
    // moving NPC that crosses a cell boundary, so it was the client scan that
    // hurt most during mass combat.
    let mut seen: std::collections::HashSet<i32> = std::collections::HashSet::new();
    // The loop body only reads, so the chained index iterators feed it
    // directly — no collected id Vec.
    for player_id in world
        .in_game_players_visible_from(old)
        .chain(world.in_game_players_visible_from(new))
    {
        if !seen.insert(player_id) {
            continue;
        }
        let Some(cid) = world.clients.client_of_player(player_id) else {
            continue;
        };
        let Some(player_region) = player_region(world, player_id) else {
            continue;
        };
        // A player only sees the NPC while sharing its instance.
        let same_instance = super::helpers::instance_of(world, player_id) == npc_instance;
        let was = regions_adjacent(old, player_region) && same_instance;
        let now = regions_adjacent(new, player_region) && same_instance;
        if was != now {
            deltas.push((cid, player_id, now));
        }
    }
    // The NpcInfo payload is identical for every gaining observer — build it
    // once and refcount it into each session (the `char_info_bytes` shape),
    // instead of re-running NpcView + visual effects + a 256-byte build per
    // observer on the mass-combat hot path.
    let info: Option<bytes::Bytes> = deltas
        .iter()
        .any(|&(_, _, appeared)| appeared)
        .then(|| npc_info_bytes(world, npc_object_id))
        .flatten();
    for (cid, player_id, appeared) in deltas {
        if appeared {
            if let (Some(cs), Some(pkt)) = (world.clients.get(&cid), info.as_ref()) {
                cs.send(pkt.clone());
            }
        } else {
            drop_target_if_pointing_at(world, player_id, npc_object_id);
            send_to_client(world, cid, server_packets::delete_object(npc_object_id));
        }
    }
}

/// The per-tick movement system plus Java's `setXYZ` → `updateWorldRegion`
/// coupling: advance every mover (`movement::tick`), then fire region switches
/// for anyone whose cell changed. `update_region` early-outs on an unchanged
/// cell, so the sweep is a cheap comparison per player on quiet ticks.
pub(crate) fn movement_tick(world: &mut World) {
    // `World.player_regions` backs every broadcast, and a site that relocates a
    // player without `World::set_player_region` would break it silently rather
    // than loudly. Re-derive it from the ECS and compare, once per tick, in
    // debug and test builds — this is the tick entry point the test harness
    // drives too (`tests::advance_world`), so the whole suite stands behind the
    // invariant. Compiled out of release entirely.
    #[cfg(debug_assertions)]
    world.debug_check_player_regions();

    let outcome = crate::model::movement::tick(world);
    // Only a player who moved this tick can have crossed a cell or zone
    // boundary here — everyone else's membership was settled by whichever
    // site last wrote their Position (see `TickOutcome::moved_players`).
    for id in &outcome.moved_players {
        update_region(world, *id);
        // Java couples `updatePosition` with `revalidateZone(false)` — the
        // 100-unit `last_validate` filter inside keeps this cheap per tick.
        super::zones::revalidate_zone(world, *id, false);
    }
    for id in outcome.moved_npcs {
        update_npc_region(world, id);
    }
    // `CreatureAI.onEvtArrived`: "if the Intention was AI_INTENTION_MOVE_TO,
    // set the Intention to AI_INTENTION_ACTIVE" — a mob that finishes its flee
    // leg starts thinking again (and re-acquires a target if the fear has since
    // worn off; if it hasn't, the next fear tick shoves it onward) — and then
    // `onEvtThink()`, right here on the 100 ms movement tick rather than at the
    // next 1 s AI period. That trailing think is what lets a mob that just
    // closed on its target swing immediately: the chase destination is already
    // shortened to attack range, so arrival means "in range".
    for id in &outcome.arrived {
        let Some(ai) = world
            .objects
            .get_component_mut::<crate::model::npc::NpcAi>(id)
        else {
            continue; // players run their own AI, not `AttackableAI`
        };
        if ai.intention == crate::model::npc::NpcIntention::MoveTo {
            ai.intention = crate::model::npc::NpcIntention::Active;
        }
        super::ai::on_npc_arrived(world, *id);
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

/// Clear `viewer`'s selection if it points at `object_id`.
///
/// Java's `removeVisibleObject` runs `setTarget(null)` before `DeleteObject`,
/// so a client never keeps a selection on something it can no longer see. The
/// no-op-when-it-points-elsewhere behaviour is what makes the symmetric pair of
/// calls on a mutual despawn safe to issue unconditionally.
fn drop_target_if_pointing_at(world: &mut World, viewer: i32, object_id: i32) {
    if world
        .objects
        .get_component::<TargetRef>(&viewer)
        .copied()
        .is_some_and(|t| t.0 == Some(object_id))
    {
        super::target::drop_target_notify(world, viewer);
    }
}

// Moved from helpers.
/// Java `World.forEachVisibleObject(origin, Creature.class, …)` — every living
/// creature (player **or** NPC) in `origin`'s own region cell or an adjacent
/// one, excluding `origin` itself.
///
/// Java's "visible" is exactly this region-neighbourhood test; there is no
/// line-of-sight or radius term in `forEachVisibleObject`, so none is applied
/// here either. Callers that need a distance or LOS filter add it themselves.
///
/// This is the general neighbour query the `RandomizeHate` deferral in the
/// hate-effects slice was waiting on: `faction_call`'s scan only ever walked
/// NPCs, so a mob could never be pointed at a *player* it wasn't already
/// fighting.
pub(crate) fn visible_creatures(world: &mut World, origin_object_id: i32) -> Vec<i32> {
    use crate::model::components::Vitals;
    let Some(origin) = region_cell_of(world, origin_object_id) else {
        return Vec::new();
    };
    // Both halves come from the region indexes. This used to sweep every
    // entity in the store — all ~34.9k NPCs — and discard the 99.9% that were
    // nowhere near the origin.
    let mut out: Vec<i32> = world
        .players_visible_from(origin)
        .chain(world.npcs_visible_from(origin))
        .filter(|&oid| {
            oid != origin_object_id
                && world
                    .objects
                    .get_component::<Vitals>(&oid)
                    .is_some_and(|v| !v.dead)
        })
        .collect();
    // Sorted so the caller's `Rnd.get(size)` index maps to a stable candidate.
    // Java's iteration order is arbitrary too, and a uniform index over a
    // sorted list is still uniform — but this makes a forced roll in tests
    // pick a *known* creature instead of whatever the ECS happened to yield.
    out.sort_unstable();
    out
}
