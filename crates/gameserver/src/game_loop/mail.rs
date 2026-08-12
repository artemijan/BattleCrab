//! Mail / post (G30) — port of Java's `RequestPostItemList` (ex 0x62),
//! `RequestReceivedPostList` (ex 0x64) and `RequestSentPostList` (ex 0x69)
//! against `MailManager`, plus the boot load and the write-through persistence
//! every mutation goes through.
//!
//! Both parties to a message may be offline, so nothing here is memory-first:
//! each change is followed by its `DbCommand` (the clan-warehouse discipline).

use crate::game_loop::helpers::{
    send_inventory_item_list, send_sm_to_player as send_sm, send_to_player,
};
use crate::model::Player;
use crate::model::components::{Trade, ZoneFlags};
use crate::model::inventory::{Inventory, ItemInstance};
use crate::model::mail::{MailListRow, MailManager, Message};
use crate::network::server_packets::{self, MailListView, SmParam, sm_ids};
use crate::world::World;

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------

/// `DbEvent::MailLoaded` — Java `MailManager.load` + `CharInfoTable`.
pub(crate) fn on_loaded(
    world: &mut World,
    messages: Vec<Message>,
    attachments: Vec<(i32, Vec<crate::character::ItemRow>)>,
    char_ids_by_name: Vec<(String, i32)>,
    block_lists: Vec<(i32, std::collections::HashSet<i32>)>,
) {
    let mut mgr = MailManager::default();
    for m in messages {
        mgr.insert(m);
    }
    for (message_id, rows) in attachments {
        mgr.attachments
            .insert(message_id, Inventory::from_rows(&rows));
    }
    tracing::info!(
        "Mail: loaded {} message(s), {} with attachments; {} character names.",
        mgr.messages.len(),
        mgr.attachments.len(),
        char_ids_by_name.len()
    );
    world.mail = mgr;
    world.char_ids_by_name = char_ids_by_name.into_iter().collect();
    tracing::info!("BlockList: loaded {} ignore list(s).", block_lists.len());
    world.block_lists = block_lists.into_iter().collect();
    schedule_all_expiries(world);
}

/// Keep the `CharInfoTable` equivalent current as characters come and go.
pub(crate) fn on_character_created(world: &mut World, name: &str, object_id: i32) {
    world
        .char_ids_by_name
        .insert(name.to_lowercase(), object_id);
}

pub(crate) fn on_character_deleted(world: &mut World, name: &str) {
    world.char_ids_by_name.remove(&name.to_lowercase());
}

/// Java `CharInfoTable.getIdByName` — works for offline characters, which is
/// the whole point (mail is addressed by name).
#[allow(dead_code)] // consumed by the send/read/attachment flows (slices 4-5)
pub(crate) fn char_id_by_name(world: &World, name: &str) -> Option<i32> {
    world.char_ids_by_name.get(&name.to_lowercase()).copied()
}

/// The reverse lookup for display. Online players win (their name is
/// authoritative); otherwise scan the boot-loaded table.
pub(crate) fn char_name_by_id(world: &World, object_id: i32) -> String {
    if let Some(p) = world.objects.get_component::<Player>(&object_id) {
        return p.name.clone();
    }
    world
        .char_ids_by_name
        .iter()
        .find(|(_, id)| **id == object_id)
        .map_or_else(String::new, |(name, _)| name.clone())
}

// ---------------------------------------------------------------------------
// Persistence helpers — every mutation writes through
//
// Consumed by the send/read/delete flows (slice 4) and the attachment flows
// (slice 5); defined here with the store they belong to.
// ---------------------------------------------------------------------------

#[allow(dead_code)] // consumed by the send/read/attachment flows (slices 4-5)
pub(crate) fn persist_message(world: &World, message_id: i32) {
    let Some(m) = world.mail.get(message_id) else {
        return;
    };
    let _ = world.db.send(crate::db::DbCommand::StoreMail {
        message: crate::db::MailRow {
            message_id: m.id,
            sender_id: m.sender_id,
            receiver_id: m.receiver_id,
            subject: m.subject.clone(),
            content: m.content.clone(),
            expiration: m.expiration,
            req_adena: m.req_adena,
            has_attachments: m.has_attachments,
            unread: m.unread,
            deleted_by_sender: m.deleted_by_sender,
            deleted_by_receiver: m.deleted_by_receiver,
            send_by_system: m.mail_type.id(),
            returned: m.returned,
        },
    });
}

