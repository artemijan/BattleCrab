//! The `/admin` surface (DASHBOARD.md §16). Every handler starts with
//! `require_admin`; there is no partially-open route in this module.
//!
//! Two rules bound what an admin can do from here:
//!
//! * **Never upward.** `accessLevel` can only be written to values ≤ 0 (ban /
//!   unban) — enforced again inside `db::admin::set_access_level`, so this
//!   module cannot become a promotion primitive even by mistake.
//! * **Never sideways.** A mutation is refused when the target's level is at or
//!   above the acting admin's own (`assert_outranks`). That stops equal admins
//!   from banning each other — and, since an admin does not outrank themself,
//!   it also makes self-ban impossible. Those cases are DB-console territory.
//!
//! Every mutation logs who did what to whom, because with shared `accessLevel
//! = 100` accounts the audit trail in the journal is all the attribution there
//! is. (A real audit table is deferred with the rest of the storage decision,
//! DASHBOARD.md §7.)

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::{Json, Router};
use serde::Deserialize;

use crate::db::accounts::{self, Account};
use crate::db::admin::{
    self, AccessLevelTarget, GameAccountInfo, MasterSort, MasterSummary, SortDir,
};
use crate::db::characters::{self, CharacterSummary};
use crate::error::{ApiError, ApiResult};
use crate::routes::{require_admin, validate_password};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/accounts", axum::routing::get(list_accounts))
        .route("/accounts/{email}", axum::routing::get(account_detail))
        .route(
            "/accounts/{email}/verify",
            axum::routing::post(verify_master),
        )
        .route(
            "/accounts/{email}/access-level",
            axum::routing::post(set_master_access_level),
        )
        .route(
            "/game-accounts",
            axum::routing::get(search_game_accounts).post(create_game_account),
        )
        .route(
            "/game-accounts/{login}/access-level",
            axum::routing::post(set_game_account_access_level),
        )
        .route(
            "/game-accounts/{login}/password",
            axum::routing::post(set_game_account_password),
        )
}

