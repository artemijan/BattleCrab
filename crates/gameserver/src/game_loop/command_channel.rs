//! Command channels (MPCC) — port of `model/CommandChannel` and the
//! `RequestExAskJoinMPCC` (0xD0:0x06) / `RequestExAcceptJoinMPCC` (0x07) /
//! `RequestExOustFromMPCC` (0x08) / `RequestExMPCCShowPartyMembersInfo`
//! (0x2D) flows, plus the `Party`-side propagation rules (member join/leave,
//! leader change, party collapse).
//!
//! Deliberate divergences from Java (each a documented Mobius hazard):
//! - `disbandChannel` iterates a snapshot with the registry entry removed
//!   first — Java mutates `_parties` while iterating and can recurse into
//!   itself through `removeParty` (`CommandChannel.java:118-131`).
//! - `RequestExAcceptJoinMPCC` re-validates that the requestor still leads a
//!   party — Java NPEs on `requestor.getParty()` if it dissolved mid-invite
//!   (`RequestExAcceptJoinMPCC.java:54`).
//! Kept faithfully even though they look odd: the duplicate SM 1580/1582 the
//! accept path sends on top of the constructor/`addParty` broadcasts, and the
//! roster query answering for any party with no shared-channel check.

use crate::game_loop::helpers::send_to_player;
use crate::game_loop::helpers::{get_others_in_matching_room, send_to_client};
use crate::model::Player;
use crate::model::command_channel::CommandChannel;
use crate::model::components::{PartyRef, PendingRequest, RequestKind};
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, PartyMemberInfoView, SmParam, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

use super::party::{REQUEST_TIMEOUT_TICKS, broadcast_to_party, install_request};
use crate::game_loop::helpers::player_name_or_empty;

/// `RequestExAskJoinMPCC.askJoinMPCC`'s right-to-form constants: clan level
/// ≥ 5 for a clan leader, the Strategy Guide item (not consumed), or pledge
/// class ≥ Baron with the Clan Imperium skill.
const STRATEGY_GUIDE_ITEM_ID: i32 = 8871;
const CLAN_IMPERIUM_SKILL_ID: i32 = 391;
const FORMING_CLAN_LEVEL: i32 = 5;
const FORMING_PLEDGE_CLASS: u8 = 5;

/// Java `Player.sendMessage` — the plain-text `$s1` system message.
fn send_text(world: &World, object_id: i32, text: &str) {
    send_sm(
        world,
        object_id,
        sm_ids::S1_TEXT,
        &[SmParam::Text(text.to_string())],
    );
}

pub(crate) fn party_id_of(world: &World, object_id: i32) -> Option<u32> {
    world
        .objects
        .get_component::<PartyRef>(&object_id)
        .map(|r| r.0)
}

/// The command channel a party belongs to (`Party._commandChannel`).
pub(crate) fn cc_id_of_party(world: &World, party_id: u32) -> Option<u32> {
    world
        .command_channels
        .iter()
        .find(|(_, cc)| cc.contains_party(party_id))
        .map(|(&id, _)| id)
}

/// `CommandChannel.getMembers()` — every member of every party, party join
/// order (Java's set iteration order is unspecified; nothing depends on it).
pub(crate) fn cc_members(world: &World, cc_id: u32) -> Vec<i32> {
    let Some(cc) = world.command_channels.get(&cc_id) else {
        return Vec::new();
    };
    cc.parties
        .iter()
        .filter_map(|pid| world.parties.get(pid))
        .flat_map(|p| p.members.iter().copied())
        .collect()
}

/// The group a party shares a kill with — Java's
/// `isInCommandChannel() ? cc.getMembers() : party.getMembers()`, which both the
/// exp/sp split and the raid-point split key their reward off. Empty for a party
/// id that no longer exists.
pub(crate) fn cc_or_party_members(world: &World, party_id: u32) -> Vec<i32> {
    match cc_id_of_party(world, party_id) {
        Some(cc_id) => cc_members(world, cc_id),
        None => world
            .parties
            .get(&party_id)
            .map(|p| p.members.clone())
            .unwrap_or_default(),
    }
}

/// `AbstractPlayerGroup.broadcastPacket` on the channel.
pub(crate) fn broadcast_to_cc(world: &World, cc_id: u32, packet: &[u8]) {
    for m in cc_members(world, cc_id) {
        send_to_player(world, m, packet.to_vec());
    }
}

pub(crate) fn broadcast_sm_to_cc(world: &World, cc_id: u32, message_id: i16, params: &[SmParam]) {
    broadcast_to_cc(
        world,
        cc_id,
        &server_packets::system_message_with(message_id, params),
    );
}

