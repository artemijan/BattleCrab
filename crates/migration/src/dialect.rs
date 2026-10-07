//! What the baselines' column types and defaults become on each backend.
//!
//! The baselines keep the Java installer's MySQL-flavoured types (`TINYINT`,
//! `MEDIUMINT`, `decimal(20,0)`, …). SQLite accepts any spelling and only
//! derives an affinity from it, so on SQLite they go through verbatim — the
//! live databases were built from exactly these strings.
//!
//! PostgreSQL rejects several of them outright, and its driver is strict where
//! SQLite is loose: an `i32` field cannot read a `BIGINT`, an `i64` cannot read
//! an `INTEGER`, and a `String` cannot read a `TIMESTAMP`. So on PostgreSQL each
//! type maps to the one the entities read it as (`crates/models`), which
//! `models/tests/postgres_schema.rs` checks column by column:
//!
//! | declared                                  | PostgreSQL         | entity    |
//! |-------------------------------------------|--------------------|-----------|
//! | `tinyint` `smallint` `mediumint` `int` `boolean` | `INTEGER`   | `i32`     |
//! | `bigint`                                  | `BIGINT`           | `i64`     |
//! | `decimal(p)`, `decimal(p,0)`              | `INTEGER` (p ≤ 9), else `BIGINT` | `i32`/`i64` |
//! | `double`, `double(p,s)`                   | `DOUBLE PRECISION` | `f64`     |
//! | `char(n)`, `varchar(n)`                   | `VARCHAR(n)`       | `String`  |
//! | `text`, `tinytext`                        | `TEXT`             | `String`  |
//! | `timestamp`, `date`                       | `TEXT`             | `String`  |
//! | `varbinary(n)`                            | `BYTEA`            | `Vec<u8>` |
//!
//! `char(n)` becomes `VARCHAR(n)` because PostgreSQL pads a `CHAR` with spaces
//! and hands the padding back — an IP address read from `lastIP` would no
//! longer equal the one written.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

fn is_postgres(manager: &SchemaManager<'_>) -> bool {
    manager.get_database_backend() == DatabaseBackend::Postgres
}

/// The column type `declared` becomes on `manager`'s backend.
pub(crate) fn ty(manager: &SchemaManager<'_>, declared: &str) -> Alias {
    if !is_postgres(manager) {
        return Alias::new(declared);
    }
    Alias::new(postgres_type(declared))
}

/// [`ty`] for a column the Java server writes fractions into although its
/// DDL says integer (`characters.curHp` and kin — `models::value::LooseF64`).
/// SQLite keeps such a value REAL anyway; on PostgreSQL the column is
/// `DOUBLE PRECISION`, so the fraction survives there too.
pub(crate) fn ty_loose_f64(manager: &SchemaManager<'_>, declared: &str) -> Alias {
    if !is_postgres(manager) {
        return Alias::new(declared);
    }
    Alias::new("DOUBLE PRECISION")
}

/// The PostgreSQL spelling of a declared type. Panics on a type the table
/// above does not cover: a new one needs a deliberate mapping, not a guess.
pub(crate) fn postgres_type(declared: &str) -> String {
    let lower = declared.trim().to_ascii_lowercase();
    let (base, args) = match lower.split_once('(') {
        Some((b, a)) => (b.trim(), Some(a.trim_end_matches(')').trim())),
        None => (lower.as_str(), None),
    };
    match base {
        "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "boolean" => "INTEGER".into(),
        "bigint" => "BIGINT".into(),
        "decimal" | "numeric" => {
            let (precision, scale) = match args.map(|a| a.split_once(',').unwrap_or((a, "0"))) {
                Some((p, s)) => (p.trim().parse::<u32>().ok(), s.trim()),
                None => (None, "0"),
            };
            assert!(
                scale == "0",
                "decimal with a fractional scale has no integer mapping: {declared}"
            );
            match precision {
                Some(p) if p <= 9 => "INTEGER".into(),
                _ => "BIGINT".into(),
            }
        }
        "double" | "real" | "float" => "DOUBLE PRECISION".into(),
        "char" | "varchar" => match args {
            Some(n) => format!("VARCHAR({n})"),
            None => "TEXT".into(),
        },
        "text" | "tinytext" | "mediumtext" | "longtext" | "timestamp" | "date" | "datetime" => {
            "TEXT".into()
        }
        "varbinary" | "blob" => "BYTEA".into(),
        _ => panic!("no PostgreSQL mapping for column type {declared:?}"),
    }
}

