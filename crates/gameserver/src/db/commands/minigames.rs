//! Monster Derby Track rows.

use super::super::warn_err;
use models::entity;
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::DatabaseConnection;
use models::sea_orm::EntityTrait;
use models::sea_orm::sea_query::OnConflict;

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
