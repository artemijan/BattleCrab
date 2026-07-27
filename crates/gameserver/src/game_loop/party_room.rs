//! Party matching rooms (G30) — port of the Java client packets
//! `RequestPartyMatchConfig` (0x7F), `RequestPartyMatchList` (0x80),
//! `RequestPartyMatchDetail` (0x81), `RequestOustFromPartyRoom` (ex 0x09),
//! `RequestDismissPartyRoom` (ex 0x0A), `RequestWithdrawPartyRoom` (ex 0x0B),
//! `RequestExitPartyMatchingWaitingRoom` (ex 0x25),
//! `RequestAskJoinPartyRoom` (ex 0x2F), `AnswerJoinPartyRoom` (ex 0x30) and
//! `RequestListPartyMatchingWaitingRoom` (ex 0x31), against
//! `model/matching/PartyMatchingRoom` + `MatchingRoomManager`.
//!
//! State lives in [`crate::model::matching_room`] on `World`; a player's
//! membership is derived from the room registry rather than mirrored on the
//! player (Java's `Player._matchingRoom`), so the two can never disagree.

use crate::model::components::{PartyRef, Position};
use crate::model::matching_room::{MatchingMemberType, RoomLevelFilter};
use crate::model::Player;
use crate::network::client_packets as cp;
use crate::network::server_packets::{
    self, sm_ids, RoomListView, RoomMemberView, SmParam, WaitingPlayerView, ROOMS_PER_PAGE,
};
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::client_for_player;

// ---------------------------------------------------------------------------
// Small lookups
// ---------------------------------------------------------------------------

/// `MapRegionManager.getBBs(player)` — the community-board region a player is
/// standing in, which is the "location" rooms are filtered by.
pub(crate) fn location_of(world: &World, object_id: i32) -> i32 {
    world
        .objects
        .get_component::<Position>(&object_id)
        .map_or(0, |p| world.data.map_region.bbs_at(p.x, p.y))
}

fn level_of(world: &World, object_id: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map_or(0, |p| p.level)
}

fn send(world: &World, object_id: i32, packet: Vec<u8>) {
    if let Some(cid) = client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(packet);
        }
    }
}

fn send_sm(world: &World, object_id: i32, message_id: i16, params: &[SmParam]) {
    send(
        world,
        object_id,
        server_packets::system_message_with(message_id, params),
    );
}

fn name_of(world: &World, object_id: i32) -> String {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map_or_else(String::new, |p| p.name.clone())
}

/// `PartyMatchingRoom.getMemberType` — a member is a `PARTY_MEMBER` only when
/// they share the *leader's* party; otherwise they are just parked in the room.
fn member_type(world: &World, room_id: i32, object_id: i32) -> MatchingMemberType {
    let Some(room) = world.matching_rooms.get(room_id) else {
        return MatchingMemberType::WaitingPlayer;
    };
    if room.is_leader(object_id) {
        return MatchingMemberType::PartyLeader;
    }
    let party_of = |oid: i32| {
        world
            .objects
            .get_component::<PartyRef>(&oid)
            .map(|PartyRef(id)| *id)
    };
    match (party_of(room.leader), party_of(object_id)) {
        (Some(a), Some(b)) if a == b => MatchingMemberType::PartyMember,
        _ => MatchingMemberType::WaitingPlayer,
    }
}

fn member_views(world: &World, room_id: i32) -> Vec<RoomMemberView> {
    let Some(room) = world.matching_rooms.get(room_id) else {
        return Vec::new();
    };
    room.all_members()
        .into_iter()
        .filter_map(|oid| {
            let p = world.objects.get_component::<Player>(&oid)?;
            Some(RoomMemberView {
                object_id: oid,
                name: p.name.clone(),
                class_id: p.class_id,
                level: p.level,
                location: location_of(world, oid),
                member_type: member_type(world, room_id, oid).id(),
            })
        })
        .collect()
}

