//! Authenticated master-account management: password, email, game accounts,
//! character list.

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
        .route("/game-accounts", axum::routing::get(list_game_accounts))
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
    let subject = account.subject().to_string();
    accounts::set_master_password(&app.pool, &subject, &hash).await?;
    tracing::info!("password changed for {subject}");

    // The old cookie signed over the old hash and is now dead — including this
    // browser's. Re-issue so the user isn't logged out of the tab they're in.
    let mut out = HeaderMap::new();
    out.insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&cookie::issue(
            &app.key,
            &subject,
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
    let new_email = validate_email(&body.email)?;
    let subject = account.subject().to_string();

    if new_email == subject {
        return Err(ApiError::BadRequest(
            "that is already your email address".into(),
        ));
    }
    // Cheap pre-check for a clear error message. The real guard is the partial
    // unique index, enforced when the link is followed — by then the address
    // may have been taken by someone else.
    if accounts::find_master_by_email(&app.pool, &new_email)
        .await?
        .is_some()
    {
        return Err(ApiError::EmailTaken);
    }

    // Deliberately does NOT write the column: the address is this account's
    // identity, so moving it before the new one is proven would lock the user
    // out of an address they may have mistyped.
    let raw = token::issue_verify_email(&app.key, &subject, &new_email);
    // site_base_url: /verify-email is an SPA route, not an API one.
    let link = format!("{}/verify-email?token={raw}", app.config.site_base_url);

    // Unlike the reset flow there is nothing to hide here — the caller is
    // authenticated and chose this address — so a delivery failure is reported.
    // Otherwise the UI would claim "check your inbox" for mail that never left.
    app.mailer
        .send_email_verification(&new_email, &new_email, &link)
        .await
        .map_err(|e| {
            tracing::error!("failed to send verification email for {subject}: {e}");
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

/// Handles both verification links, told apart by whether the payload differs
/// from the subject: registration issues `(email, email)` and only confirms the
/// address, while a change issues `(old, new)` and moves the account.
///
/// Unauthenticated by design — the signed token is the credential, and the user
/// may well open the link in a browser they are not signed in on.
async fn verify_email(
    State(app): State<AppState>,
    Query(query): Query<VerifyEmailQuery>,
) -> ApiResult<StatusCode> {
    let (subject, target) =
        token::verify_email(&app.key, &query.token).ok_or(ApiError::InvalidToken)?;

    // Confirm the account still exists — and is still at the address the token
    // was issued for — before writing.
    accounts::find_master_by_email(&app.pool, &subject)
        .await?
        .ok_or(ApiError::InvalidToken)?;

    if target == subject {
        accounts::mark_verified(&app.pool, &subject).await?;
        tracing::info!("email verified for {subject}");
    } else {
        accounts::change_master_email(&app.pool, &subject, &target).await?;
        tracing::info!("email changed from {subject} to {target}");
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn list_game_accounts(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<String>>> {
    let account = current_account(&app, &headers).await?;
    let logins = accounts::game_accounts_for_master(&app.pool, account.subject()).await?;
    Ok(Json(logins))
}

async fn list_characters(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<characters::CharacterSummary>>> {
    let account = current_account(&app, &headers).await?;
    // Characters hang off game accounts, not off the master account itself.
    let chars = characters::list_for_master(&app.pool, account.subject()).await?;
    Ok(Json(chars))
}
