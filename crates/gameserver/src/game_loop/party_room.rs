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

use crate::model::components::{InMatchingRoom, PartyRef, PendingRequest, Position, RequestKind};
use crate::model::matching_room::{MatchingMemberType, RoomKind, RoomLevelFilter};
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

pub(crate) fn send_to(world: &World, object_id: i32, packet: Vec<u8>) {
    send(world, object_id, packet);
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

/// Maintain the [`InMatchingRoom`] display mirror. The registry stays the
/// authority; this is the only writer (shared with the MPCC room flows).
pub(crate) fn set_in_room_flag(world: &mut World, object_id: i32, in_room: bool) {
    if in_room {
        world.objects.add_components(&object_id, InMatchingRoom);
    } else {
        world.objects.remove_component::<InMatchingRoom>(&object_id);
    }
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

    // Command-channel branch (Java `RequestPartyMatchConfig` lines 44-70): the
    // CC leader opening the matching UI creates his MPCC room (once); any
    // other member of a channelled party is refused the screen.
    if let Some(party_id) = world
        .objects
        .get_component::<PartyRef>(&player)
        .map(|r| r.0)
    {
        if let Some(cc_id) = super::command_channel::cc_id_of_party(world, party_id) {
            let is_cc_leader = world
                .command_channels
                .get(&cc_id)
                .is_some_and(|cc| cc.is_leader(player));
            if is_cc_leader {
                if world.matching_rooms.room_id_of(player).is_none() {
                    super::command_channel::create_cc_room(world, player, party_id);
                }
            } else {
                send_sm(
                    world,
                    player,
                    sm_ids::THE_COMMAND_CHANNEL_AFFILIATED_PARTY_S_PARTY_MEMBER_CANNOT_USE_THE_MATCHING_SCREEN,
                    &[],
                );
            }
            return;
        }
    }

    // Java: a party *member* may not browse — only an unpartied player or the
    // party leader.
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
                RoomKind::Party,
                title,
                pkt.loot_type,
                pkt.min_level,
                pkt.max_level,
                pkt.max_members.max(1),
                player,
            );
            world.matching_rooms.remove_from_waiting_list(player);
            set_in_room_flag(world, player, true);
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
        // Edit — leader only, and only their own (party) room id; MPCC rooms
        // are edited through `RequestExManageMpccRoom`.
        Some(room_id) => {
            let is_leader = world
                .matching_rooms
                .get(room_id)
                .is_some_and(|r| r.kind == RoomKind::Party && r.is_leader(player));
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

// ---------------------------------------------------------------------------
// Membership — the shared add/remove/disband core
// ---------------------------------------------------------------------------

/// Java `MatchingRoom.addMember` — dispatches on the room kind, so the
/// type-agnostic hooks (party-invite pull-in, etc.) notify with the right
/// packet family.
fn add_member(world: &mut World, room_id: i32, player: i32) -> bool {
    if world
        .matching_rooms
        .get(room_id)
        .is_some_and(|r| r.kind == RoomKind::CommandChannel)
    {
        return super::command_channel::cc_room_add_member(world, room_id, player);
    }
    add_member_party(world, room_id, player)
}

/// Java `MatchingRoom.deleteMember` — same kind dispatch as [`add_member`].
fn remove_member(world: &mut World, room_id: i32, player: i32, kicked: bool) {
    if world
        .matching_rooms
        .get(room_id)
        .is_some_and(|r| r.kind == RoomKind::CommandChannel)
    {
        super::command_channel::cc_room_remove_member(world, room_id, player, kicked);
        return;
    }
    remove_member_party(world, room_id, player, kicked);
}

/// Java `PartyMatchingRoom.notifyNewMember` (the party flavor).
/// Returns false when the room's level band or capacity refuses the joiner
/// (the joiner is told, nothing else happens).
fn add_member_party(world: &mut World, room_id: i32, player: i32) -> bool {
    let level = level_of(world, player);
    let accepted = world
        .matching_rooms
        .get(room_id)
        .is_some_and(|r| r.accepts(level));
    if !accepted {
        send_sm(
            world,
            player,
            sm_ids::YOU_DO_NOT_MEET_THE_REQUIREMENTS_TO_ENTER_THAT_PARTY_ROOM,
            &[],
        );
        return false;
    }
    let Some(room) = world.matching_rooms.get_mut(room_id) else {
        return false;
    };
    room.members.push(player);
    world.matching_rooms.remove_from_waiting_list(player);
    set_in_room_flag(world, player, true);
    broadcast_user_info(world, player);

    // Everyone already in the room learns about the newcomer...
    let name = name_of(world, player);
    let others: Vec<i32> = world
        .matching_rooms
        .get(room_id)
        .map(|r| {
            r.all_members()
                .into_iter()
                .filter(|&o| o != player)
                .collect()
        })
        .unwrap_or_default();
    let views = member_views(world, room_id);
    for oid in others {
        let pkt =
            server_packets::ex_party_room_member(member_type(world, room_id, oid).id(), &views);
        send(world, oid, pkt);
        send_sm(
            world,
            oid,
            sm_ids::C1_HAS_ENTERED_THE_PARTY_ROOM,
            &[SmParam::Text(name.clone())],
        );
    }
    // ...and the newcomer gets the room window.
    if let Some(info) = room_info_packet(world, room_id) {
        send(world, player, info);
    }
    let pkt =
        server_packets::ex_party_room_member(member_type(world, room_id, player).id(), &views);
    send(world, player, pkt);
    true
}

/// Java `PartyMatchingRoom.notifyRemovedMember` (the party flavor). `kicked`
/// selects the ousted-vs-left message pair.
fn remove_member_party(world: &mut World, room_id: i32, player: i32, kicked: bool) {
    let Some((leader_changed, room_deleted)) = world.matching_rooms.remove_member(room_id, player)
    else {
        return;
    };
    set_in_room_flag(world, player, false);
    broadcast_user_info(world, player);
    // Leaving a room puts you back on the looking-for-party list.
    world.matching_rooms.add_to_waiting_list(player);

    if !room_deleted {
        let name = name_of(world, player);
        let members: Vec<i32> = world
            .matching_rooms
            .get(room_id)
            .map(|r| r.all_members())
            .unwrap_or_default();
        let views = member_views(world, room_id);
        let info = room_info_packet(world, room_id);
        for oid in members {
            if let Some(info) = info.clone() {
                send(world, oid, info);
            }
            let pkt =
                server_packets::ex_party_room_member(member_type(world, room_id, oid).id(), &views);
            send(world, oid, pkt);
            send_sm(
                world,
                oid,
                if kicked {
                    sm_ids::C1_HAS_BEEN_KICKED_FROM_THE_PARTY_ROOM
                } else {
                    sm_ids::C1_HAS_LEFT_THE_PARTY_ROOM
                },
                &[SmParam::Text(name.clone())],
            );
            // Java sends this unconditionally; it is only true when the leader
            // actually changed.
            if leader_changed {
                send_sm(
                    world,
                    oid,
                    sm_ids::THE_LEADER_OF_THE_PARTY_ROOM_HAS_CHANGED,
                    &[],
                );
            }
        }
    }

    send_sm(
        world,
        player,
        if kicked {
            sm_ids::YOU_HAVE_BEEN_OUSTED_FROM_THE_PARTY_ROOM
        } else {
            sm_ids::YOU_HAVE_EXITED_THE_PARTY_ROOM
        },
        &[],
    );
    send(world, player, server_packets::ex_close_party_room());
}

/// Java `PartyMatchingRoom.disbandRoom`.
fn disband(world: &mut World, room_id: i32) {
    let Some(room) = world.matching_rooms.remove_room(room_id) else {
        return;
    };
    for oid in room.all_members() {
        send_sm(world, oid, sm_ids::THE_PARTY_ROOM_HAS_BEEN_DISBANDED, &[]);
        send(world, oid, server_packets::ex_close_party_room());
        set_in_room_flag(world, oid, false);
        broadcast_user_info(world, oid);
        world.matching_rooms.add_to_waiting_list(oid);
    }
}

// ---------------------------------------------------------------------------
// 0x81 RequestPartyMatchDetail — join a room
// ---------------------------------------------------------------------------

pub(crate) fn handle_request_party_match_detail(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(pkt) = cp::RequestPartyMatchDetail::read(body) else {
        return;
    };
    if world.matching_rooms.room_id_of(player).is_some() {
        return;
    }
    let room_id = if pkt.room_id > 0 {
        // Only party rooms are joinable through the party window (the CC
        // browser joins via `RequestExJoinMpccRoom`).
        world
            .matching_rooms
            .get(pkt.room_id)
            .filter(|r| r.kind == RoomKind::Party)
            .map(|r| r.id)
    } else {
        world
            .matching_rooms
            .find_room_at(pkt.location, pkt.level, |leader| location_of(world, leader))
    };
    if let Some(room_id) = room_id {
        add_member(world, room_id, player);
    }
}

// ---------------------------------------------------------------------------
// ex 0x09 RequestOustFromPartyRoom — the leader kicks a member
// ---------------------------------------------------------------------------

pub(crate) fn handle_oust_from_party_room(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(target) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };
    if target == player {
        return;
    }
    let Some(room_id) = world.matching_rooms.room_id_of(player) else {
        return;
    };
    if !world
        .matching_rooms
        .get(room_id)
        .is_some_and(|r| r.kind == RoomKind::Party && r.is_leader(player) && r.contains(target))
    {
        return;
    }

    // "You cannot dismiss a party member by force" — a room leader may not
    // kick someone who is in his own *party*; that has to go through the party
    // UI. (Java reads `player.getParty()` twice here, so the rule never fires.)
    let party_of = |w: &World, oid: i32| {
        w.objects
            .get_component::<PartyRef>(&oid)
            .map(|PartyRef(id)| *id)
    };
    if let (Some(a), Some(b)) = (party_of(world, player), party_of(world, target)) {
        if a == b {
            send_sm(
                world,
                player,
                sm_ids::YOU_CANNOT_DISMISS_A_PARTY_MEMBER_BY_FORCE,
                &[],
            );
            return;
        }
    }
    remove_member(world, room_id, target, true);
}

// ---------------------------------------------------------------------------
// ex 0x0A RequestDismissPartyRoom — the leader disbands
// ---------------------------------------------------------------------------

pub(crate) fn handle_dismiss_party_room(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    // Body is `(roomId, unused)` — Java reads and discards the second int.
    let Some(room_id) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };
    if world
        .matching_rooms
        .get(room_id)
        .is_some_and(|r| r.kind == RoomKind::Party && r.is_leader(player))
    {
        disband(world, room_id);
    }
}

