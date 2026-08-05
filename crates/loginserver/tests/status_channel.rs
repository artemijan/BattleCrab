//! The internal status channel — the dashboard's only accurate source of
//! "is a game server actually up".

mod common;

use std::sync::Arc;

use loginserver::context::LoginContext;
use loginserver::controller::{ControllerSettings, spawn};
use migration::MigratorTrait;
use models::sea_orm::{DatabaseConnection, SqlxSqliteConnector};
use tokio::io::AsyncReadExt;

/// A context with the stock `gameservers` row registered but **no** game server
/// linked — the state after a game server crashes, or before one starts.
async fn ctx_with_no_game_server() -> Arc<LoginContext> {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let db: DatabaseConnection = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
    migration::Migrator::up(&db, None).await.unwrap();
    models::repo::gameservers::register(&db, 1, "-2ad66b3f483c22be097019f55c8abdf0", "")
        .await
        .unwrap();
    let mut gs_table = loginserver::gs_table::GameServerTable::load(&db).await;
    gs_table.server_names.insert(1, "Bartz".to_string());
    let controller = spawn(
        ControllerSettings {
            auto_create_accounts: true,
            login_try_before_ban: 5,
            login_block_after_ban_ms: 900_000,
            show_licence: true,
            accept_new_gameserver: true,
        },
        db.clone(),
        gs_table,
    );
    Arc::new(LoginContext::new(common::test_config(), db, controller))
}

/// Read one line from a freshly-bound status channel, the way the dashboard
/// does: connect, read to EOF, done.
async fn read_channel(ctx: Arc<LoginContext>) -> serde_json::Value {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(loginserver::status_channel::accept_loop(listener, ctx));

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.unwrap();
    serde_json::from_slice(&buf).expect("one line of JSON")
}

/// **A registered-but-unlinked game server reports `up: false`.**
///
/// This is the case the old database-derived status could not see: the row
/// exists (the server is configured and has registered before), but no process
/// is holding the link. `characters.online` would still be set from before the
/// crash; the link is what tells the truth.
#[tokio::test]
async fn a_registered_game_server_with_no_link_is_down() {
    let body = read_channel(ctx_with_no_game_server().await).await;

    // Reaching the channel at all proves the login server is up — that is why
    // the field is constant rather than computed.
    assert_eq!(body["login"], "up");

    let servers = body["servers"].as_array().expect("servers array");
    assert_eq!(servers.len(), 1, "the registered server is still listed");
    assert_eq!(servers[0]["id"], 1);
    assert_eq!(servers[0]["name"], "Bartz");
    assert_eq!(
        servers[0]["up"], false,
        "registered is not running — no live link"
    );
    assert_eq!(servers[0]["players"], 0);
}

/// The field names are a contract with `dashboard_api`'s parser, which defaults
/// every missing field rather than erroring — so a rename there degrades
/// silently to "offline" instead of failing loudly. Pin the names here.
#[tokio::test]
async fn the_snapshot_uses_the_field_names_the_dashboard_reads() {
    let body = read_channel(ctx_with_no_game_server().await).await;
    let server = &body["servers"][0];
    for field in ["id", "name", "up", "players", "maxPlayers"] {
        assert!(
            server.get(field).is_some(),
            "snapshot must carry `{field}` — dashboard_api reads it by name"
        );
    }
}
