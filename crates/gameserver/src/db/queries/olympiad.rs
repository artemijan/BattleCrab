//! Olympiad and hero reads.

use super::super::DbEvent;
use super::super::HeroRow;
use super::super::OlympiadEomRow;
use super::super::OlympiadNobleRow;
use super::character_load::characters_by_id;
use models::entity;

use models::sea_orm::DatabaseConnection;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GrandBossManager.init`: every `grandboss_data` row. The NPC-template
/// filter (`NpcData.getTemplate != null`) runs on the game thread, which owns
/// the datapack; here we just read the table.
/// `Olympiad.load` — the single `olympiad_data` row (defaults if absent: cycle
/// 1, period 0) plus every `olympiad_nobles` record.
pub(crate) async fn load_olympiad(db: &DatabaseConnection) -> DbEvent {
    let data = entity::olympiad_data::Entity::find()
        .filter(entity::olympiad_data::Column::Id.eq(0))
        .one(db)
        .await
        .ok()
        .flatten();
    let (current_cycle, period, olympiad_end, validation_end, next_weekly_change) = match &data {
        Some(r) => (
            r.current_cycle,
            r.period,
            r.olympiad_end,
            r.validation_end,
            r.next_weekly_change,
        ),
        // Java's defaults for a database with no olympiad row yet.
        None => (1, 0, 0, 0, 0),
    };
    let nobles = entity::olympiad_nobles::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| OlympiadNobleRow {
            char_id: r.char_id,
            class_id: r.class_id,
            points: r.olympiad_points,
            comp_done: r.competitions_done,
            comp_won: r.competitions_won,
            comp_lost: r.competitions_lost,
            comp_drawn: r.competitions_drawn,
            comp_done_week: r.competitions_done_week,
        })
        .collect();
    DbEvent::OlympiadLoaded {
        current_cycle,
        period,
        olympiad_end,
        validation_end,
        next_weekly_change,
        nobles,
        eom: load_olympiad_eom(db).await,
    }
}
/// `Olympiad.getClassLeaderBoard`'s source table — the previous cycle's
/// snapshot, joined to `characters` for the display names exactly as Java's
/// `GET_EACH_CLASS_LEADER` does. Ranking happens in memory at read time.
async fn load_olympiad_eom(db: &DatabaseConnection) -> Vec<OlympiadEomRow> {
    let rows = entity::olympiad_nobles_eom::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    if rows.is_empty() {
        return Vec::new();
    }
    let chars = characters_by_id(db, rows.iter().map(|r| r.char_id)).await;
    rows.into_iter()
        .map(|r| OlympiadEomRow {
            class_id: r.class_id,
            // Java's join drops a row whose character is gone; an empty name is
            // the same thing one step later, and keeps the count honest.
            name: chars
                .iter()
                .find(|c| c.char_id == r.char_id)
                .map(|c| c.char_name.clone())
                .unwrap_or_default(),
            points: r.olympiad_points,
            comp_done: r.competitions_done,
            comp_won: r.competitions_won,
        })
        .collect()
}
/// `Hero.init` — the currently-crowned heroes (`heroes` rows with `played = 1`).
pub(crate) async fn load_heroes(db: &DatabaseConnection) -> Vec<HeroRow> {
    // The name/clan half of the row lives on `characters`; Java reads it
    // through `CharInfoTable` for the same reason there is no FK to follow.
    let heroes = entity::heroes::Entity::find()
        .filter(entity::heroes::Column::Played.eq(1))
        .all(db)
        .await
        .unwrap_or_default();
    if heroes.is_empty() {
        return Vec::new();
    }
    let chars = characters_by_id(db, heroes.iter().map(|h| h.char_id)).await;
    heroes
        .into_iter()
        .map(|h| {
            let c = chars.iter().find(|c| c.char_id == h.char_id);
            HeroRow {
                char_id: h.char_id,
                class_id: h.class_id,
                count: h.count,
                name: c.map(|c| c.char_name.clone()).unwrap_or_default(),
                clan_id: c.and_then(|c| c.clanid).unwrap_or(0),
                message: h.message,
                // Java `Boolean.parseBoolean(rset.getString(CLAIMED))` — anything
                // but "true" reads false.
                claimed: h.claimed == "true",
            }
        })
        .collect()
}
/// Every hero-diary entry (Java `Hero.loadDiary` per hero, batched here into one
/// query), oldest first: `(charId, time, action, param)`.
pub(crate) async fn load_hero_diary(db: &DatabaseConnection) -> Vec<(i32, i64, i8, i32)> {
    entity::heroes_diary::Entity::find()
        .order_by_asc(entity::heroes_diary::Column::Time)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.char_id, r.time, r.action as i8, r.param))
        .collect()
}
