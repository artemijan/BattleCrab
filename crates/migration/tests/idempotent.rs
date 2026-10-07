//! The migrations are the schema's only definition, so the property worth
//! pinning is the one deployments lean on: `up` against an up-to-date
//! database changes nothing and leaves every migration recorded.

use sea_orm_migration::MigratorTrait;
use sea_orm_migration::sea_orm::SqlxSqliteConnector;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

/// Every table with its `PRAGMA table_info`, as comparable text.
async fn snapshot(pool: &SqlitePool) -> Vec<(String, Vec<String>)> {
    let tables: Vec<String> = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type='table' \
         AND name NOT LIKE 'sqlite_%' AND name <> 'seaql_migrations' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .iter()
    .map(|r| r.get::<String, _>("name"))
    .collect();
    let mut out = Vec::new();
    for table in tables {
        let columns = sqlx::query(sqlx::AssertSqlSafe(format!(
            "PRAGMA table_info(\"{table}\")"
        )))
        .fetch_all(pool)
        .await
        .unwrap()
        .iter()
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
        .collect();
        out.push((table, columns));
    }
    out
}

#[tokio::test]
async fn up_twice_is_a_no_op() {
    // One connection: every extra connection to `:memory:` would get its own
    // empty database.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());

    migration::Migrator::up(&db, None).await.unwrap();
    let before = snapshot(&pool).await;
    assert!(!before.is_empty(), "the migrations created no tables");

    migration::Migrator::up(&db, None).await.unwrap();
    assert_eq!(
        before,
        snapshot(&pool).await,
        "a second `up` changed the schema"
    );

    let applied: i64 = sqlx::query("SELECT COUNT(*) AS n FROM seaql_migrations")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("n");
    assert_eq!(
        applied,
        migration::Migrator::migrations().len() as i64,
        "every migration should be recorded as applied"
    );
}
