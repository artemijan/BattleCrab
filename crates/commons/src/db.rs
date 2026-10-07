//! Port of `commons/database/DatabaseFactory.java`: SQLite or PostgreSQL,
//! chosen by the URL (`docs/DATABASE.md`).
//!
//! Accepts the JDBC-style URLs the config files carry —
//! `jdbc:sqlite:interlude_classic.db?journal_mode=WAL&busy_timeout=5000` or
//! `jdbc:postgresql://10.0.0.9:5432/l2?user=l2` — as well as plain
//! `postgres://` URLs, so one `URL` line in each `.ini` decides the backend for
//! every binary.
//!
//! SQLite is one file on one machine; PostgreSQL is what lets the login server,
//! game server and dashboard run on different machines.
//!
//! A PostgreSQL password does not belong in a committed `.ini`: when the URL
//! carries none, [`PASSWORD_ENV`] supplies it.

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tracing::info;

/// The PostgreSQL password, when the URL does not carry one. Environment only,
/// like the dashboard's other secrets: every `.ini` is committed.
pub const PASSWORD_ENV: &str = "L2_DATABASE_PASSWORD";

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("unsupported database URL `{0}` — use jdbc:sqlite:… or jdbc:postgresql://…")]
    UnsupportedUrl(String),
    #[error(transparent)]
    Open(#[from] OpenError),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("cannot open database `{resolved}` (from URL `{url}`): {reason}")]
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

/// Which database a URL names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Sqlite,
    Postgres,
}

impl Backend {
    /// `jdbc:postgresql:`, `postgres:` and `postgresql:` are PostgreSQL; a
    /// `jdbc:sqlite:` URL or a bare path is SQLite; any other `jdbc:` driver
    /// is refused.
    pub fn of(url: &str) -> Result<Self, DbError> {
        if url.starts_with("jdbc:postgresql:")
            || url.starts_with("postgres:")
            || url.starts_with("postgresql:")
        {
            Ok(Self::Postgres)
        } else if url.starts_with("jdbc:") && !url.starts_with("jdbc:sqlite:") {
            Err(DbError::UnsupportedUrl(url.to_string()))
        } else {
            Ok(Self::Sqlite)
        }
    }
}

/// The ORM handle every consumer should ask for.
///
/// Builds the pool itself rather than calling `Database::connect`, so the
/// JDBC prefix, SQLite's `journal_mode`/`busy_timeout` parameters,
/// executable-relative paths and the password from the environment work the
/// same in every binary — those behaviours have tests below and are the reason
/// one URL string serves them all.
pub async fn connect(
    jdbc_url: &str,
    max_connections: u32,
) -> Result<sea_orm::DatabaseConnection, DbError> {
    match Backend::of(jdbc_url)? {
        Backend::Sqlite => {
            let pool = init(jdbc_url, max_connections).await?;
            Ok(sea_orm::SqlxSqliteConnector::from_sqlx_sqlite_pool(pool))
        }
        Backend::Postgres => {
            let pool = init_postgres(jdbc_url, max_connections).await?;
            Ok(sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(
                pool,
            ))
        }
    }
}

/// Which of `tables` the database behind `db` lacks.
///
/// Opening a SQLite URL creates a missing file, and a PostgreSQL URL may name
/// an empty database: either way the connection succeeds and every query
/// fails later. Checking the tables a binary cannot run without turns that
/// into one clear error at boot.
pub async fn missing_tables<'a, C: sea_orm::ConnectionTrait>(
    db: &C,
    tables: &[&'a str],
) -> Result<Vec<&'a str>, sea_orm::DbErr> {
    let backend = db.get_database_backend();
    let sql = match backend {
        sea_orm::DatabaseBackend::Postgres => {
            "SELECT 1 FROM information_schema.tables \
             WHERE table_schema = current_schema() AND table_name = $1"
        }
        _ => "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?",
    };
    let mut missing = Vec::new();
    for table in tables {
        let found = db
            .query_one_raw(sea_orm::Statement::from_sql_and_values(
                backend,
                sql,
                [(*table).into()],
            ))
            .await?;
        if found.is_none() {
            missing.push(*table);
        }
    }
    Ok(missing)
}

