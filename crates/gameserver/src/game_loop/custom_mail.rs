//! Custom mail — port of `instancemanager/CustomMailManager`, gated on
//! `Custom/CustomMailManager.ini` (`CustomMailManagerEnabled = True` here).
//!
//! The `custom_mail` table is an **inbound** interface: an operator, a web shop
//! or a support tool writes a row, and the game server polls, converts it into
//! an ordinary in-game message with attachments, and deletes the row. Nothing
//! in the game ever writes to it.
//!
//! **A row is only delivered while its recipient is online.** Java looks the
//! player up in `World` and skips the row entirely otherwise, leaving it for a
//! later poll — so an offline character's gift waits rather than being lost,
//! and the delete only happens on the delivering pass.

use crate::db::{CustomMailRow, DbCommand};
use crate::model::mail::{MailType, Message};
use crate::world::World;

use super::helpers::client_for_player;

/// Java `Config.CUSTOM_MAIL_MANAGER_DELAY` is `DatabaseQueryDelay * 1000`, so
/// the ini's `30` is thirty **seconds** — three hundred game-loop ticks.
pub(crate) fn poll_period_ticks(world: &World) -> u64 {
    (world.cfg.custom_mail.query_delay_secs.max(1) as u64) * 10
}

/// Ask the DB thread for the pending rows. The reply arrives as
/// [`crate::db::DbEvent::CustomMailLoaded`].
pub(crate) fn poll(world: &mut World) {
    if !world.cfg.custom_mail.enabled {
        return;
    }
    let _ = world.db.send(DbCommand::LoadCustomMail);
}

/// Handle the poll's reply: deliver what we can, delete what we delivered.
pub(crate) fn apply_loaded(world: &mut World, rows: Vec<CustomMailRow>) {
    for row in rows {
        // `World.getPlayer(playerId)` + `isOnline()`: an offline recipient's row
        // is left in the table for a later pass.
        if client_for_player(world, row.receiver).is_none() {
            continue;
        }
        deliver(world, &row);
        let _ = world.db.send(DbCommand::DeleteCustomMail {
            date: row.date.clone(),
            receiver: row.receiver,
        });
        tracing::info!("CustomMail: message sent to character {}.", row.receiver);
    }
}

/// Turn one row into a system message plus its attachments.
fn deliver(world: &mut World, row: &CustomMailRow) {
    let items = parse_item_list(&row.items);
    let Some(message_id) = world.alloc_object_id() else {
        return;
    };
    // Java: a row *with* items becomes `PRIME_SHOP_GIFT`, one without stays
    // `REGULAR`. That type is Kamael-era and outside this port's enum, so a
    // gift lands as `Regular` too — the only difference is the client's icon,
    // and `MailType`'s ordinals are the wire values, so inventing one would
    // send a number this client does not know.
    let mut msg = Message::new_system_mail(
        message_id,
        row.receiver,
        row.subject.clone(),
        row.message.clone(),
        MailType::Regular,
        commons::util::now_millis(),
    );
    msg.has_attachments = !items.is_empty();
    world.mail.insert(msg);
    for (item_id, count, enchant) in &items {
        let Some(object_id) = world.alloc_object_id() else {
            break;
        };
        let catalog = &world.data.item_data;
        world
            .mail
            .attachments
            .entry(message_id)
            .or_default()
            .insert_instance(catalog, object_id, *item_id, *count, *enchant);
    }
    super::mail::persist_message(world, message_id);
    if !items.is_empty() {
        super::mail::persist_attachments(world, message_id);
    }
    // The chime and the badge, the same two the player-to-player path sends.
    super::mail::send(
        world,
        row.receiver,
        crate::network::server_packets::ex_notice_post_arrived(true),
    );
    super::mail::send_unread_count(world, row.receiver);
}

/// `itemId count enchant;itemId count;itemId;…` — Java's parser, quirks
/// included: a bare id means one of it, a two-field entry defaults the enchant
/// to 0, and anything unparseable is silently skipped rather than failing the
/// whole row.
fn parse_item_list(raw: &str) -> Vec<(i32, i64, i32)> {
    let mut out = Vec::new();
    for entry in raw.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if entry.contains(' ') {
            let parts: Vec<&str> = entry.split_whitespace().collect();
            let (Some(Ok(id)), Some(Ok(count))) = (
                parts.first().map(|p| p.parse::<i32>()),
                parts.get(1).map(|p| p.parse::<i64>()),
            ) else {
                continue;
            };
            let enchant = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
            out.push((id, count, enchant));
        } else if let Ok(id) = entry.parse::<i32>() {
            out.push((id, 1, 0));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::parse_item_list;

    /// The three shapes Java accepts, and the junk it drops.
    #[test]
    fn the_item_list_parses_each_shape() {
        assert_eq!(
            parse_item_list("57 1000;5592 3 0;1540"),
            vec![(57, 1000, 0), (5592, 3, 0), (1540, 1, 0)],
            "id+count, id+count+enchant, and a bare id"
        );
        assert_eq!(
            parse_item_list("6364 1 6"),
            vec![(6364, 1, 6)],
            "the third field is the enchant"
        );
        assert!(parse_item_list("").is_empty());
        assert_eq!(
            parse_item_list("junk;57 abc;;57 5"),
            vec![(57, 5, 0)],
            "unparseable entries are skipped, not fatal"
        );
    }
}
