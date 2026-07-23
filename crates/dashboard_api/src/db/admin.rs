//! Every query the `/admin` API is allowed to run, in one auditable place.
//!
//! Reads here are wider than the player-facing modules — an admin sees
//! `accessLevel`, `lastactive`, `created_time` and `lastIP` — but the write
//! surface stays deliberately tiny:
//!
//! * `accounts.accessLevel`, **and only to values ≤ 0** (ban / unban). The
//!   dashboard can never *grant* privileges: `set_access_level` rejects a
//!   positive level outright, so even a compromised admin session cannot mint
//!   a GM. Promotion stays a manual SQL statement by someone with DB access.
//!   The one nuance is [`create_gm_game_account`], which writes a *new* row at
//!   exactly the actor's own level — copying, never exceeding, what the actor
//!   already holds, so the no-elevation property is preserved.
//! * `accounts.is_verified` and `accounts.password` reuse the existing helpers
//!   in [`super::accounts`].
//!
//! `characters` remains strictly read-only (PLAN_DASHBOARD.md §3.2) — live
//! character state is memory-first in the game server, so there is no such
//! thing as a safe character write from here, admin or not.

use sqlx::SqlitePool;

use crate::error::{ApiError, ApiResult};

/// One master account row in the admin list, with ownership counts.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterSummary {
    pub email: String,
    pub is_verified: bool,
    pub access_level: i32,
    /// `CURRENT_TIMESTAMP` text, as SQLite stored it.
    pub created_time: String,
    /// Millis since epoch; 0 when the account never logged in anywhere.
    pub last_active: i64,
    pub game_accounts: i64,
    pub characters: i64,
}

/// A game account as the admin sees it — includes the columns the player API
/// must never return (`accessLevel`, `lastIP`).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameAccountInfo {
    pub login: String,
    /// The owning master's address; `None` for a row the login server
    /// auto-created that no master has claimed.
    pub email: Option<String>,
    pub access_level: i32,
    pub last_active: i64,
    pub last_ip: Option<String>,
    pub characters: i64,
}

/// Turns user input into a `LIKE` pattern that matches it literally.
///
/// Without the escaping, a search for `%` would match every row — harmless for
/// an admin, but it makes "search for this text" quietly mean something else.
fn like_contains(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

/// A sortable column of the master list.
///
/// This enum is the whitelist that keeps caller-supplied sort input away from
/// the SQL: an unknown name never reaches the query, it fails parsing here.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum MasterSort {
    /// Insertion order — `rowid`, not the text timestamp, so two accounts
    /// created in the same second still sort deterministically.
    #[default]
    Created,
    Email,
    AccessLevel,
    Verified,
    LastActive,
    GameAccounts,
    Characters,
}

impl MasterSort {
    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "" | "created" => Self::Created,
            "email" => Self::Email,
            "accessLevel" => Self::AccessLevel,
            "verified" => Self::Verified,
            "lastActive" => Self::LastActive,
            "gameAccounts" => Self::GameAccounts,
            "characters" => Self::Characters,
            _ => return None,
        })
    }

    /// The ORDER BY expression. Count columns sort by their SELECT alias.
    fn sql(self) -> &'static str {
        match self {
            Self::Created => "a.rowid",
            Self::Email => "a.email COLLATE NOCASE",
            Self::AccessLevel => "a.accessLevel",
            Self::Verified => "COALESCE(a.is_verified, 0)",
            Self::LastActive => "a.lastactive",
            Self::GameAccounts => "game_accounts",
            Self::Characters => "characters",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "asc" => Self::Asc,
            "" | "desc" => Self::Desc,
            _ => return None,
        })
    }

    fn sql(self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

/// The list behind `/admin/accounts`: master accounts, newest first by default.
///
/// `query` matches the master's address *or* the login of any game account
/// under it — "which master owns login X" is the single most common admin
/// lookup, and making the one search box answer it beats a second endpoint.
pub async fn list_masters(
    pool: &SqlitePool,
    query: &str,
    sort: MasterSort,
    dir: SortDir,
    limit: i64,
    offset: i64,
) -> ApiResult<(Vec<MasterSummary>, i64)> {
    // Shared by the page query and the count query so they cannot disagree.
    const WHERE: &str = "a.login IS NULL AND (? = '' \
         OR a.email LIKE ? ESCAPE '\\' COLLATE NOCASE \
         OR a.email IN (SELECT g.email FROM accounts g \
                        WHERE g.login IS NOT NULL AND g.login LIKE ? ESCAPE '\\'))";

    let pattern = like_contains(query);

    let (total,): (i64,) =
        sqlx::query_as(&format!("SELECT COUNT(*) FROM accounts a WHERE {WHERE}"))
            .bind(query)
            .bind(&pattern)
            .bind(&pattern)
            .fetch_one(pool)
            .await?;

    type Row = (String, Option<i64>, i32, String, i64, i64, i64);
    // `sort`/`dir` are interpolated, but only through the enums' fixed sql()
    // strings — nothing caller-supplied can reach the statement text.
    let order = format!("{} {}, a.rowid DESC", sort.sql(), dir.sql());
    let rows: Vec<Row> = sqlx::query_as(&format!(
        "SELECT a.email, a.is_verified, a.accessLevel, a.created_time, a.lastactive, \
           (SELECT COUNT(*) FROM accounts g \
              WHERE g.login IS NOT NULL AND g.email = a.email COLLATE NOCASE) AS game_accounts, \
           (SELECT COUNT(*) FROM characters c WHERE c.deletetime = 0 AND c.account_name IN \
              (SELECT g.login FROM accounts g \
                 WHERE g.login IS NOT NULL AND g.email = a.email COLLATE NOCASE)) AS characters \
         FROM accounts a WHERE {WHERE} \
         ORDER BY {order} LIMIT ? OFFSET ?"
    ))
    .bind(query)
    .bind(&pattern)
    .bind(&pattern)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let masters = rows
        .into_iter()
        .map(
            |(email, is_verified, access_level, created_time, last_active, games, chars)| {
                MasterSummary {
                    email,
                    is_verified: is_verified.unwrap_or(0) != 0,
                    access_level,
                    created_time,
                    last_active,
                    game_accounts: games,
                    characters: chars,
                }
            },
        )
        .collect();

    Ok((masters, total))
}

type GameAccountRow = (String, Option<String>, i32, i64, Option<String>, i64);

fn to_game_account(
    (login, email, access_level, last_active, last_ip, characters): GameAccountRow,
) -> GameAccountInfo {
    GameAccountInfo {
        login,
        email,
        access_level,
        last_active,
        last_ip,
        characters,
    }
}

const GAME_ACCOUNT_COLUMNS: &str = "a.login, a.email, a.accessLevel, a.lastactive, a.lastIP, \
     (SELECT COUNT(*) FROM characters c WHERE c.deletetime = 0 AND c.account_name = a.login)";

/// Every game account under a master address, with the admin-only columns.
pub async fn game_accounts_for_master(
    pool: &SqlitePool,
    email: &str,
) -> ApiResult<Vec<GameAccountInfo>> {
    let rows: Vec<GameAccountRow> = sqlx::query_as(&format!(
        "SELECT {GAME_ACCOUNT_COLUMNS} FROM accounts a \
         WHERE a.login IS NOT NULL AND a.email = ? COLLATE NOCASE ORDER BY a.login"
    ))
    .bind(super::accounts::normalize_email(email))
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(to_game_account).collect())
}

