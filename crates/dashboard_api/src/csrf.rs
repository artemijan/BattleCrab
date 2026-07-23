//! CSRF defence for a cookie-authenticated API.
//!
//! `SameSite=Lax` already blocks cross-site *sub-resource* requests from
//! carrying the session cookie, but it deliberately allows top-level
//! navigations — which includes a cross-site form POST in some browsers. So
//! mutating requests must additionally carry a header that a plain HTML form
//! cannot set. Any custom header forces a CORS preflight, which a cross-origin
//! attacker cannot satisfy (we send no permissive CORS headers).
//!
//! GET/HEAD are exempt: they must stay usable for links and navigation, which is
//! why they must also never mutate state.

use axum::extract::Request;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

pub const REQUIRED_HEADER: &str = "x-requested-with";

pub async fn require_custom_header(request: Request, next: Next) -> Response {
    let is_safe = matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );

    if !is_safe && !request.headers().contains_key(REQUIRED_HEADER) {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "error": { "code": "csrf", "message": "missing X-Requested-With header" }
            })),
        )
            .into_response();
    }

    next.run(request).await
}