/// The peer-protection rule. `Err` unless the actor strictly outranks the
/// target — see the module docs for why "at or above" is the refusal line.
fn assert_outranks(actor: &Account, target_level: i32) -> ApiResult<()> {
    if target_level >= actor.access_level {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub q: String,
    /// Column to sort by; `MasterSort::parse` is the whitelist.
    #[serde(default)]
    pub sort: String,
    /// `asc` or `desc` (the default).
    #[serde(default)]
    pub dir: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Caps a caller-supplied page size. 200 rows is far past what the UI shows;
/// anything larger is a mistake or a scrape, and this DB is shared with the
/// live game (DASHBOARD.md §3.3).
fn page_limit(requested: Option<i64>) -> i64 {
    requested.unwrap_or(50).clamp(1, 200)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterListResponse {
    pub total: i64,
    pub accounts: Vec<MasterSummary>,
}

async fn list_accounts(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<MasterListResponse>> {
    require_admin(&app, &headers).await?;

    // Unknown names are a 400, not a silent default: the FE builds these from
    // a fixed set, so an unrecognised value is a bug worth surfacing.
    let sort = MasterSort::parse(&query.sort)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown sort column: {:?}", query.sort)))?;
    let dir = SortDir::parse(&query.dir)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown sort direction: {:?}", query.dir)))?;

    let limit = page_limit(query.limit);
    let offset = query.offset.unwrap_or(0).max(0);
    let (accounts, total) =
        admin::list_masters(&app.db, query.q.trim(), sort, dir, limit, offset).await?;

    Ok(Json(MasterListResponse { total, accounts }))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDetailResponse {
    pub master: MasterSummary,
    pub game_accounts: Vec<GameAccountInfo>,
    pub characters: Vec<CharacterSummary>,
}

async fn account_detail(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(email): Path<String>,
) -> ApiResult<Json<AccountDetailResponse>> {
    require_admin(&app, &headers).await?;

    // Through the list query so the detail view carries the same counts and
    // fields the row it was opened from did — one projection, no drift.
    let (mut masters, _) =
        admin::list_masters(&app.db, &email, MasterSort::default(), SortDir::Desc, 1, 0).await?;
    let master = masters.pop().filter(|m| {
        // list_masters searches with LIKE; the path names one exact account.
        m.email.eq_ignore_ascii_case(email.trim())
    });
    let master = master.ok_or(ApiError::NotFound)?;

    let game_accounts = admin::game_accounts_for_master(&app.db, &master.email).await?;
    let characters = characters::list_for_master(&app.db, &master.email).await?;

    Ok(Json(AccountDetailResponse {
        master,
        game_accounts,
        characters,
    }))
}

async fn verify_master(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(email): Path<String>,
) -> ApiResult<StatusCode> {
    let actor = require_admin(&app, &headers).await?;

    let target = accounts::find_master_by_email(&app.db, &email)
        .await?
        .ok_or(ApiError::NotFound)?;
    if target.is_verified() {
        return Err(ApiError::BadRequest(
            "this address is already confirmed".into(),
        ));
    }

    accounts::mark_verified(&app.db, target.subject()).await?;
    tracing::info!(
        admin = %actor.subject(),
        target = %target.subject(),
        "admin: marked master account verified"
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct AccessLevelRequest {
    /// ≤ 0 only: 0 restores a normal account, negative bans it. Positive
    /// values are refused — the dashboard cannot grant privileges.
    pub level: i32,
}

async fn set_master_access_level(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(email): Path<String>,
    Json(body): Json<AccessLevelRequest>,
) -> ApiResult<StatusCode> {
    let actor = require_admin(&app, &headers).await?;

    let target = accounts::find_master_by_email(&app.db, &email)
        .await?
        .ok_or(ApiError::NotFound)?;
    assert_outranks(&actor, target.access_level)?;

    admin::set_access_level(
        &app.db,
        AccessLevelTarget::Master(target.subject()),
        body.level,
    )
    .await?;
    tracing::info!(
        admin = %actor.subject(),
        target = %target.subject(),
        level = body.level,
        "admin: set master account access level"
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct GameAccountSearchQuery {
    #[serde(default)]
    pub q: String,
    pub limit: Option<i64>,
}

/// Search across *all* game accounts, including rows the login server
/// auto-created that no master owns — those are unreachable from the master
/// list, and this is the only way an admin finds them.
async fn search_game_accounts(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GameAccountSearchQuery>,
) -> ApiResult<Json<Vec<GameAccountInfo>>> {
    require_admin(&app, &headers).await?;
    let rows =
        admin::search_game_accounts(&app.db, query.q.trim(), page_limit(query.limit)).await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct CreateGameAccountRequest {
    pub login: String,
    pub password: String,
}

/// Creates a GM game account: it lands under the acting admin's own master
/// address with the actor's `accessLevel` copied onto it (MVP requirement —
/// game admins are minted by dashboard admins for themselves). The level is
/// read from the actor inside `db::admin`, never from the request, so this
/// cannot grant more than the actor already holds.
async fn create_game_account(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateGameAccountRequest>,
) -> ApiResult<StatusCode> {
    let actor = require_admin(&app, &headers).await?;

    let login = crate::routes::validate_login(&body.login, &app.config)?;
    validate_password(&body.password, &app.config)?;

    let hash = commons::crypt::hash_password(&body.password);
    admin::create_gm_game_account(&app.db, &actor, &login, &hash).await?;
    tracing::info!(
        admin = %actor.subject(),
        login = %login,
        level = actor.access_level,
        "admin: created GM game account at own access level"
    );
    Ok(StatusCode::CREATED)
}

async fn set_game_account_access_level(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(login): Path<String>,
    Json(body): Json<AccessLevelRequest>,
) -> ApiResult<StatusCode> {
    let actor = require_admin(&app, &headers).await?;

    let target = accounts::find_by_login(&app.db, &login)
        .await?
        .ok_or(ApiError::NotFound)?;
    // A GM's game account is protected the same way their master account is.
    assert_outranks(&actor, target.access_level)?;

    let login = target.login.as_deref().unwrap_or_default();
    admin::set_access_level(&app.db, AccessLevelTarget::GameAccount(login), body.level).await?;
    tracing::info!(
        admin = %actor.subject(),
        target = %login,
        level = body.level,
        "admin: set game account access level"
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SetPasswordRequest {
    pub password: String,
}

/// Support path for "player lost access to a game account". Writes the game's
/// own hash, so the new password works in the real client immediately.
///
/// Deliberately game-accounts-only: a *master* password is never set by an
/// admin — that would be silent account takeover of the identity that owns
/// everything else. Masters recover through the email reset flow, which proves
/// inbox control instead of trusting whoever holds an admin session.
async fn set_game_account_password(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(login): Path<String>,
    Json(body): Json<SetPasswordRequest>,
) -> ApiResult<StatusCode> {
    let actor = require_admin(&app, &headers).await?;

    let target = accounts::find_by_login(&app.db, &login)
        .await?
        .ok_or(ApiError::NotFound)?;
    assert_outranks(&actor, target.access_level)?;

    validate_password(&body.password, &app.config)?;

    let login = target.login.as_deref().unwrap_or_default();
    let hash = commons::crypt::hash_password(&body.password);
    accounts::set_game_account_password(&app.db, login, &hash).await?;
    tracing::info!(
        admin = %actor.subject(),
        target = %login,
        "admin: reset game account password"
    );
    Ok(StatusCode::NO_CONTENT)
}
