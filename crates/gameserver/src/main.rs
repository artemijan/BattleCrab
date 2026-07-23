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
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let server_load_start = Instant::now();

    // Where the datapack lives, as a path *prefix*. The process deliberately
    // does NOT chdir into it: everything datapack-relative is addressed through
    // this root, so anything that is not part of the datapack — above all the
    // SQLite `URL`, which must name the same file as the login server's —
    // resolves against the directory the server was actually started in.
    let datapack_root = resolve_datapack_root();

    // Java: Config.load(ServerMode.GAME).
    let config = Config::load_from(&datapack_root);

    print_section("Data");
    let mut data = GameData::load_from(&datapack_root);
    // Stat ceilings + run-speed boost live in Character.ini; fold them into the
    // read-only data bundle the stat finalizers read (`GameData::combat_caps`).
    data.combat_caps = gameserver::data::CombatCaps {
        run_spd_boost: config.character.run_spd_boost,
        max_p_atk: config.character.max_p_atk,
        max_m_atk: config.character.max_m_atk,
        max_p_crit_rate: config.character.max_p_crit_rate,
        max_m_crit_rate: config.character.max_m_crit_rate,
        max_p_atk_speed: config.character.max_p_atk_speed,
        max_m_atk_speed: config.character.max_m_atk_speed,
        max_evasion: config.character.max_evasion,
        max_run_speed: config.character.max_run_speed,
        max_buff_count: config.character.max_buff_count,
        max_dance_count: config.character.max_dance_count,
    };
    // GM login-state / hero-aura settings live in General.ini; fold them into
    // the same data bundle the enter-world flow and UserInfo read.
    data.gm = gameserver::data::GmSettings {
        hero_aura: config.general.gm_hero_aura,
        startup_builder_hide: config.general.gm_startup_builder_hide,
        startup_invulnerable: config.general.gm_startup_invulnerable,
        startup_invisible: config.general.gm_startup_invisible,
        startup_silence: config.general.gm_startup_silence,
        startup_auto_list: config.general.gm_startup_auto_list,
        startup_diet_mode: config.general.gm_startup_diet_mode,
    };
    // Character.ini `EnableModifySkillDuration`/`SkillDurationList`: bake the
    // per-skill `abnormalTime` overrides into the loaded skills (Java does this
    // in the `Skill` constructor). No-op when the list is empty/disabled.
    data.skill_data
        .apply_skill_duration_list(&config.character.skill_duration_list);

    // Java: print_section("Geodata") → GeoEngine.getInstance() (scans
    // GeoDataPath for `{x}_{y}.l2j` regions; missing files just stay null).
    print_section("Geodata");
    let mut geo_engine =
        gameserver::geo::GeoEngine::load(std::path::Path::new(&config.geoengine.geodata_path));
    // Door collision polygons register before the engine is shared — the
    // path worker sees closed doors through the same queries (Java
    // `checkIfDoorsBetween` inside canSeeTarget/canMoveToTarget). The door
    // *entities* spawn on the game thread (`model::door::spawn_doors`).
    for t in &data.door_data.doors {
        geo_engine.doors.register(
            t.id,
            t.node_x,
            t.node_y,
            t.z_min(),
            t.z_max(),
            t.open_by_default,
        );
    }
    let geo = Arc::new(geo_engine);

    // Channels between the network / login-link / DB tasks and the game thread.
    let (net_tx, net_rx) = std::sync::mpsc::channel::<NetEvent>();
    let (login_tx, login_rx) = std::sync::mpsc::channel::<LoginLinkEvent>();
    let (link_tx, link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, db_cmd_rx) = tokio::sync::mpsc::unbounded_channel::<DbCommand>();
    let (db_event_tx, db_rx) = std::sync::mpsc::channel::<DbEvent>();
    let (path_tx, path_req_rx) = std::sync::mpsc::channel();
    let (path_event_tx, path_rx) = std::sync::mpsc::channel();
    // Released by the game thread once all boot data (incl. clans) is loaded,
    // gating the login-link connect so the login server can't route players to
    // a half-populated world (Java: `LoginServerThread.start()` after all data).
    let (login_ready_tx, login_ready_rx) = tokio::sync::oneshot::channel::<()>();

    // Path-worker thread (Java: CellPathFinding, run synchronously there —
    // here the game thread talks to it through the channels above). Shares
    // the read-only geodata; exits when the game thread drops its sender.
    let path_thread = gameserver::geo::worker::spawn(
        geo.clone(),
        config.geoengine.path.clone(),
        path_req_rx,
        path_event_tx,
    );

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
            login_ready_tx,
            db_rx,
            db_tx: db_tx.clone(),
            data,
            geo,
            path_tx,
            path_rx,
            path_finding: config.geoengine.path_finding,
            max_characters_per_account: config.server.max_characters_number_per_account,
            delete_days: config.character.delete_days,
            starting_adena: config.character.starting_adena,
            cfg: config.combat(),
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
    tokio::spawn(loginlink::run(link_cfg, link_rx, login_tx, login_ready_rx));

    // Client connection handler (Java: ConnectionBuilder(...).build().start()).
    let net_cfg = Arc::new(NetworkConfig {
        packet_encryption: config.server.packet_encryption,
        protocol_list: config.server.protocol_list.clone(),
        server_id: config.server_id,
        is_classic: (config.server.server_list_type & 0x400) == 0x400,
    });
    let bind = format!(
        "{}:{}",
        config.server.gameserver_hostname, config.server.port_game
    );
    let listener = TcpListener::bind(&bind).await?;
    tokio::spawn(connection::accept_loop(listener, net_tx, net_cfg));

    info!(
        "GameServer: started in {} seconds. Listening on {bind}. Max online users: {}.",
        server_load_start.elapsed().as_secs(),
        config.server.maximum_online_users
    );

    // Java: JVM shutdown hook -> Shutdown. Also handles SIGTERM (systemd's
    // default stop signal), not just SIGINT — see commons::shutdown.
    commons::shutdown::wait_for_signal().await;

    info!("GameServer: shutting down.");
    shutdown.request();
    // Join the game thread so its final tick (drain + save) completes, then
    // stop the DB thread (which flushes and closes the pool).
    tokio::task::spawn_blocking(move || game_thread.join())
        .await?
        .ok();
    // The game thread's World held the last path-request sender, so the path
    // worker is already unblocking; then flush and stop the DB thread.
    tokio::task::spawn_blocking(move || path_thread.join())
        .await?
        .ok();
    let _ = db_tx.send(DbCommand::Shutdown);
    tokio::task::spawn_blocking(move || db_thread.join())
        .await?
        .ok();
    info!("GameServer: shutdown complete.");
    Ok(())
}

