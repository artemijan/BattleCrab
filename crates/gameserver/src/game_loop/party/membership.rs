//! Member management: add/remove, withdraw, oust, leader change, disband
//! and the leave-world hook.

use super::POSITION_BROADCAST_TICKS;
use super::broadcast_to_party;
use super::clear_linked_request;
use super::invite::drop_party_if_unborn;
use super::loot::finish_loot_change;
use super::loot::finish_loot_change_inline;
use super::member_view;
use crate::game_loop::character::player_info::broadcast_user_info;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::send_sm_to_player;
use crate::game_loop::helpers::send_to_player;
use crate::model::components::PartyRef;
use crate::model::components::RequestKind;
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::network::server_packets::PartyMemberView;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::scheduler::ScheduledTask;
use crate::world::World;
/// `Party.addPartyMember` (pets/CC/duel/tactical-sign hooks dropped).
pub(crate) fn add_party_member(world: &mut World, party_id: u32, new_member: i32) {
    let Some(party) = world.parties.get(&party_id) else {
        return;
    };
    if party.contains(new_member) {
        return;
    }
    // A pending loot-rule vote dies on member change.
    if world.parties[&party_id].loot_change.is_some() {
        finish_loot_change(world, party_id, false);
    }

    let started_broadcast = {
        let party = world.parties.get_mut(&party_id).expect("checked");
        let first_add = party.members.len() == 1;
        party.members.push(new_member);
        first_add
    };
    world
        .objects
        .add_components(&new_member, PartyRef(party_id));

    let party = &world.parties[&party_id];
    let (leader, rule, members) = (party.leader(), party.distribution, party.members.clone());
    let leader_name = player_name_or_empty(world, leader);
    let new_name = player_name_or_empty(world, new_member);

    // New member: the full window (everyone but themselves) + "you joined".
    let others: Vec<PartyMemberView> = members
        .iter()
        .filter(|&&m| m != new_member)
        .filter_map(|&m| member_view(world, m))
        .collect();
    send_to_player(
        world,
        new_member,
        server_packets::party_small_window_all(leader, rule.id(), &others),
    );
    send_sm_to_player(
        world,
        new_member,
        sm_ids::YOU_HAVE_JOINED_S1_S_PARTY,
        &[SmParam::Text(leader_name)],
    );

    // Everyone (new member included, per Java's broadcast after add):
    // "C1 has joined the party"; existing members also get the Add window
    // entry and the new member's HP bar.
    let joined_sm = server_packets::system_message_with(
        sm_ids::C1_HAS_JOINED_THE_PARTY,
        &[SmParam::Text(new_name)],
    );
    broadcast_to_party(world, party_id, &joined_sm, None);
    if let Some(view) = member_view(world, new_member) {
        let add = server_packets::party_small_window_add(leader, rule.id(), &view);
        let su = server_packets::status_update(
            new_member,
            &[
                (server_packets::status_update_type::MAX_HP, view.max_hp),
                (server_packets::status_update_type::CUR_HP, view.hp),
            ],
        );
        for &m in &members {
            if m != new_member {
                send_to_player(world, m, add.clone());
            }
            send_to_player(world, m, su.clone());
        }
    }
    // `member.broadcastUserInfo()` for everyone (relation bits refresh).
    for &m in &members {
        broadcast_user_info(world, m);
    }

    // The party's tactical signs are pushed to the arriving member's client
    // (Java `addPartyMember`'s `applyTacticalSigns(player, false)` tail) —
    // the markers are party state, so a latecomer sees the ones already set.
    super::apply_tactical_signs(world, party_id, new_member, false);

    // A member joining a channelled party gets the CC window opened (Java
    // `addPartyMember`'s `ExOpenMPCC` tail).
    super::command_channel::on_party_member_added(world, party_id, new_member);

    // The 12 s position broadcast starts with the first real member add
    // (Java: `_positionBroadcastTask == null`), initial delay = period / 2.
    if started_broadcast {
        let seq = world.parties[&party_id].seq;
        world.scheduler.schedule(
            world.tick + POSITION_BROADCAST_TICKS / 2,
            ScheduledTask::PartyPositionBroadcast { party_id, seq },
        );
    }
}

