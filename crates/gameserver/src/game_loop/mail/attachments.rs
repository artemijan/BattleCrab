//! Attachments: the shared guards, take/cancel/reject flows and the COD
//! payment to the sender.

use super::*;
// ---------------------------------------------------------------------------
// Attachments — receive (ex 0x67), cancel (ex 0x6C), reject (ex 0x68)
// ---------------------------------------------------------------------------

/// Shared guard chain for the three attachment flows. `peace_sm` differs per
/// packet in Java, so the caller supplies its trio of messages.
fn attachment_guards(
    world: &World,
    player: i32,
    peace_sm: i16,
    exchange_sm: i16,
    store_sm: i16,
) -> bool {
    if !in_peace_zone(world, player) {
        send_sm(world, player, peace_sm, &[]);
        return false;
    }
    if world.objects.has_component::<Trade>(&player) {
        send_sm(world, player, exchange_sm, &[]);
        return false;
    }
    if world
        .objects
        .get_component::<Player>(&player)
        .is_some_and(|p| p.store_type != 0)
    {
        send_sm(world, player, store_sm, &[]);
        return false;
    }
    true
}

/// Total slots the attachments will take in an inventory — per item, the same
/// count `weight::slots_needed` uses for bulk-purchase validation.
fn attachment_slots(world: &World, player: i32, message_id: i32) -> usize {
    let Some(container) = world.mail.attachments.get(&message_id) else {
        return 0;
    };
    container
        .items()
        .iter()
        .map(|it| {
            crate::game_loop::weight::slots_needed(world, player, it.item_id, it.count) as usize
        })
        .sum()
}

/// `PlayerInventory.validateCapacity` — the hand-rolled copy this replaces
/// read the plain race cap, dropping the GM cap and the `EnlargeSlot` passive
/// bonus `weight::inventory_limit` folds in.
fn inventory_has_room(world: &World, player: i32, slots: usize) -> bool {
    crate::game_loop::weight::validate_capacity(world, player, slots as i64)
}

/// Hand every attachment of `message_id` to `player`, announcing each.
fn grant_attachments(world: &mut World, player: i32, message_id: i32) {
    let taken: Vec<(i32, i64, i32)> = world
        .mail
        .attachments
        .get(&message_id)
        .map(|inv| {
            inv.items()
                .iter()
                .map(|it| (it.item_id, it.count, it.enchant_level))
                .collect()
        })
        .unwrap_or_default();
    for (item_id, count, enchant) in taken {
        if let Some(oids) =
            crate::game_loop::items::add_inventory_item(world, player, item_id, count)
            && enchant > 0
            && let Some(inv) = world.objects.get_component_mut::<Inventory>(&player)
        {
            for oid in &oids {
                inv.set_item_enchant(*oid, enchant);
            }
        }
        send_sm(
            world,
            player,
            sm_ids::YOU_HAVE_ACQUIRED_S2_S1,
            &[SmParam::ItemName(item_id), SmParam::Long(count)],
        );
    }
    world.mail.attachments.remove(&message_id);
    if let Some(m) = world.mail.get_mut(message_id) {
        m.has_attachments = false;
    }
    persist_flags(world, message_id);
    persist_attachments(world, message_id);
    send_inventory_item_list(world, player);
}

