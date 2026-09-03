//! Friends, block lists, mail, and the moderation rows (punishments, bot
//! reports, petition feedback).

use super::super::BLOCK_RELATION;
use super::super::BirthdayDay;
use super::super::BirthdayMatch;
use super::super::CustomMailRow;
use super::super::DbEvent;
use super::super::EventTx;
use super::super::ItemRow;
use super::super::MailRow;
use super::super::item_row_model;
use super::super::warn_err;
use super::clear_mail_items;
use models::entity;
use models::sea_orm::ActiveValue::NotSet;
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::Condition;
use models::sea_orm::DatabaseConnection;
use models::sea_orm::sea_query::OnConflict;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

pub(super) async fn insert_friend_pair(db: &DatabaseConnection, a: i32, b: i32) {
    // Both directions in one statement, as Java's two-row INSERT does.
    warn_err(
        entity::character_friends::Entity::insert_many([
            entity::character_friends::ActiveModel {
                char_id: Set(a),
                friend_id: Set(b),
                relation: Set(0),
                memo: NotSet,
            },
            entity::character_friends::ActiveModel {
                char_id: Set(b),
                friend_id: Set(a),
                relation: Set(0),
                memo: NotSet,
            },
        ])
        .on_conflict(
            OnConflict::columns([
                entity::character_friends::Column::CharId,
                entity::character_friends::Column::FriendId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(db)
        .await,
    );
}

pub(super) async fn insert_block(db: &DatabaseConnection, owner: i32, target: i32) {
    // Java `BlockList.updateInDB(add)` — one row, one direction,
    // `relation = 1`. Unlike a friendship, blocking is not mutual.
    warn_err(
        entity::character_friends::Entity::insert(entity::character_friends::ActiveModel {
            char_id: Set(owner),
            friend_id: Set(target),
            relation: Set(BLOCK_RELATION),
            memo: NotSet,
        })
        .on_conflict(
            OnConflict::columns([
                entity::character_friends::Column::CharId,
                entity::character_friends::Column::FriendId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(db)
        .await,
    );
}

pub(super) async fn delete_block(db: &DatabaseConnection, owner: i32, target: i32) {
    warn_err(
        entity::character_friends::Entity::delete_many()
            .filter(entity::character_friends::Column::CharId.eq(owner))
            .filter(entity::character_friends::Column::FriendId.eq(target))
            .filter(entity::character_friends::Column::Relation.eq(BLOCK_RELATION))
            .exec(db)
            .await,
    );
}

/// NOTE: this one is deliberately **not** filtered by `relation`,
/// matching Java's `RequestFriendDel`
/// (`DELETE ... WHERE (charId=? AND friendId=?) OR (...)`). So
/// removing a friendship also clears a block row for the same pair
/// in either direction — a real upstream quirk, reachable when one
/// side blocks and the other later befriends and unfriends. Ported
/// as behaviour rather than intent; the block commands above are
/// relation-scoped precisely so they cannot do the reverse.
pub(super) async fn delete_friend_pair(db: &DatabaseConnection, a: i32, b: i32) {
    warn_err(
        entity::character_friends::Entity::delete_many()
            .filter(
                Condition::any()
                    .add(
                        Condition::all()
                            .add(entity::character_friends::Column::CharId.eq(a))
                            .add(entity::character_friends::Column::FriendId.eq(b)),
                    )
                    .add(
                        Condition::all()
                            .add(entity::character_friends::Column::CharId.eq(b))
                            .add(entity::character_friends::Column::FriendId.eq(a)),
                    ),
            )
            .exec(db)
            .await,
    );
}

pub(super) async fn store_mail(db: &DatabaseConnection, message: MailRow) {
    // The boolean-ish columns are enum('true','false') text.
    let b = |v: bool| if v { "true" } else { "false" }.to_string();
    warn_err(
        entity::messages::Entity::insert(entity::messages::ActiveModel {
            message_id: Set(message.message_id),
            sender_id: Set(message.sender_id),
            receiver_id: Set(message.receiver_id),
            subject: Set(Some(message.subject.clone())),
            content: Set(Some(message.content.clone())),
            expiration: Set(message.expiration),
            req_adena: Set(message.req_adena),
            has_attachments: Set(b(message.has_attachments)),
            is_unread: Set(b(message.unread)),
            is_deleted_by_sender: Set(b(message.deleted_by_sender)),
            is_deleted_by_receiver: Set(b(message.deleted_by_receiver)),
            send_by_system: Set(message.send_by_system),
            is_returned: Set(b(message.returned)),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(entity::messages::Column::MessageId)
                .update_columns([
                    entity::messages::Column::SenderId,
                    entity::messages::Column::ReceiverId,
                    entity::messages::Column::Subject,
                    entity::messages::Column::Content,
                    entity::messages::Column::Expiration,
                    entity::messages::Column::ReqAdena,
                    entity::messages::Column::HasAttachments,
                    entity::messages::Column::IsUnread,
                    entity::messages::Column::IsDeletedBySender,
                    entity::messages::Column::IsDeletedByReceiver,
                    entity::messages::Column::SendBySystem,
                    entity::messages::Column::IsReturned,
                ])
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn update_mail_flags(
    db: &DatabaseConnection,
    message_id: i32,
    unread: bool,
    has_attachments: bool,
    deleted_by_sender: bool,
    deleted_by_receiver: bool,
) {
    let b = |v: bool| if v { "true" } else { "false" };
    warn_err(
        entity::messages::Entity::update_many()
            .col_expr(entity::messages::Column::IsUnread, b(unread).into())
            .col_expr(
                entity::messages::Column::HasAttachments,
                b(has_attachments).into(),
            )
            .col_expr(
                entity::messages::Column::IsDeletedBySender,
                b(deleted_by_sender).into(),
            )
            .col_expr(
                entity::messages::Column::IsDeletedByReceiver,
                b(deleted_by_receiver).into(),
            )
            .filter(entity::messages::Column::MessageId.eq(message_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn delete_mail(db: &DatabaseConnection, message_id: i32) {
    warn_err(
        entity::messages::Entity::delete_by_id(message_id)
            .exec(db)
            .await,
    );
    clear_mail_items(db, message_id).await;
}

pub(super) async fn store_mail_items(
    db: &DatabaseConnection,
    message_id: i32,
    owner_id: i32,
    items: Vec<ItemRow>,
) {
    clear_mail_items(db, message_id).await;
    for it in &items {
        warn_err(
            entity::items::Entity::insert(item_row_model(owner_id, it, Some(("MAIL", message_id))))
                .exec(db)
                .await,
        );
    }
}

pub(super) async fn load_custom_mail(db: &DatabaseConnection, event_tx: &EventTx) {
    let rows = entity::custom_mail::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| CustomMailRow {
            date: r.date,
            receiver: r.receiver,
            subject: r.subject,
            message: r.message,
            items: r.items,
        })
        .collect();
    let _ = event_tx.send(DbEvent::CustomMailLoaded { rows });
}

pub(super) async fn load_birthdays(
    db: &DatabaseConnection,
    event_tx: &EventTx,
    days: Vec<BirthdayDay>,
) {
    // Java `TaskBirthday.QUERY`: `createDate LIKE '%-MM-DD'`, one
    // query per day it is catching up on.
    let mut rows = Vec::new();
    for day in days {
        let matches = entity::characters::Entity::find()
            .filter(entity::characters::Column::CreateDate.like(format!("%-{}", day.month_day)))
            .all(db)
            .await
            .unwrap_or_default();
        rows.extend(matches.into_iter().map(|c| BirthdayMatch {
            char_id: c.char_id,
            name: c.char_name,
            create_date: c.create_date,
            year: day.year,
        }));
    }
    let _ = event_tx.send(DbEvent::BirthdaysLoaded { rows });
}

pub(super) async fn delete_custom_mail(db: &DatabaseConnection, date: String, receiver: i32) {
    warn_err(
        entity::custom_mail::Entity::delete_many()
            .filter(entity::custom_mail::Column::Date.eq(date))
            .filter(entity::custom_mail::Column::Receiver.eq(receiver))
            .exec(db)
            .await,
    );
}

pub(super) async fn store_bot_reports(db: &DatabaseConnection, rows: Vec<(i32, i32, i64)>) {
    // Java clears first and re-inserts the whole table.
    warn_err(
        entity::bot_reported_char_data::Entity::delete_many()
            .exec(db)
            .await,
    );
    for (bot_id, reporter_id, report_date) in rows {
        warn_err(
            entity::bot_reported_char_data::Entity::insert(
                entity::bot_reported_char_data::ActiveModel {
                    bot_id: Set(bot_id),
                    reporter_id: Set(reporter_id),
                    report_date: Set(report_date),
                },
            )
            .exec(db)
            .await,
        );
    }
}

pub(super) async fn store_punishment(
    db: &DatabaseConnection,
    id: i32,
    key: String,
    affect: String,
    ptype: String,
    expiration: i64,
    reason: String,
    punished_by: String,
) {
    warn_err(
        entity::punishments::Entity::insert(entity::punishments::ActiveModel {
            id: Set(id),
            key: Set(key),
            affect: Set(affect),
            r#type: Set(ptype),
            expiration: Set(expiration),
            reason: Set(reason),
            punished_by: Set(punished_by),
        })
        .on_conflict(
            OnConflict::column(entity::punishments::Column::Id)
                .update_columns([
                    entity::punishments::Column::Key,
                    entity::punishments::Column::Affect,
                    entity::punishments::Column::Type,
                    entity::punishments::Column::Expiration,
                    entity::punishments::Column::Reason,
                    entity::punishments::Column::PunishedBy,
                ])
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn delete_punishment(db: &DatabaseConnection, id: i32) {
    warn_err(entity::punishments::Entity::delete_by_id(id).exec(db).await);
}

pub(super) async fn store_petition_feedback(
    db: &DatabaseConnection,
    char_name: String,
    gm_name: String,
    rate: i32,
    message: String,
    date: i64,
) {
    warn_err(
        entity::petition_feedback::Entity::insert(entity::petition_feedback::ActiveModel {
            char_name: Set(char_name),
            gm_name: Set(gm_name),
            rate: Set(rate),
            message: Set(message),
            date: Set(date),
        })
        .exec(db)
        .await,
    );
}
