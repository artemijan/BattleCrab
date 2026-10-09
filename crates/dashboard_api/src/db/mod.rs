//! Data access against the live game database (SQLite or PostgreSQL) — one
//! pool, four tables.
//!
//! `accounts` is writable in exactly two columns for players — plus, for the
//! admin surface only, `accessLevel` restricted to values ≤ 0 (see `admin`).
//! `characters` and `items` are read-only. `ip_bans` is admin-only and belongs
//! to the dashboard (see `ip_bans`). See DASHBOARD.md §5.5 and §16.

pub mod accounts;
pub mod admin;
pub mod characters;
pub mod ip_bans;
pub mod items;

use std::path::PathBuf;

use models::sea_orm::{ConnectionTrait, DatabaseBackend, Statement, Value};

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

/// A raw statement for `db`'s backend, written once for both.
///
/// The SQL must be portable: identifiers in double quotes (both backends read
/// them), no backend-only syntax, and `?` placeholders — which become `$1`,
/// `$2`, … on PostgreSQL. A `?` inside a single-quoted literal is left alone.
pub fn portable<C: ConnectionTrait>(db: &C, sql: &str, values: Vec<Value>) -> Statement {
    let backend = db.get_database_backend();
    let sql = match backend {
        DatabaseBackend::Postgres => numbered_placeholders(sql),
        _ => sql.to_string(),
    };
    Statement::from_sql_and_values(backend, sql, values)
}

fn numbered_placeholders(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len() + 8);
    let mut n = 0;
    let mut in_literal = false;
    for c in sql.chars() {
        match c {
            '\'' => {
                in_literal = !in_literal;
                out.push(c);
            }
            '?' if !in_literal => {
                n += 1;
                out.push('$');
                out.push_str(&n.to_string());
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_numbered_for_postgres_outside_literals() {
        assert_eq!(
            numbered_placeholders("a = ? AND b LIKE ? ESCAPE '\\' AND c = '?' AND d = ?"),
            "a = $1 AND b LIKE $2 ESCAPE '\\' AND c = '?' AND d = $3"
        );
    }

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
    async fn reports_missing_required_tables() {
        let db = models::sea_orm::Database::connect("sqlite::memory:")
            .await
            .unwrap();
        assert_eq!(
            commons::db::missing_tables(&db, &REQUIRED_TABLES)
                .await
                .unwrap(),
            vec!["accounts", "characters"]
        );

        db.execute_unprepared("CREATE TABLE accounts (login TEXT)")
            .await
            .unwrap();
        assert_eq!(
            commons::db::missing_tables(&db, &REQUIRED_TABLES)
                .await
                .unwrap(),
            vec!["characters"]
        );

        db.execute_unprepared("CREATE TABLE characters (char_name TEXT)")
            .await
            .unwrap();
        assert!(
            commons::db::missing_tables(&db, &REQUIRED_TABLES)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
