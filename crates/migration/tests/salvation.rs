//! `m20261028_000001_salvation` against a database that already holds
//! Interlude rows: the two rebuilt tables keep their data, and the grand boss
//! seed never overwrites a live boss's state.

use sea_orm_migration::MigratorTrait;
use sea_orm_migration::sea_orm::SqlxSqliteConnector;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

async fn column(pool: &SqlitePool, table: &str, name: &str) -> (String, i64, Option<String>) {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "PRAGMA table_info(\"{table}\")"
    )))
    .fetch_all(pool)
    .await
    .unwrap()
    .iter()
    .find(|r| r.get::<String, _>("name") == name)
    .map(|r| {
        (
            r.get::<String, _>("type"),
            r.get::<i64, _>("notnull"),
            r.get::<Option<String>, _>("dflt_value"),
        )
    })
    .unwrap_or_else(|| panic!("no column {table}.{name}"))
}

#[tokio::test]
async fn an_interlude_database_keeps_its_rows() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());
    let salvation = migration::Migrator::migrations().len() as u32 - 1;
    migration::Migrator::up(&db, Some(salvation)).await.unwrap();

    for sql in [
        "INSERT INTO character_shortcuts (charId, slot, page, type, shortcut_id, level, class_index) \
         VALUES (1, 0, 0, 2, 56, NULL, 0), (1, 1, 0, 2, 3, 7, 0)",
        "INSERT INTO buffer_schemes (object_id, scheme_name, skills) VALUES (1, 'pve', '1204,1040')",
        // Queen Ant mid-respawn on the live server.
        "INSERT INTO grandboss_data (boss_id, loc_x, loc_y, loc_z, heading, respawn_time, \
         currentHP, currentMP, status) VALUES (29001, 1, 2, 3, 0, 999, 10.5, 1.0, 3)",
    ] {
        sqlx::query(sql).execute(&pool).await.unwrap();
    }

    migration::Migrator::up(&db, None).await.unwrap();

    assert_eq!(
        column(&pool, "character_shortcuts", "level").await,
        ("MEDIUMINT".into(), 1, Some("0".into()))
    );
    let levels: Vec<i64> = sqlx::query("SELECT level FROM character_shortcuts ORDER BY slot")
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .map(|r| r.get("level"))
        .collect();
    assert_eq!(
        levels,
        [0, 7],
        "a NULL level reads as 0, as Java's getInt did"
    );
    let indexed: i64 = sqlx::query(
        "SELECT COUNT(*) AS n FROM sqlite_master WHERE type = 'index' AND name = 'shortcut_id'",
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .get("n");
    assert_eq!(indexed, 1, "the rebuild restores the shortcut_id index");

    assert_eq!(
        column(&pool, "buffer_schemes", "skills").await.0,
        "VARCHAR(512)"
    );
    let skills: String = sqlx::query("SELECT skills FROM buffer_schemes")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("skills");
    assert_eq!(skills, "1204,1040");

    let queen: (i64, i64) =
        sqlx::query("SELECT status, respawn_time FROM grandboss_data WHERE boss_id = 29001")
            .fetch_one(&pool)
            .await
            .map(|r| (r.get("status"), r.get("respawn_time")))
            .unwrap();
    assert_eq!(queen, (3, 999), "the seed leaves a live boss row alone");
    let seeded: i64 = sqlx::query("SELECT COUNT(*) AS n FROM grandboss_data")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("n");
    assert_eq!(seeded, 13, "Salvation's 13 rows");
}