/// ex 0x67 `RequestPostAttachment` — take the items, paying any COD price.
pub(crate) fn handle_post_attachment(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if !world.cfg.general.allow_mail || !world.cfg.general.allow_attachments {
        return;
    }
    let Some(message_id) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };
    if !attachment_guards(
        world,
        player,
        sm_ids::YOU_CANNOT_RECEIVE_IN_A_NON_PEACE_ZONE_LOCATION,
        sm_ids::YOU_CANNOT_RECEIVE_DURING_AN_EXCHANGE,
        sm_ids::YOU_CANNOT_RECEIVE_BECAUSE_THE_PRIVATE_STORE_OR_WORKSHOP_IS_IN_PROGRESS,
    ) {
        return;
    }
    let Some(m) = world.mail.get(message_id) else {
        return;
    };
    if m.receiver_id != player {
        crate::game_loop::punishment::illegal_action(
            world,
            player,
            &format!("Player {player} tried to get not own attachment!"),
        );
        return;
    }
    if !m.has_attachments {
        return;
    }
    let (sender_id, req_adena) = (m.sender_id, m.req_adena);

    let slots = attachment_slots(world, player, message_id);
    if !inventory_has_room(world, player, slots) {
        send_sm(
            world,
            player,
            sm_ids::YOU_COULD_NOT_RECEIVE_BECAUSE_YOUR_INVENTORY_IS_FULL,
            &[],
        );
        return;
    }
    // Cash on delivery: the receiver pays before anything moves.
    if req_adena > 0 {
        let has = world
            .objects
            .get_component::<Inventory>(&player)
            .map_or(0, |inv| inv.adena());
        if has < req_adena {
            send_sm(
                world,
                player,
                sm_ids::YOU_CANNOT_RECEIVE_BECAUSE_YOU_DON_T_HAVE_ENOUGH_ADENA,
                &[],
            );
            return;
        }
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
            inv.remove_item(ADENA_ID, req_adena);
        }
    }

    grant_attachments(world, player, message_id);

    let receiver_name = char_name_by_id(world, player);
    if req_adena > 0 {
        // The sender is paid whether or not they are online — an offline
        // sender's adena goes straight into their stored inventory.
        pay_sender(world, sender_id, req_adena);
        send_sm(
            world,
            sender_id,
            sm_ids::S2_HAS_MADE_A_PAYMENT_OF_S1_ADENA_PER_YOUR_PAYMENT_REQUEST_MAIL,
            &[
                SmParam::Long(req_adena),
                SmParam::Text(receiver_name.clone()),
            ],
        );
    } else {
        send_sm(
            world,
            sender_id,
            sm_ids::S1_ACQUIRED_THE_ATTACHED_ITEM_TO_YOUR_MAIL,
            &[SmParam::Text(receiver_name)],
        );
    }

    send_to_player(
        world,
        player,
        server_packets::ex_change_post_state(true, &[message_id], crate::model::mail::STATE_READ),
    );
    send_sm(world, player, sm_ids::MAIL_SUCCESSFULLY_RECEIVED, &[]);
}

/// Credit adena to a player who may be offline. Java writes an `items` row
/// directly for the offline case; the port routes an offline payout through a
/// system mail instead, so it survives without a second write path into a
/// character's stored inventory.
fn pay_sender(world: &mut World, sender_id: i32, adena: i64) {
    if world.objects.has_component::<Inventory>(&sender_id) {
        crate::game_loop::items::add_inventory_item(world, sender_id, ADENA_ID, adena);
        send_inventory_item_list(world, sender_id);
        return;
    }
    let Some(message_id) = world.alloc_object_id() else {
        return;
    };
    let mut msg = crate::model::mail::Message::new_system_mail(
        message_id,
        sender_id,
        "Payment received".to_string(),
        String::new(),
        crate::model::mail::MailType::Regular,
        commons::util::now_millis(),
    );
    msg.has_attachments = true;
    world.mail.insert(msg);
    if let Some(oid) = world.alloc_object_id() {
        let catalog = &world.data.item_data;
        world
            .mail
            .attachments
            .entry(message_id)
            .or_default()
            .insert_instance(catalog, oid, ADENA_ID, adena, 0, -1);
    }
    persist_message(world, message_id);
    persist_attachments(world, message_id);
}

