//! Port of `org.l2jmobius.gameserver.GameServer` — bootstrap in the same order
//! (GUI dropped by decision #10; ThreadPool replaced by the game thread + tokio
//! runtime per CONCURRENCY_MODEL). G0 boots config, DB, and the idle game loop;
//! network/login-link/data subsystems slot into the same order in later
//! milestones.

use std::sync::Arc;
use std::time::Instant;

use gameserver::config::Config;
use gameserver::data::GameData;
use gameserver::db::{self, DbCommand, DbEvent};
use gameserver::game_loop::{self, GameThreadChannels, Shutdown};
use gameserver::loginlink::{self, LoginLinkConfig, LoginLinkEvent};
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

    print_section("Data");
    let data = GameData::load();

    // Java: print_section("Geodata") → GeoEngine.getInstance() (scans
    // GeoDataPath for `{x}_{y}.l2j` regions; missing files just stay null).
    print_section("Geodata");
    let geo = gameserver::geo::GeoEngine::load(std::path::Path::new(&config.geoengine.geodata_path));

    // Channels between the network / login-link / DB tasks and the game thread.
    let (net_tx, net_rx) = std::sync::mpsc::channel::<NetEvent>();
    let (login_tx, login_rx) = std::sync::mpsc::channel::<LoginLinkEvent>();
    let (link_tx, link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, db_cmd_rx) = tokio::sync::mpsc::unbounded_channel::<DbCommand>();
    let (db_event_tx, db_rx) = std::sync::mpsc::channel::<DbEvent>();

    print_section("Database");
    let db_thread = db::spawn(
        config.server.database_url.clone(),
        config.server.database_max_connections,
        config.server.max_characters_number_per_account,
        db_cmd_rx,
        db_event_tx,
    );

    print_section("ThreadPool");
    info!(
        "ThreadPool: game thread + DB thread + tokio runtime (config sizes scheduled={}, instant={} kept for parity).",
        config.server.scheduled_thread_pool_size, config.server.instant_thread_pool_size
    );

    // The game thread owns World; it runs until shutdown is requested.
    let shutdown = Shutdown::new();
    let game_thread = game_loop::spawn(
        shutdown.clone(),
        GameThreadChannels {
            net_rx,
            login_rx,
            link_tx: link_tx.clone(),
            db_rx,
            db_tx: db_tx.clone(),
            data,
            geo,
            path_finding: config.geoengine.path_finding,
            max_characters_per_account: config.server.max_characters_number_per_account,
            delete_days: config.character.delete_days,
            starting_adena: config.character.starting_adena,
        },
    );

    // Login-link (Java: LoginServerThread.start()).
    let link_cfg = LoginLinkConfig {
        host: config.server.game_server_login_host.clone(),
        port: config.server.game_server_login_port,
        game_port: config.server.port_game,
        hex_id: config.hex_id.clone(),
        request_id: config.server_id,
        accept_alternate: config.server.accept_alternate_id,
        reserve_host: config.reserve_host_on_login,
        max_players: config.server.maximum_online_users,
        // Subnet→host pairs from ipconfig.xml (or auto-detected), so the login
        // server hands each client the game address for its own network.
        hosts: config.ip_config.pairs(),
        server_list_type: config.server.server_list_type,
        server_list_bracket: config.server.server_list_bracket,
        server_list_age: config.server.server_list_age,
        gmonly: config.server_gmonly,
    };
    tokio::spawn(loginlink::run(link_cfg, link_rx, login_tx));

    // Client connection handler (Java: ConnectionBuilder(...).build().start()).
    let net_cfg = Arc::new(NetworkConfig {
        packet_encryption: config.server.packet_encryption,
        protocol_list: config.server.protocol_list.clone(),
        server_id: config.server_id,
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

    // Java: JVM shutdown hook -> Shutdown.
    tokio::signal::ctrl_c().await?;

    info!("GameServer: shutting down.");
    shutdown.request();
    // Join the game thread so its final tick (drain + save) completes, then
    // stop the DB thread (which flushes and closes the pool).
    tokio::task::spawn_blocking(move || game_thread.join()).await?.ok();
    let _ = db_tx.send(DbCommand::Shutdown);
    tokio::task::spawn_blocking(move || db_thread.join()).await?.ok();
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
