//! Dashboard config, read from `dist/game/config/Dashboard.ini` through the
//! same `PropertiesParser` the login and game servers use.
//!
//! Every key can be overridden by an environment variable, which is how secrets
//! get injected in Docker/helm. `PropertiesParser` derives the variable name
//! from the *config file path*, not from the crate name — so the prefix here is
//! `DIST_GAME_CONFIG_DASHBOARD_`, e.g.
//! `DIST_GAME_CONFIG_DASHBOARD_SESSIONSECRET`. Moving this file changes the
//! variable names.

use commons::config::PropertiesParser;

pub const DASHBOARD_CONFIG_FILE: &str = "dist/game/config/Dashboard.ini";

pub struct DashboardConfig {
    pub bind_address: String,
    pub port: u16,

    /// The API's own public origin (e.g. `https://api.battlecrab.com`).
    /// Only decides whether session cookies get the `Secure` flag.
    pub public_base_url: String,

    /// Where the SPA is served (e.g. `https://battlecrab.com`).
    ///
    /// Distinct from `public_base_url` because password-reset and email
    /// verification links must land on *frontend* routes; pointing them at the
    /// API host would hand the user a URL with no page behind it.
    pub site_base_url: String,

    /// Browser origins allowed to call this API with credentials.
    ///
    /// Must be exact origins — with `Access-Control-Allow-Credentials` a
    /// wildcard is rejected by every browser, and accepting arbitrary origins
    /// would let any site drive a logged-in user's account.
    pub allowed_origins: Vec<String>,

    /// Must point at the *same* SQLite file the login/game servers use — a
    /// stale copy would silently create accounts nobody can log in with.
    pub database_url: String,
    pub database_max_connections: u32,

    /// HMAC key for session cookies and stateless tokens. Must be stable across
    /// restarts (a per-boot key logs everyone out on every deploy) and must come
    /// from the environment in production.
    pub session_secret: String,
    pub session_ttl_days: i64,

    pub registration_enabled: bool,
    pub min_password_length: usize,
    pub max_password_length: usize,
    pub max_login_length: usize,

    /// Login attempts per account/IP before the limiter starts rejecting.
    /// Load-bearing: the password hash is unsalted SHA-1 (see PLAN_DASHBOARD.md
    /// §5.2), so throttling is the primary defence against online guessing.
    pub login_rate_limit: u32,
    pub login_rate_window_secs: u64,
}

impl DashboardConfig {
    pub fn load() -> Self {
        let p = PropertiesParser::load(DASHBOARD_CONFIG_FILE);
        Self {
            bind_address: p.get_string("BindAddress", "0.0.0.0"),
            port: p.get_int("Port", 8080) as u16,
            public_base_url: p.get_string("PublicBaseUrl", "http://localhost:8080"),
            site_base_url: p.get_string("SiteBaseUrl", "http://localhost:3000"),
            allowed_origins: parse_origins(&p.get_string(
                "AllowedOrigins",
                "http://localhost:3000,http://127.0.0.1:3000",
            )),

            // Key names match `LoginServer.ini` (`URL`,
            // `MaximumDatabaseConnections`) so both servers are configured the
            // same way and the value can be copied across verbatim.
            database_url: p.get_string(
                "URL",
                "jdbc:sqlite:interlude_classic.db?journal_mode=WAL&busy_timeout=5000",
            ),
            database_max_connections: p.get_int("MaximumDatabaseConnections", 5).max(1) as u32,

            session_secret: p.get_string("SessionSecret", ""),
            session_ttl_days: p.get_long("SessionTtlDays", 7),

            registration_enabled: p.get_bool("RegistrationEnabled", true),
            min_password_length: p.get_int("MinPasswordLength", 8) as usize,
            // The client's login box caps out well before the column does; see
            // PLAN_DASHBOARD.md §12 open question 5 — confirm against the real
            // client before launch.
            max_password_length: p.get_int("MaxPasswordLength", 45) as usize,
            max_login_length: p.get_int("MaxLoginLength", 45) as usize,

            login_rate_limit: p.get_int("LoginRateLimit", 10) as u32,
            login_rate_window_secs: p.get_long("LoginRateWindowSecs", 300) as u64,
        }
    }
}

/// Splits and normalises the `AllowedOrigins` list.
///
/// An origin is scheme + host + port and nothing else, so a trailing slash or a
/// path makes the browser's comparison fail — silently, as a CORS error with no
/// server-side trace. Trim them here rather than making that a deployment
/// puzzle.
fn parse_origins(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|origin| origin.trim().trim_end_matches('/'))
        .filter(|origin| !origin.is_empty())
        .map(|origin| origin.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_origins;

    #[test]
    fn splits_and_trims_origins() {
        assert_eq!(
            parse_origins("https://battlecrab.com, https://www.battlecrab.com"),
            vec!["https://battlecrab.com", "https://www.battlecrab.com"]
        );
    }

    #[test]
    fn strips_trailing_slashes_and_blanks() {
        // "https://x.com/" never matches a browser Origin header.
        assert_eq!(
            parse_origins("https://battlecrab.com/,, "),
            vec!["https://battlecrab.com"]
        );
    }

    #[test]
    fn empty_config_yields_no_origins() {
        assert!(parse_origins("").is_empty());
        assert!(parse_origins("  ").is_empty());
    }
}
