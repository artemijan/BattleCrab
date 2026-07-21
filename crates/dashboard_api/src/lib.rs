//! Web dashboard REST API — account registration and management.
//!
//! Design: `docs/PLAN_DASHBOARD.md`. The load-bearing constraints:
//!
//! - Passwords are stored as `Base64(SHA1(pw))` because the game client
//!   requires it; web login verifies the same hash (§3.1, §5.2).
//! - `characters` is read-only — live character state is memory-first in the
//!   game server and any write would be clobbered by autosave (§3.2).
//! - No tables of our own: sessions are signed cookies and reset/verify links
//!   are signed tokens (§5.3, §5.4).

pub mod auth;
pub mod config;
pub mod csrf;
pub mod db;
pub mod error;
pub mod routes;
pub mod state;
pub mod web;

use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

pub fn app(state: AppState) -> Router {
    let api = routes::api_router()
        // Applied to the API only: the SPA fallback serves GETs and needs no
        // CSRF gate.
        .layer(axum::middleware::from_fn(csrf::require_custom_header));

    Router::new()
        .nest("/api/v1", api)
        .fallback(web::serve_spa)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