fn room_info_packet(world: &World, room_id: i32) -> Option<Vec<u8>> {
    let room = world.matching_rooms.get(room_id)?;
    Some(server_packets::party_room_info(
        room.id,
        room.max_members,
        room.min_level,
        room.max_level,
        room.loot,
        location_of(world, room.leader),
        &room.title,
    ))
}

/// Re-send the member list to everyone in the room. Each recipient gets their
/// **own** member type in the header (Java sends one packet built from a single
/// player to everybody).
fn broadcast_member_list(world: &World, room_id: i32) {
    let Some(room) = world.matching_rooms.get(room_id) else {
        return;
    };
    let views = member_views(world, room_id);
    for oid in room.all_members() {
        let pkt =
            server_packets::ex_party_room_member(member_type(world, room_id, oid).id(), &views);
        send(world, oid, pkt);
    }
}

fn broadcast_room_info(world: &World, room_id: i32) {
    let Some(room) = world.matching_rooms.get(room_id) else {
        return;
    };
    if let Some(pkt) = room_info_packet(world, room_id) {
        for oid in room.all_members() {
            send(world, oid, pkt.clone());
        }
    }
}

/// Java calls `broadcastUserInfo(UserInfoType.CLAN)` on every room membership
/// change, because the CLAN block carries the `isInMatchingRoom` flag.
fn broadcast_user_info(world: &World, object_id: i32) {
    super::party::broadcast_user_info(world, object_id);
}

// ---------------------------------------------------------------------------
// 0x7F RequestPartyMatchConfig — open the board / register as looking-for-party
// ---------------------------------------------------------------------------

pub(crate) fn handle_request_party_match_config(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(pkt) = cp::RequestPartyMatchConfig::read(body) else {
        return;
    };

    // Java: a party *member* may not browse — only an unpartied player or the
    // party leader. (The command-channel branch above it is post-Interlude and
    // intentionally not ported.)
    let in_party_but_not_leader = world
        .objects
        .get_component::<PartyRef>(&player)
        .and_then(|PartyRef(id)| world.parties.get(id))
        .is_some_and(|p| !p.is_leader(player));
    if in_party_but_not_leader {
        send_sm(
            world,
            player,
            sm_ids::THE_LIST_OF_PARTY_ROOMS_CAN_ONLY_BE_VIEWED_BY_A_PERSON_WHO_IS_NOT_PART_OF_A_PARTY,
            &[],
        );
        return;
    }

    world.matching_rooms.add_to_waiting_list(player);
    send_room_list(
        world,
        player,
        RoomLevelFilter::from_wire(pkt.level_filter),
        pkt.location,
        pkt.page,
    );
}

/// `ListPartyWaiting` for one requester, with Java's 64-per-page window.
fn send_room_list(world: &World, player: i32, filter: RoomLevelFilter, location: i32, page: i32) {
    let level = level_of(world, player);
    let ids = world
        .matching_rooms
        .find_rooms(location, filter, level, |leader| location_of(world, leader));
    let total = ids.len();
    let start = (page.max(1) as usize - 1) * ROOMS_PER_PAGE;
    let rows: Vec<RoomListView> = ids
        .into_iter()
        .skip(start)
        .take(ROOMS_PER_PAGE)
        .filter_map(|id| {
            let room = world.matching_rooms.get(id)?;
            Some(RoomListView {
                id: room.id,
                title: room.title.clone(),
                location: location_of(world, room.leader),
                min_level: room.min_level,
                max_level: room.max_level,
                max_members: room.max_members,
                leader_name: name_of(world, room.leader),
                members: room
                    .all_members()
                    .into_iter()
                    .filter_map(|oid| {
                        let p = world.objects.get_component::<Player>(&oid)?;
                        Some((p.class_id, p.name.clone()))
                    })
                    .collect(),
            })
        })
        .collect();
    send(
        world,
        player,
        server_packets::list_party_waiting(total, &rows),
    );
}

// ---------------------------------------------------------------------------
// 0x80 RequestPartyMatchList — create a room, or edit the one you lead
// ---------------------------------------------------------------------------

