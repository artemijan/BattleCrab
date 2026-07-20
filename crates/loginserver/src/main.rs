//! Port of `loginserver/LoginServer.java` — bootstrap in the same order
//! (GUI dropped by decision; ThreadPool replaced by the tokio runtime).

use std::sync::Arc;

use loginserver::config::LoginConfig;
use loginserver::context::LoginContext;
use loginserver::network;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Load Config (Java: Config.load(ServerMode.LOGIN)).
    let config = LoginConfig::load();

    // Prepare the database (Java: DatabaseFactory.init()).
    let pool = commons::db::init(&config.database_url, config.database_max_connections).await?;

    // GameServerTable: server names + registered servers.
    let gs_table = loginserver::gs_table::GameServerTable::load(&pool).await;

    // LoginController.load(): RSA + Blowfish key caches + the state actor.
    info!("Loading LoginController...");
    let controller = loginserver::controller::spawn(
        loginserver::controller::ControllerSettings {
            auto_create_accounts: config.auto_create_accounts,
            login_try_before_ban: config.login_try_before_ban,
            login_block_after_ban_ms: config.login_block_after_ban as i64 * 1000,
            show_licence: config.show_licence,
            accept_new_gameserver: config.accept_new_gameserver,
        },
        pool.clone(),
        gs_table,
    );
    let bind = format!("{}:{}", config.login_bind_address, config.port_login);
    let gs_host = if config.game_server_login_host == "*" { "0.0.0.0" } else { &config.game_server_login_host };
    let gs_bind = format!("{}:{}", gs_host, config.game_server_login_port);
    let ctx = Arc::new(LoginContext::new(config, pool, controller.clone()));

    loginserver::ban_file::load(&controller).await;

    // GameServerListener.
    let gs_listener = tokio::net::TcpListener::bind(&gs_bind).await?;
    info!("Listening for GameServers on {gs_bind}");
    tokio::spawn(loginserver::gs_link::listener::accept_loop(ctx.clone(), gs_listener));

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    info!("LoginServer: is now listening on: {bind}");
    tokio::spawn(network::client_connection::accept_loop(ctx.clone(), listener));

    // Scheduled LS restart (Java: ThreadPool.schedule(() -> shutdown(true))).
    // Exit code 2 = restart request, honored by a wrapper/orchestrator.
    let restart = async {
        if ctx.config.login_server_schedule_restart {
            let hours = ctx.config.login_server_schedule_restart_time;
            info!("Scheduled LS restart after {hours} hours.");
            tokio::time::sleep(std::time::Duration::from_secs(hours as u64 * 3600)).await;
            true
        } else {
            std::future::pending().await
        }
    };

    let restart_requested = tokio::select! {
        _ = commons::shutdown::wait_for_signal() => false,
        r = restart => r,
    };

    info!("LoginServer: shutting down.");
    if ctx.config.backup_database {
        backup_database(&ctx.config.database_url, &ctx.config.backup_path);
    }
    ctx.pool.close().await;
    if restart_requested {
        std::process::exit(2);
    }
    Ok(())
}

/// `DatabaseBackup.performBackup` for SQLite: timestamped file copy.
fn backup_database(jdbc_url: &str, backup_path: &str) {
    let path = jdbc_url.strip_prefix("jdbc:sqlite:").unwrap_or(jdbc_url);
    let path = path.split('?').next().unwrap_or(path);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let file_name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    let target_dir = std::path::Path::new(backup_path);
    let target = target_dir.join(format!("{file_name}.{timestamp}.bak"));
    if let Err(e) = std::fs::create_dir_all(target_dir).and_then(|_| std::fs::copy(path, &target).map(|_| ())) {
        tracing::warn!("Database backup failed ({}): {e}", target.display());
    } else {
        info!("Database backed up to {}", target.display());
    }
}
