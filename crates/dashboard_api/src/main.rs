use std::net::SocketAddr;
use std::sync::Arc;

use dashboard_api::config::DashboardConfig;
use dashboard_api::state::App;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = DashboardConfig::load();

    if config.session_secret.is_empty() {
        // Refuse rather than generate: a per-boot key silently logs every user
        // out on each deploy, and an empty HMAC key is not a secret at all.
        eprintln!(
            "FATAL: SessionSecret is not set. Set it in {} \
             or via DIST_GAME_CONFIG_DASHBOARD_SESSIONSECRET.",
            dashboard_api::config::DASHBOARD_CONFIG_FILE
        );
        std::process::exit(1);
    }

    // Log the resolved path: pointing at a stale copy of the DB silently creates
    // accounts nobody can log in with (PLAN_DASHBOARD.md §10).
    tracing::info!("opening database {}", config.database_url);
    let pool = match commons::db::init(&config.database_url, config.database_max_connections).await {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("FATAL: cannot open database: {e}");
            std::process::exit(1);
        }
    };

    let addr: SocketAddr = format!("{}:{}", config.bind_address, config.port)
        .parse()
        .expect("BindAddress/Port must form a valid socket address");

    if !config.public_base_url.starts_with("https://") {
        tracing::warn!(
            "PublicBaseUrl is not https — session cookies will not be marked Secure. \
             Do not run this way in production."
        );
    }

    let state = Arc::new(App::new(pool, config));
    let app = dashboard_api::app(state);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("FATAL: cannot bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("dashboard listening on http://{addr}");

    // ConnectInfo carries the peer address the rate limiter keys on.
    if let Err(e) = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    {
        tracing::error!("server error: {e}");
        std::process::exit(1);
    }
}