/// A column default, as written in the baselines, on `manager`'s backend.
///
/// On PostgreSQL: `CURRENT_TIMESTAMP` lands in a `TEXT` column, so it is
/// formatted the way SQLite writes it (`2026-10-07 12:00:00`, UTC); `FALSE`
/// lands in an `INTEGER` one; and a `0x` literal is spelled in decimal, which
/// every PostgreSQL version reads.
pub(crate) fn dflt(manager: &SchemaManager<'_>, raw: &'static str) -> Expr {
    if !is_postgres(manager) {
        return Expr::cust(raw);
    }
    Expr::cust(postgres_default(raw))
}

pub(crate) fn postgres_default(raw: &str) -> String {
    match raw.trim() {
        "CURRENT_TIMESTAMP" => {
            "to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS')".to_string()
        }
        "FALSE" => "0".to_string(),
        "TRUE" => "1".to_string(),
        hex if hex.starts_with("0x") || hex.starts_with("0X") => i64::from_str_radix(&hex[2..], 16)
            .unwrap_or_else(|_| panic!("bad hex default {raw:?}"))
            .to_string(),
        other => other.to_string(),
    }
}

/// One master account per address, case-insensitively — a partial unique
/// index neither backend's spelling of which sea-query can express.
pub(crate) fn master_email_index(manager: &SchemaManager<'_>) -> &'static str {
    if is_postgres(manager) {
        "CREATE UNIQUE INDEX IF NOT EXISTS accounts_master_email \
         ON accounts (lower(email)) WHERE login IS NULL"
    } else {
        "CREATE UNIQUE INDEX IF NOT EXISTS `accounts_master_email` \
         ON `accounts` (`email` COLLATE NOCASE) WHERE `login` IS NULL"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_the_baselines_declare_has_a_postgres_spelling() {
        for (declared, pg) in [
            ("TINYINT", "INTEGER"),
            ("mediumint", "INTEGER"),
            ("INT", "INTEGER"),
            ("boolean", "INTEGER"),
            ("bigint", "BIGINT"),
            ("decimal(3)", "INTEGER"),
            ("decimal(5,0)", "INTEGER"),
            ("decimal(11)", "BIGINT"),
            ("decimal(20,0)", "BIGINT"),
            ("double", "DOUBLE PRECISION"),
            ("double(10,2)", "DOUBLE PRECISION"),
            ("CHAR(15)", "VARCHAR(15)"),
            ("varchar(45)", "VARCHAR(45)"),
            ("tinytext", "TEXT"),
            ("timestamp", "TEXT"),
            ("date", "TEXT"),
            ("varbinary(2176)", "BYTEA"),
        ] {
            assert_eq!(postgres_type(declared), pg, "{declared}");
        }
    }

    #[test]
    #[should_panic(expected = "fractional scale")]
    fn a_fractional_decimal_is_refused_not_guessed() {
        postgres_type("decimal(30,15)");
    }

    #[test]
    fn defaults_that_postgres_reads_differently_are_rewritten() {
        assert!(postgres_default("CURRENT_TIMESTAMP").starts_with("to_char(now()"));
        assert_eq!(postgres_default("FALSE"), "0");
        assert_eq!(postgres_default("0xECF9A2"), "15530402");
        assert_eq!(postgres_default("'0'"), "'0'");
        assert_eq!(postgres_default("NULL"), "NULL");
    }
}
