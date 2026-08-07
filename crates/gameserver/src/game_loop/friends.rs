//! Port of the friend system: `clientpackets/friend/*` (invite, answer,
//! delete, list, message) + the enter/leave-world notifications. Block list,
//! memos and `RequestExFriendListExtended` are out of scope
//! (PLAN_G10_SOCIAL.md §4).

use crate::character::FriendInfo;
use crate::model::Player;
use crate::model::components::{Friends, PendingRequest, RequestKind};
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, FriendEntry, SmParam, friend_status_mode, sm_ids};
use crate::world::World;

use super::helpers::{client_for_player, send_sm_to_player as send_sm};
use super::party::{
    REQUEST_TIMEOUT_TICKS, clear_linked_request, find_player_by_name, install_request,
};

fn send_to_player(world: &World, object_id: i32, packet: Vec<u8>) {
    crate::game_loop::helpers::send_to_player(world, object_id, packet);
}

fn is_online(world: &World, object_id: i32) -> bool {
    client_for_player(world, object_id).is_some()
}

/// A friend snapshot → the packet entry (live online flag; live level/class
/// when the friend is in the world).
fn entry_of(world: &World, info: &FriendInfo) -> FriendEntry {
    let online = is_online(world, info.char_id);
    let (level, class_id) = world
        .objects
        .get_component::<Player>(&info.char_id)
        .map(|p| (p.level, p.class_id))
        .unwrap_or((info.level, info.class_id));
    FriendEntry {
        char_id: info.char_id,
        name: info.name.clone(),
        level,
        class_id,
        online,
    }
}

/// The live `FriendInfo` snapshot of an in-world player.
fn info_of(world: &World, object_id: i32) -> Option<FriendInfo> {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| FriendInfo {
            char_id: object_id,
            name: p.name.clone(),
            level: p.level,
            class_id: p.class_id,
        })
}

/// The `L2FriendList` packet for a player's current snapshot.
pub(crate) fn l2_friend_list_packet(world: &World, friends: &Friends) -> Vec<u8> {
    let entries: Vec<FriendEntry> = friends.0.iter().map(|f| entry_of(world, f)).collect();
    server_packets::l2_friend_list(&entries)
}

// ---------------------------------------------------------------------------
// Enter / leave world notifications
// ---------------------------------------------------------------------------

/// EnterWorld's friend block, run *after* the player spawned: SM 503 "your
/// friend just logged in" + `FriendStatus(ONLINE)` to each online friend.
pub(crate) fn on_enter_world(world: &World, object_id: i32) {
    let Some(friends) = world.objects.get_component::<Friends>(&object_id) else {
        return;
    };
    let name = world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let sm = server_packets::system_message_with(
        sm_ids::YOUR_FRIEND_S1_JUST_LOGGED_IN,
        &[SmParam::Text(name.clone())],
    );
    let status = server_packets::friend_status(friend_status_mode::ONLINE, &name, 0);
    for f in &friends.0 {
        if is_online(world, f.char_id) {
            send_to_player(world, f.char_id, sm.clone());
            send_to_player(world, f.char_id, status.clone());
        }
    }
}