/// `Party.getLevel()` — the highest member level.
pub(crate) fn party_level(world: &World, party_id: u32) -> i32 {
    world
        .parties
        .get(&party_id)
        .map(|p| {
            p.members
                .iter()
                .filter_map(|m| world.objects.get_component::<Player>(m))
                .map(|pl| pl.level)
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Invite (`RequestExAskJoinMPCC` → `ExAskJoinMPCC`)
// ---------------------------------------------------------------------------

/// `RequestExAskJoinMPCC.askJoinMPCC`'s "may form/lead a command channel"
/// check: clan leader of a level-5 clan, holder of a Strategy Guide, or
/// Baron+ knowing Clan Imperium.
fn has_forming_right(world: &World, object_id: i32) -> bool {
    let Some(player) = world.objects.get_component::<Player>(&object_id) else {
        return false;
    };
    if let Some(clan) = world.clans.get(&player.clan_id) {
        if clan.leader_id == object_id && clan.level >= FORMING_CLAN_LEVEL {
            return true;
        }
        if clan.pledge_class_of(object_id) >= FORMING_PLEDGE_CLASS
            && world
                .objects
                .get_component::<crate::model::components::SkillBook>(&object_id)
                .is_some_and(|book| book.0.contains_key(&CLAN_IMPERIUM_SKILL_ID))
        {
            return true;
        }
    }
    world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&object_id)
        .is_some_and(|inv| {
            inv.items()
                .iter()
                .any(|i| i.item_id == STRATEGY_GUIDE_ITEM_ID)
        })
}

pub(crate) fn handle_request_ex_ask_join_mpcc(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestExAskJoinMpcc::read(body) else {
        return;
    };
    let Some(requestor) = world.player_oid(client_id) else {
        return;
    };
    let Some((_, target)) = super::party::find_player_by_name(world, &pkt.name) else {
        return;
    };

    // The whole Java body is inside `if (player.isInParty())` — a party-less
    // requestor gets no feedback at all.
    let Some(req_party) = party_id_of(world, requestor) else {
        return;
    };
    if party_id_of(world, target) == Some(req_party) {
        return; // own party
    }
    let leads_party = world
        .parties
        .get(&req_party)
        .is_some_and(|p| p.is_leader(requestor));
    let cc_id = cc_id_of_party(world, req_party);
    let leads_channel = cc_id.is_none()
        || cc_id.is_some_and(|id| {
            world
                .command_channels
                .get(&id)
                .is_some_and(|cc| cc.is_leader(requestor))
        });
    if !leads_party || !leads_channel {
        send_sm(
            world,
            requestor,
            sm_ids::YOU_DO_NOT_HAVE_AUTHORITY_TO_INVITE_SOMEONE_TO_THE_COMMAND_CHANNEL,
            &[],
        );
        return;
    }

    // Target-side checks.
    let Some(target_party) = party_id_of(world, target) else {
        send_text(
            world,
            requestor,
            &format!(
                "{} doesn't have party and cannot be invited to Command Channel.",
                player_name_or_empty(world, target)
            ),
        );
        return;
    };
    if cc_id_of_party(world, target_party).is_some() {
        send_sm(
            world,
            requestor,
            sm_ids::C1_S_PARTY_IS_ALREADY_A_MEMBER_OF_THE_COMMAND_CHANNEL,
            &[SmParam::PlayerName(player_name_or_empty(world, target))],
        );
        return;
    }

    if !has_forming_right(world, requestor) {
        send_sm(
            world,
            requestor,
            sm_ids::COMMAND_CHANNELS_CAN_ONLY_BE_FORMED_BY_A_PARTY_LEADER_WHO_IS_ALSO_THE_LEADER_OF_A_LEVEL_5_CLAN,
            &[],
        );
        return;
    }

    // The dialog always goes to the target *party's leader*, not the clicked
    // player.
    let Some(target_leader) = world.parties.get(&target_party).map(|p| p.leader()) else {
        return;
    };
    if world
        .objects
        .get_component::<PendingRequest>(&target_leader)
        .is_some()
    {
        send_sm(
            world,
            requestor,
            sm_ids::C1_IS_ON_ANOTHER_TASK_PLEASE_TRY_AGAIN_LATER,
            &[SmParam::PlayerName(player_name_or_empty(
                world,
                target_leader,
            ))],
        );
        return;
    }

    install_request(
        world,
        requestor,
        target_leader,
        RequestKind::CommandChannelInvite,
        REQUEST_TIMEOUT_TICKS,
    );
    let requestor_name = player_name_or_empty(world, requestor);
    send_sm(
        world,
        target_leader,
        sm_ids::C1_IS_INVITING_YOU_TO_A_COMMAND_CHANNEL_DO_YOU_ACCEPT,
        &[SmParam::PlayerName(requestor_name.clone())],
    );
    send_to_player(
        world,
        target_leader,
        server_packets::ex_ask_join_mpcc(&requestor_name),
    );
    send_text(
        world,
        requestor,
        &format!(
            "You invited {} to your Command Channel.",
            player_name_or_empty(world, target_leader)
        ),
    );
}

// ---------------------------------------------------------------------------
// Accept (`RequestExAcceptJoinMPCC`)
// ---------------------------------------------------------------------------

/// `CommandChannel(Player)` — form a channel around the requestor's party.
/// Returns the new channel id.
pub(crate) fn create_channel(world: &mut World, leader: i32, leader_party: u32) -> u32 {
    let cc_id = world.next_command_channel_id;
    world.next_command_channel_id += 1;
    let level = party_level(world, leader_party);
    world
        .command_channels
        .insert(cc_id, CommandChannel::new(leader, leader_party, level));
    broadcast_to_party(
        world,
        leader_party,
        &server_packets::system_message_with(sm_ids::THE_COMMAND_CHANNEL_HAS_BEEN_FORMED, &[]),
        None,
    );
    broadcast_to_party(world, leader_party, &server_packets::ex_open_mpcc(), None);
    cc_id
}

/// `CommandChannel.addParty` — announce to the existing channel first (the
/// joining party must not receive its own add), then attach and greet.
pub(crate) fn add_party_to_channel(world: &mut World, cc_id: u32, party_id: u32) {
    let (leader_name, leader_oid, member_count) = get_party_info(world, party_id);
    broadcast_to_cc(
        world,
        cc_id,
        &server_packets::ex_mpcc_party_info_update(&leader_name, leader_oid, member_count, 1),
    );
    let level = party_level(world, party_id);
    let Some(cc) = world.command_channels.get_mut(&cc_id) else {
        return;
    };
    cc.parties.push(party_id);
    if level > cc.level {
        cc.level = level;
    }
    broadcast_to_party(
        world,
        party_id,
        &server_packets::system_message_with(sm_ids::YOU_HAVE_JOINED_THE_COMMAND_CHANNEL, &[]),
        None,
    );
    broadcast_to_party(world, party_id, &server_packets::ex_open_mpcc(), None);
}

pub(crate) fn handle_request_ex_accept_join_mpcc(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestExAcceptJoinMpcc::read(body) else {
        return;
    };
    let Some(answerer) = world.player_oid(client_id) else {
        return;
    };
    let Some(req) = world
        .objects
        .get_component::<PendingRequest>(&answerer)
        .copied()
        .filter(|r| r.answerer && r.kind == RequestKind::CommandChannelInvite)
    else {
        return;
    };
    super::party::clear_linked_request(world, answerer);
    let requestor = req.other;

    if pkt.response != 1 {
        send_text(
            world,
            requestor,
            "The player declined to join your Command Channel.",
        );
        return;
    }

    // Re-validation Java skips (it NPEs instead): both sides must still lead
    // live parties, and the answerer's party must still be channel-less.
    let Some(req_party) = party_id_of(world, requestor) else {
        return;
    };
    let Some(ans_party) = party_id_of(world, answerer) else {
        return;
    };
    if cc_id_of_party(world, ans_party).is_some() {
        return;
    }

    let mut new_cc = false;
    let cc_id = match cc_id_of_party(world, req_party) {
        Some(id) => id,
        None => {
            new_cc = true;
            create_channel(world, requestor, req_party)
        }
    };
    if new_cc {
        // Java sends the formation message to the requestor a second time on
        // top of the constructor broadcast — kept for parity.
        send_sm(
            world,
            requestor,
            sm_ids::THE_COMMAND_CHANNEL_HAS_BEEN_FORMED,
            &[],
        );
    }
    add_party_to_channel(world, cc_id, ans_party);
    if !new_cc {
        // Same duplicate on the join side (`addParty` already told the party).
        send_sm(
            world,
            answerer,
            sm_ids::YOU_HAVE_JOINED_THE_COMMAND_CHANNEL,
            &[],
        );
    }
}

// ---------------------------------------------------------------------------
// Remove / disband (`CommandChannel.removeParty` / `disbandChannel`)
// ---------------------------------------------------------------------------

/// `CommandChannel.disbandChannel` — detach every party (each gets
/// `ExCloseMPCC`), then drop the registry entry. The SM 1581 broadcast is the
/// *caller's* job (Java sends it from `removeParty` before disbanding).
pub(crate) fn disband_channel(world: &mut World, cc_id: u32) {
    let Some(cc) = world.command_channels.remove(&cc_id) else {
        return;
    };
    for party_id in cc.parties {
        broadcast_to_party(world, party_id, &server_packets::ex_close_mpcc(), None);
    }
}

/// `CommandChannel.removeParty` — detach one party; the channel disbands once
/// fewer than two parties remain.
pub(crate) fn remove_party_from_channel(world: &mut World, cc_id: u32, party_id: u32) {
    let remaining = {
        let Some(cc) = world.command_channels.get_mut(&cc_id) else {
            return;
        };
        cc.parties.retain(|&p| p != party_id);
        cc.parties.clone()
    };
    let level = remaining
        .iter()
        .map(|&p| party_level(world, p))
        .max()
        .unwrap_or(0);
    if let Some(cc) = world.command_channels.get_mut(&cc_id) {
        cc.level = level;
    }
    broadcast_to_party(world, party_id, &server_packets::ex_close_mpcc(), None);
    if remaining.len() < 2 {
        broadcast_sm_to_cc(
            world,
            cc_id,
            sm_ids::THE_COMMAND_CHANNEL_HAS_BEEN_DISBANDED,
            &[],
        );
        disband_channel(world, cc_id);
        return;
    }
    let (leader_name, leader_oid, member_count) = get_party_info(world, party_id);
    broadcast_to_cc(
        world,
        cc_id,
        &server_packets::ex_mpcc_party_info_update(&leader_name, leader_oid, member_count, 0),
    );
}

// ---------------------------------------------------------------------------
// Oust (`RequestExOustFromMPCC`)
// ---------------------------------------------------------------------------

pub(crate) fn handle_request_ex_oust_from_mpcc(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestExOustFromMpcc::read(body) else {
        return;
    };
    let Some(player) = world.player_oid(client_id) else {
        return;
    };

    // Java's single compound guard; any miss → SM 50.
    let target = super::party::find_player_by_name(world, &pkt.name).map(|(_, oid)| oid);
    let target_party = target.and_then(|t| party_id_of(world, t));
    let player_party = party_id_of(world, player);
    let player_cc = player_party.and_then(|p| cc_id_of_party(world, p));
    let target_cc = target_party.and_then(|p| cc_id_of_party(world, p));
    let is_cc_leader = player_cc.is_some_and(|id| {
        world
            .command_channels
            .get(&id)
            .is_some_and(|cc| cc.is_leader(player))
    });
    if target.is_none()
        || target_party.is_none()
        || player_party.is_none()
        || player_cc.is_none()
        || target_cc != player_cc
        || !is_cc_leader
    {
        send_sm(world, player, sm_ids::YOUR_TARGET_CANNOT_BE_FOUND, &[]);
        return;
    }
    if target == Some(player) {
        return;
    }
    let (cc_id, target_party) = (player_cc.unwrap(), target_party.unwrap());
    let ousted_leader_name = world
        .parties
        .get(&target_party)
        .map(|p| player_name_or_empty(world, p.leader()))
        .unwrap_or_default();

    remove_party_from_channel(world, cc_id, target_party);
    broadcast_to_party(
        world,
        target_party,
        &server_packets::system_message_with(
            sm_ids::YOU_WERE_DISMISSED_FROM_THE_COMMAND_CHANNEL,
            &[],
        ),
        None,
    );
    // Only when the channel survived the removal.
    if world.command_channels.contains_key(&cc_id) {
        broadcast_sm_to_cc(
            world,
            cc_id,
            sm_ids::C1_S_PARTY_HAS_BEEN_DISMISSED_FROM_THE_COMMAND_CHANNEL,
            &[SmParam::PlayerName(ousted_leader_name)],
        );
    }
}

// ---------------------------------------------------------------------------
// Roster query (`RequestExMPCCShowPartyMembersInfo`)
// ---------------------------------------------------------------------------

pub(crate) fn handle_request_ex_mpcc_show_party_members_info(
    world: &mut World,
    client_id: u32,
    body: &[u8],
) {
    let Some(pkt) = cp::RequestExMpccShowPartyMembersInfo::read(body) else {
        return;
    };
    let Some(ClientSession::InGame(_)) = world.clients.get(&client_id) else {
        return;
    };
    // Java answers for any party, queried by any player — no CC check.
    let Some(party_id) = party_id_of(world, pkt.party_leader_object_id) else {
        return;
    };
    let members: Vec<PartyMemberInfoView> = world
        .parties
        .get(&party_id)
        .map(|p| p.members.clone())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|oid| {
            world
                .objects
                .get_component::<Player>(&oid)
                .map(|pl| PartyMemberInfoView {
                    name: pl.name.clone(),
                    object_id: oid,
                    class_id: pl.class_id,
                })
        })
        .collect();
    send_to_client(
        world,
        client_id,
        server_packets::ex_mpcc_show_party_member_info(&members),
    );
}

// ---------------------------------------------------------------------------
// Party-side propagation (Java `Party` integration points)
// ---------------------------------------------------------------------------

/// `Party.addPartyMember`: a member joining a channelled party gets the CC
/// window opened.
pub(crate) fn on_party_member_added(world: &World, party_id: u32, new_member: i32) {
    if cc_id_of_party(world, party_id).is_some() {
        send_to_player(world, new_member, server_packets::ex_open_mpcc());
    }
}

/// `Party.removePartyMember`: the leaver's CC window closes.
pub(crate) fn on_party_member_removed(world: &World, party_id: u32, leaver: i32) {
    if cc_id_of_party(world, party_id).is_some() {
        send_to_player(world, leaver, server_packets::ex_close_mpcc());
    }
}

/// `Party.removePartyMember`'s collapse branch: when a channelled party
/// dissolves, the whole channel dies if it was the CC leader's party,
/// otherwise just that party is detached.
pub(crate) fn on_party_dissolving(world: &mut World, party_id: u32) {
    let Some(cc_id) = cc_id_of_party(world, party_id) else {
        return;
    };
    let leader_party_dissolved = world
        .command_channels
        .get(&cc_id)
        .zip(world.parties.get(&party_id))
        .is_some_and(|(cc, p)| p.contains(cc.leader));
    if leader_party_dissolved {
        broadcast_sm_to_cc(
            world,
            cc_id,
            sm_ids::THE_COMMAND_CHANNEL_HAS_BEEN_DISBANDED,
            &[],
        );
        disband_channel(world, cc_id);
    } else {
        remove_party_from_channel(world, cc_id, party_id);
    }
}

/// `Party.setLeader`'s CC tail: when the channel leader's party changes
/// leader, channel authority follows.
pub(crate) fn on_party_leader_changed(
    world: &mut World,
    party_id: u32,
    old_leader: i32,
    new_leader: i32,
) {
    let Some(cc_id) = cc_id_of_party(world, party_id) else {
        return;
    };
    let level = world
        .objects
        .get_component::<Player>(&new_leader)
        .map(|p| p.level)
        .unwrap_or(0);
    let Some(cc) = world.command_channels.get_mut(&cc_id) else {
        return;
    };
    if !cc.is_leader(old_leader) {
        return;
    }
    cc.leader = new_leader;
    if level > cc.level {
        cc.level = level;
    }
    broadcast_sm_to_cc(
        world,
        cc_id,
        sm_ids::COMMAND_CHANNEL_AUTHORITY_HAS_BEEN_TRANSFERRED_TO_C1,
        &[SmParam::PlayerName(player_name_or_empty(world, new_leader))],
    );
}

// ---------------------------------------------------------------------------
// MPCC matching rooms (`model/matching/CommandChannelMatchingRoom` + the ex
// 0x5A–0x61 packet family)
// ---------------------------------------------------------------------------

use crate::model::matching_room::RoomKind;
use crate::network::server_packets::{MpccRoomListView, MpccRoomMemberView, ROOMS_PER_PAGE};

/// `RequestPartyMatchConfig`'s hardcoded CC-room defaults: min level 1,
/// max level = the leader's level, 50 members.
const MPCC_ROOM_MAX_MEMBERS: i32 = 50;

/// `MatchingMemberType` ordinals for the CC-room member rows.
const TYPE_CC_LEADER: i32 = 3;
const TYPE_CC_PARTY_MEMBER: i32 = 4;
const TYPE_WAITING_PARTY: i32 = 5;
const TYPE_WAITING_PLAYER_NO_PARTY: i32 = 6;

/// `CommandChannelMatchingRoom.getMemberType`.
fn cc_room_member_type(world: &World, room_id: i32, object_id: i32) -> i32 {
    let Some(room) = world.matching_rooms.get(room_id) else {
        return TYPE_WAITING_PARTY;
    };
    if room.is_leader(object_id) {
        return TYPE_CC_LEADER;
    }
    let Some(member_party) = party_id_of(world, object_id) else {
        return TYPE_WAITING_PLAYER_NO_PARTY;
    };
    let leader_party = party_id_of(world, room.leader);
    let in_leaders_channel = leader_party
        .and_then(|p| cc_id_of_party(world, p))
        .zip(cc_id_of_party(world, member_party))
        .is_some_and(|(a, b)| a == b);
    if leader_party == Some(member_party) || in_leaders_channel {
        TYPE_CC_PARTY_MEMBER
    } else {
        TYPE_WAITING_PARTY
    }
}

fn cc_room_member_views(world: &World, room_id: i32) -> Vec<MpccRoomMemberView> {
    world
        .matching_rooms
        .get(room_id)
        .map(|r| r.all_members())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|oid| {
            let p = world.objects.get_component::<Player>(&oid)?;
            Some(MpccRoomMemberView {
                object_id: oid,
                name: p.name.clone(),
                level: p.level,
                class_id: p.class_id,
                location: super::party_room::location_of(world, oid),
                member_type: cc_room_member_type(world, room_id, oid),
            })
        })
        .collect()
}

