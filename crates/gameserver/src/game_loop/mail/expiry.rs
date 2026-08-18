//! Expiry: the 15-day timers and the return of unclaimed attachments to
//! the sender's warehouse.

use super::*;
use crate::game_loop::helpers::ms_to_ticks;
// ---------------------------------------------------------------------------
// Expiry — Java `MessageDeletionTaskManager`
// ---------------------------------------------------------------------------

/// Arm the deletion timer for one message.
pub(crate) fn schedule_expiry(world: &mut World, message_id: i32) {
    let Some(m) = world.mail.get(message_id) else {
        return;
    };
    let delay_ms = (m.expiration - commons::util::now_millis()).max(0);
    let delay_ticks = ms_to_ticks(delay_ms);
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
