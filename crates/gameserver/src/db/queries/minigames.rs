//! Monster Derby Track reads.

use models::entity;

use models::sea_orm::DatabaseConnection;
use models::sea_orm::{EntityTrait, QueryOrder};

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
