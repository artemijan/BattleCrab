//! Authenticated master-account management: password, game accounts, character
//! list.
//!
//! There is deliberately **no** change-email endpoint. A master account's
//! address is its identity, its login, and the only record of which game
//! accounts belong to it — moving it is an account migration rather than an
//! account setting, and every game account underneath has to move in the same
//! breath. `/email/verify` remains, but only to confirm the address a
//! registration was made with.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router};
use serde::Deserialize;

use crate::auth::{cookie, verify_password};
use crate::db::{accounts, characters, items};
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
        .route(
            "/characters/{name}/items",
            axum::routing::get(character_items),
        )
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
    accounts::set_master_password(&app.db, &subject, &hash).await?;
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
    accounts::find_master_by_email(&app.db, &subject)
        .await?
        .ok_or(ApiError::InvalidToken)?;

    accounts::mark_verified(&app.db, &subject).await?;
    tracing::info!("email verified for {subject}");

    Ok(StatusCode::NO_CONTENT)
}

async fn list_game_accounts(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<String>>> {
    let account = current_account(&app, &headers).await?;
    let logins = accounts::game_accounts_for_master(&app.db, account.subject()).await?;
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
        &app.db,
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
    let chars = characters::list_for_master(&app.db, account.subject()).await?;
    Ok(Json(chars))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemRow {
    /// Row identity for React keys — stacks of the same item are distinct rows.
    pub object_id: i32,
    pub item_id: i32,
    pub name: String,
    /// `Weapon`, `Armor`, `EtcItem` — or `Unknown` off-catalog.
    #[serde(rename = "type")]
    pub kind: String,
    /// Lowercased icon reference, the atlas map's key. Absent when the
    /// catalog has no icon for the item (the UI shows an empty cell).
    pub icon: Option<String>,
    pub count: i64,
    pub enchant: i32,
    pub equipped: bool,
    /// Paperdoll slot id (`model::inventory::PaperdollSlot` numbering) for
    /// worn items, so the UI can draw the equipment doll. Null in the bag.
    pub slot: Option<i32>,
    /// Quest items get their own tab, as in the game client. Off-catalog
    /// items report false.
    pub quest: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterItemsResponse {
    pub inventory: Vec<ItemRow>,
    pub warehouse: Vec<ItemRow>,
}

/// One character's inventory (worn gear included) and private warehouse,
/// enriched with names and icons from the item catalog.
///
/// A character outside the session's game accounts answers 404, not 403 — the
/// route must not confirm that a name exists to someone who does not own it.
async fn character_items(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> ApiResult<Json<CharacterItemsResponse>> {
    let account = current_account(&app, &headers).await?;

    let character = characters::find_by_name(&app.db, &name)
        .await?
        .ok_or(ApiError::NotFound)?;
    let owned = accounts::game_accounts_for_master(&app.db, account.subject())
        .await?
        .iter()
        .any(|login| login.eq_ignore_ascii_case(&character.account_name));
    if !owned {
        return Err(ApiError::NotFound);
    }

    let mut rows = items::for_character(&app.db, character.char_id).await?;
    // Worn gear first in paperdoll-slot order, then the bag in its own order.
    rows.sort_by_key(|r| (r.loc != items::LOC_EQUIPPED, r.loc_data));

    let mut response = CharacterItemsResponse {
        inventory: Vec::new(),
        warehouse: Vec::new(),
    };
    for row in rows {
        let def = app.items.get(row.item_id);
        let out = ItemRow {
            object_id: row.object_id,
            item_id: row.item_id,
            // A placeholder name rather than dropping the row: an item the
            // catalog lacks still exists and still occupies a slot.
            name: def.map_or_else(|| format!("Item {}", row.item_id), |d| d.name.clone()),
            kind: def.map_or_else(|| "Unknown".to_string(), |d| d.kind.clone()),
            icon: def.and_then(|d| d.icon.clone()),
            count: row.count,
            enchant: row.enchant,
            equipped: row.loc == items::LOC_EQUIPPED,
            slot: (row.loc == items::LOC_EQUIPPED).then_some(row.loc_data),
            quest: def.is_some_and(|d| d.quest),
        };
        if row.loc == items::LOC_WAREHOUSE {
            response.warehouse.push(out);
        } else {
            response.inventory.push(out);
        }
    }
    Ok(Json(response))
}