/// `jdbc:postgresql://host[:port]/db?user=…` → connect options, the password
/// from [`PASSWORD_ENV`] when the URL has none.
fn postgres_options(jdbc_url: &str) -> Result<PgConnectOptions, DbError> {
    let url = jdbc_url.strip_prefix("jdbc:").unwrap_or(jdbc_url);
    // sqlx reads `postgres://`; JDBC writes `postgresql://`.
    let url = match url.strip_prefix("postgresql:") {
        Some(rest) => format!("postgres:{rest}"),
        None => url.to_string(),
    };
    let options = PgConnectOptions::from_str(&url).map_err(|e| OpenError {
        url: redact(jdbc_url),
        resolved: redact(jdbc_url),
        reason: e.to_string(),
    })?;
    // No prepared-statement cache. sqlx keys it by SQL text alone, so a
    // statement first run with an `i64` parameter is reused for the next run of
    // the same SQL even when that one binds an `i32` — and PostgreSQL rejects
    // the 4-byte value against the 8-byte parameter ("insufficient data left
    // in message"). The game code binds integer literals of either width for
    // the same column in places (a character's `deletetime` is set from an
    // `i64` and cleared with `0`), and SQLite never minded. Uncached, each
    // statement is prepared with the types actually bound, and PostgreSQL
    // converts between integer widths itself.
    let options = options.statement_cache_capacity(0);
    let has_password = url.contains("password=") || url_userinfo_has_password(&url);
    Ok(match std::env::var(PASSWORD_ENV) {
        Ok(password) if !has_password && !password.is_empty() => options.password(&password),
        _ => options,
    })
}

/// `postgres://user:secret@host/…` carries a password in its userinfo.
fn url_userinfo_has_password(url: &str) -> bool {
    url.split_once("://")
        .and_then(|(_, rest)| rest.split_once('@'))
        .is_some_and(|(userinfo, _)| userinfo.contains(':'))
}

/// The URL with any password replaced, for messages and logs.
pub fn redact(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    let (head, query) = match url.split_once('?') {
        Some((h, q)) => (h, Some(q)),
        None => (url, None),
    };
    match head.split_once("://").and_then(|(scheme, rest)| {
        rest.split_once('@')
            .map(|(userinfo, host)| (scheme, userinfo, host))
    }) {
        Some((scheme, userinfo, host)) if userinfo.contains(':') => {
            let user = userinfo.split(':').next().unwrap_or("");
            out.push_str(&format!("{scheme}://{user}:***@{host}"));
        }
        _ => out.push_str(head),
    }
    if let Some(query) = query {
        let params: Vec<String> = query
            .split('&')
            .map(|kv| match kv.split_once('=') {
                Some(("password", _)) => "password=***".to_string(),
                _ => kv.to_string(),
            })
            .collect();
        out.push('?');
        out.push_str(&params.join("&"));
    }
    out
}

pub async fn init_postgres(jdbc_url: &str, max_connections: u32) -> Result<sqlx::PgPool, DbError> {
    let options = postgres_options(jdbc_url)?;
    let shown = format!(
        "postgres://{}@{}:{}/{}",
        options.get_username(),
        options.get_host(),
        options.get_port(),
        options.get_database().unwrap_or("")
    );
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await
        .map_err(|e| OpenError {
            url: redact(jdbc_url),
            resolved: shown.clone(),
            reason: e.to_string(),
        })?;
    info!("Database: Initialized ({shown})");
    Ok(pool)
}