fn cc_room_info_packet(world: &World, room_id: i32) -> Option<Vec<u8>> {
    let room = world.matching_rooms.get(room_id)?;
    Some(server_packets::ex_mpcc_room_info(
        room.id,
        room.max_members,
        room.min_level,
        room.max_level,
        room.loot,
        super::party_room::location_of(world, room.leader),
        &room.title,
    ))
}

/// `new CommandChannelMatchingRoom(...)` out of `RequestPartyMatchConfig`:
/// title = the leader's name, loot = the party's distribution type, level
/// band 1..=leader level, 50 members.
pub(crate) fn create_cc_room(world: &mut World, leader: i32, party_id: u32) {
    let (title, max_level) = world
        .objects
        .get_component::<Player>(&leader)
        .map(|p| (p.name.clone(), p.level))
        .unwrap_or_default();
    let loot = world
        .parties
        .get(&party_id)
        .map(|p| p.distribution.id())
        .unwrap_or(0);
    let room_id = world.matching_rooms.create_room(
        RoomKind::CommandChannel,
        title,
        loot,
        1,
        max_level,
        MPCC_ROOM_MAX_MEMBERS,
        leader,
    );
    world.matching_rooms.remove_from_waiting_list(leader);
    super::party_room::set_in_room_flag(world, leader, true);
    super::player_info::broadcast_user_info(world, leader);
    // `onRoomCreation` (SM 3000) + `notifyNewMember`'s new-player half.
    send_sm(
        world,
        leader,
        sm_ids::THE_COMMAND_CHANNEL_MATCHING_ROOM_WAS_CREATED,
        &[],
    );
    if let Some(info) = cc_room_info_packet(world, room_id) {
        send_to_player(world, leader, info);
    }
    let views = cc_room_member_views(world, room_id);
    send_to_player(
        world,
        leader,
        server_packets::ex_mpcc_room_member(cc_room_member_type(world, room_id, leader), &views),
    );
}

