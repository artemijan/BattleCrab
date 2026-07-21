//! Serves the embedded SPA.
//!
//! In release the built assets are baked into the binary; in debug `rust-embed`
//! reads them off disk, so `cargo run` works without a built frontend and
//! `bun --hot` can own the reload loop (PLAN_DASHBOARD.md §9).

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/dashboard/dist"]
struct Assets;

/// Static file if one matches, otherwise `index.html` so client-side routes
/// survive a hard refresh.
pub async fn serve_spa(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if let Some(response) = asset(path) {
        return response;
    }

    // An unmatched /api/* path is a missing endpoint, not a client route —
    // returning index.html there would turn a 404 into a confusing 200.
    if path.starts_with("api/") {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    asset("index.html").unwrap_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            "frontend not built — run `bun run build` in web/dashboard",
        )
            .into_response()
    })
}

fn asset(path: &str) -> Option<Response> {
    let file = Assets::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    // Hashed filenames can be cached hard; index.html must not be, or a deploy
    // leaves clients on a stale shell that references deleted bundles.
    let cache_control = if path == "index.html" {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };

    Some(
        (
            [
                (header::CONTENT_TYPE, mime.as_ref()),
                (header::CACHE_CONTROL, cache_control),
            ],
            file.data.into_owned(),
        )
            .into_response(),
    )
}
