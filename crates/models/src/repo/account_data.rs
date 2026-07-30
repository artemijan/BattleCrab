//! `account_data` — the login server's per-account key/value side table
//! (`ban_temp` today; Java also parks a handful of other vars here).

use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter};

use crate::entity::account_data::{ActiveModel, Column, Entity};

/// One variable's value, or `None` when the row does not exist.
pub async fn get<C: ConnectionTrait>(
    db: &C,
    account: &str,
    var: &str,
) -> Result<Option<String>, DbErr> {
    Ok(Entity::find()
        .filter(Column::AccountName.eq(account))
        .filter(Column::Var.eq(var))
        .one(db)
        .await?
        .and_then(|row| row.value))
}

/// `insert_or_update_account_data` — upsert on the `(account_name, var)` key.
pub async fn set<C: ConnectionTrait>(
    db: &C,
    account: &str,
    var: &str,
    value: &str,
) -> Result<(), DbErr> {
    Entity::insert(ActiveModel {
        account_name: Set(account.to_string()),
        var: Set(var.to_string()),
        value: Set(Some(value.to_string())),
    })
    .on_conflict(
        OnConflict::columns([Column::AccountName, Column::Var])
            .update_column(Column::Value)
            .to_owned(),
    )
    .exec(db)
    .await?;
    Ok(())
}