/// `CommandChannelMatchingRoom.notifyNewMember` behind `addMember`'s gate.
/// Returns false when the band/capacity refuses the joiner (SM 2996).
pub(crate) fn cc_room_add_member(world: &mut World, room_id: i32, player: i32) -> bool {
    let level = world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.level)
        .unwrap_or(0);
    let accepted = world
        .matching_rooms
        .get(room_id)
        .is_some_and(|r| r.accepts(level));
    if !accepted {
        send_sm(
            world,
            player,
            sm_ids::YOU_CANNOT_ENTER_THE_COMMAND_CHANNEL_MATCHING_ROOM_BECAUSE_YOU_DO_NOT_MEET_THE_REQUIREMENTS,
            &[],
        );
        return false;
    }
    let Some(room) = world.matching_rooms.get_mut(room_id) else {
        return false;
    };
    room.members.push(player);
    world.matching_rooms.remove_from_waiting_list(player);
    super::party_room::set_in_room_flag(world, player, true);
    super::player_info::broadcast_user_info(world, player);

    let name = player_name_or_empty(world, player);
    let (class_id, level) = world
        .objects
        .get_component::<Player>(&player)
        .map(|p| (p.class_id, p.level))
        .unwrap_or_default();
    let location = super::party_room::location_of(world, player);
    let joiner_type = cc_room_member_type(world, room_id, player);
    let others: Vec<i32> = get_others_in_matching_room(world, room_id, player);
    // Java's `notifyNewMember` bug sends each existing member *their own* row
    // here (`ExManageMpccRoomMember(member, ...)`, line 64) — the port sends
    // the joiner's row, which is what the add mode means.
    let add_row = server_packets::ex_manage_mpcc_room_member(
        0,
        player,
        &name,
        class_id,
        level,
        location,
        joiner_type,
    );
    for oid in others {
        send_to_player(world, oid, add_row.clone());
        send_sm(
            world,
            oid,
            sm_ids::C1_ENTERED_THE_COMMAND_CHANNEL_MATCHING_ROOM,
            &[SmParam::PlayerName(name.clone())],
        );
    }
    if let Some(info) = cc_room_info_packet(world, room_id) {
        send_to_player(world, player, info);
    }
    let views = cc_room_member_views(world, room_id);
    send_to_player(
        world,
        player,
        server_packets::ex_mpcc_room_member(joiner_type, &views),
    );
    true
}

