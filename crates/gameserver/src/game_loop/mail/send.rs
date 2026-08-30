//! The send flow: `RequestSendPost` with its guard chain, fee, and the
//! attachment move into the message container.

// ---------------------------------------------------------------------------
// ex 0x63 RequestSendPost — compose and send
// ---------------------------------------------------------------------------

use super::char_id_by_name;
use super::in_peace_zone;
use super::persist_attachments;
use super::persist_message;
use super::schedule_expiry;
use super::send_unread_count;
/// Java's field caps (`RequestSendPost`).
use crate::data::item_data::ADENA_ID;
use crate::game_loop::helpers::send_inventory_item_list;
use crate::game_loop::helpers::send_sm_to_player as send_sm;
use crate::game_loop::helpers::send_to_player;
use crate::model::Player;
use crate::model::components::Trade;
use crate::model::inventory::Inventory;
use crate::model::mail::Message;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::world::World;
const MAX_RECEIVER_LENGTH: usize = 16;
const MAX_SUBJECT_LENGTH: usize = 128;
const MAX_TEXT_LENGTH: usize = 512;

pub(crate) fn handle_send_post(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if !world.cfg.general.allow_mail {
        return;
    }
    let Some(mut pkt) = crate::network::client_packets::RequestSendPost::read(body) else {
        return;
    };

    // Java coerces rather than rejecting when attachments are switched off:
    // the message still goes, minus the items and the payment request.
    if !world.cfg.general.allow_attachments {
        pkt.items.clear();
        pkt.is_cod = false;
        pkt.req_adena = 0;
    }

    // --- guard chain, in Java's order ------------------------------------
    if !pkt.items.is_empty() && !in_peace_zone(world, player) {
        send_sm(
            world,
            player,
            sm_ids::YOU_CANNOT_FORWARD_IN_A_NON_PEACE_ZONE_LOCATION,
            &[],
        );
        return;
    }
    if world.objects.has_component::<Trade>(&player) {
        send_sm(
            world,
            player,
            sm_ids::YOU_CANNOT_FORWARD_DURING_AN_EXCHANGE,
            &[],
        );
        return;
    }
    if world
        .objects
        .get_component::<Player>(&player)
        .is_some_and(|p| p.store_type != 0)
    {
        send_sm(
            world,
            player,
            sm_ids::YOU_CANNOT_FORWARD_BECAUSE_THE_PRIVATE_STORE_OR_WORKSHOP_IS_IN_PROGRESS,
            &[],
        );
        return;
    }
    if pkt.receiver.chars().count() > MAX_RECEIVER_LENGTH {
        send_sm(
            world,
            player,
            sm_ids::THE_ALLOWED_LENGTH_FOR_RECIPIENT_EXCEEDED,
            &[],
        );
        return;
    }
    // Java uses the title message for an over-long body too ("not found
    // message for this").
    if pkt.subject.chars().count() > MAX_SUBJECT_LENGTH
        || pkt.text.chars().count() > MAX_TEXT_LENGTH
    {
        send_sm(
            world,
            player,
            sm_ids::THE_ALLOWED_LENGTH_FOR_A_TITLE_EXCEEDED,
            &[],
        );
        return;
    }
    if pkt.items.len() > crate::network::client_packets::RequestSendPost::MAX_ATTACHMENTS {
        send_sm(
            world,
            player,
            sm_ids::ITEM_SELECTION_IS_POSSIBLE_UP_TO_8,
            &[],
        );
        return;
    }
    if pkt.req_adena < 0 {
        return;
    }
    if pkt.is_cod {
        if pkt.req_adena == 0 {
            send_sm(
                world,
                player,
                sm_ids::WHEN_NOT_ENTERING_THE_AMOUNT_FOR_THE_PAYMENT_REQUEST_YOU_CANNOT_SEND_ANY_MAIL,
                &[],
            );
            return;
        }
        if pkt.items.is_empty() {
            send_sm(
                world,
                player,
                sm_ids::IT_S_A_PAYMENT_REQUEST_TRANSACTION_PLEASE_ATTACH_THE_ITEM,
                &[],
            );
            return;
        }
    }

    let Some(receiver_id) = char_id_by_name(world, &pkt.receiver) else {
        send_sm(
            world,
            player,
            sm_ids::WHEN_THE_RECIPIENT_DOESN_T_EXIST_SENDING_MAIL_IS_NOT_POSSIBLE,
            &[],
        );
        return;
    };
    if receiver_id == player {
        send_sm(
            world,
            player,
            sm_ids::YOU_CANNOT_SEND_A_MAIL_TO_YOURSELF,
            &[],
        );
        return;
    }
    // A non-GM may not mail the GM staff.
    let sender_is_gm = world
        .objects
        .get_component::<Player>(&player)
        .is_some_and(|p| p.is_gm(&world.data));
    let receiver_is_gm = world
        .objects
        .get_component::<Player>(&receiver_id)
        .is_some_and(|p| p.is_gm(&world.data));
    if receiver_is_gm && !sender_is_gm {
        send_sm(
            world,
            player,
            sm_ids::YOU_CANNOT_SEND_MAIL_TO_THE_GM_STAFF,
            &[SmParam::Text(pkt.receiver.clone())],
        );
        return;
    }
    // Java `RequestSendPost`: `BlockList.isInBlockList(receiverId, senderId)`,
    // immediately before the outbox-size check.
    //
    // **`isInBlockList`, not `isBlocked`** — the persisted list only. Mail is
    // deliberately not refused just because the addressee sits in
    // message-refusal mode: that is a live chat toggle, and the static
    // `isInBlockList` is what Java reaches for here precisely because the
    // receiver may be offline, where no such flag exists to read.
    if crate::game_loop::social::chat::block_list::is_in_block_list(world, receiver_id, player) {
        send_sm(
            world,
            player,
            sm_ids::C1_HAS_BLOCKED_YOU_YOU_CANNOT_SEND_MAIL_TO_C1,
            &[SmParam::Text(pkt.receiver.clone())],
        );
        return;
    }
    if world.mail.outbox_size(player) >= crate::model::mail::MAILBOX_LIMIT
        || world.mail.inbox_size(receiver_id) >= crate::model::mail::MAILBOX_LIMIT
    {
        send_sm(
            world,
            player,
            sm_ids::THE_MAIL_LIMIT_240_HAS_BEEN_EXCEEDED_AND_THIS_CANNOT_BE_FORWARDED,
            &[],
        );
        return;
    }

    // --- fee + attachment transfer ---------------------------------------
    let Some(message_id) = world.alloc_object_id() else {
        return;
    };
    let fee =
        server_packets::MESSAGE_FEE + server_packets::MESSAGE_FEE_PER_SLOT * pkt.items.len() as i64;
    // Adena being *attached* is not available to pay the fee.
    let attached_adena: i64 = pkt
        .items
        .iter()
        .filter(|(oid, _)| {
            world
                .objects
                .get_component::<Inventory>(&player)
                .and_then(|inv| inv.item_by_object_id(*oid))
                .is_some_and(|(item_id, _)| item_id == ADENA_ID)
        })
        .map(|(_, count)| *count)
        .sum();

    // Every attachment must exist, be unequipped and be sendable — Java
    // refuses the whole mail otherwise, before charging anything.
    for (object_id, count) in &pkt.items {
        let ok = world
            .objects
            .get_component::<Inventory>(&player)
            .is_some_and(|inv| {
                inv.paperdoll_slot_of(*object_id).is_none()
                    && inv
                        .item_by_object_id(*object_id)
                        .is_some_and(|(item_id, have)| {
                            have >= *count
                                && *count > 0
                                // Java `RequestSendPost`: `!item.isTradeable()
                                // || item.isEquipped()` refuses the whole mail.
                                && world
                                    .data
                                    .item_data
                                    .get(item_id)
                                    .is_some_and(|t| !t.is_quest_item && t.is_tradable())
                        })
            });
        if !ok {
            send_sm(
                world,
                player,
                sm_ids::THE_ITEM_THAT_YOU_RE_TRYING_TO_SEND_CANNOT_BE_FORWARDED,
                &[],
            );
            return;
        }
    }

    let adena = world
        .objects
        .get_component::<Inventory>(&player)
        .map_or(0, |inv| inv.adena());
    if adena - attached_adena < fee {
        send_sm(
            world,
            player,
            sm_ids::YOU_CANNOT_FORWARD_BECAUSE_YOU_DON_T_HAVE_ENOUGH_ADENA,
            &[],
        );
        return;
    }

    let mut message = Message::new_player_mail(
        message_id,
        player,
        receiver_id,
        pkt.is_cod,
        pkt.subject.clone(),
        pkt.text.clone(),
        pkt.req_adena,
        commons::util::now_millis(),
    );
    message.has_attachments = !pkt.items.is_empty();
    world.mail.insert(message);

    // Charge the fee, then move each attachment into the message's container.
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
        inv.remove_item(ADENA_ID, fee);
    }
    for (object_id, count) in &pkt.items {
        move_to_attachments(world, player, message_id, *object_id, *count);
    }

    persist_message(world, message_id);
    if !pkt.items.is_empty() {
        persist_attachments(world, message_id);
    }
    schedule_expiry(world, message_id);
    // Re-send the full item list rather than a delta — a partial-stack move
    // creates a new object id, which an InventoryUpdate cannot express.
    send_inventory_item_list(world, player);
    send_to_player(world, player, server_packets::ex_notice_post_sent(true));
    send_sm(world, player, sm_ids::MAIL_SUCCESSFULLY_SENT, &[]);
    // The recipient, if online, gets the chime and a fresh badge.
    send_to_player(
        world,
        receiver_id,
        server_packets::ex_notice_post_arrived(true),
    );
    send_unread_count(world, receiver_id);
}

/// Move one inventory item (or part of a stack) into a message's attachment
/// container, allocating a fresh object id when the source keeps a remainder.
fn move_to_attachments(
    world: &mut World,
    player: i32,
    message_id: i32,
    object_id: i32,
    count: i64,
) {
    let Some((item_id, have)) = world
        .objects
        .get_component::<Inventory>(&player)
        .and_then(|inv| inv.item_by_object_id(object_id))
    else {
        return;
    };
    let moved = count.min(have);
    if moved <= 0 {
        return;
    }
    let enchant = world
        .objects
        .get_component::<Inventory>(&player)
        .and_then(|inv| inv.by_object_id(object_id).map(|it| it.enchant_level))
        .unwrap_or(0);
    // A partial stack leaves the original id with the sender.
    let dst_oid = if moved < have {
        match world.alloc_object_id() {
            Some(id) => id,
            None => return,
        }
    } else {
        object_id
    };
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
        inv.remove_by_object_id(object_id, moved);
    }
    let catalog = &world.data.item_data;
    world
        .mail
        .attachments
        .entry(message_id)
        .or_default()
        // `mana` -1: mail attachments demand tradability, which no shadow
        // item has.
        .insert_instance(catalog, dst_oid, item_id, moved, enchant, -1);
}