// ---------------------------------------------------------------------------
// ex 0x0B RequestWithdrawPartyRoom — leave the room you are in
// ---------------------------------------------------------------------------

pub(crate) fn handle_withdraw_party_room(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(packet_room) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };
    let Some(room_id) = world.matching_rooms.room_id_of(player) else {
        return;
    };
    if room_id != packet_room
        || !world
            .matching_rooms
            .get(room_id)
            .is_some_and(|r| r.kind == RoomKind::Party)
    {
        return;
    }
    remove_member(world, room_id, player, false);
}

// ---------------------------------------------------------------------------
// ex 0x2F / ex 0x30 — invite a player to the room, and their answer
// ---------------------------------------------------------------------------

pub(crate) fn handle_ask_join_party_room(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(name) = commons::network::PacketReader::new(body).read_string() else {
        return;
    };
    // Java dereferences `getMatchingRoom()` inside the packet ctor without
    // checking it — an inviter with no room NPEs there.
    let Some(room_id) = world.matching_rooms.room_id_of(player) else {
        return;
    };

    let Some((_, target)) = super::party::find_player_by_name(world, &name) else {
        send_sm(world, player, sm_ids::THAT_PLAYER_IS_NOT_ONLINE, &[]);
        return;
    };
    if target == player {
        return;
    }
    if world.objects.has_component::<PendingRequest>(&target) {
        send_sm(
            world,
            player,
            sm_ids::C1_IS_ON_ANOTHER_TASK_PLEASE_TRY_AGAIN_LATER,
            &[SmParam::Text(name.clone())],
        );
        return;
    }

    super::party::install_request(
        world,
        player,
        target,
        RequestKind::PartyRoomInvite { room_id },
        super::party::REQUEST_TIMEOUT_TICKS,
    );
    let inviter = name_of(world, player);
    let title = world
        .matching_rooms
        .get(room_id)
        .map_or_else(String::new, |r| r.title.clone());
    send(
        world,
        target,
        server_packets::ex_ask_join_party_room(&inviter, &title),
    );
}

