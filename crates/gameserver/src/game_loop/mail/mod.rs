//! Mail / post (G30) — port of Java's `RequestPostItemList` (ex 0x62),
//! `RequestReceivedPostList` (ex 0x64) and `RequestSentPostList` (ex 0x69)
//! against `MailManager`, plus the boot load and the write-through persistence
//! every mutation goes through.
//!
//! Both parties to a message may be offline, so nothing here is memory-first:
//! each change is followed by its `DbCommand` (the clan-warehouse discipline).

use crate::game_loop::helpers::{send_sm_to_player as send_sm, send_to_player};
use crate::model::components::ZoneFlags;
use crate::model::inventory::{Inventory, ItemInstance};
use crate::model::mail::MailListRow;
use crate::network::server_packets::{self, MailListView, sm_ids};
use crate::world::World;

mod attachments;
mod expiry;
mod read;
mod send;
mod store;

pub(crate) use attachments::{
    handle_cancel_post_attachment, handle_post_attachment, handle_reject_post_attachment,
};
pub(crate) use expiry::{handle_expiry, schedule_all_expiries, schedule_expiry};
pub(crate) use read::{
    handle_delete_received_post, handle_delete_sent_post, handle_received_post, handle_sent_post,
};
pub(crate) use send::handle_send_post;
pub(crate) use store::{
    char_id_by_name, char_name_by_id, delete_message, on_character_created, on_character_deleted,
    on_loaded, persist_attachments, persist_flags, persist_message,
};
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
            inv.unequipped_with_templates(&world.data.item_data)
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
