//! Party matching room packets (G30) — `ListPartyWaiting` (0x9C),
//! `PartyRoomInfo` (0x9D), `ExPartyRoomMember` (0xFE 0x08),
//! `ExClosePartyRoom` (0xFE 0x09), `ExAskJoinPartyRoom` (0xFE 0x35) and
//! `ExListPartyMatchingWaitingRoom` (0xFE 0x36).
//!
//! Interlude scoping (see PLAN_G30_MAIL_PARTY_MATCHING.md): the instance-time
//! sub-blocks Java appends inside `ExPartyRoomMember` /
//! `ExListPartyMatchingWaitingRoom`, and the two trailing party-count ints of
//! `ListPartyWaiting` (its own source marks them `// Helios`), are **not**
//! written — an Interlude client desyncs on them.

use commons::network::PacketWriter;

use super::ex;
use super::opcodes;

/// Java's page size for both matching lists (`NUM_PER_PAGE`).
pub const ROOMS_PER_PAGE: usize = 64;

/// One member row of a room, as the room packets need it.
pub struct RoomMemberView {
    pub object_id: i32,
    pub name: String,
    pub class_id: i32,
    pub level: i32,
    /// The member's community-board region (`MapRegionManager.getBBs`).
    pub location: i32,
    /// `MatchingMemberType` ordinal for this member.
    pub member_type: i32,
}

/// One room row of the room-list packet.
pub struct RoomListView {
    pub id: i32,
    pub title: String,
    pub location: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub max_members: i32,
    pub leader_name: String,
    /// `(class id, name)` per member, leader first.
    pub members: Vec<(i32, String)>,
}

/// One row of the looking-for-party list.
pub struct WaitingPlayerView {
    pub name: String,
    pub class_id: i32,
    pub level: i32,
}

/// `ListPartyWaiting` (0x9C) — the room browser. `total` is the unpaged match
/// count; `rooms` is the current page.
pub fn list_party_waiting(total: usize, rooms: &[RoomListView]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::LIST_PARTY_WAITING);
    w.write_i32(total as i32);
    w.write_i32(rooms.len() as i32);
    for room in rooms {
        w.write_i32(room.id);
        w.write_string(&room.title);
        w.write_i32(room.location);
        w.write_i32(room.min_level);
        w.write_i32(room.max_level);
        w.write_i32(room.max_members);
        w.write_string(&room.leader_name);
        w.write_i32(room.members.len() as i32);
        for (class_id, name) in &room.members {
            w.write_i32(*class_id);
            w.write_string(name);
        }
    }
    w.into_bytes()
}

/// `PartyRoomInfo` (0x9D) — the room's own settings panel.
#[allow(clippy::too_many_arguments)]
pub fn party_room_info(
    room_id: i32,
    max_members: i32,
    min_level: i32,
    max_level: i32,
    loot_type: i32,
    location: i32,
    title: &str,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PARTY_ROOM_INFO);
    w.write_i32(room_id);
    w.write_i32(max_members);
    w.write_i32(min_level);
    w.write_i32(max_level);
    w.write_i32(loot_type);
    w.write_i32(location);
    w.write_string(title);
    w.into_bytes()
}

/// `ExPartyRoomMember` (0xFE 0x08) — the full member list. `recipient_type` is
/// the `MatchingMemberType` of the player this copy is being sent to.
///
/// Java builds the packet from the *removed* player when a member leaves, so
/// every recipient is told the leaver's type; the port passes the recipient's
/// own type, which is what the field means.
pub fn ex_party_room_member(recipient_type: i32, members: &[RoomMemberView]) -> Vec<u8> {
    let mut w = ex(opcodes::EX_PARTY_ROOM_MEMBER);
    w.write_i32(recipient_type);
    w.write_i32(members.len() as i32);
    for m in members {
        w.write_i32(m.object_id);
        w.write_string(&m.name);
        w.write_i32(m.class_id);
        w.write_i32(m.level);
        w.write_i32(m.location);
        w.write_i32(m.member_type);
    }
    w.into_bytes()
}

/// `ExClosePartyRoom` (0xFE 0x09) — close the room window. No body.
pub fn ex_close_party_room() -> Vec<u8> {
    ex(opcodes::EX_CLOSE_PARTY_ROOM).into_bytes()
}

/// `ExAskJoinPartyRoom` (0xFE 0x35) — the invite confirmation dialog.
pub fn ex_ask_join_party_room(inviter_name: &str, room_title: &str) -> Vec<u8> {
    let mut w = ex(opcodes::EX_ASK_JOIN_PARTY_ROOM);
    w.write_string(inviter_name);
    w.write_string(room_title);
    w.into_bytes()
}

/// `ExListPartyMatchingWaitingRoom` (0xFE 0x36) — the looking-for-party list.
pub fn ex_list_party_matching_waiting_room(total: usize, players: &[WaitingPlayerView]) -> Vec<u8> {
    let mut w = ex(opcodes::EX_LIST_PARTY_MATCHING_WAITING_ROOM);
    w.write_i32(total as i32);
    w.write_i32(players.len() as i32);
    for p in players {
        w.write_string(&p.name);
        w.write_i32(p.class_id);
        w.write_i32(p.level);
    }
    w.into_bytes()
}