pub(crate) fn handle_answer_join_party_room(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(answer) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };

    // Clearing first means every path below leaves both sides free — Java has
    // an early return that strands `_activeRequester` set forever.
    let Some(req) = super::party::clear_linked_request(world, player) else {
        send_sm(world, player, sm_ids::THAT_PLAYER_IS_NOT_ONLINE, &[]);
        return;
    };
    let RequestKind::PartyRoomInvite { room_id } = req.kind else {
        return;
    };
    if answer != 1 {
        send_sm(
            world,
            req.other,
            sm_ids::THE_RECIPIENT_OF_YOUR_INVITATION_DID_NOT_ACCEPT_THE_PARTY_MATCHING_INVITATION,
            &[],
        );
        return;
    }
    if world.matching_rooms.get(room_id).is_none()
        || world.matching_rooms.room_id_of(player).is_some()
    {
        return;
    }
    add_member(world, room_id, player);
}

// ---------------------------------------------------------------------------
// Cross-system hooks
// ---------------------------------------------------------------------------

/// Logout / disconnect (`Player.deleteMe`): leave the room, then drop off the
/// waiting list — in that order, because leaving re-adds you to it.
pub(crate) fn on_player_leave_world(world: &mut World, object_id: i32) {
    if let Some(room_id) = world.matching_rooms.room_id_of(object_id) {
        remove_member(world, room_id, object_id, false);
    }
    world.matching_rooms.remove_from_waiting_list(object_id);
}

/// Java `RequestWithDrawalParty`: leaving your *party* also leaves the
/// matching room.
pub(crate) fn on_party_withdraw(world: &mut World, object_id: i32) {
    if let Some(room_id) = world.matching_rooms.room_id_of(object_id) {
        remove_member(world, room_id, object_id, false);
    }
}

/// Java `RequestAnswerJoinParty`: accepting a party invite from someone who
/// leads a matching room also puts you in that room.
pub(crate) fn on_party_invite_accepted(world: &mut World, requestor: i32, joiner: i32) {
    if world.matching_rooms.room_id_of(joiner).is_some() {
        return;
    }
    let Some(room_id) = world.matching_rooms.room_id_of(requestor) else {
        return;
    };
    add_member(world, room_id, joiner);
}
