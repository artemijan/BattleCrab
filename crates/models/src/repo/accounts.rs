//! `accounts` — the one table every binary touches.
//!
//! Ported from `LoginController.java` + `data/sql/sqlite/login/account.sql`.

use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter};

use crate::entity::accounts::{ActiveModel, Column, Entity, Model};

/// The access level a temporary ban masks the stored one with (Java's
/// `CASE WHEN … THEN accessLevel ELSE -1 END`).
pub const BANNED_ACCESS_LEVEL: i32 = -1;

/// One account row plus the ban-adjusted access level.
#[derive(Debug, Clone)]
pub struct AccountRow {
    pub model: Model,
    /// `model.access_level`, or [`BANNED_ACCESS_LEVEL`] while an
    /// `account_data.ban_temp` timestamp is still in the future.
    pub effective_access_level: i32,
}

/// A game account by login name.
///
/// Never matches a dashboard master account: those carry a NULL login, and
/// `login = ?` does not match NULL.
pub async fn find<C: ConnectionTrait>(db: &C, login: &str) -> Result<Option<Model>, DbErr> {
    Entity::find().filter(Column::Login.eq(login)).one(db).await
}

/// `-- QUERY: select_account_info` — the account plus its effective access
/// level, which a live `ban_temp` row drops to [`BANNED_ACCESS_LEVEL`].
///
/// Java does this as one LEFT JOIN with a CASE; two statements express the same
/// rule without hiding it inside SQL, and the second only runs for accounts that
/// exist.
pub async fn find_with_ban<C: ConnectionTrait>(
    db: &C,
    login: &str,
    now_millis: i64,
) -> Result<Option<AccountRow>, DbErr> {
    let Some(model) = find(db, login).await? else {
        return Ok(None);
    };
    // `login` is nullable (a NULL marks a dashboard master account), but this
    // lookup matched on it, so it is present.
    let account_name = model.login.clone().unwrap_or_default();
    let banned_until = super::account_data::get(db, &account_name, "ban_temp")
        .await?
        .and_then(|v| v.parse::<i64>().ok());
    let effective_access_level = match banned_until {
        Some(until) if until >= now_millis => BANNED_ACCESS_LEVEL,
        _ => model.access_level,
    };
    Ok(Some(AccountRow {
        model,
        effective_access_level,
    }))
}

/// `AUTOCREATE_ACCOUNTS` — the row the login server writes when an unknown
/// login authenticates successfully.
pub async fn create<C: ConnectionTrait>(
    db: &C,
    login: &str,
    pass_hash: &str,
    now_millis: i64,
    ip: &str,
) -> Result<(), DbErr> {
    ActiveModel {
        login: Set(Some(login.to_string())),
        password: Set(Some(pass_hash.to_string())),
        lastactive: Set(now_millis),
        access_level: Set(0),
        last_ip: Set(Some(ip.to_string())),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// `ACCOUNT_INFO_UPDATE` — last-seen bookkeeping after a successful login.
pub async fn touch<C: ConnectionTrait>(
    db: &C,
    login: &str,
    ip: &str,
    now_millis: i64,
) -> Result<(), DbErr> {
    Entity::update_many()
        .col_expr(Column::Lastactive, now_millis.into())
        .col_expr(Column::LastIp, ip.into())
        .filter(Column::Login.eq(login))
        .exec(db)
        .await?;
    Ok(())
}

/// Which game server the account last played on (`RequestServerLogin`).
pub async fn set_last_server<C: ConnectionTrait>(
    db: &C,
    login: &str,
    server_id: i32,
) -> Result<(), DbErr> {
    Entity::update_many()
        .col_expr(Column::LastServer, server_id.into())
        .filter(Column::Login.eq(login))
        .exec(db)
        .await?;
    Ok(())
}

/// `ChangeAccessLevel` from the game server (`//ban`, `//unban`, …).
pub async fn set_access_level<C: ConnectionTrait>(
    db: &C,
    login: &str,
    level: i32,
) -> Result<(), DbErr> {
    Entity::update_many()
        .col_expr(Column::AccessLevel, level.into())
        .filter(Column::Login.eq(login))
        .exec(db)
        .await?;
    Ok(())
}

/// Stores a new password hash. Returns whether a row actually changed, which is
/// what the "password change was unsuccessful" reply keys off.
pub async fn set_password<C: ConnectionTrait>(
    db: &C,
    login: &str,
    pass_hash: &str,
) -> Result<bool, DbErr> {
    let res = Entity::update_many()
        .col_expr(Column::Password, pass_hash.into())
        .filter(Column::Login.eq(login))
        .exec(db)
        .await?;
    Ok(res.rows_affected > 0)
}

/// `PlayerTracert` — the client's own IP plus the four hops behind it.
pub async fn set_tracert<C: ConnectionTrait>(
    db: &C,
    login: &str,
    pc_ip: &str,
    hops: [&str; 4],
) -> Result<(), DbErr> {
    Entity::update_many()
        .col_expr(Column::PcIp, pc_ip.into())
        .col_expr(Column::Hop1, hops[0].into())
        .col_expr(Column::Hop2, hops[1].into())
        .col_expr(Column::Hop3, hops[2].into())
        .col_expr(Column::Hop4, hops[3].into())
        .filter(Column::Login.eq(login))
        .exec(db)
        .await?;
    Ok(())
}

/// `accounts_ipauth` split into (whitelist, blacklist). Rows whose `ip` does
/// not parse are dropped, matching Java's `InetAddress.getByName` guard.
pub async fn ip_auth<C: ConnectionTrait>(
    db: &C,
    login: &str,
) -> Result<(Vec<String>, Vec<String>), DbErr> {
    use crate::entity::accounts_ipauth;

    let rows = accounts_ipauth::Entity::find()
        .filter(accounts_ipauth::Column::Login.eq(login))
        .all(db)
        .await?;

    let mut white = Vec::new();
    let mut black = Vec::new();
    for row in rows {
        if row.ip.parse::<std::net::IpAddr>().is_err() {
            continue;
        }
        match row.r#type.as_deref() {
            Some("allow") => white.push(row.ip),
            Some("deny") => black.push(row.ip),
            _ => {}
        }
    }
    Ok((white, black))
}
