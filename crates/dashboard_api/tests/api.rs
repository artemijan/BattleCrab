//! End-to-end tests over the real axum stack and a real SQLite schema.
//!
//! The schema below is copied from the shipped `accounts`/`characters` DDL, so
//! these tests exercise the same column names and nullability the live game DB
//! has — a query that works here works against the real file.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use dashboard_api::config::DashboardConfig;
use dashboard_api::state::App;
use http_body_util::BodyExt;
use sqlx::SqlitePool;
use tower::ServiceExt;

const ACCOUNTS_DDL: &str = "CREATE TABLE accounts (
    login VARCHAR(45) NOT NULL default '' PRIMARY KEY,
    password VARCHAR(45),
    email varchar(255) DEFAULT NULL,
    created_time timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    lastactive bigint NOT NULL DEFAULT '0',
    accessLevel TINYINT NOT NULL DEFAULT 0,
    lastIP CHAR(15) NULL DEFAULT NULL,
    lastServer TINYINT DEFAULT 1
)";

const CHARACTERS_DDL: &str = "CREATE TABLE characters (
    account_name VARCHAR(45) DEFAULT NULL,
    charId INT NOT NULL DEFAULT 0,
    char_name VARCHAR(35) NOT NULL,
    level TINYINT DEFAULT NULL,
    sex TINYINT DEFAULT NULL,
    race TINYINT DEFAULT NULL,
    classid TINYINT DEFAULT NULL,
    deletetime bigint NOT NULL DEFAULT '0',
    online TINYINT DEFAULT NULL,
    onlinetime INT DEFAULT NULL,
    lastAccess bigint NOT NULL DEFAULT '0'
)";

fn test_config() -> DashboardConfig {
    DashboardConfig {
        bind_address: "127.0.0.1".into(),
        port: 0,
        public_base_url: "http://localhost".into(),
        site_base_url: "https://battlecrab.com".into(),
        allowed_origins: vec!["https://battlecrab.com".into()],
        database_url: String::new(),
        database_max_connections: 1,
        session_secret: "test-secret".into(),
        session_ttl_days: 7,
        registration_enabled: true,
        min_password_length: 8,
        max_password_length: 45,
        max_login_length: 45,
        login_rate_limit: 5,
        login_rate_window_secs: 300,
    }
}

async fn test_app() -> (axum::Router, SqlitePool) {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(ACCOUNTS_DDL).execute(&pool).await.unwrap();
    sqlx::query(CHARACTERS_DDL).execute(&pool).await.unwrap();

    let state = Arc::new(App::new(pool.clone(), test_config()));
    (dashboard_api::app(state), pool)
}

/// axum's ConnectInfo extractor needs a peer address; `oneshot` doesn't set one.
fn with_peer(mut req: Request<Body>) -> Request<Body> {
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 1234))));
    req
}