/// ex 0x6C `RequestCancelPostAttachment` — the sender takes it all back.
pub(crate) fn handle_cancel_post_attachment(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if !world.cfg.general.allow_mail || !world.cfg.general.allow_attachments {
        return;
    }
    let Some(message_id) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };
    let Some(m) = world.mail.get(message_id) else {
        return;
    };
    if m.sender_id != player {
        crate::game_loop::punishment::illegal_action(
            world,
            player,
            &format!("Player {player} tried to cancel not own post!"),
        );
        return;
    }
    if !attachment_guards(
        world,
        player,
        sm_ids::YOU_CANNOT_CANCEL_IN_A_NON_PEACE_ZONE_LOCATION,
        sm_ids::YOU_CANNOT_CANCEL_DURING_AN_EXCHANGE,
        sm_ids::YOU_CANNOT_CANCEL_BECAUSE_THE_PRIVATE_STORE_OR_WORKSHOP_IS_IN_PROGRESS,
    ) {
        return;
    }
    let receiver_id = world.mail.get(message_id).map_or(0, |m| m.receiver_id);
    let empty = world
        .mail
        .attachments
        .get(&message_id)
        .is_none_or(|inv| inv.items().is_empty());
    if !world
        .mail
        .get(message_id)
        .is_some_and(|m| m.has_attachments)
        || empty
    {
        send_sm(
            world,
            player,
            sm_ids::YOU_CANNOT_CANCEL_SENT_MAIL_SINCE_THE_RECIPIENT_RECEIVED_IT,
            &[],
        );
        return;
    }
    let slots = attachment_slots(world, player, message_id);
    if !inventory_has_room(world, player, slots) {
        send_sm(
            world,
            player,
            sm_ids::YOU_COULD_NOT_CANCEL_RECEIPT_BECAUSE_YOUR_INVENTORY_IS_FULL,
            &[],
        );
        return;
    }

    grant_attachments(world, player, message_id);
    let sender_name = char_name_by_id(world, player);
    send_sm(
        world,
        receiver_id,
        sm_ids::S1_CANCELED_THE_SENT_MAIL,
        &[SmParam::Text(sender_name)],
    );
    send_to_player(
        world,
        receiver_id,
        server_packets::ex_change_post_state(
            true,
            &[message_id],
            crate::model::mail::STATE_DELETED,
        ),
    );
    // A cancelled mail is gone for both sides — the fee is not refunded.
    delete_message(world, message_id);
    send_to_player(
        world,
        player,
        server_packets::ex_change_post_state(
            false,
            &[message_id],
            crate::model::mail::STATE_DELETED,
        ),
    );
    send_sm(world, player, sm_ids::MAIL_SUCCESSFULLY_CANCELLED, &[]);
}

/// ex 0x68 `RequestRejectPostAttachment` — the receiver sends it back.
pub(crate) fn handle_reject_post_attachment(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if !world.cfg.general.allow_mail || !world.cfg.general.allow_attachments {
        return;
    }
    let Some(message_id) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };
    if refuse_attachments_outside_peace_zone(world, player) {
        return;
    }
    let Some(m) = world.mail.get(message_id) else {
        return;
    };
    if m.receiver_id != player {
        crate::game_loop::punishment::illegal_action(
            world,
            player,
            &format!("Player {player} tried to reject not own attachment!"),
        );
        return;
    }
    if !m.has_attachments || m.mail_type.is_system() {
        return;
    }
    let sender_id = m.sender_id;

    // Java builds a *new* message back to the original sender and moves the
    // container onto it, leaving the original row behind minus its items.
    let Some(return_id) = world.alloc_object_id() else {
        return;
    };
    let mut returned = crate::model::mail::Message::new_player_mail(
        return_id,
        sender_id,
        sender_id,
        false,
        String::new(),
        String::new(),
        0,
        commons::util::now_millis(),
    );
    returned.returned = true;
    returned.deleted_by_sender = true;
    returned.has_attachments = true;
    world.mail.insert(returned);
    if let Some(container) = world.mail.attachments.remove(&message_id) {
        world.mail.attachments.insert(return_id, container);
    }
    if let Some(m) = world.mail.get_mut(message_id) {
        m.has_attachments = false;
    }
    persist_flags(world, message_id);
    persist_attachments(world, message_id);
    persist_message(world, return_id);
    persist_attachments(world, return_id);
    schedule_expiry(world, return_id);

    send_sm(world, player, sm_ids::MAIL_SUCCESSFULLY_RETURNED, &[]);
    send_to_player(
        world,
        player,
        server_packets::ex_change_post_state(
            true,
            &[message_id],
            crate::model::mail::STATE_REJECTED,
        ),
    );
    let rejecter = char_name_by_id(world, player);
    send_sm(
        world,
        sender_id,
        sm_ids::S1_RETURNED_THE_MAIL,
        &[SmParam::Text(rejecter)],
    );
    send_to_player(
        world,
        sender_id,
        server_packets::ex_notice_post_arrived(true),
    );
    send_unread_count(world, sender_id);
}
