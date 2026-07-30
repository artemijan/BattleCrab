//! Player-facing writes. Confined to `accounts.login`, `accounts.password`,
//! `accounts.email` and `accounts.is_verified` (PLAN_DASHBOARD.md §5.5).
//!
//! Never touch `accessLevel` from a player-facing path (privilege escalation) —
//! the single, ban-only exception is `admin::set_access_level`, which refuses
//! any value above 0. `lastIP`/`pcIp`/`hop*` and `lastServer` stay the login
//! server's alone.
//!
//! # Master accounts vs game accounts
//!
//! One table serves two kinds of row, told apart by `login`:
//!
//! * **master account** — `login IS NULL`. This is the dashboard identity: it
//!   is keyed by `email`, carries `is_verified` 0 or 1, and can never log into
//!   the game, because every login-server query is `WHERE login = ?` and no
//!   NULL row matches that.
//! * **game account** — `login` is the name typed into the game client. Its
//!   `email` is a copy of its master's, which is what links the two, and its
//!   `is_verified` is NULL.

use models::entity::accounts::{ActiveModel, Column, Entity, Model};
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::sea_query::Expr;
use models::sea_orm::{
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder, SqlErr,
};

use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone)]
pub struct Account {
    /// `None` for a master account — see the module docs.
    pub login: Option<String>,
    pub password: String,
    pub email: Option<String>,
    pub is_verified: Option<i64>,
    pub access_level: i32,
}

impl Account {
    /// The subject a session cookie or one-time token is issued for. A master
    /// account has no login, so its address is the only stable handle it has.
    pub fn subject(&self) -> &str {
        self.email.as_deref().unwrap_or_default()
    }

    pub fn is_master(&self) -> bool {
        self.login.is_none()
    }

    pub fn is_verified(&self) -> bool {
        self.is_verified.unwrap_or(0) != 0
    }
}

fn to_account(model: Model) -> Account {
    Account {
        login: model.login,
        password: model.password.unwrap_or_default(),
        email: model.email,
        is_verified: model.is_verified.map(i64::from),
        access_level: model.access_level,
    }
}

/// `email = ? COLLATE NOCASE`.
///
/// Addresses are normalised to lowercase on the way in, but rows written before
/// that rule existed — and rows the game server wrote — are not, so the
/// collation stays. SeaORM's expression builder cannot attach a collation,
/// hence the custom fragment; the value is still bound, not interpolated.
pub(crate) fn email_eq(email: &str) -> Expr {
    Expr::cust_with_values("email = ? COLLATE NOCASE", [email])
}

/// Logins are matched case-insensitively and stored lowercase, matching
/// `loginserver::dao` (`AccountInfo` lowercases, and the game keys its authed
/// clients on the lowercase form).
pub fn normalize_login(login: &str) -> String {
    login.trim().to_lowercase()
}

/// Addresses are stored lowercased and compared case-insensitively. The local
/// part is technically case-sensitive per RFC 5321, but no mail provider in
/// practice treats it that way, and users do not recall how they capitalised it.
pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// Looks up a game account by login name. Cannot return a master account:
/// `login = ?` never matches a NULL login.
pub async fn find_by_login(db: &DatabaseConnection, login: &str) -> ApiResult<Option<Account>> {
    Ok(Entity::find()
        .filter(Column::Login.eq(normalize_login(login)))
        .one(db)
        .await?
        .map(to_account))
}

/// Looks up the master account for an address — the dashboard's login path.
///
/// The `login IS NULL` predicate is what makes this the *master* lookup. Game
/// accounts carry their master's address, so without it a sub-account row could
/// be returned here and authenticated against.
pub async fn find_master_by_email(
    db: &DatabaseConnection,
    email: &str,
) -> ApiResult<Option<Account>> {
    Ok(Entity::find()
        .filter(Column::Login.is_null())
        .filter(email_eq(&normalize_email(email)))
        .one(db)
        .await?
        .map(to_account))
}

/// Creates a master account: NULL login, address as identity, unverified.
///
/// Leans on the `accounts_master_email` partial unique index rather than a
/// check-then-insert, so two concurrent registrations for one address cannot
/// both succeed. (MariaDB cannot express that index — see the note in its
/// `accounts.sql`; the dashboard only runs on SQLite today.)
pub async fn create_master(
    db: &DatabaseConnection,
    email: &str,
    password_hash: &str,
) -> ApiResult<Account> {
    let email = normalize_email(email);
    let now_millis = crate::auth::now_unix() * 1000;

    let result = Entity::insert(ActiveModel {
        login: Set(None),
        password: Set(Some(password_hash.to_string())),
        email: Set(Some(email.clone())),
        is_verified: Set(Some(0)),
        lastactive: Set(now_millis),
        access_level: Set(0),
        last_ip: Set(None),
        ..Default::default()
    })
    .exec(db)
    .await;

    match result {
        Ok(_) => Ok(Account {
            login: None,
            password: password_hash.to_string(),
            email: Some(email),
            is_verified: Some(0),
            access_level: 0,
        }),
        Err(e) if is_unique_violation(&e) => Err(ApiError::EmailTaken),
        Err(e) => Err(e.into()),
    }
}

