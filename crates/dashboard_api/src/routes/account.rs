//! Authenticated account management: password, email, character list.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router};
use serde::Deserialize;

use crate::auth::{cookie, token, verify_password};
use crate::db::{accounts, characters};
use crate::error::{ApiError, ApiResult};
use crate::routes::{current_account, validate_email, validate_password};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/password", axum::routing::post(change_password))
        .route("/email", axum::routing::post(change_email))
        .route("/email/verify", axum::routing::get(verify_email))
        .route("/characters", axum::routing::get(list_characters))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

async fn change_password(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordRequest>,
) -> ApiResult<impl IntoResponse> {
    let account = current_account(&app, &headers).await?;

    if !verify_password(&body.current_password, &account.password) {
        return Err(ApiError::InvalidCredentials);
    }
    validate_password(&body.new_password, &app.config)?;

    let hash = commons::crypt::hash_password(&body.new_password);
    accounts::set_password(&app.pool, &account.login, &hash).await?;
    tracing::info!("password changed for {}", account.login);

    // The old cookie signed over the old hash and is now dead — including this
    // browser's. Re-issue so the user isn't logged out of the tab they're in.
    let mut out = HeaderMap::new();
    out.insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&cookie::issue(
            &app.key,
            &account.login,
            &hash,
            app.config.session_ttl_days,
            app.secure_cookies,
        ))
        .map_err(|_| ApiError::BadRequest("invalid session".into()))?,
    );

    Ok((out, StatusCode::NO_CONTENT))
}

#[derive(Deserialize)]
pub struct ChangeEmailRequest {
    pub email: String,
}

async fn change_email(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ChangeEmailRequest>,
) -> ApiResult<StatusCode> {
    let account = current_account(&app, &headers).await?;
    let email = validate_email(&body.email)?;

    // Deliberately does NOT write the column — `accounts.email` is written only
    // by the verification handler, which is what makes a stored address mean
    // "verified" without an `email_verified` column (PLAN_DASHBOARD.md §5.4).
    let raw = token::issue_verify_email(&app.key, &account.login, &email);
    // site_base_url: /verify-email is an SPA route, not an API one.
    let link = format!("{}/verify-email?token={raw}", app.config.site_base_url);

    // Unlike the reset flow there is nothing to hide here — the caller is
    // authenticated and chose this address — so a delivery failure is reported.
    // Otherwise the UI would claim "check your inbox" for mail that never left.
    app.mailer
        .send_email_verification(&email, &account.login, &link)
        .await
        .map_err(|e| {
            tracing::error!("failed to send verification email for {}: {e}", account.login);
            ApiError::Internal(crate::error::anyhow_lite::Error(
                "could not send the verification email".into(),
            ))
        })?;

    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize)]
pub struct VerifyEmailQuery {
    pub token: String,
}

async fn verify_email(
    State(app): State<AppState>,
    Query(query): Query<VerifyEmailQuery>,
) -> ApiResult<StatusCode> {
    let (login, email) = token::verify_email(&app.key, &query.token).ok_or(ApiError::InvalidToken)?;

    // Confirm the account still exists before writing.
    accounts::find(&app.pool, &login)
        .await?
        .ok_or(ApiError::InvalidToken)?;

    accounts::set_email(&app.pool, &login, &email).await?;
    tracing::info!("email verified for {login}");
    Ok(StatusCode::NO_CONTENT)
}

async fn list_characters(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<characters::CharacterSummary>>> {
    let account = current_account(&app, &headers).await?;
    let chars = characters::list_for_account(&app.pool, &account.login).await?;
    Ok(Json(chars))
}