// ---------------------------------------------------------------------------
// Leave / oust / disband / leader change
// ---------------------------------------------------------------------------

/// `PartyMessageType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LeaveType {
    Left,
    Expelled,
    Disconnected,
    None,
}

pub(crate) fn handle_request_withdrawal_party(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if let Some(PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&player).copied() {
        remove_party_member(world, party_id, player, LeaveType::Left);
    }
    // Java `RequestWithDrawalParty` also drops the player from their matching
    // room (G30).
    super::rooms::on_party_withdraw(world, player);
}

pub(crate) fn handle_request_oust_party_member(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(name) = cp::social::read_name(body) else {
        return;
    };
    let Some(PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&player).copied() else {
        return;
    };
    if !world
        .parties
        .get(&party_id)
        .is_some_and(|p| p.is_leader(player))
    {
        return;
    }
    let victim = world.parties[&party_id]
        .members
        .iter()
        .copied()
        .find(|&m| player_name_or_empty(world, m).eq_ignore_ascii_case(&name));
    if let Some(victim) = victim {
        remove_party_member(world, party_id, victim, LeaveType::Expelled);
    }
}

pub(crate) fn handle_request_change_party_leader(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(name) = cp::social::read_name(body) else {
        return;
    };
    let Some(PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&player).copied() else {
        return;
    };
    if !world
        .parties
        .get(&party_id)
        .is_some_and(|p| p.is_leader(player))
    {
        return;
    }
    let target = world.parties[&party_id]
        .members
        .iter()
        .copied()
        .find(|&m| player_name_or_empty(world, m).eq_ignore_ascii_case(&name));
    let Some(target) = target else {
        // Java answers this to the *named* player object being absent from
        // the member list.
        send_sm_to_player(
            world,
            player,
            sm_ids::YOU_MAY_ONLY_TRANSFER_PARTY_LEADERSHIP,
            &[],
        );
        return;
    };
    if target == player {
        send_sm_to_player(
            world,
            player,
            sm_ids::SLOW_DOWN_YOU_ARE_ALREADY_THE_PARTY_LEADER,
            &[],
        );
        return;
    }
    {
        let party = world.parties.get_mut(&party_id).expect("checked");
        let idx = party
            .members
            .iter()
            .position(|&m| m == target)
            .expect("checked");
        party.members.swap(0, idx);
    }
    announce_new_leader(world, party_id);
    // CC authority follows the leading party's leadership (Java
    // `Party.setLeader`'s CC tail, SM 1589).
    super::command_channel::on_party_leader_changed(world, party_id, player, target);
}

/// SM 1384 + `broadcastToPartyMembersNewLeader` (window rebuild for all).
pub(super) fn announce_new_leader(world: &mut World, party_id: u32) {
    let leader_name = player_name_or_empty(world, world.parties[&party_id].leader());
    let sm = server_packets::system_message_with(
        sm_ids::C1_HAS_BECOME_THE_PARTY_LEADER,
        &[SmParam::Text(leader_name)],
    );
    broadcast_to_party(world, party_id, &sm, None);

    let party = &world.parties[&party_id];
    let (leader, rule, members) = (party.leader(), party.distribution, party.members.clone());
    for &m in &members {
        send_to_player(world, m, server_packets::party_small_window_delete_all());
        let others: Vec<PartyMemberView> = members
            .iter()
            .filter(|&&o| o != m)
            .filter_map(|&o| member_view(world, o))
            .collect();
        send_to_player(
            world,
            m,
            server_packets::party_small_window_all(leader, rule.id(), &others),
        );
        broadcast_user_info(world, m);
    }
}

