//! Web dashboard REST API — account registration and management.
//!
//! Design: `docs/DASHBOARD.md`. The load-bearing constraints:
//!
//! - Passwords are stored as `Base64(SHA1(pw))` because the game client
//!   requires it; web login verifies the same hash (§3.1, §5.2).
//! - `characters` is read-only — live character state is memory-first in the
//!   game server and any write would be clobbered by autosave (§3.2).
//! - No tables of our own: sessions are signed cookies and reset/verify links
//!   are signed tokens (§5.3, §5.4).

pub mod auth;
pub mod config;
pub mod cors;
pub mod csrf;
pub mod db;
pub mod error;
pub mod items;
pub mod mail;
pub mod routes;
pub mod state;
pub mod turnstile;
pub mod web;

use std::time::Duration;

use axum::Router;
use axum::http::request::Parts;
use axum::http::{HeaderValue, Method, header};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::state::AppState;

/// CORS for the SPA calling this API from another origin
/// (`battlecrab.com` → `api.battlecrab.com`).
///
/// Credentialed CORS is strict, and each rule below is load-bearing:
///
/// - The allowed origin is decided by `cors::OriginPolicy` — `battlecrab.com`
///   and its subdomains over HTTPS, nothing else. `Allow-Origin: *` is rejected
///   by browsers whenever credentials are sent, and echoing back whatever
///   `Origin` arrives would let any website drive a logged-in user's account.
/// - `Allow-Credentials: true` is required or the browser sends no session
///   cookie and every authenticated call 401s.
/// - `X-Requested-With` must be allowed: the client sends it on mutations and
///   the CSRF gate rejects requests without it (`csrf.rs`). It is also what
///   forces a preflight, which is the point.
/// - `Vary: Origin` is set by `CorsLayer` automatically — without it a shared
///   cache could serve one origin's `Allow-Origin` to another.
///
/// Note the cookie itself stays `SameSite=Lax`: `battlecrab.com` and
/// `api.battlecrab.com` are cross-*origin* but same-*site* (same registrable
/// domain), so Lax cookies are still sent. A frontend on a genuinely different
/// domain would need `SameSite=None`, which is a deliberate change, not a
/// default.
fn cors_layer(state: &AppState) -> CorsLayer {
    let policy = state.origin_policy.clone();

    if policy.is_empty() {
        tracing::warn!(
            "AllowedOrigins is empty — no browser origin may call this API. \
             Set it to the site's domain, e.g. battlecrab.com"
        );
    } else {
        tracing::info!("CORS origin rules: {:?}", policy.rules());
    }

    // A predicate rather than a fixed list, so `battlecrab.com` covers every
    // current and future subdomain without redeploying. `OriginPolicy` decides;
    // it is the only place the matching rules live, and it is tested against
    // lookalike domains.
    let allow_origin = AllowOrigin::predicate(move |origin: &HeaderValue, _parts: &Parts| {
        origin
            .to_str()
            .map(|origin| policy.allows(origin))
            .unwrap_or(false)
    });

    CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            header::CONTENT_TYPE,
            header::HeaderName::from_static("x-requested-with"),
        ])
        .max_age(Duration::from_secs(600))
}

pub fn app(state: AppState) -> Router {
    // Layer order matters: CORS is outermost so that *error* responses carry the
    // headers too. Without that a 401 or 429 reaches the browser as an opaque
    // "network error" and the SPA cannot show why the request failed.
    let api = routes::api_router()
        // Applied to the API only: the SPA fallback serves GETs and needs no
        // CSRF gate. OPTIONS is exempt inside, so preflights pass through.
        .layer(axum::middleware::from_fn(csrf::require_custom_header))
        .layer(cors_layer(&state));

    Router::new()
        .nest("/api/v1", api)
        .fallback(web::serve_spa)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
