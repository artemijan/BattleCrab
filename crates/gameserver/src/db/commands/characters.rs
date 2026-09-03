//! Character, account and sub-class writes — the `DbCommand` arms that
//! touch a player's own rows.

use super::super::DbEvent;
use super::super::EventTx;
use super::super::count_characters;
use super::super::name_exists;
use super::super::reload;
use super::super::warn_err;
use super::set_char_col;
use models::entity;
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::DatabaseConnection;
use models::sea_orm::sea_query::CaseStatement;
use models::sea_orm::sea_query::Expr;
use models::sea_orm::sea_query::OnConflict;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

pub(super) async fn mark_delete(
    db: &DatabaseConnection,
    event_tx: &EventTx,
    client_id: u32,
    account: String,
    char_id: i32,
    delete_time: i64,
) {
    set_char_col(
        db,
        char_id,
        entity::characters::Column::Deletetime,
        delete_time.into(),
    )
    .await;
    reload(db, event_tx, client_id, account, true).await;
}

pub(super) async fn restore_character(
    db: &DatabaseConnection,
    event_tx: &EventTx,
    client_id: u32,
    account: String,
    char_id: i32,
) {
    set_char_col(
        db,
        char_id,
        entity::characters::Column::Deletetime,
        0.into(),
    )
    .await;
    reload(db, event_tx, client_id, account, true).await;
}

pub(super) async fn send_char_count(db: &DatabaseConnection, event_tx: &EventTx, account: String) {
    let (count, del_times) = count_characters(db, &account).await;
    let _ = event_tx.send(DbEvent::CharCount {
        account,
        count,
        del_times,
    });
}

pub(super) async fn check_name_creatable(
    db: &DatabaseConnection,
    event_tx: &EventTx,
    client_id: u32,
    name: String,
) {
    // RequestCharacterNameCreatable: NAME_ALREADY_EXISTS=2,
    // INVALID_LENGTH=3, creatable=-1 (validity was checked already).
    let result = if name_exists(db, &name).await {
        2
    } else if name.chars().count() > 16 {
        3
    } else {
        -1
    };
    let _ = event_tx.send(DbEvent::NameCreatable { client_id, result });
}

pub(super) async fn delete_quest_rows(
    db: &DatabaseConnection,
    char_id: i32,
    quest_names: Vec<String>,
) {
    warn_err(
        entity::character_quests::Entity::delete_many()
            .filter(entity::character_quests::Column::CharId.eq(char_id))
            .filter(entity::character_quests::Column::Name.is_in(quest_names))
            .exec(db)
            .await,
    );
}

pub(super) async fn wipe_subclass_slot(
    db: &DatabaseConnection,
    char_id: i32,
    class_index: i32,
    old_class_id: i32,
) {
    warn_err(
        entity::character_subclasses::Entity::delete_many()
            .filter(entity::character_subclasses::Column::CharId.eq(char_id))
            .filter(entity::character_subclasses::Column::ClassId.eq(old_class_id))
            .exec(db)
            .await,
    );
    warn_err(
        entity::character_skills::Entity::delete_many()
            .filter(entity::character_skills::Column::CharId.eq(char_id))
            .filter(entity::character_skills::Column::ClassIndex.eq(class_index))
            .exec(db)
            .await,
    );
    warn_err(
        entity::character_hennas::Entity::delete_many()
            .filter(entity::character_hennas::Column::CharId.eq(char_id))
            .filter(entity::character_hennas::Column::ClassIndex.eq(class_index))
            .exec(db)
            .await,
    );
    warn_err(
        entity::character_shortcuts::Entity::delete_many()
            .filter(entity::character_shortcuts::Column::CharId.eq(char_id))
            .filter(entity::character_shortcuts::Column::ClassIndex.eq(class_index))
            .exec(db)
            .await,
    );
}

