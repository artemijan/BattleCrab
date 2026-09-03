//! Boot load, the char-name index, and the write-through persistence of
//! messages, flags and attachments.

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------

use super::schedule_all_expiries;
use crate::model::Player;
/// `DbEvent::MailLoaded` — Java `MailManager.load` + `CharInfoTable`.
use crate::model::inventory::Inventory;
use crate::model::mail::MailManager;
use crate::model::mail::Message;
use crate::world::World;
pub(crate) fn on_loaded(
    world: &mut World,
    messages: Vec<Message>,
    attachments: Vec<(i32, Vec<crate::db::ItemRow>)>,
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