#[allow(dead_code)] // consumed by the send/read/attachment flows (slices 4-5)
pub(crate) fn persist_flags(world: &World, message_id: i32) {
    let Some(m) = world.mail.get(message_id) else {
        return;
    };
    let _ = world.db.send(crate::db::DbCommand::UpdateMailFlags {
        message_id,
        unread: m.unread,
        has_attachments: m.has_attachments,
        deleted_by_sender: m.deleted_by_sender,
        deleted_by_receiver: m.deleted_by_receiver,
    });
}

#[allow(dead_code)] // consumed by the send/read/attachment flows (slices 4-5)
pub(crate) fn persist_attachments(world: &World, message_id: i32) {
    let owner_id = world.mail.get(message_id).map_or(0, |m| m.sender_id);
    let items = world
        .mail
        .attachments
        .get(&message_id)
        .map(|inv| inv.to_rows())
        .unwrap_or_default();
    let _ = world.db.send(crate::db::DbCommand::StoreMailItems {
        message_id,
        owner_id,
        items,
    });
}

/// Drop a message everywhere — memory, its row, and its attachment rows.
#[allow(dead_code)] // consumed by the send/read/attachment flows (slices 4-5)
pub(crate) fn delete_message(world: &mut World, message_id: i32) {
    world.mail.remove(message_id);
    let _ = world
        .db
        .send(crate::db::DbCommand::DeleteMail { message_id });
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// `Player.isInsideZone(PEACE)` — several mail actions are peace-zone only.
pub(crate) fn in_peace_zone(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<ZoneFlags>(&object_id)
        .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Peace))
}

/// Java's `if (!player.isInsideZone(ZoneId.PEACE)) { sendPacket(CANT_SEND_MAIL
/// _WITH_ATTACHMENTS_OUTSIDE_PEACE_ZONE); return; }` — the gate in front of
/// every attachment-bearing mail operation, so a besieged player can neither
/// stash loot into the post nor pull it back out mid-fight.
///
/// `true` means the caller should stop.
fn refuse_attachments_outside_peace_zone(world: &World, player: i32) -> bool {
    if in_peace_zone(world, player) {
        return false;
    }
    send_sm(
        world,
        player,
        sm_ids::YOU_CANNOT_RECEIVE_OR_SEND_MAIL_WITH_ATTACHED_ITEMS_IN_NON_PEACE_ZONE_REGIONS,
        &[],
    );
    true
}

fn now_seconds() -> i32 {
    (commons::util::now_millis() / 1000) as i32
}

/// Refresh the unread badge (Java sends this on login and after every change).
pub(crate) fn send_unread_count(world: &World, object_id: i32) {
    let count = world.mail.unread_count(object_id);
    send_to_player(
        world,
        object_id,
        server_packets::ex_unread_mail_count(count),
    );
}

/// Java `EnterWorld`: the unread count and the silent "you have mail" notice.
///
/// **Both are gated on actually having unread mail** (Java's two
/// `hasUnreadPost(player)` checks). `ExNoticePostArrived` is what lights the
/// client's mail indicator, and the client keeps it lit until the player opens
/// a message — so sending it unconditionally leaves every mail-less character
/// with an indicator over an empty mailbox.
pub(crate) fn on_enter_world(world: &World, object_id: i32) {
    if world.mail.unread_count(object_id) == 0 {
        return;
    }
    send_unread_count(world, object_id);
    if world.cfg.general.allow_mail {
        send_to_player(
            world,
            object_id,
            server_packets::ex_notice_post_arrived(false),
        );
    }
}

/// Resolve the attachment container of a message into packet-ready pairs.
#[allow(dead_code)] // consumed by the read/attachment flows (slices 4-5)
pub(crate) fn attachment_views(
    world: &World,
    message_id: i32,
) -> Vec<(&ItemInstance, &crate::data::item_data::ItemTemplate)> {
    world
        .mail
        .attachments
        .get(&message_id)
        .map(|inv| {
            inv.items()
                .iter()
                .filter_map(|it| world.data.item_data.get(it.item_id).map(|t| (it, t)))
                .collect()
        })
        .unwrap_or_default()
}

