//! `ip_bans` — the IP ban list, and the only one: placed from the dashboard,
//! by the login server (failed passwords, a game server's temp ban) and by the
//! game server's unauthenticated-connection strikes; enforced by both servers
//! on every new connection. Being a table, it survives restarts.
//!
//! The matching rule is Java's `isBannedAddress`: a ban on an address covers
//! that address, and a ban whose trailing octets are `0` covers the whole
//! subnet — `10.1.2.0` is 10.1.2.*, `10.1.0.0` is 10.1.*.*. [`covering`] is
//! that rule, in one place for the login server's in-memory list, this table,
//! and the dashboard's "who does this ban disconnect".

use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DbErr, EntityTrait, QueryFilter, TransactionSession,
    TransactionTrait,
};

use crate::entity::ip_bans::{ActiveModel, Column, Entity, Model};

/// Every ban entry that would cover `ip`: the address itself, then its
/// `.0`, `.0.0` and `.0.0.0` subnets. Anything that is not a dotted quad
/// (IPv6, garbage) can only be matched exactly.
pub fn covering(ip: &str) -> Vec<String> {
    let mut out = vec![ip.to_string()];
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() == 4 {
        out.push(format!("{}.{}.{}.0", parts[0], parts[1], parts[2]));
        out.push(format!("{}.{}.0.0", parts[0], parts[1]));
        out.push(format!("{}.0.0.0", parts[0]));
    }
    out
}

/// The ban in force on `ip` at `now_millis`, if any. Expired rows are left in
/// place: the dashboard lists them as expired until someone removes them.
pub async fn active_for<C: ConnectionTrait>(
    db: &C,
    ip: &str,
    now_millis: i64,
) -> Result<Option<Model>, DbErr> {
    Entity::find()
        .filter(Column::Ip.is_in(covering(ip)))
        .filter(
            Condition::any()
                .add(Column::ExpiresAt.lte(0))
                .add(Column::ExpiresAt.gt(now_millis)),
        )
        .one(db)
        .await
}

/// Bans `ip`, replacing any ban already on it: banning an address again (say,
/// after its ban expired) is a new ban, by whoever placed it. `expires_at` is
/// epoch ms, `0` for permanent.
pub async fn ban<C: ConnectionTrait>(
    db: &C,
    ip: &str,
    expires_at: i64,
    reason: &str,
    banned_by: &str,
    now_millis: i64,
) -> Result<Model, DbErr> {
    let row = ActiveModel {
        ip: Set(ip.to_string()),
        expires_at: Set(expires_at),
        reason: Set(reason.to_string()),
        banned_by: Set(banned_by.to_string()),
        created_at: Set(now_millis),
    };
    Entity::insert(row)
        .on_conflict(
            OnConflict::column(Column::Ip)
                .update_columns([
                    Column::ExpiresAt,
                    Column::Reason,
                    Column::BannedBy,
                    Column::CreatedAt,
                ])
                .to_owned(),
        )
        .exec_with_returning(db)
        .await
}

/// An automatic ban: like [`ban`], but never shortens a ban already in force
/// on `ip` — a 15-minute failed-password ban must not cut an admin's
/// permanent one down to 15 minutes. Returns whether it wrote anything.
pub async fn ban_at_least<C: ConnectionTrait + TransactionTrait>(
    db: &C,
    ip: &str,
    expires_at: i64,
    reason: &str,
    banned_by: &str,
    now_millis: i64,
) -> Result<bool, DbErr> {
    let txn = db.begin().await?;
    if let Some(current) = Entity::find_by_id(ip.to_string()).one(&txn).await?
        && outlasts(current.expires_at, expires_at, now_millis)
    {
        return Ok(false);
    }
    ban(&txn, ip, expires_at, reason, banned_by, now_millis).await?;
    txn.commit().await?;
    Ok(true)
}

/// Whether a ban ending at `current` (0 = never) is in force at `now` and
/// lasts at least as long as one ending at `new`.
fn outlasts(current: i64, new: i64, now: i64) -> bool {
    let active = current <= 0 || current > now;
    active && (current <= 0 || (new > 0 && current >= new))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ban_covers_its_address_and_zeroed_subnets() {
        assert_eq!(
            covering("10.1.2.3"),
            vec!["10.1.2.3", "10.1.2.0", "10.1.0.0", "10.0.0.0"]
        );
        assert_eq!(covering("::1"), vec!["::1"]);
    }

    #[test]
    fn an_automatic_ban_only_ever_lengthens() {
        let now = 1_000;
        // Permanent stays permanent.
        assert!(outlasts(0, 5_000, now));
        assert!(outlasts(0, 0, now));
        // A longer timed ban stays; a shorter one or a permanent one replaces it.
        assert!(outlasts(9_000, 5_000, now));
        assert!(!outlasts(2_000, 5_000, now));
        assert!(!outlasts(9_000, 0, now));
        // An expired row is replaced whatever its length was.
        assert!(!outlasts(500, 5_000, now));
    }
}
