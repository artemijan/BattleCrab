use std::net::SocketAddr;
use std::sync::Arc;

use dashboard_api::config::DashboardConfig;
use dashboard_api::state::App;

#[tokio::main]
async fn main() {
    // Shares the game server's `Logging.ini` and log directory, distinguished by
    // the `dashboard_api` filename prefix — this binary reads `Dashboard.ini`
    // out of `dist/game/config` too, so it is launched from the same place.
    //
    // Note the `process::exit` paths below bypass this guard, so their last few
    // lines may not reach the *file*. They are startup-fatal and the console
    // layer writes synchronously, so the message still reaches the journal.
    let _log_guard = commons::logging::init("dist/game/", "dashboard_api");
    commons::logging::install_panic_hook();

    // Account creation, password resets and admin actions on other people's
    // accounts are records, so this service gets the never-dropped sink too.
    //
    // Its own directory, NOT the game server's. Both resolve against
    // `dist/game`, so sharing `log/audit` would put two processes on the same
    // NDJSON files — interleaved appends, and worse, two independent retention
    // sweeps deleting each other's rotated files. Overridden here rather than in
    // `Logging.ini` because both services read that same file.
    let mut audit_config = commons::audit::AuditConfig::load("dist/game/");
    audit_config.directory = "log/audit-dashboard".to_string();
    let _audit_guard = commons::audit::init("dist/game/", &audit_config);

    let config = DashboardConfig::load();

    // Refuse rather than generate: a per-boot key silently logs every user out
    // on each deploy, and an absent or weak HMAC key means forgeable session
    // cookies and forgeable password-reset links.
    if let Err(reason) = dashboard_api::config::validate_session_secret(&config.session_secret) {
        eprintln!("FATAL: {reason}");
        std::process::exit(1);
    }

    // Resolve and log the absolute path. `commons::db::init` opens with
    // `create_if_missing(true)`, so a wrong path does not fail — it silently
    // produces an empty database, and every request 500s at runtime instead.
    // Naming the path here, and refusing to boot below, is what makes a
    // misconfigured URL obvious (DASHBOARD.md §10).
    let db_path = dashboard_api::db::sqlite_path(&config.database_url);
    let absolute = db_path.as_ref().map(|p| {
        std::env::current_dir()
            .map(|cwd| cwd.join(p))
            .unwrap_or_else(|_| p.clone())
    });

    match &absolute {
        Some(path) => tracing::info!("opening database {}", path.display()),
        None => tracing::info!("opening database {}", config.database_url),
    }

    // Refuse to create one. If the file is absent the URL is wrong, or the
    // server was started from the wrong working directory.
    if let (Some(path), Some(shown)) = (&db_path, &absolute)
        && !path.exists()
    {
        eprintln!(
            "FATAL: database file does not exist:\n  {}\n\n\
                 dashboard_api will not create one — it must open the SAME SQLite file the \
                 login and game servers use.\n\
                 Run it from the directory that file lives in, or set an absolute path via \
                 DIST_GAME_CONFIG_DASHBOARD_URL.\n\
                 Current working directory: {}",
            shown.display(),
            std::env::current_dir().unwrap_or_default().display()
        );
        std::process::exit(1);
    }

    let db = match commons::db::connect(&config.database_url, config.database_max_connections).await
    {
        Ok(db) => db,
        Err(e) => {
            eprintln!("FATAL: cannot open database: {e}");
            std::process::exit(1);
        }
    };

    // The file existing is not enough — it may be an empty database created by
    // an earlier misconfigured run, which is exactly what produces a stream of
    // "no such table: characters" 500s rather than a startup failure.
    match dashboard_api::db::missing_tables(&db).await {
        Ok(missing) if !missing.is_empty() => {
            eprintln!(
                "FATAL: database is missing required table(s): {}\n  {}\n\n\
                 This is not the game database. The usual cause is an empty file created by a \
                 previous run with a wrong path or working directory.\n\
                 Point DIST_GAME_CONFIG_DASHBOARD_URL at the real interlude_classic.db (the same \
                 one dist/login/config/LoginServer.ini uses), and delete the empty file.",
                missing.join(", "),
                absolute
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| config.database_url.clone()),
            );
            std::process::exit(1);
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("FATAL: cannot inspect database schema: {e}");
            std::process::exit(1);
        }
    }

    let addr: SocketAddr = format!("{}:{}", config.bind_address, config.port)
        .parse()
        .expect("BindAddress/Port must form a valid socket address");

    if !config.public_base_url.starts_with("https://") {
        tracing::warn!(
            "PublicBaseUrl is not https — session cookies will not be marked Secure. \
             Do not run this way in production."
        );
    }

    let state = Arc::new(App::new(db, config));

    if !state.mailer.is_enabled() {
        // Not fatal — the API is fully usable without it — but password reset
        // and email verification silently do nothing useful, so it must not be
        // discoverable only by a user never receiving mail.
        // Name the variables that are actually missing rather than the whole
        // set: "one of these is unset" is the message that sent the last
        // debugging session looking at the wrong one.
        let missing: Vec<&str> = [
            dashboard_api::config::SMTP_HOST_ENV,
            dashboard_api::config::SMTP_USERNAME_ENV,
            dashboard_api::config::SMTP_PASSWORD_ENV,
        ]
        .into_iter()
        .filter(|var| std::env::var(var).unwrap_or_default().trim().is_empty())
        .collect();

        tracing::warn!(
            "email is DISABLED — unset or empty: {}. Password-reset and verification links \
             will be written to this log instead of being sent. Every SMTP setting comes from \
             the environment (systemd EnvironmentFile), never from Dashboard.ini.",
            missing.join(", "),
        );
    }
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
