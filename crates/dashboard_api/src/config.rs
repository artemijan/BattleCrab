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

/// The **entire** SMTP configuration is environment-only — not just the
/// credentials.
///
/// The secrets have to be, because `Dashboard.ini` is committed. The host, port
/// and from-address moved out for a different reason: the deploy script used to
/// write them into the remote ini with `sed "s|^SmtpHost.*|...|"`, which exits
/// 0 having changed nothing when the key is absent. A remote ini seeded before
/// those keys existed therefore never received them, and mail stayed silently
/// disabled while the deploy reported success. With no ini key to miss, the
/// value either arrives in the environment or is visibly absent at boot.
///
/// With Amazon SES the username/password are **SES SMTP credentials** (SES
/// console -> SMTP settings -> Create SMTP credentials), not AWS access keys.
/// SES derives the SMTP password from a secret access key; pasting the access
/// key itself is a common and confusing failure.
pub const SMTP_HOST_ENV: &str = "DASHBOARD_SMTP_HOST";
pub const SMTP_PORT_ENV: &str = "DASHBOARD_SMTP_PORT";
pub const SMTP_FROM_ENV: &str = "DASHBOARD_SMTP_FROM";
pub const SMTP_USERNAME_ENV: &str = "DASHBOARD_SMTP_USERNAME";
pub const SMTP_PASSWORD_ENV: &str = "DASHBOARD_SMTP_PASSWORD";

/// Every SMTP variable, for the boot diagnostics. Kept in one place so a newly
/// added one cannot be left out of the "what is missing" report.
pub const SMTP_ENV_VARS: [&str; 5] = [
    SMTP_HOST_ENV,
    SMTP_PORT_ENV,
    SMTP_FROM_ENV,
    SMTP_USERNAME_ENV,
    SMTP_PASSWORD_ENV,
];

/// Ini keys that used to configure SMTP. Presence now means a stale config that
/// is being silently ignored, which is worth saying out loud.
pub const OBSOLETE_SMTP_INI_KEYS: [&str; 6] = [
    "SmtpHost",
    "SmtpPort",
    "SmtpFrom",
    "SmtpUsername",
    "SmtpPassword",
    "SmtpUser",
];

/// Default envelope sender, used when `DASHBOARD_SMTP_FROM` is unset.
const DEFAULT_SMTP_FROM: &str = "BattleCrab <no-reply@battlecrab.com>";

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

        // Any SMTP key left in the ini is now dead weight. For the credentials
        // it also means one may have been committed at some point; for the rest
        // it means someone is editing a file the server no longer reads, and
        // wondering why nothing changes.
        for key in OBSOLETE_SMTP_INI_KEYS {
            if p.contains_key(key) {
                tracing::error!(
                    "{} defines {key} — it is IGNORED. All SMTP settings come from the \
                     environment now ({}). Remove the key{}",
                    DASHBOARD_CONFIG_FILE,
                    SMTP_ENV_VARS.join(", "),
                    if key.contains("assword") || key.contains("ser") {
                        ", and rotate the credential if it was ever committed."
                    } else {
                        "."
                    },
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

            // SMTP is environment-only, all of it — see SMTP_HOST_ENV.
            smtp_host: std::env::var(SMTP_HOST_ENV).unwrap_or_default(),
            // A malformed port is not silently coerced to the default: that
            // would turn a typo into a connection failure against the wrong
            // port, diagnosed nowhere.
            smtp_port: match std::env::var(SMTP_PORT_ENV) {
                Ok(v) if !v.trim().is_empty() => v.trim().parse().unwrap_or_else(|_| {
                    tracing::error!("${SMTP_PORT_ENV} is not a port number: {v:?} — using 587");
                    587
                }),
                _ => 587,
            },
            smtp_from: std::env::var(SMTP_FROM_ENV)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_SMTP_FROM.to_string()),
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

    /// The shipped ini must define no SMTP key at all.
    ///
    /// This is the actual regression: the deploy script rewrote these keys with
    /// `sed "s|^SmtpHost.*|...|"`, which changes nothing and still exits 0 when
    /// the key is absent from the remote copy — so mail stayed disabled while
    /// the deploy reported success. Re-adding a key here would revive that
    /// whole failure mode, and would look like it works locally.
    #[test]
    fn the_shipped_ini_defines_no_smtp_keys() {
        let ini = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/config/Dashboard.ini"
        ))
        .expect("dist/game/config/Dashboard.ini should be readable");

        for line in ini.lines() {
            let line = line.trim();
            // Comments are where these keys are *documented*, which is fine.
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            for key in OBSOLETE_SMTP_INI_KEYS {
                assert!(
                    !line.starts_with(key),
                    "Dashboard.ini still defines {key} ({line:?}) — SMTP is environment-only; \
                     see {} ",
                    SMTP_ENV_VARS.join(", "),
                );
            }
        }
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
