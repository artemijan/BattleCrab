//! `gameservers` — the registered game servers a login server will accept a
//! hexid from (`GameServerTable.java`).

use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DbErr, EntityTrait};

use crate::entity::gameservers::{ActiveModel, Entity, Model};

/// Every registered server, in table order.
pub async fn all<C: ConnectionTrait>(db: &C) -> Result<Vec<Model>, DbErr> {
    Entity::find().all(db).await
}

/// `registerServerOnDB` — records a server that completed the hexid handshake.
pub async fn register<C: ConnectionTrait>(
    db: &C,
    server_id: i32,
    hexid: &str,
    host: &str,
) -> Result<(), DbErr> {
    ActiveModel {
        server_id: Set(server_id),
        hexid: Set(hexid.to_string()),
        host: Set(host.to_string()),
    }
    .insert(db)
    .await?;
    Ok(())
}
