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

/// The only place the session signing key is read from.
///
/// A dedicated variable rather than `PropertiesParser`'s path-derived override
/// (`DIST_GAME_CONFIG_DASHBOARD_SESSIONSECRET`), because this value must not be
/// settable from the config file at all — the point is that there is no
/// file-shaped path for it to leak through.
pub const SESSION_SECRET_ENV: &str = "DASHBOARD_SESSION_SECRET";

/// Minimum accepted length for the signing key.
///
/// 32 hex chars is 128 bits, the floor for an HMAC key that guards session
/// cookies. `openssl rand -hex 32` produces 64.
pub const MIN_SESSION_SECRET_LEN: usize = 32;

/// SMTP credentials, environment-only for the same reason as the session key:
/// `Dashboard.ini` is committed, so no secret may live in it.
///
/// With Amazon SES these are **SES SMTP credentials** (SES console -> SMTP
/// settings -> Create SMTP credentials), not AWS access keys. SES derives the
/// SMTP password from a secret access key; pasting the access key itself is a
/// common and confusing failure.
pub const SMTP_USERNAME_ENV: &str = "DASHBOARD_SMTP_USERNAME";
pub const SMTP_PASSWORD_ENV: &str = "DASHBOARD_SMTP_PASSWORD";

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

    /// Browser origins allowed to call this API with credentials, as the raw
    /// config string. Parsed by `cors::OriginPolicy`, which is where the
    /// matching rules and their tests live.
    ///
    /// A bare domain means that domain and its subdomains over HTTPS; an entry
    /// with a scheme is matched exactly. Never a wildcard: with
    /// `Access-Control-Allow-Credentials` browsers reject `*`, and accepting
    /// arbitrary origins would let any site drive a logged-in user's account.
    pub allowed_origins: String,

    /// Must point at the *same* SQLite file the login/game servers use — a
    /// stale copy would silently create accounts nobody can log in with.
    pub database_url: String,
    pub database_max_connections: u32,

    /// HMAC key for session cookies and password-reset / email-verification
    /// tokens.
    ///
    /// Read from the `DASHBOARD_SESSION_SECRET` environment variable **only**,
    /// never from `Dashboard.ini`. That file is committed to the repository, so
    /// a secret placed in it is one `git add` away from living in history
    /// forever — and from being copied to every clone and CI runner.
    ///
    /// Must also be stable across restarts: regenerating it invalidates every
    /// session and every outstanding reset link.
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

    /// SES SMTP endpoint, e.g. `email-smtp.eu-west-1.amazonaws.com`. Empty
    /// disables email entirely (links are logged instead — see `mail`).
    pub smtp_host: String,
    /// 587/25/2587 use STARTTLS; 465/2465 are TLS from the first byte. `mail`
    /// picks the right mode from this number.
    pub smtp_port: u16,
    /// Envelope sender. Must be an identity verified in SES, in production as
    /// well as in the sandbox.
    pub smtp_from: String,
    /// From `DASHBOARD_SMTP_USERNAME` — never the config file.
    pub smtp_username: String,
    /// From `DASHBOARD_SMTP_PASSWORD` — never the config file.
    pub smtp_password: String,
}

