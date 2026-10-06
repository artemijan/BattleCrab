//! `repo::ip_bans` against a migrated database: an admin's ban replaces
//! whatever was there, an automatic one only ever lengthens it.

use migration::MigratorTrait;
use models::repo::ip_bans::{active_for, ban, ban_at_least};
use models::sea_orm::{Database, DatabaseConnection};

async fn migrated() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    db
}

const NOW: i64 = 1_759_000_000_000;

#[tokio::test]
async fn an_automatic_ban_never_shortens_one_in_force() {
    let db = migrated().await;
    ban(&db, "10.0.0.1", 0, "admin", "admin@example.com", NOW)
        .await
        .unwrap();

    let wrote = ban_at_least(&db, "10.0.0.1", NOW + 900_000, "auto", "login_server", NOW)
        .await
        .unwrap();
    assert!(!wrote, "a permanent ban is not cut to 15 minutes");
    let kept = active_for(&db, "10.0.0.1", NOW).await.unwrap().unwrap();
    assert_eq!(
        (kept.expires_at, kept.banned_by.as_str()),
        (0, "admin@example.com")
    );

    // An expired ban, or none, is (re)placed.
    ban(
        &db,
        "10.0.0.2",
        NOW - 1,
        "old",
        "admin@example.com",
        NOW - 10,
    )
    .await
    .unwrap();
    assert!(
        ban_at_least(&db, "10.0.0.2", NOW + 900_000, "auto", "login_server", NOW)
            .await
            .unwrap()
    );
    assert!(
        ban_at_least(&db, "10.0.0.3", 0, "auto", "game_server", NOW)
            .await
            .unwrap()
    );
    assert_eq!(
        active_for(&db, "10.0.0.2", NOW)
            .await
            .unwrap()
            .unwrap()
            .banned_by,
        "login_server"
    );

    // An admin's ban replaces even a longer one: it is a deliberate edit.
    ban(
        &db,
        "10.0.0.3",
        NOW + 60_000,
        "shorter",
        "admin@example.com",
        NOW,
    )
    .await
    .unwrap();
    assert_eq!(
        active_for(&db, "10.0.0.3", NOW)
            .await
            .unwrap()
            .unwrap()
            .expires_at,
        NOW + 60_000
    );
}
