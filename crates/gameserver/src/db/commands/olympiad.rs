//! Olympiad and hero rows.

use super::super::HeroRow;
use super::super::OlympiadNobleRow;
use super::super::warn_err;
use models::entity;
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::DatabaseConnection;
use models::sea_orm::sea_query::OnConflict;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

pub(super) async fn save_olympiad(
    db: &DatabaseConnection,
    current_cycle: i32,
    period: i32,
    olympiad_end: i64,
    validation_end: i64,
    next_weekly_change: i64,
    nobles: Vec<OlympiadNobleRow>,
) {
    warn_err(
        entity::olympiad_data::Entity::insert(entity::olympiad_data::ActiveModel {
            id: Set(0),
            current_cycle: Set(current_cycle),
            period: Set(period),
            olympiad_end: Set(olympiad_end),
            validation_end: Set(validation_end),
            next_weekly_change: Set(next_weekly_change),
        })
        .on_conflict(
            OnConflict::column(entity::olympiad_data::Column::Id)
                .update_columns([
                    entity::olympiad_data::Column::CurrentCycle,
                    entity::olympiad_data::Column::Period,
                    entity::olympiad_data::Column::OlympiadEnd,
                    entity::olympiad_data::Column::ValidationEnd,
                    entity::olympiad_data::Column::NextWeeklyChange,
                ])
                .to_owned(),
        )
        .exec(db)
        .await,
    );
    for n in nobles {
        warn_err(
            entity::olympiad_nobles::Entity::insert(entity::olympiad_nobles::ActiveModel {
                char_id: Set(n.char_id),
                class_id: Set(n.class_id),
                olympiad_points: Set(n.points),
                competitions_done: Set(n.comp_done),
                competitions_won: Set(n.comp_won),
                competitions_lost: Set(n.comp_lost),
                competitions_drawn: Set(n.comp_drawn),
                competitions_done_week: Set(n.comp_done_week),
            })
            .on_conflict(
                OnConflict::column(entity::olympiad_nobles::Column::CharId)
                    .update_columns([
                        entity::olympiad_nobles::Column::ClassId,
                        entity::olympiad_nobles::Column::OlympiadPoints,
                        entity::olympiad_nobles::Column::CompetitionsDone,
                        entity::olympiad_nobles::Column::CompetitionsWon,
                        entity::olympiad_nobles::Column::CompetitionsLost,
                        entity::olympiad_nobles::Column::CompetitionsDrawn,
                        entity::olympiad_nobles::Column::CompetitionsDoneWeek,
                    ])
                    .to_owned(),
            )
            .exec(db)
            .await,
        );
    }
}

pub(super) async fn save_heroes(db: &DatabaseConnection, heroes: Vec<HeroRow>) {
    // `Hero.computeNewHeroes` replaces the active crown.
    warn_err(entity::heroes::Entity::delete_many().exec(db).await);
    for h in heroes {
        warn_err(
            entity::heroes::Entity::insert(entity::heroes::ActiveModel {
                char_id: Set(h.char_id),
                class_id: Set(h.class_id),
                count: Set(h.count),
                played: Set(1),
                claimed: Set(if h.claimed { "true" } else { "false" }.to_string()),
                ..Default::default()
            })
            .on_conflict(
                OnConflict::column(entity::heroes::Column::CharId)
                    .update_columns([
                        entity::heroes::Column::ClassId,
                        entity::heroes::Column::Count,
                        entity::heroes::Column::Played,
                        entity::heroes::Column::Claimed,
                    ])
                    .to_owned(),
            )
            .exec(db)
            .await,
        );
    }
}

pub(super) async fn snapshot_olympiad_eom(db: &DatabaseConnection) {
    // Java runs `TRUNCATE olympiad_nobles_eom` then
    // `INSERT INTO olympiad_nobles_eom SELECT … FROM olympiad_nobles`.
    warn_err(
        entity::olympiad_nobles_eom::Entity::delete_many()
            .exec(db)
            .await,
    );
    let live = entity::olympiad_nobles::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    for n in live {
        warn_err(
            entity::olympiad_nobles_eom::Entity::insert(entity::olympiad_nobles_eom::ActiveModel {
                char_id: Set(n.char_id),
                class_id: Set(n.class_id),
                olympiad_points: Set(n.olympiad_points),
                competitions_done: Set(n.competitions_done),
                competitions_won: Set(n.competitions_won),
                competitions_lost: Set(n.competitions_lost),
                competitions_drawn: Set(n.competitions_drawn),
            })
            .exec(db)
            .await,
        );
    }
}

pub(super) async fn claim_hero(db: &DatabaseConnection, char_id: i32) {
    warn_err(
        entity::heroes::Entity::update_many()
            .col_expr(entity::heroes::Column::Claimed, "true".into())
            .filter(entity::heroes::Column::CharId.eq(char_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn save_hero_diary(
    db: &DatabaseConnection,
    char_id: i32,
    time: i64,
    action: i32,
    param: i32,
) {
    warn_err(
        entity::heroes_diary::Entity::insert(entity::heroes_diary::ActiveModel {
            char_id: Set(char_id),
            time: Set(time),
            action: Set(action),
            param: Set(param),
        })
        .exec(db)
        .await,
    );
}
