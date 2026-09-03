//! Lottery and Monster Derby Track rows.

use super::super::DbEvent;
use super::super::EventTx;
use super::super::warn_err;
use models::entity;
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::DatabaseConnection;
use models::sea_orm::sea_query::OnConflict;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

pub(super) async fn store_lottery(db: &DatabaseConnection, idnr: i32, enddate: i64, prize: i64) {
    warn_err(
        entity::lottery::Entity::insert(entity::lottery::ActiveModel {
            id: Set(1),
            idnr: Set(idnr),
            enddate: Set(enddate),
            prize: Set(prize),
            newprize: Set(prize),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::columns([entity::lottery::Column::Id, entity::lottery::Column::Idnr])
                .update_columns([
                    entity::lottery::Column::Enddate,
                    entity::lottery::Column::Prize,
                    entity::lottery::Column::Newprize,
                ])
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn finish_lottery(
    db: &DatabaseConnection,
    idnr: i32,
    prize: i64,
    newprize: i64,
    number1: i32,
    number2: i32,
    prize1: i64,
    prize2: i64,
    prize3: i64,
) {
    warn_err(
        entity::lottery::Entity::update_many()
            .col_expr(entity::lottery::Column::Finished, 1.into())
            .col_expr(entity::lottery::Column::Prize, prize.into())
            .col_expr(entity::lottery::Column::Newprize, newprize.into())
            .col_expr(entity::lottery::Column::Number1, number1.into())
            .col_expr(entity::lottery::Column::Number2, number2.into())
            .col_expr(entity::lottery::Column::Prize1, prize1.into())
            .col_expr(entity::lottery::Column::Prize2, prize2.into())
            .col_expr(entity::lottery::Column::Prize3, prize3.into())
            .filter(entity::lottery::Column::Id.eq(1))
            .filter(entity::lottery::Column::Idnr.eq(idnr))
            .exec(db)
            .await,
    );
}

pub(super) async fn increase_lottery_prize(db: &DatabaseConnection, idnr: i32, prize: i64) {
    warn_err(
        entity::lottery::Entity::update_many()
            .col_expr(entity::lottery::Column::Prize, prize.into())
            .col_expr(entity::lottery::Column::Newprize, prize.into())
            .filter(entity::lottery::Column::Id.eq(1))
            .filter(entity::lottery::Column::Idnr.eq(idnr))
            .exec(db)
            .await,
    );
}

pub(super) async fn load_lottery_tickets(db: &DatabaseConnection, event_tx: &EventTx, round: i32) {
    // Lottery tickets are ordinary items (id 4442) whose
    // `custom_type1` is the round they were bought in.
    let rows = entity::items::Entity::find()
        .filter(entity::items::Column::ItemId.eq(4442))
        .filter(entity::items::Column::CustomType1.eq(round))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            (
                r.object_id,
                r.enchant_level.unwrap_or(0),
                r.custom_type2.unwrap_or(0),
            )
        })
        .collect();
    let _ = event_tx.send(DbEvent::LotteryTicketsLoaded { round, rows });
}

pub(super) async fn save_mdt_history(
    db: &DatabaseConnection,
    race_id: i32,
    first: i32,
    second: i32,
    odd_rate: f64,
) {
    warn_err(
        entity::mdt_history::Entity::insert(entity::mdt_history::ActiveModel {
            race_id: Set(race_id),
            first: Set(Some(first)),
            second: Set(Some(second)),
            odd_rate: Set(Some(odd_rate)),
        })
        .on_conflict(
            OnConflict::column(entity::mdt_history::Column::RaceId)
                .update_columns([
                    entity::mdt_history::Column::First,
                    entity::mdt_history::Column::Second,
                    entity::mdt_history::Column::OddRate,
                ])
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn save_mdt_bet(db: &DatabaseConnection, lane: i32, bet: i64) {
    warn_err(
        entity::mdt_bets::Entity::insert(entity::mdt_bets::ActiveModel {
            lane_id: Set(lane),
            bet: Set(Some(bet)),
        })
        .on_conflict(
            OnConflict::column(entity::mdt_bets::Column::LaneId)
                .update_column(entity::mdt_bets::Column::Bet)
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn clear_mdt_bets(db: &DatabaseConnection) {
    warn_err(
        entity::mdt_bets::Entity::update_many()
            .col_expr(entity::mdt_bets::Column::Bet, 0.into())
            .exec(db)
            .await,
    );
}
