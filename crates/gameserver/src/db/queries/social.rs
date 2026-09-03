//! Mail, block lists and the moderation-row reads.

use super::super::ItemRow;
use commons::util::now_millis;
use models::entity;

use models::sea_orm::DatabaseConnection;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// Every persisted item auction + its bids, plus the next auction id (Java
/// `ItemAuctionManager` boot load: `MAX(auctionId)+1` and each instance's
/// `loadAuction`). Empty on this dist.
/// Java `MailManager.load` + the `loc = 'MAIL'` item rows, in one pass.
/// Tolerates the tables being absent (a minimal test schema has neither).
pub(crate) async fn load_mail(
    db: &DatabaseConnection,
) -> (Vec<crate::model::mail::Message>, Vec<(i32, Vec<ItemRow>)>) {
    use crate::model::mail::{MailType, Message};

    // The flag columns are enum('true','false') text; older rows may carry '1'.
    let truthy = |v: &str| v.eq_ignore_ascii_case("true") || v == "1";
    let messages = entity::messages::Entity::find()
        .order_by_asc(entity::messages::Column::Expiration)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| Message {
            id: r.message_id,
            sender_id: r.sender_id,
            receiver_id: r.receiver_id,
            subject: r.subject.unwrap_or_default(),
            content: r.content.unwrap_or_default(),
            expiration: r.expiration,
            req_adena: r.req_adena,
            has_attachments: truthy(&r.has_attachments),
            unread: truthy(&r.is_unread),
            deleted_by_sender: truthy(&r.is_deleted_by_sender),
            deleted_by_receiver: truthy(&r.is_deleted_by_receiver),
            mail_type: MailType::from_id(r.send_by_system),
            returned: truthy(&r.is_returned),
        })
        .collect();

    let mut by_message: std::collections::HashMap<i32, Vec<ItemRow>> =
        std::collections::HashMap::new();
    for r in entity::items::Entity::find()
        .filter(entity::items::Column::Loc.eq("MAIL"))
        .all(db)
        .await
        .unwrap_or_default()
    {
        // Attachments hang off the message through `loc_data`.
        let message_id = r.loc_data.unwrap_or(0);
        by_message.entry(message_id).or_default().push(ItemRow {
            object_id: r.object_id,
            item_id: r.item_id.unwrap_or(0),
            count: r.count,
            enchant_level: r.enchant_level.unwrap_or(0),
            loc: "MAIL".to_string(),
            loc_data: message_id,
            custom_type1: r.custom_type1.unwrap_or(0),
            custom_type2: r.custom_type2.unwrap_or(0),
            mana_left: r.mana_left,
            time: r.time as i32,
            augment_mineral: 0,
            augment_option1: 0,
            augment_option2: 0,
        });
    }
    (messages, by_message.into_iter().collect())
}
/// `PunishmentManager.load` (G31): every active punishment, minus the rows that
/// have already expired (Java skips them, counting them as "expired"). Returns
/// `(next_id, rows)` — `next_id` seeds the game-thread id allocator. Fail-open
/// (empty) if the table is absent, like a minimal test schema.
/// Java `BotReportTable.loadReportedCharData` — every stored report row.
/// Fail-open (empty) if the table is absent, like the other boot loaders.
pub(crate) async fn load_bot_reports(db: &DatabaseConnection) -> Vec<(i32, i32, i64)> {
    entity::bot_reported_char_data::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.bot_id, r.reporter_id, r.report_date))
        .collect()
}
pub(crate) async fn load_punishments(
    db: &DatabaseConnection,
) -> (i32, Vec<crate::model::punishment::Punishment>) {
    use crate::model::punishment::{Punishment, PunishmentAffect, PunishmentType};

    let now = now_millis();
    let all = entity::punishments::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    let rows: Vec<Punishment> = all
        .iter()
        .filter_map(|row| {
            let affect = PunishmentAffect::from_name(&row.affect)?;
            let ptype = PunishmentType::from_name(&row.r#type)?;
            // Java's `load` skips already-expired rows.
            if row.expiration > 0 && now > row.expiration {
                return None;
            }
            Some(Punishment {
                id: row.id,
                key: row.key.clone(),
                affect,
                ptype,
                expiration: row.expiration,
                reason: row.reason.clone(),
                punished_by: row.punished_by.clone(),
            })
        })
        .collect();

    // The id allocator must clear *every* persisted id, not just the still-active
    // ones — an expired row we filtered out above may still own the max id until
    // the operator purges it, and reusing that id would collide on INSERT.
    // `all` (not `rows`) on purpose: an expired row we filtered out still owns
    // its id.
    let loaded_max = all.iter().map(|row| row.id).max().unwrap_or(0);
    let next_id = (loaded_max + 1).max(1);
    (next_id, rows)
}
pub(crate) const BLOCK_RELATION: i32 = 1;
/// **Every** character's block list — Java `BlockList.loadList`, but read in
/// one pass at boot rather than per player, because the port keeps them in one
/// world-level map (see `World::block_lists`).
///
/// Java skips a row pointing at the owner (`friendId == objId`); kept, since a
/// self-block would make a player deaf to their own broadcast.
pub(crate) async fn load_all_block_lists(
    db: &DatabaseConnection,
) -> Vec<(i32, std::collections::HashSet<i32>)> {
    let mut out: std::collections::HashMap<i32, std::collections::HashSet<i32>> =
        std::collections::HashMap::new();
    for row in entity::character_friends::Entity::find()
        .filter(entity::character_friends::Column::Relation.eq(BLOCK_RELATION))
        .all(db)
        .await
        .unwrap_or_default()
    {
        if row.char_id != row.friend_id {
            out.entry(row.char_id).or_default().insert(row.friend_id);
        }
    }
    out.into_iter().collect()
}
