//! Data access against the live game SQLite DB — one pool, two tables.
//!
//! `accounts` is writable in exactly two columns; `characters` is read-only.
//! See PLAN_DASHBOARD.md §5.5.

pub mod accounts;
pub mod characters;

use std::path::PathBuf;

use sqlx::SqlitePool;

/// Tables this crate cannot run without.
pub const REQUIRED_TABLES: [&str; 2] = ["accounts", "characters"];

/// Pulls the filesystem path out of a `jdbc:sqlite:` URL.
///
/// Only used for diagnostics — `commons::db::init` does the real parsing. We
/// re-derive it here so a failure can name the absolute path that was actually
/// opened, which is the one fact that makes a misconfigured URL obvious.
pub fn sqlite_path(jdbc_url: &str) -> Option<PathBuf> {
    let rest = jdbc_url
        .strip_prefix("jdbc:sqlite:")
        .or_else(|| jdbc_url.strip_prefix("sqlite://"))
        .or_else(|| jdbc_url.strip_prefix("sqlite:"))?;
    let path = rest.split('?').next().unwrap_or(rest);
    if path.is_empty() || path.starts_with(':') {
        return None; // ":memory:" and friends have no path
    }
    Some(PathBuf::from(path))
}

/// Returns the required tables that are **missing** from the open database.
///
/// Exists because `commons::db::init` opens with `create_if_missing(true)`: a
/// wrong path does not fail, it silently produces an empty database, and every
/// request then fails at runtime instead of at boot. Checking once at startup
/// turns that into a single actionable error.
pub async fn missing_tables(pool: &SqlitePool) -> Result<Vec<&'static str>, sqlx::Error> {
    let mut missing = Vec::new();
    for table in REQUIRED_TABLES {
        let found: Option<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name=?")
                .bind(table)
                .fetch_optional(pool)
                .await?;
        if found.is_none() {
            missing.push(table);
        }
    }
    Ok(missing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_path_from_a_jdbc_url() {
        assert_eq!(
            sqlite_path("jdbc:sqlite:interlude_classic.db?journal_mode=WAL&busy_timeout=5000"),
            Some(PathBuf::from("interlude_classic.db"))
        );
        assert_eq!(
            sqlite_path("jdbc:sqlite:/abs/path/game.db"),
            Some(PathBuf::from("/abs/path/game.db"))
        );
    }

    #[test]
    fn in_memory_urls_have_no_path() {
        assert_eq!(sqlite_path("sqlite::memory:"), None);
        assert_eq!(sqlite_path("not-a-sqlite-url"), None);
    }

    #[tokio::test]
    async fn reports_missing_tables() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        assert_eq!(
            missing_tables(&pool).await.unwrap(),
            vec!["accounts", "characters"]
        );

        sqlx::query("CREATE TABLE accounts (login TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(missing_tables(&pool).await.unwrap(), vec!["characters"]);

        sqlx::query("CREATE TABLE characters (char_name TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        assert!(missing_tables(&pool).await.unwrap().is_empty());
    }
}
