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

use sqlx::SqlitePool;

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

type Row = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    i32,
);

const COLUMNS: &str = "login, password, email, is_verified, accessLevel";

fn to_account((login, password, email, is_verified, access_level): Row) -> Account {
    Account {
        login,
        password: password.unwrap_or_default(),
        email,
        is_verified,
        access_level,
    }
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
pub async fn find_by_login(pool: &SqlitePool, login: &str) -> ApiResult<Option<Account>> {
    let row: Option<Row> =
        sqlx::query_as(&format!("SELECT {COLUMNS} FROM accounts WHERE login = ?"))
            .bind(normalize_login(login))
            .fetch_optional(pool)
            .await?;

    Ok(row.map(to_account))
}

/// Looks up the master account for an address — the dashboard's login path.
///
/// The `login IS NULL` predicate is what makes this the *master* lookup. Game
/// accounts carry their master's address, so without it a sub-account row could
/// be returned here and authenticated against.
pub async fn find_master_by_email(pool: &SqlitePool, email: &str) -> ApiResult<Option<Account>> {
    let row: Option<Row> = sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM accounts WHERE login IS NULL AND email = ? COLLATE NOCASE"
    ))
    .bind(normalize_email(email))
    .fetch_optional(pool)
    .await?;

    Ok(row.map(to_account))
}

/// Creates a master account: NULL login, address as identity, unverified.
///
/// Leans on the `accounts_master_email` partial unique index rather than a
/// check-then-insert, so two concurrent registrations for one address cannot
/// both succeed. (MariaDB cannot express that index — see the note in its
/// `accounts.sql`; the dashboard only runs on SQLite today.)
pub async fn create_master(
    pool: &SqlitePool,
    email: &str,
    password_hash: &str,
) -> ApiResult<Account> {
    let email = normalize_email(email);
    let now_millis = crate::auth::now_unix() * 1000;

    let result = sqlx::query(
        "INSERT INTO accounts (login, password, email, is_verified, lastactive, accessLevel, lastIP) \
         VALUES (NULL, ?, ?, 0, ?, 0, NULL)",
    )
    .bind(password_hash)
    .bind(&email)
    .bind(now_millis)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(Account {
            login: None,
            password: password_hash.to_string(),
            email: Some(email),
            is_verified: Some(0),
            access_level: 0,
        }),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Err(ApiError::EmailTaken),
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
    pool: &SqlitePool,
    master_email: &str,
    login: &str,
    password_hash: &str,
    max_per_master: usize,
) -> ApiResult<()> {
    let login = normalize_login(login);
    let email = normalize_email(master_email);
    let now_millis = crate::auth::now_unix() * 1000;

    // IMMEDIATE takes the write lock up front, so the count cannot be read
    // against a snapshot that another writer is already invalidating.
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
            return Err(ApiError::LoginTaken)
        }
        Err(e) => return Err(e.into()),
    }

    tx.commit().await?;
    Ok(())
}

/// Every game account belonging to a master address.
pub async fn game_accounts_for_master(pool: &SqlitePool, email: &str) -> ApiResult<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT login FROM accounts WHERE login IS NOT NULL AND email = ? COLLATE NOCASE \
         ORDER BY login",
    )
    .bind(normalize_email(email))
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|(login,)| login).collect())
}

pub async fn set_master_password(
    pool: &SqlitePool,
    email: &str,
    password_hash: &str,
) -> ApiResult<()> {
    sqlx::query(
        "UPDATE accounts SET password = ? WHERE login IS NULL AND email = ? COLLATE NOCASE",
    )
    .bind(password_hash)
    .bind(normalize_email(email))
    .execute(pool)
    .await?;
    Ok(())
}

/// Changes a game account's password. Callers must first confirm the account
/// belongs to the authenticated master — this helper does not check.
pub async fn set_game_account_password(
    pool: &SqlitePool,
    login: &str,
    password_hash: &str,
) -> ApiResult<()> {
    sqlx::query("UPDATE accounts SET password = ? WHERE login = ?")
        .bind(password_hash)
        .bind(normalize_login(login))
        .execute(pool)
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
pub async fn mark_verified(pool: &SqlitePool, email: &str) -> ApiResult<()> {
    sqlx::query(
        "UPDATE accounts SET is_verified = 1 WHERE login IS NULL AND email = ? COLLATE NOCASE",
    )
    .bind(normalize_email(email))
    .execute(pool)
    .await?;
    Ok(())
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
