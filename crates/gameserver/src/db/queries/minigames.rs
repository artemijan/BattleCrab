//! Lottery and Monster Derby Track reads.

use models::entity;

use models::sea_orm::DatabaseConnection;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// The most recent lottery round (Java `Lottery.SELECT_LAST_LOTTERY`). `None`
/// when the table is empty or unavailable.
pub(crate) async fn load_lottery(
    db: &DatabaseConnection,
) -> Option<crate::model::lottery::LotteryRow> {
    let row = entity::lottery::Entity::find()
        .filter(entity::lottery::Column::Id.eq(1))
        .order_by_desc(entity::lottery::Column::Idnr)
        .one(db)
        .await
        .ok()
        .flatten()?;
    Some(crate::model::lottery::LotteryRow {
        idnr: row.idnr,
        prize: row.prize,
        newprize: row.newprize,
        enddate: row.enddate,
        finished: row.finished == 1,
    })
}
/// Every finished lottery round's draw result (Java re-queries per
/// `checkTicket`; loaded once at boot into the game-thread cache).
pub(crate) async fn load_lottery_draws(
    db: &DatabaseConnection,
) -> Vec<(i32, crate::model::lottery::DrawnRound)> {
    entity::lottery::Entity::find()
        .filter(entity::lottery::Column::Id.eq(1))
        .filter(entity::lottery::Column::Finished.eq(1))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            (
                row.idnr,
                crate::model::lottery::DrawnRound {
                    number1: row.number1,
                    number2: row.number2,
                    prize1: row.prize1,
                    prize2: row.prize2,
                    prize3: row.prize3,
                },
            )
        })
        .collect()
}
/// Every Monster Race history record, oldest first (Java `MonsterRace
/// .loadHistory` — also fixes the current race number by the row count).
pub(crate) async fn load_mdt_history(
    db: &DatabaseConnection,
) -> Vec<crate::model::monster_race::HistoryInfo> {
    entity::mdt_history::Entity::find()
        .order_by_asc(entity::mdt_history::Column::RaceId)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| crate::model::monster_race::HistoryInfo {
            race_id: row.race_id,
            first: row.first.unwrap_or(0),
            second: row.second.unwrap_or(0),
            odd_rate: row.odd_rate.unwrap_or(0.0),
        })
        .collect()
}
/// The current lane bets (Java `MonsterRace.loadBets`): `(lane_id, bet)`.
pub(crate) async fn load_mdt_bets(db: &DatabaseConnection) -> Vec<(i32, i64)> {
    entity::mdt_bets::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.lane_id, row.bet.unwrap_or(0)))
        .collect()
}