/// `CommandChannelMatchingRoom.notifyRemovedMember` behind `deleteMember`.
pub(crate) fn cc_room_remove_member(world: &mut World, room_id: i32, player: i32, kicked: bool) {
    let Some((_leader_changed, room_deleted)) = world.matching_rooms.remove_member(room_id, player)
    else {
        return;
    };
    super::party_room::set_in_room_flag(world, player, false);
    super::player_info::broadcast_user_info(world, player);
    world.matching_rooms.add_to_waiting_list(player);

    if !room_deleted {
        let members: Vec<i32> = world
            .matching_rooms
            .get(room_id)
            .map(|r| r.all_members())
            .unwrap_or_default();
        let views = cc_room_member_views(world, room_id);
        let info = cc_room_info_packet(world, room_id);
        for oid in members {
            if let Some(info) = info.clone() {
                send_to_player(world, oid, info);
            }
            // Java computes the member-type field for the *removed* player;
            // the port sends the recipient's own type (same deliberate fix as
            // the party-room port).
            send_to_player(
                world,
                oid,
                server_packets::ex_mpcc_room_member(
                    cc_room_member_type(world, room_id, oid),
                    &views,
                ),
            );
        }
    }
    send_sm(
        world,
        player,
        if kicked {
            sm_ids::YOU_WERE_EXPELLED_FROM_THE_COMMAND_CHANNEL_MATCHING_ROOM
        } else {
            sm_ids::YOU_EXITED_FROM_THE_COMMAND_CHANNEL_MATCHING_ROOM
        },
        &[],
    );
}