fn post(path: &str, body: serde_json::Value) -> Request<Body> {
    with_peer(
        Request::builder()
            .method("POST")
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .header("X-Requested-With", "XMLHttpRequest")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
}

fn get_with_cookie(path: &str, cookie: &str) -> Request<Body> {
    with_peer(
        Request::builder()
            .uri(path)
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap(),
    )
}

fn session_cookie(response: &axum::http::Response<Body>) -> String {
    response
        .headers()
        .get(header::SET_COOKIE)
        .expect("expected a session cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

async fn body_json(response: axum::http::Response<Body>) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

#[tokio::test]
async fn register_stores_the_hash_the_game_client_expects() {
    let (app, pool) = test_app().await;

    let response = app
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "login": "Alice", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // This is the acceptance test for the whole design: the stored value must be
    // Base64(SHA1(pw)) and the login lowercased, exactly as the login server
    // writes it — otherwise the account cannot log into the real client.
    let (login, password): (String, String) =
        sqlx::query_as("SELECT login, password FROM accounts")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(login, "alice");
    assert_eq!(password, commons::crypt::hash_password("correct-horse"));
}

#[tokio::test]
async fn register_then_login_then_read_characters() {
    let (app, pool) = test_app().await;

    let registered = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "login": "alice", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    assert_eq!(registered.status(), StatusCode::CREATED);

    sqlx::query(
        "INSERT INTO characters (account_name, charId, char_name, level, sex, race, classid, online, onlinetime, lastAccess)
         VALUES ('alice', 1, 'Shen', 42, 0, 1, 10, 1, 3600, 100)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let logged_in = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "login": "alice", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    assert_eq!(logged_in.status(), StatusCode::OK);
    let cookie = session_cookie(&logged_in);

    let chars = app
        .oneshot(get_with_cookie("/api/v1/account/characters", &cookie))
        .await
        .unwrap();
    assert_eq!(chars.status(), StatusCode::OK);

    let body = body_json(chars).await;
    assert_eq!(body[0]["name"], "Shen");
    assert_eq!(body[0]["level"], 42);
    // The projection must not leak columns the API has no business exposing.
    assert!(body[0].get("x").is_none());
    assert!(body[0].get("accessLevel").is_none());
}

#[tokio::test]
async fn characters_are_scoped_to_the_session_account() {
    let (app, pool) = test_app().await;

    for name in ["alice", "mallory"] {
        app.clone()
            .oneshot(post(
                "/api/v1/auth/register",
                serde_json::json!({ "login": name, "password": "correct-horse" }),
            ))
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO characters (account_name, charId, char_name, level, lastAccess)
         VALUES ('alice', 1, 'AliceChar', 10, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let logged_in = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "login": "mallory", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    let cookie = session_cookie(&logged_in);

    let chars = app
        .oneshot(get_with_cookie("/api/v1/account/characters", &cookie))
        .await
        .unwrap();
    let body = body_json(chars).await;
    assert_eq!(body.as_array().unwrap().len(), 0, "must not see another account's characters");
}

#[tokio::test]
async fn deleted_characters_are_hidden() {
    let (app, pool) = test_app().await;
    app.clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "login": "alice", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO characters (account_name, charId, char_name, level, deletetime, lastAccess)
         VALUES ('alice', 1, 'Doomed', 10, 999999, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let logged_in = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "login": "alice", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    let cookie = session_cookie(&logged_in);

    let chars = app
        .oneshot(get_with_cookie("/api/v1/account/characters", &cookie))
        .await
        .unwrap();
    assert_eq!(body_json(chars).await.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn duplicate_registration_is_rejected() {
    let (app, _pool) = test_app().await;
    let body = serde_json::json!({ "login": "alice", "password": "correct-horse" });

    let first = app.clone().oneshot(post("/api/v1/auth/register", body.clone())).await.unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);

    // Same name in different case must still collide — logins are normalized.
    let second = app
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "login": "ALICE", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn login_with_a_wrong_password_is_rejected() {
    let (app, _pool) = test_app().await;
    app.clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "login": "alice", "password": "correct-horse" }),
        ))
        .await
        .unwrap();

    let response = app
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "login": "alice", "password": "wrong-horse" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unknown_account_and_wrong_password_are_indistinguishable() {
    let (app, _pool) = test_app().await;
    app.clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "login": "alice", "password": "correct-horse" }),
        ))
        .await
        .unwrap();

    let wrong_password = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "login": "alice", "password": "nope-nope-nope" }),
        ))
        .await
        .unwrap();
    let no_such_account = app
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "login": "nobody", "password": "nope-nope-nope" }),
        ))
        .await
        .unwrap();

    assert_eq!(wrong_password.status(), no_such_account.status());
    assert_eq!(body_json(wrong_password).await, body_json(no_such_account).await);
}