/// Opens a SQLite database. [`connect`] is what servers use; this is the pool
/// for the code that needs SQLite itself.
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
    if Backend::of(url)? != Backend::Sqlite {
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

/// Databases for tests in every crate: a temporary SQLite file by default, or
/// a scratch database on the PostgreSQL server [`testing::ENV`] names. Setting
/// that variable runs the same suites against PostgreSQL.
#[doc(hidden)]
pub mod testing {
    use sea_orm::ConnectionTrait;

    /// e.g. `postgres://l2:l2@localhost:55432/l2` — a server the tests may
    /// create and drop databases on.
    pub const ENV: &str = "L2R_TEST_DATABASE_URL";

    /// One empty database, for one test. Migrate it and use [`TestDb::url`];
    /// it is removed when dropped.
    pub struct TestDb {
        pub url: String,
        place: Place,
    }

    enum Place {
        Dir(std::path::PathBuf),
        Postgres { admin_url: String, name: String },
    }

    impl TestDb {
        pub async fn new(tag: &str) -> Self {
            let unique = format!(
                "{tag}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            match std::env::var(ENV) {
                Ok(admin_url) if !admin_url.is_empty() => {
                    let name = format!("l2r_test_{unique}").to_lowercase();
                    let admin = sea_orm::Database::connect(admin_url.as_str())
                        .await
                        .unwrap_or_else(|e| panic!("{ENV}: {e}"));
                    admin
                        .execute_unprepared(&format!("CREATE DATABASE {name}"))
                        .await
                        .unwrap();
                    admin.close().await.ok();
                    let url = with_database(&admin_url, &name);
                    Self {
                        url,
                        place: Place::Postgres { admin_url, name },
                    }
                }
                _ => {
                    let dir = std::env::temp_dir().join(format!("l2r_{unique}"));
                    std::fs::create_dir_all(&dir).unwrap();
                    Self {
                        url: format!("jdbc:sqlite:{}", dir.join("test.db").display()),
                        place: Place::Dir(dir),
                    }
                }
            }
        }

        pub fn is_postgres(&self) -> bool {
            matches!(self.place, Place::Postgres { .. })
        }

        /// Removes it now rather than at the end of the scope.
        pub async fn remove(self) {
            drop(self);
        }
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            match &self.place {
                Place::Dir(dir) => {
                    let _ = std::fs::remove_dir_all(dir);
                }
                // `Drop` cannot await, and may run inside a runtime that
                // cannot be blocked on, so the drop runs on a thread of its
                // own with a runtime of its own. `WITH (FORCE)` ends any
                // connection a test left open.
                Place::Postgres { admin_url, name } => {
                    let (admin_url, name) = (admin_url.clone(), name.clone());
                    let _ = std::thread::spawn(move || {
                        let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                        else {
                            return;
                        };
                        rt.block_on(async {
                            if let Ok(admin) = sea_orm::Database::connect(admin_url.as_str()).await
                            {
                                let _ = admin
                                    .execute_unprepared(&format!(
                                        "DROP DATABASE IF EXISTS {name} WITH (FORCE)"
                                    ))
                                    .await;
                                admin.close().await.ok();
                            }
                        });
                    })
                    .join();
                }
            }
        }
    }

    /// `admin_url` with its database name replaced by `name`.
    fn with_database(admin_url: &str, name: &str) -> String {
        let (base, query) = match admin_url.split_once('?') {
            Some((b, q)) => (b, format!("?{q}")),
            None => (admin_url, String::new()),
        };
        let after_scheme = base.find("://").map(|i| i + 3).unwrap_or(0);
        let server = match base[after_scheme..].find('/') {
            Some(i) => &base[..after_scheme + i],
            None => base,
        };
        format!("{server}/{name}{query}")
    }

    #[cfg(test)]
    #[test]
    fn the_database_name_is_swapped_and_the_rest_kept() {
        assert_eq!(
            with_database("postgres://l2:pw@db:5432/l2?sslmode=disable", "t1"),
            "postgres://l2:pw@db:5432/t1?sslmode=disable"
        );
        assert_eq!(with_database("postgres://db", "t1"), "postgres://db/t1");
    }
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
    fn rejects_other_drivers() {
        assert!(parse_jdbc_sqlite_url("jdbc:mariadb://localhost/db").is_err());
        assert!(Backend::of("jdbc:mariadb://localhost/db").is_err());
    }

    #[test]
    fn the_url_picks_the_backend() {
        assert_eq!(Backend::of("jdbc:sqlite:x.db").unwrap(), Backend::Sqlite);
        assert_eq!(Backend::of("x.db").unwrap(), Backend::Sqlite);
        for pg in [
            "jdbc:postgresql://10.0.0.9:5432/l2?user=l2",
            "postgres://l2@db/l2",
            "postgresql://db/l2",
        ] {
            assert_eq!(Backend::of(pg).unwrap(), Backend::Postgres, "{pg}");
        }
    }

    #[test]
    fn a_jdbc_postgres_url_reads_host_database_and_user() {
        let o = postgres_options("jdbc:postgresql://10.0.0.9:5433/l2db?user=game").unwrap();
        assert_eq!(
            (
                o.get_host(),
                o.get_port(),
                o.get_database(),
                o.get_username()
            ),
            ("10.0.0.9", 5433, Some("l2db"), "game")
        );
    }

    #[test]
    fn passwords_never_reach_a_message() {
        assert_eq!(
            redact("jdbc:postgresql://db/l2?user=l2&password=hunter2&sslmode=require"),
            "jdbc:postgresql://db/l2?user=l2&password=***&sslmode=require"
        );
        assert_eq!(
            redact("postgres://l2:hunter2@db:5432/l2"),
            "postgres://l2:***@db:5432/l2"
        );
        assert_eq!(
            redact("jdbc:sqlite:x.db?journal_mode=WAL"),
            "jdbc:sqlite:x.db?journal_mode=WAL"
        );
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
