//! Registration, login, logout, and the stateless password-reset flow.

use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::auth::{cookie, token, verify_password};
use crate::db::accounts;
use crate::error::{ApiError, ApiResult};
use crate::routes::{current_account, validate_login, validate_password};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", axum::routing::post(register))
        .route("/login", axum::routing::post(login))
        .route("/logout", axum::routing::post(logout))
        .route("/me", axum::routing::get(me))
        .route("/forgot-password", axum::routing::post(forgot_password))
        .route("/reset-password", axum::routing::post(reset_password))
}

#[derive(Deserialize)]
pub struct Credentials {
    pub login: String,
    pub password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub login: String,
    pub email: Option<String>,
}

async fn register(
    State(app): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<Credentials>,
) -> ApiResult<impl IntoResponse> {
    if !app.config.registration_enabled {
        return Err(ApiError::RegistrationDisabled);
    }
    if !app.register_limiter.check(&peer.ip().to_string()) {
        return Err(ApiError::RateLimited);
    }

    let login = validate_login(&body.login, &app.config)?;
    validate_password(&body.password, &app.config)?;

    // The game's scheme — see PLAN_DASHBOARD.md §3.1. A different hash here
    // produces an account the real client cannot log into.
    let hash = commons::crypt::hash_password(&body.password);
    accounts::create(&app.pool, &login, &hash).await?;
    tracing::info!("registered account {login}");

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&cookie::issue(
            &app.key,
            &login,
            &hash,
            app.config.session_ttl_days,
            app.secure_cookies,
        ))
        .map_err(|_| ApiError::BadRequest("invalid session".into()))?,
    );

    Ok((
        StatusCode::CREATED,
        headers,
        Json(AccountView { login, email: None }),
    ))
}

async fn login(
    State(app): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<Credentials>,
) -> ApiResult<impl IntoResponse> {
    let login = accounts::normalize_login(&body.login);

    // Limit per-IP *and* per-account: neither a spray from many IPs at one
    // account nor a walk of many accounts from one IP should get a free pass.
    let ip_key = format!("ip:{}", peer.ip());
    let account_key = format!("account:{login}");
    if !app.login_limiter.check(&ip_key) || !app.login_limiter.check(&account_key) {
        tracing::warn!("rate limited login for {login} from {}", peer.ip());
        return Err(ApiError::RateLimited);
    }

    let account = accounts::find(&app.pool, &login).await?;

    // Verify even when the account is missing, against a dummy hash, so the
    // response time doesn't reveal which logins exist.
    let stored = account.as_ref().map(|a| a.password.clone()).unwrap_or_default();
    let ok = verify_password(&body.password, &stored);

    let Some(account) = account.filter(|_| ok) else {
        return Err(ApiError::InvalidCredentials);
    };

    // A legitimate user who fumbled a few attempts shouldn't stay throttled.
    app.login_limiter.reset(&ip_key);
    app.login_limiter.reset(&account_key);

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&cookie::issue(
            &app.key,
            &account.login,
            &account.password,
            app.config.session_ttl_days,
            app.secure_cookies,
        ))
        .map_err(|_| ApiError::BadRequest("invalid session".into()))?,
    );

    Ok((
        headers,
        Json(AccountView {
            login: account.login,
            email: account.email,
        }),
    ))
}

async fn logout(State(app): State<AppState>) -> ApiResult<impl IntoResponse> {
    // Clears this browser's cookie. There is no server-side session to delete,
    // so "log out everywhere" is a password change (PLAN_DASHBOARD.md §5.3).
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&cookie::clear(app.secure_cookies))
            .map_err(|_| ApiError::BadRequest("invalid session".into()))?,
    );
    Ok((headers, StatusCode::NO_CONTENT))
}

async fn me(State(app): State<AppState>, headers: HeaderMap) -> ApiResult<Json<AccountView>> {
    let account = current_account(&app, &headers).await?;
    Ok(Json(AccountView {
        login: account.login,
        email: account.email,
    }))
}

#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

async fn forgot_password(
    State(app): State<AppState>,
    Json(body): Json<ForgotPasswordRequest>,
) -> ApiResult<StatusCode> {
    // Always 202, whether or not the address exists — otherwise this endpoint
    // becomes an account-enumeration oracle.
    if let Some(account) = accounts::find_by_email(&app.pool, &body.email).await? {
        let raw = token::issue_reset(&app.key, &account.login, &account.password);
        // site_base_url, not public_base_url: /reset-password is a route in the
        // SPA, which no longer lives on the API's origin.
        let link = format!("{}/reset-password?token={raw}", app.config.site_base_url);

        if let Some(email) = account.email.as_deref() {
            // A delivery failure must NOT change the response: whether the
            // address exists, and whether SES accepted the message, are both
            // invisible to the caller by design. Log it and move on.
            if let Err(e) = app
                .mailer
                .send_password_reset(email, &account.login, &link)
                .await
            {
                tracing::error!("failed to send reset email for {}: {e}", account.login);
            }
        }
    }
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}

async fn reset_password(
    State(app): State<AppState>,
    Json(body): Json<ResetPasswordRequest>,
) -> ApiResult<StatusCode> {
    validate_password(&body.password, &app.config)?;

    // The token carries the login in cleartext, but is only trusted after the
    // HMAC verifies against that account's *current* hash — which is what makes
    // it single-use (PLAN_DASHBOARD.md §5.4).
    let claimed_login = decode_login_hint(&body.token).ok_or(ApiError::InvalidToken)?;
    let account = accounts::find(&app.pool, &claimed_login)
        .await?
        .ok_or(ApiError::InvalidToken)?;

    let login = token::verify_reset(&app.key, &body.token, &account.password)
        .ok_or(ApiError::InvalidToken)?;

    let hash = commons::crypt::hash_password(&body.password);
    accounts::set_password(&app.pool, &login, &hash).await?;
    tracing::info!("password reset for {login}");

    // Every outstanding session and reset link is now dead: both sign over the
    // old hash.
    Ok(StatusCode::NO_CONTENT)
}

/// Reads the login out of a token *without* trusting it — used only to look up
/// which account's hash to verify the signature against.
fn decode_login_hint(raw: &str) -> Option<String> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let decoded = URL_SAFE_NO_PAD.decode(raw).ok()?;
    let joined = String::from_utf8(decoded).ok()?;
    joined.split('|').nth(1).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SigningKey;

    #[test]
    fn login_hint_is_extracted_from_a_real_token() {
        let key = SigningKey::new("k");
        let raw = token::issue_reset(&key, "alice", "hash");
        assert_eq!(decode_login_hint(&raw).as_deref(), Some("alice"));
    }

    #[test]
    fn login_hint_rejects_garbage_without_panicking() {
        assert!(decode_login_hint("!!!").is_none());
        assert!(decode_login_hint("").is_none());
    }
}
