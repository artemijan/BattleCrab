//! Registration, login, logout, email verification, and the stateless
//! password-reset flow.
//!
//! Everything here operates on **master accounts** (`accounts.login IS NULL`),
//! whose identity is their email address — see the `db::accounts` module docs.
//! Game accounts have login names but are never a dashboard session subject.

use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::auth::{cookie, token, verify_password};
use crate::db::accounts;
use crate::error::{ApiError, ApiResult};
use crate::routes::{current_account, validate_email, validate_password};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", axum::routing::post(register))
        .route("/login", axum::routing::post(login))
        .route("/logout", axum::routing::post(logout))
        .route("/me", axum::routing::get(me))
        .route("/resend-verification", axum::routing::post(resend_verification))
        .route("/forgot-password", axum::routing::post(forgot_password))
        .route("/reset-password", axum::routing::post(reset_password))
}

#[derive(Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub email: Option<String>,
    pub is_verified: bool,
}

impl From<&accounts::Account> for AccountView {
    fn from(account: &accounts::Account) -> Self {
        AccountView {
            email: account.email.clone(),
            is_verified: account.is_verified(),
        }
    }
}

/// Builds the `Set-Cookie` header for a session on the given subject.
fn session_headers(app: &AppState, subject: &str, password_hash: &str) -> ApiResult<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&cookie::issue(
            &app.key,
            subject,
            password_hash,
            app.config.session_ttl_days,
            app.secure_cookies,
        ))
        .map_err(|_| ApiError::BadRequest("invalid session".into()))?,
    );
    Ok(headers)
}

/// Sends the "confirm your address" mail. A delivery failure is logged, never
/// surfaced: registration itself succeeded, and failing the request would tell
/// the user their account does not exist when it does. `/resend-verification`
/// is the retry path.
async fn send_verification(app: &AppState, email: &str) {
    // Subject and payload are both the address: for a master account the
    // address *is* the account, and there is no separate pending value.
    let raw = token::issue_verify_email(&app.key, email, email);
    // site_base_url: /verify-email is an SPA route, not an API one.
    let link = format!("{}/verify-email?token={raw}", app.config.site_base_url);

    if let Err(e) = app.mailer.send_email_verification(email, email, &link).await {
        tracing::error!("failed to send verification email for {email}: {e}");
    }
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

    let email = validate_email(&body.email)?;
    validate_password(&body.password, &app.config)?;

    // The game's scheme — see PLAN_DASHBOARD.md §3.1. The master account never
    // logs into the game, but the game accounts created under it reuse this
    // hashing, so there is one implementation for both.
    let hash = commons::crypt::hash_password(&body.password);
    let account = accounts::create_master(&app.pool, &email, &hash).await?;
    tracing::info!("registered master account {email}");

    send_verification(&app, &email).await;

    let headers = session_headers(&app, account.subject(), &hash)?;
    Ok((StatusCode::CREATED, headers, Json(AccountView::from(&account))))
}

async fn login(
    State(app): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<Credentials>,
) -> ApiResult<impl IntoResponse> {
    // Not `validate_email`: a stricter validator would turn a login attempt for
    // an address that predates the current rules into a 400 instead of a clean
    // "invalid credentials". Normalize only.
    let email = accounts::normalize_email(&body.email);

    // Limit per-IP *and* per-account: neither a spray from many IPs at one
    // account nor a walk of many accounts from one IP should get a free pass.
    let ip_key = format!("ip:{}", peer.ip());
    let account_key = format!("account:{email}");
    if !app.login_limiter.check(&ip_key) || !app.login_limiter.check(&account_key) {
        tracing::warn!("rate limited login for {email} from {}", peer.ip());
        return Err(ApiError::RateLimited);
    }

    // Master lookup only: a game account shares this address, and matching one
    // here would let a sub-account password open the owner's dashboard.
    let account = accounts::find_master_by_email(&app.pool, &email).await?;

    // Verify even when the account is missing, against a dummy hash, so the
    // response time doesn't reveal which addresses exist.
    let stored = account.as_ref().map(|a| a.password.clone()).unwrap_or_default();
    let ok = verify_password(&body.password, &stored);

    let Some(account) = account.filter(|_| ok) else {
        return Err(ApiError::InvalidCredentials);
    };

    // A legitimate user who fumbled a few attempts shouldn't stay throttled.
    app.login_limiter.reset(&ip_key);
    app.login_limiter.reset(&account_key);

    // An unverified address can still sign in — it just carries `isVerified:
    // false` so the SPA can nag. Locking the account out would strand anyone
    // whose verification mail bounced, with no way to ask for another.
    let headers = session_headers(&app, account.subject(), &account.password)?;
    Ok((headers, Json(AccountView::from(&account))))
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
    Ok(Json(AccountView::from(&account)))
}

/// Re-sends the confirmation mail for the signed-in account.
async fn resend_verification(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    let account = current_account(&app, &headers).await?;

    if account.is_verified() {
        // Nothing to confirm. Reporting success would be a lie the UI acts on.
        return Err(ApiError::BadRequest("this address is already confirmed".into()));
    }

    let Some(email) = account.email.as_deref() else {
        return Err(ApiError::BadRequest("no email address on file".into()));
    };
    send_verification(&app, email).await;
    Ok(StatusCode::ACCEPTED)
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
    if let Some(account) = accounts::find_master_by_email(&app.pool, &body.email).await? {
        let subject = account.subject().to_string();
        let raw = token::issue_reset(&app.key, &subject, &account.password);
        // site_base_url, not public_base_url: /reset-password is a route in the
        // SPA, which no longer lives on the API's origin.
        let link = format!("{}/reset-password?token={raw}", app.config.site_base_url);

        // A delivery failure must NOT change the response: whether the address
        // exists, and whether SES accepted the message, are both invisible to
        // the caller by design. Log it and move on.
        if let Err(e) = app.mailer.send_password_reset(&subject, &subject, &link).await {
            tracing::error!("failed to send reset email for {subject}: {e}");
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

    // The token carries the address in cleartext, but is only trusted after the
    // HMAC verifies against that account's *current* hash — which is what makes
    // it single-use (PLAN_DASHBOARD.md §5.4).
    let claimed = decode_subject_hint(&body.token).ok_or(ApiError::InvalidToken)?;
    let account = accounts::find_master_by_email(&app.pool, &claimed)
        .await?
        .ok_or(ApiError::InvalidToken)?;

    let subject = token::verify_reset(&app.key, &body.token, &account.password)
        .ok_or(ApiError::InvalidToken)?;

    let hash = commons::crypt::hash_password(&body.password);
    accounts::set_master_password(&app.pool, &subject, &hash).await?;
    tracing::info!("password reset for {subject}");

    // Every outstanding session and reset link is now dead: both sign over the
    // old hash.
    Ok(StatusCode::NO_CONTENT)
}

/// Reads the subject out of a token *without* trusting it — used only to look
/// up which account's hash to verify the signature against.
fn decode_subject_hint(raw: &str) -> Option<String> {
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
    fn subject_hint_is_extracted_from_a_real_token() {
        let key = SigningKey::new("k");
        let raw = token::issue_reset(&key, "alice@example.com", "hash");
        assert_eq!(
            decode_subject_hint(&raw).as_deref(),
            Some("alice@example.com")
        );
    }

    #[test]
    fn subject_hint_rejects_garbage_without_panicking() {
        assert!(decode_subject_hint("!!!").is_none());
        assert!(decode_subject_hint("").is_none());
    }
}
