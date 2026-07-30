pub mod account;
pub mod admin;
pub mod auth;
pub mod status;

use axum::Router;
use axum::http::HeaderMap;

use crate::auth::cookie;
use crate::config::DashboardConfig;
use crate::db::accounts::{self, Account};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/account", account::router())
        .nest("/admin", admin::router())
        .merge(status::router())
}

/// Resolves the session cookie to the master account that owns the session.
///
/// Re-reads the account on every request so the cookie's signature can be
/// checked against the *current* password hash — that check is what makes a
/// password change invalidate outstanding sessions (PLAN_DASHBOARD.md §5.3).
///
/// The lookup goes through `find_master_by_email`, so a game account can never
/// become the subject of a session even if one is forged into the cookie.
pub async fn current_account(app: &AppState, headers: &HeaderMap) -> ApiResult<Account> {
    let raw = cookie::extract(headers).ok_or(ApiError::Unauthorized)?;

    // The address inside the cookie is untrusted until the HMAC verifies; it is
    // used only to find which account's hash to verify against.
    let hint = raw.split('|').next().unwrap_or_default();
    if hint.is_empty() {
        return Err(ApiError::Unauthorized);
    }

    let account = accounts::find_master_by_email(&app.db, hint)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    cookie::authenticate(&app.key, &raw, &account.password).ok_or(ApiError::Unauthorized)?;

    // A banned master's sessions die here, on the next request — which is the
    // instant revocation the signed-cookie design otherwise lacks, and it works
    // exactly because the account is re-read every time.
    if account.access_level < 0 {
        return Err(ApiError::Unauthorized);
    }

    Ok(account)
}

/// `current_account`, plus the admin gate: the session's master account must
/// carry `accessLevel >= AdminAccessLevel`.
///
/// The check runs per request against the *stored* level, so demoting an admin
/// takes effect on their next call — there is no admin flag baked into the
/// cookie to go stale.
pub async fn require_admin(app: &AppState, headers: &HeaderMap) -> ApiResult<Account> {
    let account = current_account(app, headers).await?;
    if account.access_level < app.config.admin_access_level {
        return Err(ApiError::Forbidden);
    }
    Ok(account)
}

/// Account names must survive the round trip into the game client's login box.
/// Restricting to ASCII alphanumerics keeps us inside what the client accepts;
/// see PLAN_DASHBOARD.md §12 open question 5 — this needs confirming against a
/// real client before launch, and is deliberately conservative until then.
pub fn validate_login(login: &str, config: &DashboardConfig) -> ApiResult<String> {
    let normalized = accounts::normalize_login(login);

    if normalized.len() < 4 {
        return Err(ApiError::BadRequest(
            "username must be at least 4 characters".into(),
        ));
    }
    if normalized.len() > config.max_login_length {
        return Err(ApiError::BadRequest(format!(
            "username must be at most {} characters",
            config.max_login_length
        )));
    }
    if !normalized.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(ApiError::BadRequest(
            "username may contain only letters and digits".into(),
        ));
    }
    Ok(normalized)
}

/// Length is the meaningful lever here: the stored hash is unsalted SHA-1
/// (PLAN_DASHBOARD.md §5.2), so a short password is cheap to crack offline if
/// the table ever leaks.
pub fn validate_password(password: &str, config: &DashboardConfig) -> ApiResult<()> {
    if password.len() < config.min_password_length {
        return Err(ApiError::BadRequest(format!(
            "password must be at least {} characters",
            config.min_password_length
        )));
    }
    if password.len() > config.max_password_length {
        return Err(ApiError::BadRequest(format!(
            "password must be at most {} characters",
            config.max_password_length
        )));
    }
    if !password.is_ascii() {
        // The client's login box is not UTF-8 friendly; a non-ASCII password
        // would hash fine here and then be untypeable in the game.
        return Err(ApiError::BadRequest(
            "password must use ASCII characters only".into(),
        ));
    }
    Ok(())
}