/// `CommandChannelMatchingRoom.disbandRoom`.
fn cc_room_disband(world: &mut World, room_id: i32) {
    let Some(room) = world.matching_rooms.remove_room(room_id) else {
        return;
    };
    for oid in room.all_members() {
        send_sm(
            world,
            oid,
            sm_ids::THE_COMMAND_CHANNEL_MATCHING_ROOM_WAS_CANCELLED,
            &[],
        );
        send_to_player(world, oid, server_packets::ex_dissmiss_mpcc_room());
        super::party_room::set_in_room_flag(world, oid, false);
        super::player_info::broadcast_user_info(world, oid);
        world.matching_rooms.add_to_waiting_list(oid);
    }
}

/// The CC room a player is in, when it is a CC room.
fn cc_room_of(world: &World, player: i32) -> Option<i32> {
    world
        .matching_rooms
        .room_of(player)
        .filter(|r| r.kind == RoomKind::CommandChannel)
        .map(|r| r.id)
}

pub(crate) fn handle_request_ex_list_mpcc_waiting(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestExListMpccWaiting::read(body) else {
        return;
    };
    let Some(ClientSession::InGame(_)) = world.clients.get(&client_id) else {
        return;
    };
    // Java filters by the packet's own level field, not the player's.
    let ids = world
        .matching_rooms
        .find_cc_rooms(pkt.location, pkt.level, |leader| {
            super::party_room::location_of(world, leader)
        });
    let total = ids.len();
    let start = (pkt.page.max(1) as usize - 1) * ROOMS_PER_PAGE;
    let rows: Vec<MpccRoomListView> = ids
        .into_iter()
        .skip(start)
        .take(ROOMS_PER_PAGE)
        .filter_map(|id| {
            let room = world.matching_rooms.get(id)?;
            Some(MpccRoomListView {
                id: room.id,
                title: room.title.clone(),
                member_count: room.member_count(),
                min_level: room.min_level,
                max_level: room.max_level,
                location: super::party_room::location_of(world, room.leader),
                max_members: room.max_members,
                leader_name: player_name_or_empty(world, room.leader),
            })
        })
        .collect();
    send_to_client(
        world,
        client_id,
        server_packets::ex_list_mpcc_waiting(total, &rows),
    );
}