/// Search over *all* game accounts by login, masterless ones included.
///
/// The master list cannot reach a row the login server auto-created with no
/// email — this is the only way an admin finds those.
pub async fn search_game_accounts(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
) -> ApiResult<Vec<GameAccountInfo>> {
    let rows: Vec<GameAccountRow> = sqlx::query_as(&format!(
        "SELECT {GAME_ACCOUNT_COLUMNS} FROM accounts a \
         WHERE a.login IS NOT NULL AND (? = '' OR a.login LIKE ? ESCAPE '\\') \
         ORDER BY a.login LIMIT ?"
    ))
    .bind(query)
    .bind(like_contains(query))
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(to_game_account).collect())
}

/// Creates a game account under the acting admin's own master address, with
/// the actor's `accessLevel` copied onto it — a GM game account.
///
/// This is the crate's second `accessLevel` write, and it keeps the "never
/// upward" property structurally: the level written is read from `actor`
/// inside this function, not taken as a parameter, so no caller can mint an
/// account above (or below a different admin than) itself. The player-facing
/// `create_game_account` cap (`MaxGameAccounts`) deliberately does not apply —
/// this is a privileged, logged path, not a farmable one.
pub async fn create_gm_game_account(
    pool: &SqlitePool,
    actor: &super::accounts::Account,
    login: &str,
    password_hash: &str,
) -> ApiResult<()> {
    let now_millis = crate::auth::now_unix() * 1000;

    let result = sqlx::query(
        "INSERT INTO accounts (login, password, email, is_verified, lastactive, accessLevel, lastIP) \
         VALUES (?, ?, ?, NULL, ?, ?, NULL)",
    )
    .bind(super::accounts::normalize_login(login))
    .bind(password_hash)
    .bind(super::accounts::normalize_email(actor.subject()))
    .bind(now_millis)
    .bind(actor.access_level)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Err(ApiError::LoginTaken),
        Err(e) => Err(e.into()),
    }
}

/// Which row a ban/unban targets.
pub enum AccessLevelTarget<'a> {
    /// A master account, by address — bans it out of the *dashboard*.
    Master(&'a str),
    /// A game account, by login — bans it out of the *game* (the login server
    /// refuses `accessLevel < 0`).
    GameAccount(&'a str),
}

/// The only `accessLevel` write in the crate, and it refuses to elevate.
///
/// The `level <= 0` check lives here rather than only in the handler so that no
/// future caller — however convinced it has validated — can turn this into a
/// promotion primitive. See the module docs.
pub async fn set_access_level(
    pool: &SqlitePool,
    target: AccessLevelTarget<'_>,
    level: i32,
) -> ApiResult<()> {
    if level > 0 {
        return Err(ApiError::BadRequest(
            "access level can only be lowered to 0 or a ban value from here".into(),
        ));
    }

    let result = match target {
        AccessLevelTarget::Master(email) => {
            sqlx::query(
                "UPDATE accounts SET accessLevel = ? \
                 WHERE login IS NULL AND email = ? COLLATE NOCASE",
            )
            .bind(level)
            .bind(super::accounts::normalize_email(email))
            .execute(pool)
            .await?
        }
        AccessLevelTarget::GameAccount(login) => {
            sqlx::query("UPDATE accounts SET accessLevel = ? WHERE login = ?")
                .bind(level)
                .bind(super::accounts::normalize_login(login))
                .execute(pool)
                .await?
        }
    };

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_input_is_matched_literally() {
        assert_eq!(like_contains("alice"), "%alice%");
        // A wildcard in the input must not stay a wildcard.
        assert_eq!(like_contains("100%"), "%100\\%%");
        assert_eq!(like_contains("a_b"), "%a\\_b%");
        assert_eq!(like_contains("a\\b"), "%a\\\\b%");
    }
}
