//! `m20261029_000001_paperdoll_37` against Interlude rows: every equipped item
//! from slot 17 up moves past the five new agathion slots, and nothing else
//! moves.

use sea_orm_migration::MigratorTrait;
use sea_orm_migration::sea_orm::SqlxSqliteConnector;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

async fn rows(pool: &SqlitePool) -> Vec<(i64, String, i64)> {
    sqlx::query("SELECT object_id, loc, loc_data FROM items ORDER BY object_id")
        .fetch_all(pool)
        .await
        .unwrap()
        .iter()
        .map(|r| (r.get("object_id"), r.get("loc"), r.get("loc_data")))
        .collect()
}

#[tokio::test]
async fn interlude_slots_from_17_move_past_the_agathions() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());
    let paperdoll = migration::Migrator::migrations()
        .iter()
        .position(|m| m.name() == "m20261029_000001_paperdoll_37")
        .unwrap() as u32;
    migration::Migrator::up(&db, Some(paperdoll)).await.unwrap();

    // Interlude indices: 16 RBracelet, 17 Deco1, 23 Cloak, 24 Belt,
    // 31 BroochJewel6; a bag item and pet gear carry their own loc_data.
    sqlx::query(
        "INSERT INTO items (owner_id, object_id, item_id, count, enchant_level, loc, loc_data) \
         VALUES (1, 1, 100, 1, 0, 'PAPERDOLL', 16), \
                (1, 2, 101, 1, 0, 'PAPERDOLL', 17), \
                (1, 3, 102, 1, 0, 'PAPERDOLL', 23), \
                (1, 4, 103, 1, 0, 'PAPERDOLL', 24), \
                (1, 5, 104, 1, 0, 'PAPERDOLL', 31), \
                (1, 6, 105, 1, 0, 'INVENTORY', 20), \
                (9, 7, 106, 1, 0, 'PET_EQUIP', 6)",
    )
    .execute(&pool)
    .await
    .unwrap();

    migration::Migrator::up(&db, None).await.unwrap();
    assert_eq!(
        rows(&pool).await,
        [
            (1, "PAPERDOLL".into(), 16),
            (2, "PAPERDOLL".into(), 22),
            (3, "PAPERDOLL".into(), 28),
            (4, "PAPERDOLL".into(), 29),
            (5, "PAPERDOLL".into(), 36),
            (6, "INVENTORY".into(), 20),
            (7, "PET_EQUIP".into(), 6),
        ]
    );

    // An agathion worn on Salvation has nowhere to go on the way down.
    sqlx::query(
        "INSERT INTO items (owner_id, object_id, item_id, count, enchant_level, loc, loc_data) \
         VALUES (1, 8, 107, 1, 0, 'PAPERDOLL', 19)",
    )
    .execute(&pool)
    .await
    .unwrap();
    migration::Migrator::down(&db, Some(1)).await.unwrap();
    assert_eq!(
        rows(&pool).await,
        [
            (1, "PAPERDOLL".into(), 16),
            (2, "PAPERDOLL".into(), 17),
            (3, "PAPERDOLL".into(), 23),
            (4, "PAPERDOLL".into(), 24),
            (5, "PAPERDOLL".into(), 31),
            (6, "INVENTORY".into(), 20),
            (7, "PET_EQUIP".into(), 6),
            (8, "INVENTORY".into(), 0),
        ]
    );
}