pub(crate) fn handle_request_party_match_list(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(pkt) = cp::RequestPartyMatchList::read(body) else {
        return;
    };
    let existing = world.matching_rooms.room_id_of(player);

    match existing {
        // Create. Java only creates when `roomId <= 0`; when `roomId > 0` and
        // the player has no room it dereferences null, so the port just drops
        // the packet.
        None if pkt.room_id <= 0 => {
            let title = sanitize_title(&pkt.title);
            let room_id = world.matching_rooms.create_room(
                title,
                pkt.loot_type,
                pkt.min_level,
                pkt.max_level,
                pkt.max_members.max(1),
                player,
            );
            world.matching_rooms.remove_from_waiting_list(player);
            broadcast_user_info(world, player);

            // Java's `onRoomCreation`: the room list, then the "created" SM.
            send_room_list(world, player, RoomLevelFilter::All, -1, 1);
            if let Some(info) = room_info_packet(world, room_id) {
                send(world, player, info);
            }
            broadcast_member_list(world, room_id);
            send_sm(world, player, sm_ids::YOU_HAVE_CREATED_A_PARTY_ROOM, &[]);
        }
        None => {}
        // Edit — leader only, and only their own room id.
        Some(room_id) => {
            let is_leader = world
                .matching_rooms
                .get(room_id)
                .is_some_and(|r| r.is_leader(player));
            if room_id != pkt.room_id || !is_leader {
                return;
            }
            if let Some(room) = world.matching_rooms.get_mut(room_id) {
                room.loot = pkt.loot_type;
                room.min_level = pkt.min_level;
                room.max_level = pkt.max_level;
                room.max_members = pkt.max_members.max(1);
                room.title = sanitize_title(&pkt.title);
            }
            broadcast_room_info(world, room_id);
        }
    }
}

/// The client's room-title box is 21 chars (`party_matching_history.title`).
fn sanitize_title(raw: &str) -> String {
    raw.chars().take(21).collect()
}

// ---------------------------------------------------------------------------
// ex 0x25 RequestExitPartyMatchingWaitingRoom
// ---------------------------------------------------------------------------

pub(crate) fn handle_exit_waiting_room(world: &mut World, client_id: u32) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    world.matching_rooms.remove_from_waiting_list(player);
}

// ---------------------------------------------------------------------------
// ex 0x31 RequestListPartyMatchingWaitingRoom
// ---------------------------------------------------------------------------

pub(crate) fn handle_list_waiting_room(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(pkt) = cp::RequestListPartyMatchingWaitingRoom::read(body) else {
        return;
    };

    let query = pkt.query.unwrap_or_default().to_lowercase();
    let matches: Vec<i32> = world
        .matching_rooms
        .waiting_list()
        .iter()
        .copied()
        .filter(|oid| {
            let Some(p) = world.objects.get_component::<Player>(oid) else {
                return false;
            };
            p.level >= pkt.min_level
                && p.level <= pkt.max_level
                && (pkt.class_ids.is_empty() || pkt.class_ids.contains(&p.class_id))
                // Java lowercases the *name* but not the query it compares
                // against, so a mixed-case search never matches; the port
                // lowercases both.
                && (query.is_empty() || p.name.to_lowercase().contains(&query))
        })
        .collect();

    let total = matches.len();
    let start = (pkt.page.max(1) as usize - 1) * ROOMS_PER_PAGE;
    let rows: Vec<WaitingPlayerView> = matches
        .into_iter()
        .skip(start)
        .take(ROOMS_PER_PAGE)
        .filter_map(|oid| {
            let p = world.objects.get_component::<Player>(&oid)?;
            Some(WaitingPlayerView {
                name: p.name.clone(),
                class_id: p.class_id,
                level: p.level,
            })
        })
        .collect();
    send(
        world,
        player,
        server_packets::ex_list_party_matching_waiting_room(total, &rows),
    );
}
