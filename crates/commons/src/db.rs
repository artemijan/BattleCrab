//! Port of `commons/database/DatabaseFactory.java`, SQLite-only per decision #9.
//!
//! Accepts the JDBC-style URL from the existing config files
//! (`jdbc:sqlite:../../interlude_classic.db?journal_mode=WAL&busy_timeout=5000`)
//! so `LoginServer.ini` keeps working unchanged.

use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use tracing::info;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("unsupported database URL `{0}` — only SQLite is supported")]
    UnsupportedUrl(String),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

pub async fn init(jdbc_url: &str, max_connections: u32) -> Result<SqlitePool, DbError> {
    let (path, params) = parse_jdbc_sqlite_url(jdbc_url)?;

    let mut options = SqliteConnectOptions::from_str(&format!("sqlite://{path}"))
        .map_err(DbError::Sqlx)?
        .create_if_missing(true);

    for (key, value) in &params {
        match key.as_str() {
            "journal_mode" => {
                let mode = SqliteJournalMode::from_str(value).unwrap_or(SqliteJournalMode::Wal);
                options = options.journal_mode(mode);
            }
            "busy_timeout" => {
                if let Ok(ms) = value.parse::<u64>() {
                    options = options.busy_timeout(Duration::from_millis(ms));
                }
            }
            _ => {}
        }
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await?;

    info!("Database: Initialized ({path})");
    Ok(pool)
}

/// `jdbc:sqlite:PATH?k=v&k=v` → (PATH, params). A bare path is accepted too.
fn parse_jdbc_sqlite_url(url: &str) -> Result<(String, Vec<(String, String)>), DbError> {
    if url.starts_with("jdbc:") && !url.starts_with("jdbc:sqlite:") {
        return Err(DbError::UnsupportedUrl(url.to_string()));
    }
    let rest = url.strip_prefix("jdbc:sqlite:").unwrap_or(url);
    let (path, query) = match rest.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (rest, None),
    };
    let params = query
        .map(|q| {
            q.split('&')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        })
        .unwrap_or_default();
    Ok((path.to_string(), params))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jdbc_sqlite_url() {
        let (path, params) =
            parse_jdbc_sqlite_url("jdbc:sqlite:../../interlude_classic.db?journal_mode=WAL&busy_timeout=5000").unwrap();
        assert_eq!(path, "../../interlude_classic.db");
        assert_eq!(params[0], ("journal_mode".into(), "WAL".into()));
        assert_eq!(params[1], ("busy_timeout".into(), "5000".into()));
    }

    #[test]
    fn rejects_non_sqlite() {
        assert!(parse_jdbc_sqlite_url("jdbc:mariadb://localhost/db").is_err());
    }
}