#[tokio::test]
async fn protected_routes_reject_missing_and_forged_cookies() {
    let (app, _pool) = test_app().await;
    app.clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "login": "alice", "password": "correct-horse" }),
        ))
        .await
        .unwrap();

    let no_cookie = app
        .clone()
        .oneshot(with_peer(
            Request::builder()
                .uri("/api/v1/account/characters")
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(no_cookie.status(), StatusCode::UNAUTHORIZED);

    let forged = app
        .oneshot(get_with_cookie(
            "/api/v1/account/characters",
            "bc_session=alice|99999999999|deadbeef",
        ))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn changing_the_password_invalidates_the_old_session() {
    let (app, _pool) = test_app().await;
    let registered = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "login": "alice", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    let old_cookie = session_cookie(&registered);

    // Sanity: the cookie works before the change.
    let before = app
        .clone()
        .oneshot(get_with_cookie("/api/v1/auth/me", &old_cookie))
        .await
        .unwrap();
    assert_eq!(before.status(), StatusCode::OK);

    let changed = app
        .clone()
        .oneshot(with_peer(
            Request::builder()
                .method("POST")
                .uri("/api/v1/account/password")
                .header(header::CONTENT_TYPE, "application/json")
                .header("X-Requested-With", "XMLHttpRequest")
                .header(header::COOKIE, &old_cookie)
                .body(Body::from(
                    serde_json::json!({
                        "currentPassword": "correct-horse",
                        "newPassword": "battery-staple"
                    })
                    .to_string(),
                ))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(changed.status(), StatusCode::NO_CONTENT);

    // The old cookie signed over the old hash — it must be dead now. This is
    // the design's only session-revocation mechanism (PLAN_DASHBOARD.md §5.3).
    let after = app
        .oneshot(get_with_cookie("/api/v1/auth/me", &old_cookie))
        .await
        .unwrap();
    assert_eq!(after.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_is_rate_limited() {
    let (app, _pool) = test_app().await;
    app.clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "login": "alice", "password": "correct-horse" }),
        ))
        .await
        .unwrap();

    let mut saw_rate_limit = false;
    for _ in 0..10 {
        let response = app
            .clone()
            .oneshot(post(
                "/api/v1/auth/login",
                serde_json::json!({ "login": "alice", "password": "wrong-horse" }),
            ))
            .await
            .unwrap();
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            saw_rate_limit = true;
            break;
        }
    }
    assert!(saw_rate_limit, "brute force must eventually be throttled");
}

#[tokio::test]
async fn weak_passwords_are_rejected_at_registration() {
    let (app, _pool) = test_app().await;
    let response = app
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "login": "alice", "password": "short" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn mutations_without_the_csrf_header_are_refused() {
    let (app, _pool) = test_app().await;

    // Exactly what a cross-site <form> POST can produce: no custom header.
    let response = app
        .oneshot(with_peer(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "login": "alice", "password": "correct-horse" }).to_string(),
                ))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn unknown_api_routes_404_instead_of_returning_the_spa() {
    let (app, _pool) = test_app().await;
    let response = app
        .oneshot(with_peer(
            Request::builder()
                .uri("/api/v1/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn server_status_is_public_and_counts_online_players() {
    let (app, pool) = test_app().await;
    sqlx::query(
        "INSERT INTO characters (account_name, charId, char_name, online, lastAccess)
         VALUES ('alice', 1, 'A', 1, 1), ('bob', 2, 'B', 0, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(with_peer(
            Request::builder()
                .uri("/api/v1/server/status")
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await["playersOnline"], 1);
}

// ---------------------------------------------------------------------------
// CORS
//
// The SPA calls this API from another origin (battlecrab.com ->
// api.battlecrab.com). Credentialed CORS fails closed and silently — the
// browser just reports a network error — so these assert the exact headers a
// browser requires rather than trusting the layer's defaults.
// ---------------------------------------------------------------------------

const SITE: &str = "https://battlecrab.com";

fn preflight(path: &str, origin: &str, method: &str) -> Request<Body> {
    with_peer(
        Request::builder()
            .method("OPTIONS")
            .uri(path)
            .header(header::ORIGIN, origin)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, method)
            .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type,x-requested-with")
            .body(Body::empty())
            .unwrap(),
    )
}

#[tokio::test]
async fn preflight_from_the_site_is_allowed_with_credentials() {
    let (app, _pool) = test_app().await;

    let response = app.oneshot(preflight("/api/v1/auth/login", SITE, "POST")).await.unwrap();
    let headers = response.headers();

    assert!(response.status().is_success(), "preflight must not fail: {}", response.status());
    // Must echo the exact origin — "*" is rejected by browsers when credentials
    // are involved.
    assert_eq!(
        headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
        SITE
    );
    // Without this the browser sends no session cookie at all.
    assert_eq!(headers.get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).unwrap(), "true");

    // X-Requested-With must be allowed or every mutation is blocked by the
    // CSRF gate it exists to satisfy.
    let allowed = headers
        .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
        .unwrap()
        .to_str()
        .unwrap()
        .to_ascii_lowercase();
    assert!(allowed.contains("x-requested-with"), "got: {allowed}");
    assert!(allowed.contains("content-type"), "got: {allowed}");
}

#[tokio::test]
async fn preflight_from_an_unknown_origin_gets_no_grant() {
    let (app, _pool) = test_app().await;

    let response = app
        .oneshot(preflight("/api/v1/auth/login", "https://evil.example", "POST"))
        .await
        .unwrap();

    // The absence of the header is what makes the browser block it; a body or
    // status alone would not.
    assert!(
        response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none(),
        "must not grant access to an unlisted origin"
    );
}

#[tokio::test]
async fn actual_response_carries_cors_headers() {
    let (app, _pool) = test_app().await;

    let response = app
        .oneshot(with_peer(
            Request::builder()
                .uri("/api/v1/server/status")
                .header(header::ORIGIN, SITE)
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), SITE);
    // Shared caches must not serve one origin's grant to another.
    let vary = response.headers().get(header::VARY).unwrap().to_str().unwrap().to_ascii_lowercase();
    assert!(vary.contains("origin"), "expected Vary: Origin, got: {vary}");
}

#[tokio::test]
async fn error_responses_also_carry_cors_headers() {
    let (app, _pool) = test_app().await;

    // A 401 without CORS headers reaches the SPA as an opaque network error,
    // so it could not tell the user their credentials were wrong.
    let response = app
        .oneshot(with_peer(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header(header::ORIGIN, SITE)
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), SITE);
}

#[tokio::test]
async fn session_cookie_is_usable_cross_origin() {
    let (app, _pool) = test_app().await;

    let registered = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "login": "alice", "password": "correct-horse" }),
        ))
        .await
        .unwrap();

    let set_cookie = registered
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();

    // battlecrab.com and api.battlecrab.com are cross-origin but same-*site*,
    // so Lax still sends the cookie. It must NOT be SameSite=Strict, which
    // would block it, and it must stay HttpOnly so script cannot read it.
    assert!(set_cookie.contains("SameSite=Lax"), "got: {set_cookie}");
    assert!(set_cookie.contains("HttpOnly"), "got: {set_cookie}");
    assert!(!set_cookie.contains("SameSite=Strict"), "got: {set_cookie}");
}
