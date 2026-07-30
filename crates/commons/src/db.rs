//! Port of `commons/database/DatabaseFactory.java`, SQLite-only per decision #9.
//!
//! Accepts the JDBC-style URL from the existing config files
//! (`jdbc:sqlite:../../interlude_classic.db?journal_mode=WAL&busy_timeout=5000`)
//! so `LoginServer.ini` keeps working unchanged.

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tracing::info;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("unsupported database URL `{0}` — only SQLite is supported")]
    UnsupportedUrl(String),
    #[error(transparent)]
    Open(#[from] OpenError),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("cannot open SQLite database at `{resolved}` (from URL `{url}`): {reason}")]
pub struct OpenError {
    pub url: String,
    pub resolved: String,
    pub reason: String,
}

/// Directory the running executable lives in, which is what a relative database
/// path is resolved against. Falls back to the working directory only if the
/// platform cannot report the executable's location.
pub fn executable_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

/// The ORM handle every consumer should ask for.
///
/// Wraps the pool [`init`] builds rather than calling `Database::connect`, so
/// the JDBC prefix, `journal_mode`/`busy_timeout` parameters and
/// executable-relative path resolution keep working exactly as they did — those
/// behaviours have tests below and are the reason one URL string serves every
/// binary.
pub async fn connect(
    jdbc_url: &str,
    max_connections: u32,
) -> Result<sea_orm::DatabaseConnection, DbError> {
    let pool = init(jdbc_url, max_connections).await?;
    Ok(sea_orm::SqlxSqliteConnector::from_sqlx_sqlite_pool(pool))
}

pub async fn init(jdbc_url: &str, max_connections: u32) -> Result<SqlitePool, DbError> {
    let (path, params) = parse_jdbc_sqlite_url(jdbc_url)?;

    // A relative path is resolved against the **executable's** directory, not
    // the working directory.
    //
    // All three binaries deploy alongside the database, so this makes one URL
    // string correct for every one of them and independent of how the unit was
    // started — the login and game servers previously had to disagree about
    // the string to name the same file, and a `WorkingDirectory` change was
    // enough to silently point a server at a different database.
    let resolved = if std::path::Path::new(&path).is_absolute() {
        std::path::PathBuf::from(&path)
    } else {
        executable_dir().join(&path)
    };

    // Fail clearly when the parent directory is missing: SQLite's "code 14" is
    // unhelpfully vague about this.
    if let Some(parent) = resolved.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        return Err(DbError::Open(OpenError {
            url: jdbc_url.to_string(),
            resolved: resolved.display().to_string(),
            reason: format!("parent directory {} does not exist", parent.display()),
        }));
    }

    // `filename` rather than parsing a `sqlite://` URL: the resolved path is
    // absolute and may contain spaces or `?`/`#`, which URL parsing would
    // mangle or treat as query separators.
    let mut options = SqliteConnectOptions::new()
        .filename(&resolved)
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
        .await
        .map_err(|e| OpenError {
            url: jdbc_url.to_string(),
            resolved: resolved.display().to_string(),
            reason: e.to_string(),
        })?;

    // Canonicalize for the log only: `current_exe` can report the path as it
    // was invoked, so the join reads like `dist/game/../../target/debug/x.db`
    // — technically correct, and useless when you are trying to work out which
    // file the server actually opened.
    let shown = std::fs::canonicalize(&resolved).unwrap_or_else(|_| resolved.clone());
    info!("Database: Initialized ({})", shown.display());
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
        let (path, params) = parse_jdbc_sqlite_url(
            "jdbc:sqlite:../../interlude_classic.db?journal_mode=WAL&busy_timeout=5000",
        )
        .unwrap();
        assert_eq!(path, "../../interlude_classic.db");
        assert_eq!(params[0], ("journal_mode".into(), "WAL".into()));
        assert_eq!(params[1], ("busy_timeout".into(), "5000".into()));
    }

    #[test]
    fn rejects_non_sqlite() {
        assert!(parse_jdbc_sqlite_url("jdbc:mariadb://localhost/db").is_err());
    }

    /// A relative database path must follow the executable, not the working
    /// directory — that is the whole point of resolving it this way, and it is
    /// what lets one URL string serve the login server, game server and
    /// dashboard no matter which directory their unit files start them in.
    #[tokio::test]
    async fn a_relative_path_opens_next_to_the_executable() {
        let exe_dir = executable_dir();
        let name = format!("commons_db_test_{}.db", std::process::id());
        let expected = exe_dir.join(&name);
        let _ = std::fs::remove_file(&expected);

        // Under `cargo test` the working directory is the crate root while the
        // test binary lives in target/…/deps, so the two are already different
        // and a cwd-relative implementation would put the file in the crate
        // root. Deliberately does NOT chdir: that is process-global state and
        // would race the other tests in this binary.
        let cwd = std::env::current_dir().unwrap();
        assert_ne!(cwd, exe_dir, "test needs cwd and exe dir to differ");
        let _ = std::fs::remove_file(cwd.join(&name));

        let pool = init(&format!("jdbc:sqlite:{name}"), 1).await.unwrap();
        drop(pool);

        assert!(
            expected.exists(),
            "expected the database beside the executable at {}",
            expected.display()
        );
        assert!(
            !cwd.join(&name).exists(),
            "must not have been created in the working directory"
        );
        let _ = std::fs::remove_file(&expected);
    }

    #[tokio::test]
    async fn an_absolute_path_is_left_alone() {
        let dir = std::env::temp_dir().join(format!("commons_db_abs_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("abs.db");

        let pool = init(&format!("jdbc:sqlite:{}", file.display()), 1)
            .await
            .unwrap();
        drop(pool);

        assert!(
            file.exists(),
            "absolute paths must not be re-rooted at the executable"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