pub(super) async fn store_sub_class(
    db: &DatabaseConnection,
    char_id: i32,
    class_id: i32,
    class_index: i32,
    level: i32,
    exp: i64,
    sp: i64,
) {
    warn_err(
        entity::character_subclasses::Entity::insert(entity::character_subclasses::ActiveModel {
            char_id: Set(char_id),
            class_id: Set(class_id),
            exp: Set(exp),
            sp: Set(sp),
            level: Set(level),
            vitality_points: Set(0),
            class_index: Set(class_index),
            dual_class: Set(0),
        })
        .on_conflict(
            OnConflict::columns([
                entity::character_subclasses::Column::CharId,
                entity::character_subclasses::Column::ClassId,
            ])
            .update_columns([
                entity::character_subclasses::Column::Exp,
                entity::character_subclasses::Column::Sp,
                entity::character_subclasses::Column::Level,
                entity::character_subclasses::Column::ClassIndex,
            ])
            .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn set_access_level(db: &DatabaseConnection, char_id: i32, level: i32) {
    set_char_col(
        db,
        char_id,
        entity::characters::Column::Accesslevel,
        level.into(),
    )
    .await;
}

pub(super) async fn store_account_var(
    db: &DatabaseConnection,
    account_name: String,
    var: String,
    value: String,
) {
    warn_err(
        entity::account_gsdata::Entity::insert(entity::account_gsdata::ActiveModel {
            account_name: Set(account_name),
            var: Set(var),
            value: Set(value),
        })
        .on_conflict(
            OnConflict::columns([
                entity::account_gsdata::Column::AccountName,
                entity::account_gsdata::Column::Var,
            ])
            .update_column(entity::account_gsdata::Column::Value)
            .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn store_char_var(
    db: &DatabaseConnection,
    char_id: i32,
    var: String,
    value: String,
) {
    // The table has no unique key, so replace by delete + insert
    // (Java `REMOVE_UNCLAIMED_POINTS` then `INSERT_UNCLAIMED_POINTS`).
    warn_err(
        entity::character_variables::Entity::delete_many()
            .filter(entity::character_variables::Column::CharId.eq(char_id))
            .filter(entity::character_variables::Column::Var.eq(var.clone()))
            .exec(db)
            .await,
    );
    warn_err(
        entity::character_variables::Entity::insert(entity::character_variables::ActiveModel {
            char_id: Set(char_id),
            var: Set(var),
            val: Set(value),
        })
        .exec(db)
        .await,
    );
}

pub(super) async fn store_premium(db: &DatabaseConnection, account_name: String, enddate: i64) {
    warn_err(
        entity::account_premium::Entity::insert(entity::account_premium::ActiveModel {
            account_name: Set(account_name),
            enddate: Set(enddate),
        })
        .on_conflict(
            OnConflict::column(entity::account_premium::Column::AccountName)
                .update_column(entity::account_premium::Column::Enddate)
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn delete_premium(db: &DatabaseConnection, account_name: String) {
    warn_err(
        entity::account_premium::Entity::delete_by_id(account_name)
            .exec(db)
            .await,
    );
}

pub(super) async fn store_buffer_scheme(
    db: &DatabaseConnection,
    object_id: i32,
    scheme_name: String,
    skills: String,
) {
    warn_err(
        entity::buffer_schemes::Entity::insert(entity::buffer_schemes::ActiveModel {
            object_id: Set(object_id),
            scheme_name: Set(scheme_name),
            skills: Set(skills),
        })
        .on_conflict(
            OnConflict::columns([
                entity::buffer_schemes::Column::ObjectId,
                entity::buffer_schemes::Column::SchemeName,
            ])
            .update_column(entity::buffer_schemes::Column::Skills)
            .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn delete_buffer_scheme(
    db: &DatabaseConnection,
    object_id: i32,
    scheme_name: String,
) {
    warn_err(
        entity::buffer_schemes::Entity::delete_by_id((object_id, scheme_name))
            .exec(db)
            .await,
    );
}

pub(super) async fn store_favorite(
    db: &DatabaseConnection,
    fav_id: i32,
    player_id: i32,
    title: String,
    bypass: String,
    add_date: String,
) {
    warn_err(
        entity::bbs_favorites::Entity::insert(entity::bbs_favorites::ActiveModel {
            fav_id: Set(fav_id),
            player_id: Set(player_id),
            fav_title: Set(title),
            fav_bypass: Set(bypass),
            fav_add_date: Set(add_date),
        })
        .on_conflict(
            OnConflict::column(entity::bbs_favorites::Column::FavId)
                .update_columns([
                    entity::bbs_favorites::Column::PlayerId,
                    entity::bbs_favorites::Column::FavTitle,
                    entity::bbs_favorites::Column::FavBypass,
                    entity::bbs_favorites::Column::FavAddDate,
                ])
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn delete_favorite(db: &DatabaseConnection, player_id: i32, fav_id: i32) {
    warn_err(
        entity::bbs_favorites::Entity::delete_many()
            .filter(entity::bbs_favorites::Column::PlayerId.eq(player_id))
            .filter(entity::bbs_favorites::Column::FavId.eq(fav_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn reset_recommends(db: &DatabaseConnection) {
    // Java `DailyTaskManager.resetRecommends`: rec_left → 0 for
    // everyone; rec_have → 0 for those at/under 20, else -20.
    warn_err(
        entity::character_reco_bonus::Entity::update_many()
            .col_expr(entity::character_reco_bonus::Column::RecLeft, 0.into())
            .col_expr(entity::character_reco_bonus::Column::RecHave, 0.into())
            .filter(entity::character_reco_bonus::Column::RecHave.lte(20))
            .exec(db)
            .await,
    );
    // `ExprTrait` is imported here rather than at module scope: it
    // adds `min`/`max`/`add` to *every* type, which shadows the
    // `Ord` ones everywhere else in this file.
    use models::sea_orm::sea_query::ExprTrait as _;
    warn_err(
        entity::character_reco_bonus::Entity::update_many()
            .col_expr(entity::character_reco_bonus::Column::RecLeft, 0.into())
            .col_expr(
                entity::character_reco_bonus::Column::RecHave,
                Expr::col(entity::character_reco_bonus::Column::RecHave).sub(20),
            )
            .filter(entity::character_reco_bonus::Column::RecHave.gt(20))
            .exec(db)
            .await,
    );
}

pub(super) async fn reset_world_chat_points(db: &DatabaseConnection) {
    // Java `resetWorldChatPoints`:
    // `UPDATE character_variables SET val = 0 WHERE var = ?`,
    // unfiltered by character exactly as upstream.
    warn_err(
        entity::character_variables::Entity::update_many()
            .col_expr(entity::character_variables::Column::Val, "0".into())
            .filter(
                entity::character_variables::Column::Var
                    .eq(crate::model::components::WORLD_CHAT_USED),
            )
            .exec(db)
            .await,
    );
}

pub(super) async fn reset_vitality(db: &DatabaseConnection, weekly: bool) {
    // Java `resetVitalityDaily`/`resetVitalityWeekly` — both the
    // `characters` and `character_subclasses` rows. `MAX/4` is added
    // uncapped (as Java does); the read-side clamp hides any overflow.
    const MAX: i32 = 140_000;
    // `ExprTrait` is imported here rather than at module scope: it
    // adds `min`/`max`/`add` to *every* type, which shadows the
    // `Ord` ones everywhere else in this file.
    use models::sea_orm::sea_query::ExprTrait as _;
    // Daily adds a quarter of the cap unless the pool is already
    // full; weekly refills it outright.
    fn refill<C: ColumnTrait>(col: C, weekly: bool) -> Expr {
        if weekly {
            Expr::value(MAX)
        } else {
            CaseStatement::new()
                .case(Expr::col(col).eq(MAX), Expr::col(col))
                .finally(Expr::col(col).add(MAX / 4))
                .into()
        }
    }
    warn_err(
        entity::characters::Entity::update_many()
            .col_expr(
                entity::characters::Column::VitalityPoints,
                refill(entity::characters::Column::VitalityPoints, weekly),
            )
            .exec(db)
            .await,
    );
    warn_err(
        entity::character_subclasses::Entity::update_many()
            .col_expr(
                entity::character_subclasses::Column::VitalityPoints,
                refill(entity::character_subclasses::Column::VitalityPoints, weekly),
            )
            .exec(db)
            .await,
    );
}

pub(super) async fn repair_character(db: &DatabaseConnection, char_name: String) {
    // Java `AdminRepairChar`, verbatim. Best-effort: each statement
    // is independent, keyed by name / resolved id.
    warn_err(
        entity::characters::Entity::update_many()
            .col_expr(entity::characters::Column::X, (-84318).into())
            .col_expr(entity::characters::Column::Y, 244579.into())
            .col_expr(entity::characters::Column::Z, (-3730).into())
            .filter(entity::characters::Column::CharName.eq(&char_name))
            .exec(db)
            .await,
    );
    let obj_id = entity::characters::Entity::find()
        .filter(entity::characters::Column::CharName.eq(&char_name))
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|c| c.char_id);
    if let Some(obj_id) = obj_id {
        warn_err(
            entity::character_shortcuts::Entity::delete_many()
                .filter(entity::character_shortcuts::Column::CharId.eq(obj_id))
                .exec(db)
                .await,
        );
        warn_err(
            entity::items::Entity::update_many()
                .col_expr(entity::items::Column::Loc, "INVENTORY".into())
                .filter(entity::items::Column::OwnerId.eq(obj_id))
                .exec(db)
                .await,
        );
    }
}