/// Locates the datapack and returns it as a path **prefix** (empty, or ending
/// in `/`) to be joined onto datapack-relative paths.
///
/// Explicitly does not change the working directory. The server used to chdir
/// into `dist/game`, which made every relative path in every ini resolve
/// against the datapack — including `URL`, the SQLite file shared with the
/// login server. That is the wrong default for exactly that one setting: the
/// two servers then need *different* URL strings to name the *same* file, and
/// making them match silently points the game server at its own empty database
/// (SQLite creates rather than fails), which surfaces as "no such table".
///
/// `DATAPACK_ROOT` overrides the search for deployments that keep the datapack
/// somewhere other than beside the binary.
fn resolve_datapack_root() -> String {
    const CONFIG: &str = gameserver::config::server::SERVER_CONFIG_FILE;

    if let Ok(root) = std::env::var("DATAPACK_ROOT") {
        let root = if root.ends_with('/') {
            root
        } else {
            format!("{root}/")
        };
        info!("GameServer: datapack root from DATAPACK_ROOT: {root}");
        return root;
    }
    // Launched from inside the datapack (the deployed layout).
    if std::path::Path::new(CONFIG).exists() {
        return String::new();
    }
    // Launched from the repo root (the development layout).
    if std::path::Path::new("dist/game").join(CONFIG).exists() {
        info!("GameServer: datapack root: dist/game");
        return "dist/game/".to_string();
    }
    // Fall through with an empty root so the config loader reports the missing
    // file against the working directory, which is where the user expects it.
    warn!("GameServer: could not locate {CONFIG}; using the working directory.");
    String::new()
}

fn print_section(section: &str) {
    let mut s = format!("=[ {section} ]");
    while s.len() < 61 {
        s.insert(0, '-');
    }
    info!("{s}");
}
