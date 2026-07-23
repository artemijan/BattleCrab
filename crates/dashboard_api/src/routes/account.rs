//! Authenticated master-account management: password, game accounts, character
//! list.
//!
//! There is deliberately **no** change-email endpoint. A master account's
//! address is its identity, its login, and the only record of which game
//! accounts belong to it — moving it is an account migration rather than an
//! account setting, and every game account underneath has to move in the same
//! breath. `/email/verify` remains, but only to confirm the address a
//! registration was made with.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router};
use serde::Deserialize;

use crate::auth::{cookie, verify_password};
use crate::db::{accounts, characters};
use crate::error::{ApiError, ApiResult};
use crate::routes::{current_account, validate_login, validate_password};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/password", axum::routing::post(change_password))
        .route("/email/verify", axum::routing::get(verify_email))
        .route(
            "/game-accounts",
            axum::routing::get(list_game_accounts).post(create_game_account),
        )
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
pub struct VerifyEmailQuery {
    pub token: String,
}

/// Confirms the address a registration was made with.
///
/// This only ever sets `is_verified`; it never moves an account to a different
/// address. `token::verify_email` refuses any token whose payload differs from
/// its subject, which is what neutralises the change-email links the removed
/// flow used to issue — those are still correctly signed and may still be
/// sitting in inboxes.
///
/// Unauthenticated by design — the signed token is the credential, and the user
/// may well open the link in a browser they are not signed in on.
async fn verify_email(
    State(app): State<AppState>,
    Query(query): Query<VerifyEmailQuery>,
) -> ApiResult<StatusCode> {
    let subject =
        crate::auth::token::verify_email(&app.key, &query.token).ok_or(ApiError::InvalidToken)?;

    // Confirm the account still exists at the address the token names.
    accounts::find_master_by_email(&app.pool, &subject)
        .await?
        .ok_or(ApiError::InvalidToken)?;

    accounts::mark_verified(&app.pool, &subject).await?;
    tracing::info!("email verified for {subject}");

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

#[derive(Deserialize)]
pub struct CreateGameAccountRequest {
    pub login: String,
    pub password: String,
}

#[derive(serde::Serialize)]
pub struct CreateGameAccountResponse {
    pub login: String,
}

/// Creates a game account — a login/password the game client can use — under
/// the signed-in master account's address.
///
/// The shared address is the *only* thing recording ownership (there is no
/// foreign key), so the address written here is always taken from the session,
/// never from the request body.
async fn create_game_account(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateGameAccountRequest>,
) -> ApiResult<impl IntoResponse> {
    let account = current_account(&app, &headers).await?;

    // Ownership is derived from the address, and account recovery is delivered
    // to it. Creating game accounts under an address nobody has proven would
    // hand real game assets to whoever actually reads that inbox — and this is
    // the first point where an unconfirmed address costs anything, which is
    // what makes the verification step meaningful at all.
    if !account.is_verified() {
        return Err(ApiError::EmailNotVerified);
    }

    let login = validate_login(&body.login, &app.config)?;
    validate_password(&body.password, &app.config)?;

    // Same hash the login server writes, so the row is indistinguishable from
    // one the game auto-created.
    let hash = commons::crypt::hash_password(&body.password);
    let subject = account.subject().to_string();

    accounts::create_game_account(
        &app.pool,
        &subject,
        &login,
        &hash,
        app.config.max_game_accounts,
    )
    .await?;

    tracing::info!("created game account {login} for {subject}");
    Ok((
        StatusCode::CREATED,
        Json(CreateGameAccountResponse { login }),
    ))
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
