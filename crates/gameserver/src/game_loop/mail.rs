//! Mail / post (G30) — port of Java's `RequestPostItemList` (ex 0x62),
//! `RequestReceivedPostList` (ex 0x64) and `RequestSentPostList` (ex 0x69)
//! against `MailManager`, plus the boot load and the write-through persistence
//! every mutation goes through.
//!
//! Both parties to a message may be offline, so nothing here is memory-first:
//! each change is followed by its `DbCommand` (the clan-warehouse discipline).

use crate::model::components::ZoneFlags;
use crate::model::inventory::{Inventory, ItemInstance};
use crate::model::mail::{MailListRow, MailManager, Message};
use crate::model::Player;
use crate::network::server_packets::{self, sm_ids, MailListView, SmParam};
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::client_for_player;

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------

/// `DbEvent::MailLoaded` — Java `MailManager.load` + `CharInfoTable`.
pub(crate) fn on_loaded(
    world: &mut World,
    messages: Vec<Message>,
    attachments: Vec<(i32, Vec<crate::character::ItemRow>)>,
    char_ids_by_name: Vec<(String, i32)>,
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
        .find(|(_, &id)| id == object_id)
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

pub(crate) fn send(world: &World, object_id: i32, packet: Vec<u8>) {
    if let Some(cid) = client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(packet);
        }
    }
}

pub(crate) fn send_sm(world: &World, object_id: i32, message_id: i16, params: &[SmParam]) {
    send(
        world,
        object_id,
        server_packets::system_message_with(message_id, params),
    );
}

/// `Player.isInsideZone(PEACE)` — several mail actions are peace-zone only.
pub(crate) fn in_peace_zone(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<ZoneFlags>(&object_id)
        .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Peace))
}

fn now_seconds() -> i32 {
    (commons::util::now_millis() / 1000) as i32
}

/// Refresh the unread badge (Java sends this on login and after every change).
pub(crate) fn send_unread_count(world: &World, object_id: i32) {
    let count = world.mail.unread_count(object_id);
    send(
        world,
        object_id,
        server_packets::ex_unread_mail_count(count),
    );
}

/// Java `EnterWorld`: the unread count, plus a silent "you have mail" when the
/// inbox holds anything unread.
pub(crate) fn on_enter_world(world: &World, object_id: i32) {
    if !world.cfg.general.allow_mail {
        return;
    }
    send_unread_count(world, object_id);
    send(
        world,
        object_id,
        server_packets::ex_notice_post_arrived(false),
    );
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
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    if !world.cfg.general.allow_mail {
        return;
    }
    let rows = MailListRow::inbox(&world.mail, player);
    let views = list_views(world, rows);
    send(
        world,
        player,
        server_packets::ex_show_received_post_list(now_seconds(), &views),
    );
}

pub(crate) fn handle_sent_post_list(world: &mut World, client_id: u32) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    if !world.cfg.general.allow_mail {
        return;
    }
    let rows = MailListRow::outbox(&world.mail, player);
    let views = list_views(world, rows);
    send(
        world,
        player,
        server_packets::ex_show_sent_post_list(now_seconds(), &views),
    );
}

// ---------------------------------------------------------------------------
// ex 0x62 RequestPostItemList — what the compose window may attach
// ---------------------------------------------------------------------------

pub(crate) fn handle_post_item_list(world: &mut World, client_id: u32) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    if !world.cfg.general.allow_mail || !world.cfg.general.allow_attachments {
        return;
    }
    if !in_peace_zone(world, player) {
        send_sm(
            world,
            player,
            sm_ids::YOU_CANNOT_RECEIVE_OR_SEND_MAIL_WITH_ATTACHED_ITEMS_IN_NON_PEACE_ZONE_REGIONS,
            &[],
        );
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
                // TODO(G30+): Java filters on `isTradeable()`, which is a
                // per-item flag this port does not model; quest items are the
                // only category it currently distinguishes (same proxy
                // `game_loop::trade` uses).
                .filter(|(_, t)| !t.is_quest_item)
                .collect()
        })
        .unwrap_or_default();
    send(
        world,
        player,
        server_packets::ex_reply_post_item_list(&items),
    );
}
