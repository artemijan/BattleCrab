//! Command channel / MPCC packets — `ExOpenMPCC` (0xFE 0x12), `ExCloseMPCC`
//! (0xFE 0x13), `ExAskJoinMPCC` (0xFE 0x1A), `ExMPCCShowPartyMemberInfo`
//! (0xFE 0x4C), `ExMPCCPartyInfoUpdate` (0xFE 0x5C), and the MPCC matching
//! room family: `ExMPCCRoomInfo` (0x9C), `ExListMpccWaiting` (0x9D),
//! `ExDissmissMPCCRoom` (0x9E — Java's spelling), `ExMPCCRoomMember` (0xA0),
//! `ExMPCCPartymasterList` (0xA3), `ExManageMpccRoomMember` (0x0A, sharing
//! Java's `EX_MANAGE_PARTY_ROOM_MEMBER` id with the party-room variant).
//!
//! `ExMultiPartyCommandChannelInfo` (0x31) *is* constructed — by the
//! `/channelinfo` user command (`usercommandhandlers/ChannelInfo`), which the
//! G15.5 user-command sweep wired; an earlier note here called it dead code.

use commons::network::PacketWriter;

use super::opcodes;

fn ex(sub: i16) -> PacketWriter {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(sub);
    w
}

/// `ExOpenMPCC` — open the command channel window. No body.
pub fn ex_open_mpcc() -> Vec<u8> {
    ex(opcodes::EX_OPEN_MPCC).into_bytes()
}

/// `ExCloseMPCC` — close the command channel window. No body.
pub fn ex_close_mpcc() -> Vec<u8> {
    ex(opcodes::EX_CLOSE_MPCC).into_bytes()
}

/// `ExAskJoinMPCC` — the yes/no invite dialog on the target party's leader.
pub fn ex_ask_join_mpcc(requestor_name: &str) -> Vec<u8> {
    let mut w = ex(opcodes::EX_ASK_JOIN_MPCC);
    w.write_string(requestor_name);
    w.write_i32(0); // unknown field — Java writes 0 and does not know what it is either
    w.into_bytes()
}

/// `ExMPCCPartyInfoUpdate` — a party joined (`mode` 1) or left (`mode` 0)
/// the channel.
pub fn ex_mpcc_party_info_update(
    leader_name: &str,
    leader_object_id: i32,
    member_count: i32,
    mode: i32,
) -> Vec<u8> {
    let mut w = ex(opcodes::EX_MPCC_PARTY_INFO_UPDATE);
    w.write_string(leader_name);
    w.write_i32(leader_object_id);
    w.write_i32(member_count);
    w.write_i32(mode);
    w.into_bytes()
}

/// One row of `ExMPCCShowPartyMemberInfo` — `(name, object id, class id)`,
/// party order (leader first).
pub struct PartyMemberInfoView {
    pub name: String,
    pub object_id: i32,
    pub class_id: i32,
}

/// `ExMPCCShowPartyMemberInfo` — a party's roster, queried by the CC window.
pub fn ex_mpcc_show_party_member_info(members: &[PartyMemberInfoView]) -> Vec<u8> {
    let mut w = ex(opcodes::EX_MPCC_SHOW_PARTY_MEMBER_INFO);
    w.write_i32(members.len() as i32);
    for m in members {
        w.write_string(&m.name);
        w.write_i32(m.object_id);
        w.write_i32(m.class_id);
    }
    w.into_bytes()
}

// ---- MPCC matching rooms -------------------------------------------------

/// One room row of `ExListMpccWaiting`.
pub struct MpccRoomListView {
    pub id: i32,
    pub title: String,
    pub member_count: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub location: i32,
    pub max_members: i32,
    pub leader_name: String,
}

/// `ExListMpccWaiting` — the CC-room browser. `total` is the unpaged match
/// count; `rooms` is the current page (Java pages by 64 like the party list).
pub fn ex_list_mpcc_waiting(total: usize, rooms: &[MpccRoomListView]) -> Vec<u8> {
    let mut w = ex(opcodes::EX_LIST_MPCC_WAITING);
    w.write_i32(total as i32);
    w.write_i32(rooms.len() as i32);
    for r in rooms {
        w.write_i32(r.id);
        w.write_string(&r.title);
        w.write_i32(r.member_count);
        w.write_i32(r.min_level);
        w.write_i32(r.max_level);
        w.write_i32(r.location);
        w.write_i32(r.max_members);
        w.write_string(&r.leader_name);
    }
    w.into_bytes()
}