pub(crate) fn handle_request_ex_manage_mpcc_room(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestExManageMpccRoom::read(body) else {
        return;
    };
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let leads = cc_room_of(world, player)
        .filter(|&id| id == pkt.room_id)
        .and_then(|id| world.matching_rooms.get(id))
        .is_some_and(|r| r.is_leader(player));
    if !leads {
        return;
    }
    if let Some(room) = world.matching_rooms.get_mut(pkt.room_id) {
        // Java applies the edit with no range validation at all.
        room.title = pkt.title;
        room.max_members = pkt.max_members;
        room.min_level = pkt.min_level;
        room.max_level = pkt.max_level;
    }
    let members: Vec<i32> = world
        .matching_rooms
        .get(pkt.room_id)
        .map(|r| r.all_members())
        .unwrap_or_default();
    if let Some(info) = cc_room_info_packet(world, pkt.room_id) {
        for oid in members {
            send_to_player(world, oid, info.clone());
        }
    }
    send_sm(
        world,
        player,
        sm_ids::THE_COMMAND_CHANNEL_MATCHING_ROOM_INFORMATION_WAS_EDITED,
        &[],
    );
}

pub(crate) fn handle_request_ex_join_mpcc_room(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestExJoinMpccRoom::read(body) else {
        return;
    };
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if world.matching_rooms.room_id_of(player).is_some() {
        return;
    }
    if world
        .matching_rooms
        .get(pkt.room_id)
        .is_some_and(|r| r.kind == RoomKind::CommandChannel)
    {
        cc_room_add_member(world, pkt.room_id, player);
    }
}

pub(crate) fn handle_request_ex_oust_from_mpcc_room(
    world: &mut World,
    client_id: u32,
    body: &[u8],
) {
    let Some(pkt) = cp::RequestExOustFromMpccRoom::read(body) else {
        return;
    };
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(room_id) = cc_room_of(world, player).filter(|&id| {
        world
            .matching_rooms
            .get(id)
            .is_some_and(|r| r.is_leader(player))
    }) else {
        return;
    };
    // Java kicks any online player by object id without a membership check —
    // the port requires the target to actually be in the room.
    if world
        .matching_rooms
        .get(room_id)
        .is_some_and(|r| r.contains(pkt.object_id) && pkt.object_id != player)
    {
        cc_room_remove_member(world, room_id, pkt.object_id, true);
    }
}

pub(crate) fn handle_request_ex_dismiss_mpcc_room(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if let Some(room_id) = cc_room_of(world, player).filter(|&id| {
        world
            .matching_rooms
            .get(id)
            .is_some_and(|r| r.is_leader(player))
    }) {
        cc_room_disband(world, room_id);
    }
}

pub(crate) fn handle_request_ex_withdraw_mpcc_room(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if let Some(room_id) = cc_room_of(world, player) {
        cc_room_remove_member(world, room_id, player, false);
    }
}

