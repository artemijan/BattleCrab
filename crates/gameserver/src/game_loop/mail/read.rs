//! Reading and deleting mail: the received/sent post windows and the
//! delete pair.

// ---------------------------------------------------------------------------
// ex 0x66 RequestReceivedPost / ex 0x6B RequestSentPost — open one message
// ---------------------------------------------------------------------------

use super::attachment_views;
use super::char_name_by_id;
use super::delete_message;
use super::persist_flags;
use super::refuse_attachments_outside_peace_zone;
use super::send_unread_count;
use crate::game_loop::helpers::send_to_player;
use crate::network::server_packets;
use crate::world::World;
pub(crate) fn handle_received_post(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if !world.cfg.general.allow_mail {
        return;
    }
    let Some(message_id) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };
    let Some(m) = world.mail.get(message_id) else {
        return;
    };
    if m.receiver_id != player {
        crate::game_loop::moderation::punishment::illegal_action(
            world,
            player,
            &format!("Player {player} tried to receive not own post!"),
        );
        return;
    }
    if m.deleted_by_receiver {
        return;
    }
    if m.has_attachments && refuse_attachments_outside_peace_zone(world, player) {
        return;
    }

    let sender_name = if m.mail_type.is_system() {
        "System".to_string()
    } else {
        char_name_by_id(world, m.sender_id)
    };
    let (subject, content, locked, req_adena, has_attachments, returned) = (
        m.subject.clone(),
        m.content.clone(),
        m.is_locked(),
        m.req_adena,
        m.has_attachments,
        m.returned,
    );
    let items = attachment_views(world, message_id);
    let pkt = server_packets::ex_reply_received_post(
        message_id,
        locked,
        &sender_name,
        &subject,
        &content,
        &items,
        req_adena,
        has_attachments,
        returned,
    );
    send_to_player(world, player, pkt);
    send_to_player(
        world,
        player,
        server_packets::ex_change_post_state(true, &[message_id], crate::model::mail::STATE_READ),
    );

    // Java `markAsRead()` — only writes when it actually changes.
    if world.mail.get(message_id).is_some_and(|m| m.unread) {
        if let Some(m) = world.mail.get_mut(message_id) {
            m.unread = false;
        }
        persist_flags(world, message_id);
        send_unread_count(world, player);
    }
}

pub(crate) fn handle_sent_post(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if !world.cfg.general.allow_mail {
        return;
    }
    let Some(message_id) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };
    let Some(m) = world.mail.get(message_id) else {
        return;
    };
    if m.sender_id != player {
        crate::game_loop::moderation::punishment::illegal_action(
            world,
            player,
            &format!("Player {player} tried to read not own post!"),
        );
        return;
    }
    if m.deleted_by_sender {
        return;
    }
    if m.has_attachments && refuse_attachments_outside_peace_zone(world, player) {
        return;
    }
    let receiver_name = char_name_by_id(world, m.receiver_id);
    let (subject, content, locked, req_adena, has_attachments, returned) = (
        m.subject.clone(),
        m.content.clone(),
        m.is_locked(),
        m.req_adena,
        m.has_attachments,
        m.returned,
    );
    let items = attachment_views(world, message_id);
    let pkt = server_packets::ex_reply_sent_post(
        message_id,
        locked,
        &receiver_name,
        &subject,
        &content,
        &items,
        req_adena,
        has_attachments,
        returned,
    );
    // Java sends no `markAsRead` and no state change for the outbox.
    send_to_player(world, player, pkt);
}

// ---------------------------------------------------------------------------
// ex 0x65 / ex 0x6A — delete from one side's list
// ---------------------------------------------------------------------------

pub(crate) fn handle_delete_received_post(world: &mut World, client_id: u32, body: &[u8]) {
    handle_delete_post(world, client_id, body, true);
}

pub(crate) fn handle_delete_sent_post(world: &mut World, client_id: u32, body: &[u8]) {
    handle_delete_post(world, client_id, body, false);
}

fn handle_delete_post(world: &mut World, client_id: u32, body: &[u8], received: bool) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if !world.cfg.general.allow_mail {
        return;
    }
    let Some(pkt) = crate::network::client_packets::social::DeletePostList::read(body) else {
        return;
    };
    if refuse_attachments_outside_peace_zone(world, player) {
        return;
    }

    // Java aborts the *whole* batch on the first message that still has
    // attachments or is already deleted — only a missing id is skipped.
    let mut deleted = Vec::new();
    for id in &pkt.message_ids {
        let Some(m) = world.mail.get(*id) else {
            continue;
        };
        let owner = if received { m.receiver_id } else { m.sender_id };
        if owner != player {
            // Java: "... tried to delete not own post!" — a punish, not a
            // silent refusal.
            crate::game_loop::moderation::punishment::illegal_action(
                world,
                player,
                &format!("Player {player} tried to delete not own post!"),
            );
            return;
        }
        let already = if received {
            m.deleted_by_receiver
        } else {
            m.deleted_by_sender
        };
        if m.has_attachments || already {
            return;
        }
        deleted.push(*id);
    }

    for id in &deleted {
        let drop_row = {
            let Some(m) = world.mail.get_mut(*id) else {
                continue;
            };
            if received {
                m.deleted_by_receiver = true;
            } else {
                m.deleted_by_sender = true;
            }
            m.deleted_by_receiver && m.deleted_by_sender
        };
        // Java's `setDeletedBy*` drops the row once both sides are done with it.
        if drop_row {
            delete_message(world, *id);
        } else {
            persist_flags(world, *id);
        }
    }

    send_to_player(
        world,
        player,
        server_packets::ex_change_post_state(received, &deleted, crate::model::mail::STATE_DELETED),
    );
    if received {
        send_unread_count(world, player);
    }
}
