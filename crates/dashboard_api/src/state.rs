use std::sync::Arc;

use models::sea_orm::DatabaseConnection;

use crate::auth::SigningKey;
use crate::auth::ratelimit::RateLimiter;
use crate::config::DashboardConfig;
use crate::cors::OriginPolicy;
use crate::mail::Mailer;
use crate::turnstile::TurnstileVerifier;

pub type AppState = Arc<App>;

pub struct App {
    pub db: DatabaseConnection,
    pub config: DashboardConfig,
    /// Item id → name/type/icon from the datapack, for the character-items
    /// endpoint. Empty when `GameDataDir` is unset or absent.
    pub items: crate::items::Catalog,
    pub key: SigningKey,
    /// Which browser origins may call the API (see `cors`).
    pub origin_policy: OriginPolicy,
    /// Sends verification / reset links. A no-op that logs when SMTP is
    /// unconfigured, so local development needs no mail server.
    pub mailer: Mailer,
    pub login_limiter: RateLimiter,
    pub register_limiter: RateLimiter,
    /// Throttles `/auth/forgot-password`, which sends email: without a cap one
    /// host could bomb an inbox and burn the SES sender reputation.
    pub forgot_limiter: RateLimiter,
    /// Verifies Turnstile tokens. A pass-everything no-op when the secret is
    /// unconfigured, so local development needs no Cloudflare account.
    pub turnstile: TurnstileVerifier,
    /// Whether to mark cookies `Secure`. Off for plain-HTTP local dev, since a
    /// `Secure` cookie is silently dropped by the browser over http://.
    pub secure_cookies: bool,
}

impl App {
    pub fn new(db: DatabaseConnection, config: DashboardConfig) -> Self {
        let key = SigningKey::new(&config.session_secret);
        let mut origin_policy = OriginPolicy::parse(&config.allowed_origins);
        // Debug builds whitelist the local dev server, so an SPA on :3000 can
        // call the API cross-origin without editing AllowedOrigins. Exact
        // origins only — no scheme/port wiggle room — and `cfg!` compiles this
        // out of release builds entirely, so production config stays the only
        // authority there.
        if cfg!(debug_assertions) {
            origin_policy.push_exact("http://localhost:3000");
            origin_policy.push_exact("http://127.0.0.1:3000");
        }
        let mailer = Mailer::from_config(&config);
        let login_limiter =
            RateLimiter::new(config.login_rate_limit, config.login_rate_window_secs);
        // Registration is rarer than login; a tighter budget over a longer
        // window keeps one host from farming accounts.
        let register_limiter = RateLimiter::new(5, 3600);
        // Same shape and rationale as registration: each request sends a mail.
        let forgot_limiter = RateLimiter::new(5, 3600);
        let turnstile = TurnstileVerifier::from_config(&config);
        let secure_cookies = config.public_base_url.starts_with("https://");
        let items = crate::items::Catalog::load(&config.game_data_dir);
        Self {
            db,
            config,
            items,
            key,
            origin_policy,
            mailer,
            login_limiter,
            register_limiter,
            forgot_limiter,
            turnstile,
            secure_cookies,
        }
    }
}
