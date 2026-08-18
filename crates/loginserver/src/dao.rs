//! Account access for the login flow — `LoginController.java`'s share of the
//! `accounts` table, mapped onto the session's [`AccountInfo`].
//!
//! The queries themselves live in `models::repo::accounts`: the game server and
//! the dashboard read the same table, and one shared temporary-ban rule beats
//! three copies of it.

use models::sea_orm::ConnectionTrait;

use crate::session::AccountInfo;

/// The account plus its ban-adjusted access level, or `None` when there is no
/// such login. A database error reads as "no such account", which is what the
/// previous `.ok()?` did — either way the caller fails the login.
pub async fn select_account_info<C: ConnectionTrait>(
    db: &C,
    login: &str,
    now_millis: i64,
) -> Option<AccountInfo> {
    let row = models::repo::accounts::find_with_ban(db, login, now_millis)
        .await
        .ok()??;
    Some(AccountInfo {
        // Java `AccountInfo` lowercases the login (accounts are
        // case-insensitive). Everything keyed by `info.login` — including the
        // login server's `authed_clients`, matched against the game server's
        // lowercase `PlayerAuthRequest` — depends on this.
        login: row.model.login.unwrap_or_default().to_lowercase(),
        pass_hash: row.model.password.unwrap_or_default(),
        access_level: row.effective_access_level,
        last_server: row.model.last_server.unwrap_or_default(),
    })
}

pub async fn update_account_info<C: ConnectionTrait>(
    db: &C,
    login: &str,
    ip: &str,
    now_millis: i64,
) {
    let _ = models::repo::accounts::touch(db, login, ip, now_millis).await;
}

/// Returns (whitelist, blacklist) from `accounts_ipauth`.
pub async fn select_ipauth<C: ConnectionTrait>(db: &C, login: &str) -> (Vec<String>, Vec<String>) {
    models::repo::accounts::ip_auth(db, login)
        .await
        .unwrap_or_default()
}
