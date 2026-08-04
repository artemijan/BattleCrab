//! Public server status. No auth.

use axum::extract::State;
use axum::{Json, Router};
use serde::Serialize;

use crate::db::characters;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/server/status", axum::routing::get(status))
        .route("/health", axum::routing::get(health))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub online: bool,
    pub players_online: i64,
}

async fn status(State(app): State<AppState>) -> ApiResult<Json<ServerStatus>> {
    let players_online = characters::online_count(&app.db).await?;
    Ok(Json(ServerStatus {
        // TODO(D4): this reports "the DB is reachable", not "the game server is
        // up" — a crash can leave `online` flags set. An internal endpoint on
        // the game server would be accurate (DASHBOARD.md §12 q3).
        online: true,
        players_online,
    }))
}

/// Liveness for the container orchestrator — does not touch the DB.
async fn health() -> &'static str {
    "ok"
}
