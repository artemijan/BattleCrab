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
    tokio::spawn(network::client_connection::accept_loop(ctx, listener));

    tokio::signal::ctrl_c().await?;
    info!("LoginServer: shutting down.");
    Ok(())
}
