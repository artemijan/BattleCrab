//! Port of `org.l2jmobius.gameserver.GameServer` — bootstrap in the same order
//! (GUI dropped by decision #10; ThreadPool replaced by the game thread + tokio
//! runtime per CONCURRENCY_MODEL). G0 boots config, DB, and the idle game loop;
//! network/login-link/data subsystems slot into the same order in later
//! milestones.

use std::sync::Arc;
use std::time::Instant;

use gameserver::config::Config;
use gameserver::game_loop::{self, Shutdown};
use gameserver::network::connection::{self, NetworkConfig};
use gameserver::network::NetEvent;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let server_load_start = Instant::now();

    // The game server runs with `dist/game` as its working directory (Java's
    // start scripts cd there) so the ini paths resolve unedited. If launched
    // from the repo root during development, step into it automatically.
    ensure_datapack_cwd();

    // Java: Config.load(ServerMode.GAME).
    let config = Config::load();

    print_section("Database");
    let pool = commons::db::init(&config.server.database_url, config.server.database_max_connections).await?;

    print_section("ThreadPool");
    info!(
        "ThreadPool: game thread + tokio runtime (config sizes scheduled={}, instant={} kept for parity).",
        config.server.scheduled_thread_pool_size, config.server.instant_thread_pool_size
    );

    // The game thread owns World; it runs until shutdown is requested.
    let (net_tx, net_rx) = std::sync::mpsc::channel::<NetEvent>();
    let shutdown = Shutdown::new();
    let game_thread = game_loop::spawn(shutdown.clone(), net_rx);

    // Client connection handler (Java: ConnectionBuilder(...).build().start()).
    let net_cfg = Arc::new(NetworkConfig {
        packet_encryption: config.server.packet_encryption,
        protocol_list: config.server.protocol_list.clone(),
        server_id: config.server.request_id,
        is_classic: (config.server.server_list_type & 0x400) == 0x400,
    });
    let bind = format!("{}:{}", config.server.gameserver_hostname, config.server.port_game);
    let listener = TcpListener::bind(&bind).await?;
    tokio::spawn(connection::accept_loop(listener, net_tx, net_cfg));

    info!(
        "GameServer: started in {} seconds. Listening on {bind}. Max online users: {}.",
        server_load_start.elapsed().as_secs(),
        config.server.maximum_online_users
    );

    // Login-link lands in G2; for now the loop runs idle until ctrl-c
    // (Java: JVM shutdown hook -> Shutdown).
    tokio::signal::ctrl_c().await?;

    info!("GameServer: shutting down.");
    shutdown.request();
    // Join the game thread so its final tick (drain + save) completes.
    tokio::task::spawn_blocking(move || game_thread.join()).await?.ok();
    pool.close().await;
    info!("GameServer: shutdown complete.");
    Ok(())
}

/// Ensure the working directory is the game datapack (`dist/game`). No-op if the
/// config is already reachable (i.e. we were launched from `dist/game`).
fn ensure_datapack_cwd() {
    if std::path::Path::new(gameserver::config::server::SERVER_CONFIG_FILE).exists() {
        return;
    }
    let datapack = std::path::Path::new("dist/game");
    if datapack.join(gameserver::config::server::SERVER_CONFIG_FILE).exists() {
        std::env::set_current_dir(datapack).expect("failed to chdir into dist/game");
        info!("GameServer: working directory set to dist/game.");
    }
}

fn print_section(section: &str) {
    let mut s = format!("=[ {section} ]");
    while s.len() < 61 {
        s.insert(0, '-');
    }
    info!("{s}");
}