/// Creates a game account under a master's address. `is_verified` stays NULL,
/// which is what marks the row as a sub-account rather than an identity.
///
/// The column set matches `loginserver::dao`'s auto-create so a
/// dashboard-created row is indistinguishable from one the game made.
///
/// `max_per_master` caps how many a single master may own. The count and the
/// insert share one transaction because SQLite would otherwise happily let two
/// concurrent requests both read `max - 1` and both insert.
pub async fn create_game_account(
    db: &DatabaseConnection,
    master_email: &str,
    login: &str,
    password_hash: &str,
    max_per_master: usize,
) -> ApiResult<()> {
    let login = normalize_login(login);
    let email = normalize_email(master_email);
    let now_millis = crate::auth::now_unix() * 1000;

    // `BEGIN IMMEDIATE` takes the write lock up front, so the count cannot be
    // read against a snapshot another writer is already invalidating. SeaORM's
    // `begin()` is always DEFERRED — under which the losing request fails with
    // a busy error instead of a clean "too many game accounts" — so this one
    // transaction is driven through the pool underneath the connection.
    let pool = db.get_sqlite_connection_pool();
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;

    let (existing,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM accounts WHERE login IS NOT NULL AND email = ? COLLATE NOCASE",
    )
    .bind(&email)
    .fetch_one(&mut *tx)
    .await?;

    if existing as usize >= max_per_master {
        return Err(ApiError::TooManyGameAccounts(max_per_master));
    }

    let result = sqlx::query(
        "INSERT INTO accounts (login, password, email, is_verified, lastactive, accessLevel, lastIP) \
         VALUES (?, ?, ?, NULL, ?, 0, NULL)",
    )
    .bind(&login)
    .bind(password_hash)
    .bind(&email)
    .bind(now_millis)
    .execute(&mut *tx)
    .await;

    match result {
        Ok(_) => {}
        // Covers a login taken by *anyone*, including another master's game
        // account — the column is globally unique, as the login server needs.
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(ApiError::LoginTaken);
        }
        Err(e) => return Err(e.into()),
    }

    tx.commit().await?;
    Ok(())
}

/// Every game account belonging to a master address.
pub async fn game_accounts_for_master(
    db: &DatabaseConnection,
    email: &str,
) -> ApiResult<Vec<String>> {
    Ok(Entity::find()
        .filter(Column::Login.is_not_null())
        .filter(email_eq(&normalize_email(email)))
        .order_by_asc(Column::Login)
        .all(db)
        .await?
        .into_iter()
        .filter_map(|row| row.login)
        .collect())
}

pub async fn set_master_password(
    db: &DatabaseConnection,
    email: &str,
    password_hash: &str,
) -> ApiResult<()> {
    Entity::update_many()
        .col_expr(Column::Password, password_hash.into())
        .filter(Column::Login.is_null())
        .filter(email_eq(&normalize_email(email)))
        .exec(db)
        .await?;
    Ok(())
}

/// Changes a game account's password. Callers must first confirm the account
/// belongs to the authenticated master — this helper does not check.
pub async fn set_game_account_password(
    db: &DatabaseConnection,
    login: &str,
    password_hash: &str,
) -> ApiResult<()> {
    Entity::update_many()
        .col_expr(Column::Password, password_hash.into())
        .filter(Column::Login.eq(normalize_login(login)))
        .exec(db)
        .await?;
    Ok(())
}

// There is deliberately no `change_master_email`. A master's address is the
// only record of which game accounts belong to it, so moving one would have to
// rewrite every game account row in the same transaction — and the address is
// simultaneously the account's login. That makes it an account migration
// rather than a settable field, and the dashboard does not offer it.

/// Written *only* by the verification-link handler. Registration now stores the
/// address up front, so `is_verified` — not the mere presence of `email` — is
/// what records that the address was proven (superseding PLAN_DASHBOARD.md §5.4).
pub async fn mark_verified(db: &DatabaseConnection, email: &str) -> ApiResult<()> {
    Entity::update_many()
        .col_expr(Column::IsVerified, 1.into())
        .filter(Column::Login.is_null())
        .filter(email_eq(&normalize_email(email)))
        .exec(db)
        .await?;
    Ok(())
}

/// Whether a failed write was a unique-index collision — the signal that an
/// address or login is already taken, as opposed to a real database fault.
fn is_unique_violation(e: &DbErr) -> bool {
    matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logins_normalize_to_lowercase_and_trim() {
        assert_eq!(normalize_login("  Alice  "), "alice");
        assert_eq!(normalize_login("BOB"), "bob");
    }

    #[test]
    fn emails_normalize_to_lowercase_and_trim() {
        assert_eq!(normalize_email(" Alice@Example.COM "), "alice@example.com");
    }

    #[test]
    fn a_master_account_is_identified_by_its_address() {
        let master = Account {
            login: None,
            password: "h".into(),
            email: Some("a@b.c".into()),
            is_verified: Some(1),
            access_level: 0,
        };
        assert!(master.is_master());
        assert!(master.is_verified());
        assert_eq!(master.subject(), "a@b.c");
    }

    #[test]
    fn a_game_account_is_never_a_dashboard_identity() {
        let game = Account {
            login: Some("alice".into()),
            password: "h".into(),
            email: Some("a@b.c".into()),
            is_verified: None,
            access_level: 0,
        };
        assert!(!game.is_master());
        // NULL is_verified means "sub-account", not "verified".
        assert!(!game.is_verified());
    }
}