pub(crate) fn handle_request_ex_mpcc_partymaster_list(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(room_id) = cc_room_of(world, player) else {
        return;
    };
    // Distinct party-leader names of the room's members (Java collects into a
    // Set; insertion order here, which the client treats as unordered).
    let mut names: Vec<String> = Vec::new();
    for oid in world
        .matching_rooms
        .get(room_id)
        .map(|r| r.all_members())
        .unwrap_or_default()
    {
        let Some(leader) = party_id_of(world, oid)
            .and_then(|pid| world.parties.get(&pid))
            .map(|p| p.leader())
        else {
            continue;
        };
        let name = player_name_or_empty(world, leader);
        if !names.contains(&name) {
            names.push(name);
        }
    }
    send_to_client(
        world,
        client_id,
        server_packets::ex_mpcc_partymaster_list(&names),
    );
}

// ---------------------------------------------------------------------------
// Raid looting rights (`Attackable._firstCommandChannelAttacked` +
// `Player.isInLooterParty` + the drop-ownership half of
// `ItemData.createItem("loot")`)
// ---------------------------------------------------------------------------

use super::helpers::send_sm_to_player as send_sm;
use crate::model::components::RaidLootRights;

/// `Attackable.reduceCurrentHp`'s loot-privilege block: the first command
/// channel of `RaidLootRightsCCSize`+ members to strike a raid boss (never a
/// minion) owns its drops; every later hit from the same channel refreshes
/// the claim. Java polls a 10 s timer to expire it — the port expires lazily
/// via [`loot_rights_cc`].
pub(crate) fn on_raid_attacked_loot_rights(world: &mut World, npc_oid: i32, attacker_oid: i32) {
    // Boss only (`!isMinion()`), and only a real raid.
    if world
        .objects
        .has_component::<crate::game_loop::minions::MinionOf>(&npc_oid)
        || !super::raid_curse::gives_raid_curse(world, npc_oid)
    {
        return;
    }
    // The acting player: a servitor/pet hit counts for its owner.
    let player = if world.objects.has_component::<Player>(&attacker_oid) {
        attacker_oid
    } else if let Some(s) = world
        .objects
        .get_component::<crate::model::components::ServitorOf>(&attacker_oid)
    {
        s.owner_object_id
    } else {
        return;
    };
    let Some(cc_id) = party_id_of(world, player).and_then(|pid| cc_id_of_party(world, pid)) else {
        return;
    };
    if (cc_members(world, cc_id).len() as i32) < world.cfg.character.raid_loot_rights_cc_size {
        return;
    }
    let now = world.tick;
    match loot_rights_cc(world, npc_oid) {
        // Same channel: refresh the claim.
        Some(holder) if holder == cc_id => {
            if let Some(r) = world.objects.get_component_mut::<RaidLootRights>(&npc_oid) {
                r.last_attack_tick = now;
            }
        }
        // Another channel still holds an unexpired claim: nothing.
        Some(_) => {}
        // Free (or expired): claim + announce. Java's announcement is a
        // `CreatureSay(null, PARTYROOM_ALL, "", ...)` — object id 0, empty
        // name (retail SM 1869/1870 are unused there).
        None => {
            world.objects.add_components(
                &npc_oid,
                RaidLootRights {
                    cc_id,
                    last_attack_tick: now,
                },
            );
            broadcast_to_cc(
                world,
                cc_id,
                &server_packets::creature_say(
                    0,
                    crate::enums::ChatType::PartyroomAll,
                    "",
                    "You have looting rights!",
                    None,
                ),
            );
        }
    }
}

/// The command channel currently holding looting rights on this NPC, if the
/// claim hasn't expired (`RaidLootRightsInterval` since the last hit).
pub(crate) fn loot_rights_cc(world: &World, npc_oid: i32) -> Option<u32> {
    let r = world.objects.get_component::<RaidLootRights>(&npc_oid)?;
    let interval_ticks = world.cfg.character.raid_loot_rights_interval * 10;
    (world.tick.saturating_sub(r.last_attack_tick) <= interval_ticks
        && world.command_channels.contains_key(&r.cc_id))
    .then_some(r.cc_id)
}

/// `Player.isInLooterParty(ownerId)` called on the picker: true when the
/// picker's command channel (or, outside one, their party) contains the drop
/// owner.
pub(crate) fn is_in_looter_party(world: &World, picker: i32, owner: i32) -> bool {
    let Some(picker_party) = party_id_of(world, picker) else {
        return false;
    };
    if let Some(cc_id) = cc_id_of_party(world, picker_party) {
        return cc_members(world, cc_id).contains(&owner);
    }
    world
        .parties
        .get(&picker_party)
        .is_some_and(|p| p.contains(owner))
}

fn get_party_info(world: &World, party_id: u32) -> (String, i32, i32) {
    world
        .parties
        .get(&party_id)
        .map(|p| {
            (
                player_name_or_empty(world, p.leader()),
                p.leader(),
                p.members.len() as i32,
            )
        })
        .unwrap_or_default()
}
