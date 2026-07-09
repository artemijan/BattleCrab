//! Port of `loginserver/LoginServer.java` — bootstrap in the same order
//! (GUI dropped by decision; ThreadPool replaced by the tokio runtime).

mod config;

use config::LoginConfig;
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
    let accounts: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts").fetch_one(&pool).await?;
    info!("Database ready, {} accounts.", accounts.0);

    // M2+: LoginController, GameServerTable, ban file, GS listener, client listener.
    info!(
        "LoginServer scaffold up. Client listener will bind {}:{}, GS listener {}:{}.",
        config.login_bind_address,
        config.port_login,
        config.game_server_login_host,
        config.game_server_login_port
    );

    Ok(())
}