fn list_views(world: &World, rows: Vec<MailListRow>) -> Vec<MailListView> {
    rows.into_iter()
        .map(|r| MailListView {
            message_id: r.message_id,
            subject: r.subject,
            counterparty: if r.system_sender {
                "System".to_string()
            } else {
                char_name_by_id(world, r.counterparty_id)
            },
            locked: r.locked,
            expiration_seconds: r.expiration_seconds,
            unread: r.unread,
            has_attachments: r.has_attachments,
            returned: r.returned,
            mail_type: r.mail_type,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// ex 0x64 RequestReceivedPostList / ex 0x69 RequestSentPostList
// ---------------------------------------------------------------------------

pub(crate) fn handle_received_post_list(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if !world.cfg.general.allow_mail {
        return;
    }
    let rows = MailListRow::inbox(&world.mail, player);
    let views = list_views(world, rows);
    send_to_player(
        world,
        player,
        server_packets::ex_show_received_post_list(now_seconds(), &views),
    );
}

pub(crate) fn handle_sent_post_list(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if !world.cfg.general.allow_mail {
        return;
    }
    let rows = MailListRow::outbox(&world.mail, player);
    let views = list_views(world, rows);
    send_to_player(
        world,
        player,
        server_packets::ex_show_sent_post_list(now_seconds(), &views),
    );
}

// ---------------------------------------------------------------------------
// ex 0x62 RequestPostItemList — what the compose window may attach
// ---------------------------------------------------------------------------

pub(crate) fn handle_post_item_list(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if !world.cfg.general.allow_mail || !world.cfg.general.allow_attachments {
        return;
    }
    if refuse_attachments_outside_peace_zone(world, player) {
        return;
    }
    // Java `PlayerInventory.getAvailableItems(allowAdena=true,
    // allowNonTradeable=false)`: everything unequipped and tradeable.
    let items: Vec<(&ItemInstance, &crate::data::item_data::ItemTemplate)> = world
        .objects
        .get_component::<Inventory>(&player)
        .map(|inv| {
            inv.items()
                .iter()
                .filter(|it| inv.paperdoll_slot_of(it.object_id).is_none())
                .filter_map(|it| world.data.item_data.get(it.item_id).map(|t| (it, t)))
                .filter(|(_, t)| !t.is_quest_item && t.is_tradable())
                .collect()
        })
        .unwrap_or_default();
    send_to_player(
        world,
        player,
        server_packets::ex_reply_post_item_list(&items),
    );
}

// ---------------------------------------------------------------------------
// ex 0x63 RequestSendPost — compose and send
// ---------------------------------------------------------------------------

/// Java's field caps (`RequestSendPost`).
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
    if super::block_list::is_in_block_list(world, receiver_id, player) {
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

    let mut message = crate::model::mail::Message::new_player_mail(
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

/// Java's adena item id.
const ADENA_ID: i32 = 57;

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

// ---------------------------------------------------------------------------
// ex 0x66 RequestReceivedPost / ex 0x6B RequestSentPost — open one message
// ---------------------------------------------------------------------------

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
        super::punishment::illegal_action(
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
        super::punishment::illegal_action(
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
    let Some(pkt) = crate::network::client_packets::DeletePostList::read(body) else {
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
            super::punishment::illegal_action(
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

/// Total slots the attachments will take in an inventory (Java counts a
/// non-stackable per unit, a stackable as one slot unless already held).
fn attachment_slots(world: &World, player: i32, message_id: i32) -> usize {
    let Some(container) = world.mail.attachments.get(&message_id) else {
        return 0;
    };
    let inv = world.objects.get_component::<Inventory>(&player);
    container
        .items()
        .iter()
        .map(|it| {
            let stackable = world
                .data
                .item_data
                .get(it.item_id)
                .is_some_and(|t| t.is_stackable);
            if !stackable {
                it.count.max(1) as usize
            } else if inv.is_some_and(|i| i.count_of(it.item_id) > 0) {
                0
            } else {
                1
            }
        })
        .sum()
}

fn inventory_has_room(world: &World, player: i32, slots: usize) -> bool {
    let Some(inv) = world.objects.get_component::<Inventory>(&player) else {
        return false;
    };
    let race = world
        .objects
        .get_component::<Player>(&player)
        .map_or(0, |p| p.race);
    let limit = world.cfg.character.inventory_limit(race) as usize;
    inv.non_quest_size(&world.data.item_data) + slots <= limit
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
        if let Some(oids) = super::items::add_inventory_item(world, player, item_id, count)
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
        super::punishment::illegal_action(
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
        super::items::add_inventory_item(world, sender_id, ADENA_ID, adena);
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
        super::punishment::illegal_action(
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
        super::punishment::illegal_action(
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

// ---------------------------------------------------------------------------
// Expiry — Java `MessageDeletionTaskManager`
// ---------------------------------------------------------------------------

/// Arm the deletion timer for one message.
pub(crate) fn schedule_expiry(world: &mut World, message_id: i32) {
    let Some(m) = world.mail.get(message_id) else {
        return;
    };
    let delay_ms = (m.expiration - commons::util::now_millis()).max(0);
    let delay_ticks = (delay_ms / 100) as u64;
    world.scheduler.schedule(
        world.tick + delay_ticks,
        crate::scheduler::ScheduledTask::MailExpire { message_id },
    );
}

/// Arm a timer for every loaded message — Java re-registers the whole table
/// with `MessageDeletionTaskManager` at boot.
pub(crate) fn schedule_all_expiries(world: &mut World) {
    let ids: Vec<i32> = world.mail.messages.keys().copied().collect();
    for id in ids {
        schedule_expiry(world, id);
    }
}

/// `ScheduledTask::MailExpire`: return any attachments to the sender and drop
/// the message. A timer that fires early (the expiration moved) re-arms.
pub(crate) fn handle_expiry(world: &mut World, message_id: i32) {
    let Some(m) = world.mail.get(message_id) else {
        return; // already gone
    };
    if !m.is_expired(commons::util::now_millis()) {
        schedule_expiry(world, message_id);
        return;
    }
    let (sender_id, receiver_id, has_attachments) = (m.sender_id, m.receiver_id, m.has_attachments);

    if has_attachments {
        // Java returns the items to the sender's *warehouse* — no inventory
        // capacity to fail against, and it works while they are offline.
        if let Some(container) = world.mail.attachments.remove(&message_id) {
            return_to_warehouse(world, sender_id, container);
        }
        for who in [sender_id, receiver_id] {
            send_sm(
                world,
                who,
                sm_ids::THE_MAIL_WAS_RETURNED_DUE_TO_THE_EXCEEDED_WAITING_TIME,
                &[],
            );
        }
    }
    delete_message(world, message_id);
    send_unread_count(world, receiver_id);
}

/// Move a message's leftover attachments into the sender's warehouse — Java
/// `Mail.returnToWh`. The warehouse is used rather than the inventory because
/// it has no capacity gate to fail against and the sender is usually offline.
fn return_to_warehouse(world: &mut World, sender_id: i32, container: Inventory) {
    let rows = container.to_rows();
    if rows.is_empty() {
        return;
    }
    if world
        .objects
        .has_component::<crate::model::inventory::Warehouse>(&sender_id)
    {
        // Online: into the live container, persisted with the owner.
        for r in &rows {
            let catalog = &world.data.item_data;
            if let Some(wh) = world
                .objects
                .get_component_mut::<crate::model::inventory::Warehouse>(&sender_id)
            {
                wh.0.insert_instance(
                    catalog,
                    r.object_id,
                    r.item_id,
                    r.count,
                    r.enchant_level,
                    r.mana_left,
                );
            }
        }
        return;
    }
    // Offline: park the rows at the warehouse location directly. Additive, so
    // it cannot clobber the rest of their warehouse.
    let _ = world
        .db
        .send(crate::db::DbCommand::StoreOfflineWarehouseItems {
            owner_id: sender_id,
            items: rows,
        });
}
