use std::sync::Arc;

use sqlx::SqlitePool;

use crate::auth::ratelimit::RateLimiter;
use crate::cors::OriginPolicy;
use crate::auth::SigningKey;
use crate::config::DashboardConfig;

pub type AppState = Arc<App>;

pub struct App {
    pub pool: SqlitePool,
    pub config: DashboardConfig,
    pub key: SigningKey,
    /// Which browser origins may call the API (see `cors`).
    pub origin_policy: OriginPolicy,
    pub login_limiter: RateLimiter,
    pub register_limiter: RateLimiter,
    /// Whether to mark cookies `Secure`. Off for plain-HTTP local dev, since a
    /// `Secure` cookie is silently dropped by the browser over http://.
    pub secure_cookies: bool,
}

impl App {
    pub fn new(pool: SqlitePool, config: DashboardConfig) -> Self {
        let key = SigningKey::new(&config.session_secret);
        let origin_policy = OriginPolicy::parse(&config.allowed_origins);
        let login_limiter = RateLimiter::new(config.login_rate_limit, config.login_rate_window_secs);
        // Registration is rarer than login; a tighter budget over a longer
        // window keeps one host from farming accounts.
        let register_limiter = RateLimiter::new(5, 3600);
        let secure_cookies = config.public_base_url.starts_with("https://");
        Self {
            pool,
            config,
            key,
            origin_policy,
            login_limiter,
            register_limiter,
            secure_cookies,
        }
    }
}
