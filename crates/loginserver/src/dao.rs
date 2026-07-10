//! Account SQL from `LoginController.java` + `data/sql/sqlite/login/account.sql`.

use sqlx::SqlitePool;

use crate::session::AccountInfo;

/// `-- QUERY: select_account_info` (SQLite dialect): accessLevel becomes -1
/// while an `account_data.ban_temp` timestamp is still in the future.
const SELECT_ACCOUNT_INFO: &str = "SELECT login, password, CASE WHEN (? > value OR value IS NULL) THEN accessLevel ELSE -1 END AS accessLevel, lastServer \
     FROM accounts LEFT JOIN (account_data) ON (account_data.account_name=accounts.login AND account_data.var='ban_temp') WHERE login=?";

const AUTOCREATE_ACCOUNT: &str =
    "INSERT INTO accounts (login, password, lastactive, accessLevel, lastIP) values (?, ?, ?, ?, ?)";

const ACCOUNT_INFO_UPDATE: &str = "UPDATE accounts SET lastactive = ?, lastIP = ? WHERE login = ?";

const ACCOUNT_IPAUTH_SELECT: &str = "SELECT * FROM accounts_ipauth WHERE login = ?";

pub async fn select_account_info(pool: &SqlitePool, login: &str, now_millis: i64) -> Option<AccountInfo> {
    // Java binds the timestamp as a string; SQLite compares it numerically either way.
    let row: Option<(String, Option<String>, i32, i32)> = sqlx::query_as(SELECT_ACCOUNT_INFO)
        .bind(now_millis.to_string())
        .bind(login)
        .fetch_optional(pool)
        .await
        .ok()?;
    row.map(|(login, password, access_level, last_server)| AccountInfo {
        // Java `AccountInfo` constructor lowercases the login (case-insensitive
        // accounts). Everything keyed by `info.login` — including the login
        // server's `authed_clients` matched against the game's lowercase
        // `PlayerAuthRequest` — must be lowercase.
        login: login.to_lowercase(),
        pass_hash: password.unwrap_or_default(),
        access_level,
        last_server,
    })
}

pub async fn auto_create_account(
    pool: &SqlitePool,
    login: &str,
    pass_hash: &str,
    now_millis: i64,
    ip: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(AUTOCREATE_ACCOUNT)
        .bind(login)
        .bind(pass_hash)
        .bind(now_millis)
        .bind(0)
        .bind(ip)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_account_info(pool: &SqlitePool, login: &str, ip: &str, now_millis: i64) {
    let _ = sqlx::query(ACCOUNT_INFO_UPDATE).bind(now_millis).bind(ip).bind(login).execute(pool).await;
}

/// Returns (whitelist, blacklist) from `accounts_ipauth`.
pub async fn select_ipauth(pool: &SqlitePool, login: &str) -> (Vec<String>, Vec<String>) {
    let mut white = Vec::new();
    let mut black = Vec::new();
    let rows: Vec<(String, String, Option<String>)> =
        sqlx::query_as(ACCOUNT_IPAUTH_SELECT).bind(login).fetch_all(pool).await.unwrap_or_default();
    for (_login, ip, kind) in rows {
        if ip.parse::<std::net::IpAddr>().is_err() {
            continue;
        }
        match kind.as_deref() {
            Some("allow") => white.push(ip),
            Some("deny") => black.push(ip),
            _ => {}
        }
    }
    (white, black)
}