/// `Player.deleteMe` → `notifyFriends(MODE_OFFLINE)`. Runs before despawn
/// (needs the leaver's components).
pub(crate) fn on_leave_world(world: &World, object_id: i32) {
    let Some(friends) = world.objects.get_component::<Friends>(&object_id) else {
        return;
    };
    let name = world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let status = server_packets::friend_status(friend_status_mode::OFFLINE, &name, object_id);
    for f in &friends.0 {
        if f.char_id != object_id && is_online(world, f.char_id) {
            send_to_player(world, f.char_id, status.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// Invite / answer
// ---------------------------------------------------------------------------

pub(crate) fn handle_request_friend_invite(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(name) = cp::read_name(body) else {
        return;
    };

    let Some((_, friend)) = find_player_by_name(world, &name) else {
        send_sm(world, player, sm_ids::FRIEND_INVITE_TARGET_NOT_FOUND, &[]);
        return;
    };
    if friend == player {
        send_sm(
            world,
            player,
            sm_ids::YOU_CANNOT_ADD_YOURSELF_TO_YOUR_OWN_FRIEND_LIST,
            &[],
        );
        return;
    }
    // Java `RequestFriendInvite` checks **both** lists, in this order and
    // *before* the already-a-friend test, with deliberately different answers:
    // being blocked *by* the target is a literal line that does not name them,
    // while having blocked the target names them.
    if super::block_list::is_blocked(world, friend, player) {
        super::admin::send_message(world, client_id, "You are in target's block list.");
        return;
    }
    if super::block_list::is_blocked(world, player, friend) {
        send_sm(
            world,
            player,
            sm_ids::YOU_HAVE_BLOCKED_C1,
            &[SmParam::Text(name.clone())],
        );
        return;
    }
    if world
        .objects
        .get_component::<Friends>(&player)
        .is_some_and(|fl| fl.0.iter().any(|f| f.char_id == friend))
    {
        send_sm(
            world,
            player,
            sm_ids::THIS_PLAYER_IS_ALREADY_REGISTERED_ON_YOUR_FRIENDS_LIST,
            &[],
        );
        return;
    }
    if world.objects.has_component::<PendingRequest>(&friend)
        || world.objects.has_component::<PendingRequest>(&player)
    {
        send_sm(
            world,
            player,
            sm_ids::C1_IS_ON_ANOTHER_TASK_PLEASE_TRY_AGAIN_LATER,
            &[SmParam::Text(name.clone())],
        );
        return;
    }

    install_request(
        world,
        player,
        friend,
        RequestKind::FriendInvite,
        REQUEST_TIMEOUT_TICKS,
    );
    let requestor_name = world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    send_to_player(
        world,
        friend,
        server_packets::friend_add_request(&requestor_name),
    );
    send_sm(
        world,
        player,
        sm_ids::YOU_VE_REQUESTED_C1_TO_BE_ON_YOUR_FRIENDS_LIST,
        &[SmParam::Text(name)],
    );
}

pub(crate) fn handle_request_answer_friend_invite(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let response = cp::read_friend_answer(body).unwrap_or(0);

    let Some(req) = world
        .objects
        .get_component::<PendingRequest>(&player)
        .copied()
    else {
        return;
    };
    let (RequestKind::FriendInvite, true) = (req.kind, req.answerer) else {
        return;
    };
    clear_linked_request(world, player);
    let requestor = req.other;

    if !is_online(world, requestor) {
        return;
    }
    let already = world
        .objects
        .get_component::<Friends>(&player)
        .is_some_and(|fl| fl.0.iter().any(|f| f.char_id == requestor))
        || world
            .objects
            .get_component::<Friends>(&requestor)
            .is_some_and(|fl| fl.0.iter().any(|f| f.char_id == player));
    if already {
        let name = world
            .objects
            .get_component::<Player>(&player)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        send_sm(
            world,
            requestor,
            sm_ids::C1_IS_ALREADY_ON_YOUR_FRIEND_LIST,
            &[SmParam::Text(name)],
        );
        return;
    }

    if response != 1 {
        send_sm(
            world,
            requestor,
            sm_ids::YOU_HAVE_FAILED_TO_ADD_A_FRIEND,
            &[],
        );
        return;
    }

    let (Some(player_info), Some(requestor_info)) =
        (info_of(world, player), info_of(world, requestor))
    else {
        return;
    };
    let _ = world.db.send(crate::db::DbCommand::InsertFriendPair {
        a: requestor,
        b: player,
    });
    if let Some(fl) = world.objects.get_component_mut::<Friends>(&requestor) {
        fl.0.push(player_info.clone());
    }
    if let Some(fl) = world.objects.get_component_mut::<Friends>(&player) {
        fl.0.push(requestor_info.clone());
    }

    send_sm(world, requestor, sm_ids::FRIEND_ADDED_SUCCESSFULLY, &[]);
    send_sm(
        world,
        requestor,
        sm_ids::S1_HAS_BEEN_ADDED_TO_YOUR_FRIENDS_LIST,
        &[SmParam::Text(player_info.name.clone())],
    );
    send_sm(
        world,
        player,
        sm_ids::S1_HAS_BEEN_ADDED_TO_YOUR_FRIENDS_LIST_2,
        &[SmParam::Text(requestor_info.name.clone())],
    );
    // Both sides' client lists learn the new (online) friend.
    send_to_player(
        world,
        player,
        server_packets::friend_add_request_result(1, &entry_of(world, &requestor_info)),
    );
    send_to_player(
        world,
        requestor,
        server_packets::friend_add_request_result(1, &entry_of(world, &player_info)),
    );
}

// ---------------------------------------------------------------------------
// Delete / list / message
// ---------------------------------------------------------------------------

pub(crate) fn handle_request_friend_del(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(name) = cp::read_name(body) else {
        return;
    };

    // Java resolves the name through `CharInfoTable` (works offline); our
    // loaded snapshot carries every friend's name — not on the list and
    // unknown names answer the same SM 171.
    let friend = world
        .objects
        .get_component::<Friends>(&player)
        .and_then(|fl| {
            fl.0.iter()
                .find(|f| f.name.eq_ignore_ascii_case(&name))
                .cloned()
        });
    let Some(friend) = friend else {
        send_sm(
            world,
            player,
            sm_ids::C1_IS_NOT_ON_YOUR_FRIEND_LIST,
            &[SmParam::Text(name)],
        );
        return;
    };

    let _ = world.db.send(crate::db::DbCommand::DeleteFriendPair {
        a: player,
        b: friend.char_id,
    });
    if let Some(fl) = world.objects.get_component_mut::<Friends>(&player) {
        fl.0.retain(|f| f.char_id != friend.char_id);
    }
    send_sm(
        world,
        player,
        sm_ids::S1_HAS_BEEN_REMOVED_FROM_YOUR_FRIENDS_LIST_2,
        &[SmParam::Text(friend.name.clone())],
    );
    send_to_player(
        world,
        player,
        server_packets::friend_remove(&friend.name, 1),
    );

    // The (online) ex-friend's side updates too.
    if is_online(world, friend.char_id) {
        let player_name = world
            .objects
            .get_component::<Player>(&player)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        if let Some(fl) = world.objects.get_component_mut::<Friends>(&friend.char_id) {
            fl.0.retain(|f| f.char_id != player);
        }
        send_to_player(
            world,
            friend.char_id,
            server_packets::friend_remove(&player_name, 1),
        );
    }
}

pub(crate) fn handle_request_friend_list(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(friends) = world.objects.get_component::<Friends>(&player).cloned() else {
        return;
    };
    send_sm(world, player, sm_ids::FRIENDS_LIST_HEADER, &[]);
    for f in &friends.0 {
        let id = if is_online(world, f.char_id) {
            sm_ids::S1_CURRENTLY_ONLINE
        } else {
            sm_ids::S1_CURRENTLY_OFFLINE
        };
        send_sm(world, player, id, &[SmParam::Text(f.name.clone())]);
    }
    send_sm(world, player, sm_ids::FRIENDS_LIST_FOOTER, &[]);
}

pub(crate) fn handle_request_send_friend_msg(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(pkt) = cp::RequestSendFriendMsg::read(body) else {
        return;
    };
    if pkt.message.is_empty() || pkt.message.chars().count() > 300 {
        return;
    }
    // The receiver must be online and have the *sender* on their list.
    let receiver = find_player_by_name(world, &pkt.receiver)
        .map(|(_, oid)| oid)
        .filter(|&oid| {
            world
                .objects
                .get_component::<Friends>(&oid)
                .is_some_and(|fl| fl.0.iter().any(|f| f.char_id == player))
        });
    let Some(receiver) = receiver else {
        send_sm(world, player, sm_ids::THAT_PLAYER_IS_NOT_ONLINE, &[]);
        return;
    };
    let sender_name = world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    send_to_player(
        world,
        receiver,
        server_packets::l2_friend_say(&sender_name, &pkt.receiver, &pkt.message),
    );
}