impl DashboardConfig {
    pub fn load() -> Self {
        let p = PropertiesParser::load(DASHBOARD_CONFIG_FILE);

        // A secret in the committed ini is ignored, but its presence means one
        // may already have been committed — say so loudly rather than silently
        // doing the right thing.
        for key in ["SmtpUsername", "SmtpPassword", "SmtpUser"] {
            if p.contains_key(key) {
                tracing::error!(
                    "{} defines {key} — it is IGNORED (SMTP credentials come from ${} / ${} only). \
                     Remove the key, and rotate the credential if it was ever committed.",
                    DASHBOARD_CONFIG_FILE,
                    SMTP_USERNAME_ENV,
                    SMTP_PASSWORD_ENV,
                );
            }
        }

        if p.contains_key("SessionSecret") {
            tracing::error!(
                "{} defines SessionSecret — it is IGNORED (the secret comes from ${} only). \
                 Remove the key, and rotate the value if it was ever committed.",
                DASHBOARD_CONFIG_FILE,
                SESSION_SECRET_ENV,
            );
        }

        Self {
            bind_address: p.get_string("BindAddress", "0.0.0.0"),
            port: p.get_int("Port", 8080) as u16,
            public_base_url: p.get_string("PublicBaseUrl", "http://localhost:8080"),
            site_base_url: p.get_string("SiteBaseUrl", "https://battlecrab.com"),
            allowed_origins: p.get_string("AllowedOrigins", "battlecrab.com"),

            // Key names match `LoginServer.ini` (`URL`,
            // `MaximumDatabaseConnections`) so both servers are configured the
            // same way and the value can be copied across verbatim.
            database_url: p.get_string(
                "URL",
                "jdbc:sqlite:interlude_classic.db?journal_mode=WAL&busy_timeout=5000",
            ),
            database_max_connections: p.get_int("MaximumDatabaseConnections", 5).max(1) as u32,

            // Deliberately NOT p.get_string: the value must never be readable
            // from the committed config file. See `SESSION_SECRET_ENV`.
            session_secret: std::env::var(SESSION_SECRET_ENV).unwrap_or_default(),
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

            smtp_host: p.get_string("SmtpHost", ""),
            smtp_port: p.get_int("SmtpPort", 587) as u16,
            smtp_from: p.get_string("SmtpFrom", "BattleCrab <no-reply@battlecrab.com>"),
            // Secrets: environment only, like the session key.
            smtp_username: std::env::var(SMTP_USERNAME_ENV).unwrap_or_default(),
            smtp_password: std::env::var(SMTP_PASSWORD_ENV).unwrap_or_default(),
        }
    }
}


/// Why a session secret is unusable, if it is.
///
/// Checked at startup and fatal: booting with a weak or absent signing key
/// would mean forgeable session cookies and forgeable password-reset links.
pub fn validate_session_secret(secret: &str) -> Result<(), String> {
    if secret.is_empty() {
        return Err(format!(
            "${SESSION_SECRET_ENV} is not set.\n\n\
             The session signing key is read from the environment only — never from \
             {DASHBOARD_CONFIG_FILE}, which is committed to the repository.\n\n\
             Generate one:    openssl rand -hex 32\n\
             Then export it:  {SESSION_SECRET_ENV}=<value>\n\n\
             It must stay the same across restarts; changing it invalidates every session \
             and every outstanding password-reset link."
        ));
    }

    if secret.chars().count() < MIN_SESSION_SECRET_LEN {
        return Err(format!(
            "${SESSION_SECRET_ENV} is too short ({} chars, minimum {MIN_SESSION_SECRET_LEN}).\n\
             A short key can be brute-forced, which would let an attacker forge session \
             cookies and password-reset links.\n\n\
             Generate one:    openssl rand -hex 32",
            secret.chars().count(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod secret_tests {
    use super::*;

    #[test]
    fn rejects_an_empty_secret() {
        let err = validate_session_secret("").unwrap_err();
        assert!(err.contains(SESSION_SECRET_ENV));
        // The message has to say how to produce one, not just that it is wrong.
        assert!(err.contains("openssl rand -hex 32"));
    }

    #[test]
    fn rejects_a_short_secret() {
        assert!(validate_session_secret("tooshort").is_err());
        assert!(validate_session_secret(&"a".repeat(MIN_SESSION_SECRET_LEN - 1)).is_err());
    }

    #[test]
    fn accepts_a_secret_at_or_above_the_floor() {
        assert!(validate_session_secret(&"a".repeat(MIN_SESSION_SECRET_LEN)).is_ok());
        // What `openssl rand -hex 32` actually emits.
        assert!(validate_session_secret(&"0123456789abcdef".repeat(4)).is_ok());
    }

    #[test]
    fn counts_characters_not_bytes() {
        // A multi-byte string long enough in bytes but not in characters must
        // still be rejected — otherwise `len()` would wave it through.
        let short_but_multibyte = "é".repeat(MIN_SESSION_SECRET_LEN - 1);
        assert!(short_but_multibyte.len() >= MIN_SESSION_SECRET_LEN);
        assert!(validate_session_secret(&short_but_multibyte).is_err());
    }
}
