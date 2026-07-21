//! The only module in this crate that writes. Writes are confined to
//! `accounts.password` and `accounts.email` (PLAN_DASHBOARD.md §5.5).
//!
//! Never touch `accessLevel` (privilege escalation), `lastIP`/`pcIp`/`hop*` or
//! `lastServer` — the login server owns those columns.

use sqlx::SqlitePool;

use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Account {
    pub login: String,
    pub password: String,
    pub email: Option<String>,
    pub access_level: i32,
}

/// Logins are matched case-insensitively and stored lowercase, matching
/// `loginserver::dao` (`AccountInfo` lowercases, and the game keys its authed
/// clients on the lowercase form).
pub fn normalize_login(login: &str) -> String {
    login.trim().to_lowercase()
}

pub async fn find(pool: &SqlitePool, login: &str) -> ApiResult<Option<Account>> {
    let row: Option<(String, Option<String>, Option<String>, i32)> = sqlx::query_as(
        "SELECT login, password, email, accessLevel FROM accounts WHERE login = ?",
    )
    .bind(normalize_login(login))
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(login, password, email, access_level)| Account {
        login,
        password: password.unwrap_or_default(),
        email,
        access_level,
    }))
}

/// Finds the single account owning an email address, for password reset.
pub async fn find_by_email(pool: &SqlitePool, email: &str) -> ApiResult<Option<Account>> {
    let row: Option<(String, Option<String>, Option<String>, i32)> = sqlx::query_as(
        "SELECT login, password, email, accessLevel FROM accounts WHERE email = ? COLLATE NOCASE",
    )
    .bind(email.trim())
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(login, password, email, access_level)| Account {
        login,
        password: password.unwrap_or_default(),
        email,
        access_level,
    }))
}

/// Inserts a new account, mirroring `loginserver::dao::auto_create_account`'s
/// column set so a web-created row is indistinguishable from an in-game one.
///
/// Relies on the `login` PRIMARY KEY rather than a check-then-insert: the game
/// server may auto-create the same name between our check and our write.
pub async fn create(pool: &SqlitePool, login: &str, password_hash: &str) -> ApiResult<()> {
    let login = normalize_login(login);
    let now_millis = crate::auth::now_unix() * 1000;

    let result = sqlx::query(
        "INSERT INTO accounts (login, password, lastactive, accessLevel, lastIP) VALUES (?, ?, ?, 0, NULL)",
    )
    .bind(&login)
    .bind(password_hash)
    .bind(now_millis)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Err(ApiError::LoginTaken),
        Err(e) => Err(e.into()),
    }
}

pub async fn set_password(pool: &SqlitePool, login: &str, password_hash: &str) -> ApiResult<()> {
    sqlx::query("UPDATE accounts SET password = ? WHERE login = ?")
        .bind(password_hash)
        .bind(normalize_login(login))
        .execute(pool)
        .await?;
    Ok(())
}

/// Written *only* by the verification-link handler, which is what makes a
/// non-null `email` mean "verified" (PLAN_DASHBOARD.md §5.4).
pub async fn set_email(pool: &SqlitePool, login: &str, email: &str) -> ApiResult<()> {
    sqlx::query("UPDATE accounts SET email = ? WHERE login = ?")
        .bind(email.trim())
        .bind(normalize_login(login))
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
}
