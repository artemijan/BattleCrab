//! World-state rows: grand bosses, NPC respawns, ground items and the
//! global variable table.

use super::super::warn_err;
use models::entity;
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::DatabaseConnection;
use models::sea_orm::sea_query::OnConflict;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

pub(super) async fn store_grand_boss(
    db: &DatabaseConnection,
    boss: crate::model::grand_boss::GrandBoss,
) {
    warn_err(
        entity::grandboss_data::Entity::update_many()
            .col_expr(entity::grandboss_data::Column::LocX, boss.loc_x.into())
            .col_expr(entity::grandboss_data::Column::LocY, boss.loc_y.into())
            .col_expr(entity::grandboss_data::Column::LocZ, boss.loc_z.into())
            .col_expr(entity::grandboss_data::Column::Heading, boss.heading.into())
            .col_expr(
                entity::grandboss_data::Column::RespawnTime,
                boss.respawn_time.into(),
            )
            .col_expr(
                entity::grandboss_data::Column::CurrentHp,
                boss.current_hp.into(),
            )
            .col_expr(
                entity::grandboss_data::Column::CurrentMp,
                boss.current_mp.into(),
            )
            .col_expr(entity::grandboss_data::Column::Status, boss.status.into())
            .filter(entity::grandboss_data::Column::BossId.eq(boss.boss_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn delete_pet_row(db: &DatabaseConnection, collar_object_id: i32) {
    warn_err(
        entity::pets::Entity::delete_by_id(collar_object_id)
            .exec(db)
            .await,
    );
}

pub(super) async fn store_npc_respawn(
    db: &DatabaseConnection,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    respawn_time: i64,
    cur_hp: f64,
    cur_mp: f64,
) {
    warn_err(
        entity::npc_respawns::Entity::insert(entity::npc_respawns::ActiveModel {
            id: Set(npc_id),
            x: Set(x),
            y: Set(y),
            z: Set(z),
            heading: Set(heading),
            respawn_time: Set(respawn_time),
            current_hp: Set(cur_hp),
            current_mp: Set(cur_mp),
        })
        .on_conflict(
            OnConflict::column(entity::npc_respawns::Column::Id)
                .update_columns([
                    entity::npc_respawns::Column::X,
                    entity::npc_respawns::Column::Y,
                    entity::npc_respawns::Column::Z,
                    entity::npc_respawns::Column::Heading,
                    entity::npc_respawns::Column::RespawnTime,
                    entity::npc_respawns::Column::CurrentHp,
                    entity::npc_respawns::Column::CurrentMp,
                ])
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn delete_npc_respawn(db: &DatabaseConnection, npc_id: i32) {
    warn_err(
        entity::npc_respawns::Entity::delete_by_id(npc_id)
            .exec(db)
            .await,
    );
}

pub(super) async fn save_global_variable(db: &DatabaseConnection, var: String, value: String) {
    warn_err(
        entity::global_variables::Entity::insert(entity::global_variables::ActiveModel {
            var: Set(var),
            value: Set(Some(value)),
        })
        .on_conflict(
            OnConflict::column(entity::global_variables::Column::Var)
                .update_column(entity::global_variables::Column::Value)
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}