/// `ExMPCCRoomInfo` — the room's settings panel.
pub fn ex_mpcc_room_info(
    room_id: i32,
    max_members: i32,
    min_level: i32,
    max_level: i32,
    loot_type: i32,
    location: i32,
    title: &str,
) -> Vec<u8> {
    let mut w = ex(opcodes::EX_MPCC_ROOM_INFO);
    w.write_i32(room_id);
    w.write_i32(max_members);
    w.write_i32(min_level);
    w.write_i32(max_level);
    w.write_i32(loot_type);
    w.write_i32(location);
    w.write_string(title);
    w.into_bytes()
}

/// One member row of `ExMPCCRoomMember` — note level precedes class id here,
/// while [`ex_manage_mpcc_room_member`] writes class id first.
pub struct MpccRoomMemberView {
    pub object_id: i32,
    pub name: String,
    pub level: i32,
    pub class_id: i32,
    pub location: i32,
    /// `MatchingMemberType` ordinal (3 = CC leader, 4 = CC party member,
    /// 5 = waiting party, 6 = waiting player without a party).
    pub member_type: i32,
}

/// `ExMPCCRoomMember` — the room's member list. `recipient_type` is the
/// `MatchingMemberType` ordinal of the player this copy goes to.
pub fn ex_mpcc_room_member(recipient_type: i32, members: &[MpccRoomMemberView]) -> Vec<u8> {
    let mut w = ex(opcodes::EX_MPCC_ROOM_MEMBER);
    w.write_i32(recipient_type);
    w.write_i32(members.len() as i32);
    for m in members {
        w.write_i32(m.object_id);
        w.write_string(&m.name);
        w.write_i32(m.level);
        w.write_i32(m.class_id);
        w.write_i32(m.location);
        w.write_i32(m.member_type);
    }
    w.into_bytes()
}

/// `ExManageMpccRoomMember` — a single-member add/update/delete row.
/// `mode`: 0 = add, 1 = update, 2 = delete (`ExManagePartyRoomMemberType`).
pub fn ex_manage_mpcc_room_member(
    mode: i32,
    object_id: i32,
    name: &str,
    class_id: i32,
    level: i32,
    location: i32,
    member_type: i32,
) -> Vec<u8> {
    let mut w = ex(opcodes::EX_MANAGE_PARTY_ROOM_MEMBER);
    w.write_i32(mode);
    w.write_i32(object_id);
    w.write_string(name);
    w.write_i32(class_id);
    w.write_i32(level);
    w.write_i32(location);
    w.write_i32(member_type);
    w.into_bytes()
}

/// `ExDissmissMPCCRoom` — the room was disbanded (close the window). No body.
pub fn ex_dissmiss_mpcc_room() -> Vec<u8> {
    ex(opcodes::EX_DISSMISS_MPCC_ROOM).into_bytes()
}

/// `ExMPCCPartymasterList` — the distinct party leader names in the room.
pub fn ex_mpcc_partymaster_list(names: &[String]) -> Vec<u8> {
    let mut w = ex(opcodes::EX_MPCC_PARTYMASTER_LIST);
    w.write_i32(names.len() as i32);
    for n in names {
        w.write_string(n);
    }
    w.into_bytes()
}

/// `ExMultiPartyCommandChannelInfo` — the `/channelinfo` window: the channel
/// leader, its total member count, and one row per party (leader name, leader
/// object id, member count). The loot int is Java's hard-coded 0.
pub fn ex_multi_party_command_channel_info(
    leader_name: &str,
    member_count: i32,
    parties: &[(String, i32, i32)],
) -> Vec<u8> {
    let mut w = ex(opcodes::EX_MULTI_PARTY_COMMAND_CHANNEL_INFO);
    w.write_string(leader_name);
    w.write_i32(0); // channel loot (Java writes 0)
    w.write_i32(member_count);
    w.write_i32(parties.len() as i32);
    for (name, leader_oid, count) in parties {
        w.write_string(name);
        w.write_i32(*leader_oid);
        w.write_i32(*count);
    }
    w.into_bytes()
}