/// `Party.removePartyMember(player, type)`.
pub(crate) fn remove_party_member(
    world: &mut World,
    party_id: u32,
    leaver: i32,
    leave_type: LeaveType,
) {
    let Some(party) = world.parties.get(&party_id) else {
        return;
    };
    if !party.contains(leaver) {
        return;
    }
    let was_leader = party.is_leader(leaver);
    let two_left = party.members.len() == 2;
    let leader_quit = was_leader
        && !world.cfg.character.alt_leave_party_leader
        && leave_type != LeaveType::Disconnected
        && leave_type != LeaveType::None;
    if two_left || leader_quit {
        disband_party(world, party_id);
        return;
    }

    {
        let party = world.parties.get_mut(&party_id).expect("checked");
        party.members.retain(|&m| m != leaver);
        if party.loot_change.is_some() {
            // Member change voids the vote (answers count against a stale
            // member set otherwise).
            finish_loot_change_inline(party);
        }
    }
    world.objects.remove_component::<PartyRef>(&leaver);

    let leaver_name = player_name_or_empty(world, leaver);
    match leave_type {
        LeaveType::Expelled => {
            send_sm_to_player(
                world,
                leaver,
                sm_ids::YOU_HAVE_BEEN_EXPELLED_FROM_THE_PARTY,
                &[],
            );
            let sm = server_packets::system_message_with(
                sm_ids::C1_WAS_EXPELLED_FROM_THE_PARTY,
                &[SmParam::Text(leaver_name.clone())],
            );
            broadcast_to_party(world, party_id, &sm, None);
        }
        LeaveType::Left | LeaveType::Disconnected => {
            send_sm_to_player(
                world,
                leaver,
                sm_ids::YOU_HAVE_WITHDRAWN_FROM_THE_PARTY,
                &[],
            );
            let sm = server_packets::system_message_with(
                sm_ids::C1_HAS_LEFT_THE_PARTY,
                &[SmParam::Text(leaver_name.clone())],
            );
            broadcast_to_party(world, party_id, &sm, None);
        }
        LeaveType::None => {}
    }

    send_to_player(
        world,
        leaver,
        server_packets::party_small_window_delete_all(),
    );
    let delete = server_packets::party_small_window_delete(leaver, &leaver_name);
    broadcast_to_party(world, party_id, &delete, None);
    broadcast_user_info(world, leaver);

    // The leaver's CC window closes (Java `removePartyMember`'s `ExCloseMPCC`
    // tail). Java also leaves `CommandChannel._commandLeader` stale when the
    // CC leader disconnects but their party survives — kept.
    super::command_channel::on_party_member_removed(world, party_id, leaver);

    // `applyTacticalSigns(player, true)` — the leaver's client drops the
    // markers; the party keeps the signs themselves for whoever stays.
    super::apply_tactical_signs(world, party_id, leaver, true);

    if was_leader {
        announce_new_leader(world, party_id);
    }
}

/// `Party.disbandParty` — SM 203 to everyone, all windows cleared, party
/// gone. (Java re-enters `removePartyMember` per member with `_disbanding`
/// set; the observable packets are the dissolve SM + each member's
/// `PartySmallWindowDeleteAll`, which this sends directly.)
pub(crate) fn disband_party(world: &mut World, party_id: u32) {
    // A collapsing party takes its command channel down with it when it is
    // the CC leader's party, or just detaches otherwise (Java
    // `removePartyMember`'s size==1 branch).
    super::command_channel::on_party_dissolving(world, party_id);
    let Some(party) = world.parties.get_mut(&party_id) else {
        return;
    };
    party.seq = party.seq.wrapping_add(1); // kill outstanding tasks
    let members = party.members.clone();
    let sm = server_packets::system_message_with(sm_ids::THE_PARTY_HAS_DISPERSED, &[]);
    for &m in &members {
        // Java reaches this through `removePartyMember` per member, so every
        // one of them gets the sign wipe; done here for the same reason the
        // dissolve SM is.
        super::apply_tactical_signs(world, party_id, m, true);
        world.objects.remove_component::<PartyRef>(&m);
        send_to_player(world, m, sm.clone());
        send_to_player(world, m, server_packets::party_small_window_delete_all());
    }
    world.parties.remove(&party_id);
    for &m in &members {
        broadcast_user_info(world, m);
    }
}

/// Player left the world (logout/restart/disconnect): party + request
/// cleanup. Called by `store_and_remove_player`.
pub(crate) fn on_player_leave_world(world: &mut World, object_id: i32) {
    if let Some(req) = clear_linked_request(world, object_id) {
        // A vanished inviter's embryo party dies with the invite.
        if let RequestKind::PartyInvite { party_id } = req.kind {
            drop_party_if_unborn(world, party_id);
        }
    }
    if let Some(PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&object_id).copied() {
        remove_party_member(world, party_id, object_id, LeaveType::Disconnected);
    }
}