/// Shape check only — real validation is still "the verification mail arrived".
/// It is deliberately not an RFC 5322 parser; the goal is to reject typos and
/// non-addresses (the address is now the account identity), not to litigate
/// exotic-but-legal forms.
///
/// Returns the normalized (trimmed, lowercased) address, which is what gets
/// stored and compared.
pub fn validate_email(email: &str) -> ApiResult<String> {
    let email = accounts::normalize_email(email);

    let reject = || ApiError::BadRequest("that doesn't look like an email address".into());

    if email.len() < 3 || email.len() > 254 || email.contains(char::is_whitespace) {
        return Err(reject());
    }

    let (local, domain) = email.split_once('@').ok_or_else(reject)?;

    // A second '@' lands in `domain`; catching it here also rejects "a@@b".
    if local.is_empty() || domain.contains('@') {
        return Err(reject());
    }
    // Require a dotted domain with non-empty labels: "user@localhost" and
    // "user@example." cannot receive the verification mail this flow depends on.
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 || labels.iter().any(|l| l.is_empty()) {
        return Err(reject());
    }

    Ok(email)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> DashboardConfig {
        DashboardConfig {
            bind_address: "127.0.0.1".into(),
            port: 0,
            public_base_url: "http://localhost".into(),
            site_base_url: "http://localhost:3000".into(),
            allowed_origins: "battlecrab.com".into(),
            database_url: String::new(),
            database_max_connections: 1,
            session_secret: "s".into(),
            session_ttl_days: 7,
            registration_enabled: true,
            min_password_length: 8,
            max_password_length: 45,
            max_login_length: 45,
            max_game_accounts: 5,
            login_rate_limit: 10,
            login_rate_window_secs: 300,
            admin_access_level: 100,
            smtp_host: String::new(),
            smtp_port: 587,
            smtp_from: "BattleCrab <no-reply@battlecrab.com>".into(),
            smtp_username: String::new(),
            smtp_password: String::new(),
        }
    }

    #[test]
    fn login_is_normalized_and_bounded() {
        assert_eq!(validate_login(" Alice ", &config()).unwrap(), "alice");
        assert!(validate_login("abc", &config()).is_err());
        assert!(validate_login(&"a".repeat(46), &config()).is_err());
    }

    #[test]
    fn login_rejects_characters_the_client_cannot_type() {
        assert!(validate_login("alice bob", &config()).is_err());
        assert!(validate_login("alice!", &config()).is_err());
        assert!(validate_login("алиса", &config()).is_err());
        assert!(validate_login("alice123", &config()).is_ok());
    }

    #[test]
    fn password_length_is_enforced_both_ways() {
        assert!(validate_password("short", &config()).is_err());
        assert!(validate_password("longenough", &config()).is_ok());
        assert!(validate_password(&"a".repeat(46), &config()).is_err());
    }

    #[test]
    fn password_must_be_ascii_so_it_is_typeable_in_the_client() {
        assert!(validate_password("pässwörd1", &config()).is_err());
    }

    #[test]
    fn email_shape_is_checked_loosely() {
        assert!(validate_email("a@b.c").is_ok());
        assert_eq!(validate_email(" A@B.c ").unwrap(), "a@b.c");
        assert!(validate_email("nope").is_err());
        assert!(validate_email("a@@b").is_err());
        assert!(validate_email("@b.c").is_err());
        assert!(validate_email("a b@c.d").is_err());
    }

    #[test]
    fn email_must_have_a_deliverable_domain() {
        // The whole flow hangs on a verification mail arriving, so an address
        // that cannot receive one is rejected up front.
        assert!(validate_email("user@localhost").is_err());
        assert!(validate_email("user@example.").is_err());
        assert!(validate_email("user@.com").is_err());
        assert!(validate_email("user@sub.example.com").is_ok());
    }
}
