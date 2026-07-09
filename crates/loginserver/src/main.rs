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
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Load Config (Java: Config.load(ServerMode.LOGIN)).
    let config = LoginConfig::load();

    // Prepare the database (Java: DatabaseFactory.init()).
    let pool = commons::db::init(&config.database_url, config.database_max_connections).await?;

    // LoginController.load(): RSA + Blowfish key caches + the state actor.
    info!("Loading LoginController...");
    let controller = loginserver::controller::spawn(
        loginserver::controller::ControllerSettings {
            auto_create_accounts: config.auto_create_accounts,
            login_try_before_ban: config.login_try_before_ban,
            login_block_after_ban_ms: config.login_block_after_ban as i64 * 1000,
        },
        pool.clone(),
    );
    let bind = format!("{}:{}", config.login_bind_address, config.port_login);
    let ctx = Arc::new(LoginContext::new(config, pool, controller.clone()));

    loginserver::ban_file::load(&controller).await;

    // M4: GS listener.

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    info!("LoginServer: is now listening on: {bind}");
    tokio::spawn(network::client_connection::accept_loop(ctx, listener));

    tokio::signal::ctrl_c().await?;
    info!("LoginServer: shutting down.");
    Ok(())
}
