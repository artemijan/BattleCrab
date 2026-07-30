//! The migrations must reproduce `dist/db_installer/sql/sqlite/**` exactly.
//!
//! That tree is authoritative (CLAUDE.md): it is what the Java installer
//! applies and what every live database was built from. The baseline migrations
//! are a transcription of it, and a transcription of 100 tables is exactly the
//! kind of thing that goes wrong quietly — one missing `NOT NULL`, one default
//! dropped — and only surfaces as a constraint error months later.
//!
//! So: build one database from the migrations, one from the dist DDL, and
//! compare them column by column, index by index.

use sea_orm_migration::MigratorTrait;
use sea_orm_migration::sea_orm::SqlxSqliteConnector;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

/// `PRAGMA table_info` for one table, as comparable text.
async fn columns(pool: &SqlitePool, table: &str) -> Vec<String> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "PRAGMA table_info(\"{table}\")"
    )))
    .fetch_all(pool)
    .await
    .unwrap();
    rows.iter()
        .map(|r| {
            format!(
                "{} {} notnull={} default={:?} pk={}",
                r.get::<String, _>("name"),
                r.get::<String, _>("type").to_uppercase(),
                r.get::<i64, _>("notnull"),
                r.get::<Option<String>, _>("dflt_value"),
                r.get::<i64, _>("pk"),
            )
        })
        .collect()
}

/// Every index on one table, as comparable text. Auto-indexes are included:
/// a `UNIQUE` constraint that went missing must fail this test even though it
/// has no `CREATE INDEX` of its own.
async fn indexes(pool: &SqlitePool, table: &str) -> Vec<String> {
    let list = sqlx::query(sqlx::AssertSqlSafe(format!(
        "PRAGMA index_list(\"{table}\")"
    )))
    .fetch_all(pool)
    .await
    .unwrap();
    let mut out = Vec::new();
    for row in list {
        let name: String = row.get("name");
        let cols = sqlx::query(sqlx::AssertSqlSafe(format!(
            "PRAGMA index_info(\"{name}\")"
        )))
        .fetch_all(pool)
        .await
        .unwrap()
        .iter()
        .map(|r| r.get::<Option<String>, _>("name").unwrap_or_default())
        .collect::<Vec<_>>()
        .join(",");
        // The generated name of an auto-index is an implementation detail
        // (`sqlite_autoindex_<table>_<n>`); what matters is that the same
        // columns are constrained the same way.
        let named = if name.starts_with("sqlite_autoindex_") {
            String::from("<auto>")
        } else {
            name
        };
        out.push(format!(
            "{named} unique={} partial={} ({cols})",
            row.get::<i64, _>("unique"),
            row.get::<i64, _>("partial"),
        ));
    }
    out.sort();
    out
}

async fn tables(pool: &SqlitePool) -> Vec<String> {
    sqlx::query(
        "SELECT name FROM sqlite_master WHERE type='table' \
         AND name NOT LIKE 'sqlite_%' AND name <> 'seaql_migrations' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .iter()
    .map(|r| r.get::<String, _>("name"))
    .collect()
}

async fn memory_pool() -> SqlitePool {
    // One connection: every extra connection to `:memory:` would get its own
    // empty database.
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap()
}

/// Applies the migrations to a fresh in-memory database.
async fn migrated() -> SqlitePool {
    let pool = memory_pool().await;
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());
    migration::Migrator::up(&db, None).await.unwrap();
    pool
}

/// Applies the dist DDL to a fresh in-memory database, the way the installer
/// does: every `.sql` file, login first.
async fn from_dist() -> SqlitePool {
    let pool = memory_pool().await;
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/db_installer/sql/sqlite"
    );
    for dir in ["login", "game"] {
        let mut files: Vec<_> = std::fs::read_dir(format!("{root}/{dir}"))
            .unwrap_or_else(|e| panic!("cannot read {root}/{dir}: {e}"))
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "sql"))
            .collect();
        files.sort();
        for file in files {
            // Whole file at once: these `.sql` files carry trailing comments
            // with semicolons in them, so splitting on `;` mangles them.
            let sql = std::fs::read_to_string(&file).unwrap();
            sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
                .execute(&pool)
                .await
                .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        }
    }
    pool
}

#[tokio::test]
async fn migrations_reproduce_the_dist_schema() {
    let migrated = migrated().await;
    let dist = from_dist().await;

    let migrated_tables = tables(&migrated).await;
    let dist_tables = tables(&dist).await;
    assert_eq!(
        migrated_tables, dist_tables,
        "table set differs between the migrations and dist/db_installer"
    );

    for table in &dist_tables {
        assert_eq!(
            columns(&migrated, table).await,
            columns(&dist, table).await,
            "column definitions differ for `{table}`"
        );
        assert_eq!(
            indexes(&migrated, table).await,
            indexes(&dist, table).await,
            "indexes differ for `{table}`"
        );
    }
}

/// `up` must be a no-op against a database that already has the schema — that
/// is what lets a live deployment adopt the migrations without a rewrite.
#[tokio::test]
async fn up_is_idempotent_on_an_existing_database() {
    let pool = from_dist().await;
    let before = {
        let mut snapshot = Vec::new();
        for table in tables(&pool).await {
            snapshot.push((table.clone(), columns(&pool, &table).await));
        }
        snapshot
    };

    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());
    migration::Migrator::up(&db, None).await.unwrap();

    let mut after = Vec::new();
    for table in tables(&pool).await {
        after.push((table.clone(), columns(&pool, &table).await));
    }
    assert_eq!(
        before, after,
        "`up` changed an already-provisioned database"
    );

    // And it recorded itself, so a second run has nothing to do.
    let applied: i64 = sqlx::query("SELECT COUNT(*) AS n FROM seaql_migrations")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("n");
    assert_eq!(applied, 3, "every migration should be recorded as applied");
}
