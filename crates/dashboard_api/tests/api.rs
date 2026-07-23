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
    login VARCHAR(45) DEFAULT NULL,
    password VARCHAR(45),
    email varchar(255) DEFAULT NULL,
    is_verified TINYINT DEFAULT NULL,
    created_time timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    lastactive bigint NOT NULL DEFAULT '0',
    accessLevel TINYINT NOT NULL DEFAULT 0,
    lastIP CHAR(15) NULL DEFAULT NULL,
    lastServer TINYINT DEFAULT 1,
    UNIQUE (login)
)";

/// The partial unique index is the only thing stopping two master accounts from
/// sharing an address, so it belongs in the test schema — without it,
/// `duplicate_registration_is_rejected` would pass for the wrong reason.
const ACCOUNTS_MASTER_EMAIL_INDEX: &str =
    "CREATE UNIQUE INDEX accounts_master_email ON accounts (email COLLATE NOCASE) \
     WHERE login IS NULL";

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
        allowed_origins: "battlecrab.com".into(),
        database_url: String::new(),
        database_max_connections: 1,
        session_secret: "test-secret".into(),
        session_ttl_days: 7,
        registration_enabled: true,
        min_password_length: 8,
        max_password_length: 45,
        max_login_length: 45,
        // Deliberately small so the cap is reachable in a test without
        // hammering the endpoint.
        max_game_accounts: 3,
        login_rate_limit: 5,
        login_rate_window_secs: 300,
        admin_access_level: 100,
        // Email stays disabled in tests: the mailer then logs instead of
        // sending, so nothing tries to reach an SMTP server.
        smtp_host: String::new(),
        smtp_port: 587,
        smtp_from: "BattleCrab <no-reply@battlecrab.com>".into(),
        smtp_username: String::new(),
        smtp_password: String::new(),
    }
}

async fn test_app() -> (axum::Router, SqlitePool) {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(ACCOUNTS_DDL).execute(&pool).await.unwrap();
    sqlx::query(ACCOUNTS_MASTER_EMAIL_INDEX).execute(&pool).await.unwrap();
    sqlx::query(CHARACTERS_DDL).execute(&pool).await.unwrap();

    let state = Arc::new(App::new(pool.clone(), test_config()));
    (dashboard_api::app(state), pool)
}

/// Adds a game account under a master address — the row the dashboard will
/// create in a later milestone, and the only thing `characters` can hang off.
async fn add_game_account(pool: &SqlitePool, email: &str, login: &str) {
    sqlx::query(
        "INSERT INTO accounts (login, password, email, is_verified, lastactive, accessLevel)
         VALUES (?, 'x', ?, NULL, 0, 0)",
    )
    .bind(login)
    .bind(email)
    .execute(pool)
    .await
    .unwrap();
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

fn post_with_cookie(path: &str, cookie: &str, body: serde_json::Value) -> Request<Body> {
    with_peer(
        Request::builder()
            .method("POST")
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .header("X-Requested-With", "XMLHttpRequest")
            .header(header::COOKIE, cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
}

/// Registers a master account, confirms its address, and returns the session
/// cookie.
///
/// Game-account creation is gated on a confirmed address, so the interesting
/// state for these tests is "verified", not "just registered". The column is
/// set directly rather than through the link — the link itself has its own test
/// (`a_master_account_starts_unverified_and_the_link_verifies_it`).
async fn verified_master(pool: &SqlitePool, app: &axum::Router, email: &str) -> String {
    let registered = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "email": email, "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    assert_eq!(registered.status(), StatusCode::CREATED);

    sqlx::query("UPDATE accounts SET is_verified = 1 WHERE login IS NULL AND email = ?")
        .bind(email)
        .execute(pool)
        .await
        .unwrap();

    session_cookie(&registered)
}

/// A verified master promoted to admin the only way that exists: a direct
/// `accessLevel` write, as an operator with DB access would do it. There is
/// deliberately no API path to reach this state.
async fn admin_master(pool: &SqlitePool, app: &axum::Router, email: &str) -> String {
    let cookie = verified_master(pool, app, email).await;
    sqlx::query("UPDATE accounts SET accessLevel = 100 WHERE login IS NULL AND email = ?")
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
    cookie
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
            serde_json::json!({ "email": "Alice@Example.com", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // This is the acceptance test for the whole design: the stored value must be
    // Base64(SHA1(pw)), exactly as the login server writes it — the game
    // accounts created under this master reuse the hash, so a mismatch here
    // produces accounts the real client cannot log into.
    //
    // `login` must be NULL and the address stored lowercased: that pair is what
    // marks the row a master account rather than something the game can see.
    let (login, email, password, is_verified): (Option<String>, String, String, Option<i64>) =
        sqlx::query_as("SELECT login, email, password, is_verified FROM accounts")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(login, None, "a master account has no game login");
    assert_eq!(email, "alice@example.com");
    assert_eq!(password, commons::crypt::hash_password("correct-horse"));
    assert_eq!(is_verified, Some(0), "a fresh registration is unverified");
}

#[tokio::test]
async fn a_game_account_cannot_sign_into_the_dashboard() {
    let (app, pool) = test_app().await;

    // A game account carrying the same address as a master must never satisfy
    // the dashboard login — otherwise a leaked sub-account password would open
    // the owner's dashboard.
    sqlx::query(
        "INSERT INTO accounts (login, password, email, is_verified, lastactive, accessLevel)
         VALUES ('alice', ?, 'alice@example.com', NULL, 0, 0)",
    )
    .bind(commons::crypt::hash_password("correct-horse"))
    .execute(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn registration_requires_something_that_looks_like_an_address() {
    let (app, _pool) = test_app().await;

    for bad in ["alice", "alice@localhost", "alice@", "@example.com", "a b@c.d"] {
        let response = app
            .clone()
            .oneshot(post(
                "/api/v1/auth/register",
                serde_json::json!({ "email": bad, "password": "correct-horse" }),
            ))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{bad} should not be accepted as an email address"
        );
    }
}

#[tokio::test]
async fn register_then_login_then_read_characters() {
    let (app, pool) = test_app().await;

    let registered = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    assert_eq!(registered.status(), StatusCode::CREATED);

    // Characters hang off a *game* account, which is linked to the master by
    // the shared address.
    add_game_account(&pool, "alice@example.com", "alice1").await;
    sqlx::query(
        "INSERT INTO characters (account_name, charId, char_name, level, sex, race, classid, online, onlinetime, lastAccess)
         VALUES ('alice1', 1, 'Shen', 42, 0, 1, 10, 1, 3600, 100)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let logged_in = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
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

    for name in ["alice@example.com", "mallory@example.com"] {
        app.clone()
            .oneshot(post(
                "/api/v1/auth/register",
                serde_json::json!({ "email": name, "password": "correct-horse" }),
            ))
            .await
            .unwrap();
    }
    add_game_account(&pool, "alice@example.com", "alice1").await;
    sqlx::query(
        "INSERT INTO characters (account_name, charId, char_name, level, lastAccess)
         VALUES ('alice1', 1, 'AliceChar', 10, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let logged_in = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "email": "mallory@example.com", "password": "correct-horse" }),
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
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    add_game_account(&pool, "alice@example.com", "alice1").await;
    sqlx::query(
        "INSERT INTO characters (account_name, charId, char_name, level, deletetime, lastAccess)
         VALUES ('alice1', 1, 'Doomed', 10, 999999, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let logged_in = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
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
async fn characters_from_every_game_account_are_listed_together() {
    let (app, pool) = test_app().await;

    let registered = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    let cookie = session_cookie(&registered);

    // Two game accounts under one master, plus a third under someone else.
    add_game_account(&pool, "alice@example.com", "alice1").await;
    add_game_account(&pool, "alice@example.com", "alice2").await;
    add_game_account(&pool, "mallory@example.com", "mallory1").await;
    sqlx::query(
        "INSERT INTO characters (account_name, charId, char_name, level, lastAccess) VALUES
         ('alice1', 1, 'First', 10, 200),
         ('alice2', 2, 'Second', 20, 100),
         ('mallory1', 3, 'NotYours', 30, 300)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let chars = app
        .clone()
        .oneshot(get_with_cookie("/api/v1/account/characters", &cookie))
        .await
        .unwrap();
    let body = body_json(chars).await;
    let names: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["First", "Second"], "ordered by lastAccess desc");
    // The owning game account has to come along, or the UI cannot group them.
    assert_eq!(body[0]["accountName"], "alice1");
    assert_eq!(body[1]["accountName"], "alice2");

    let listed = app
        .oneshot(get_with_cookie("/api/v1/account/game-accounts", &cookie))
        .await
        .unwrap();
    assert_eq!(
        body_json(listed).await,
        serde_json::json!(["alice1", "alice2"])
    );
}

#[tokio::test]
async fn a_game_account_is_created_under_the_masters_address() {
    let (app, pool) = test_app().await;
    let cookie = verified_master(&pool, &app, "alice@example.com").await;

    let created = app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/account/game-accounts",
            &cookie,
            serde_json::json!({ "login": "Alice1", "password": "game-password" }),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_eq!(body_json(created).await["login"], "alice1");

    // The whole ownership model is "same address, non-null login, NULL
    // is_verified". Each part is load-bearing: the address is the only link
    // back to the master, the login is what the game matches on, and a
    // non-NULL is_verified would make this row look like a second identity.
    let (login, email, password, is_verified): (String, String, String, Option<i64>) =
        sqlx::query_as("SELECT login, email, password, is_verified FROM accounts WHERE login IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(login, "alice1", "logins are stored lowercased");
    assert_eq!(email, "alice@example.com");
    assert_eq!(
        password,
        commons::crypt::hash_password("game-password"),
        "must be the game's own hash or the client cannot log in with it"
    );
    assert_eq!(is_verified, None, "a game account is not an identity");

    let listed = app
        .oneshot(get_with_cookie("/api/v1/account/game-accounts", &cookie))
        .await
        .unwrap();
    assert_eq!(body_json(listed).await, serde_json::json!(["alice1"]));
}

/// The exploit this design exists to prevent: a game account carries its
/// master's address, so if the dashboard login ever matched on address alone, a
/// game password would open the owner's dashboard.
#[tokio::test]
async fn a_dashboard_created_game_account_cannot_sign_into_the_dashboard() {
    let (app, pool) = test_app().await;
    let cookie = verified_master(&pool, &app, "alice@example.com").await;

    app.clone()
        .oneshot(post_with_cookie(
            "/api/v1/account/game-accounts",
            &cookie,
            serde_json::json!({ "login": "alice1", "password": "game-password" }),
        ))
        .await
        .unwrap();

    let response = app
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "email": "alice@example.com", "password": "game-password" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_unverified_master_cannot_create_game_accounts() {
    let (app, _pool) = test_app().await;

    // Registered but never confirmed — the address may belong to someone else.
    let registered = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    let cookie = session_cookie(&registered);

    let response = app
        .oneshot(post_with_cookie(
            "/api/v1/account/game-accounts",
            &cookie,
            serde_json::json!({ "login": "alice1", "password": "game-password" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(body_json(response).await["error"]["code"], "email_not_verified");
}

#[tokio::test]
async fn creating_a_game_account_requires_a_session() {
    let (app, _pool) = test_app().await;

    let response = app
        .oneshot(post(
            "/api/v1/account/game-accounts",
            serde_json::json!({ "login": "alice1", "password": "game-password" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// A taken login must be refused rather than reassigned — an UPDATE-shaped bug
/// here would let anyone take over an existing account by "creating" it.
#[tokio::test]
async fn a_login_taken_by_another_master_is_refused_and_left_untouched() {
    let (app, pool) = test_app().await;
    add_game_account(&pool, "mallory@example.com", "taken").await;

    let cookie = verified_master(&pool, &app, "alice@example.com").await;
    let response = app
        .oneshot(post_with_cookie(
            "/api/v1/account/game-accounts",
            &cookie,
            serde_json::json!({ "login": "taken", "password": "game-password" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(body_json(response).await["error"]["code"], "login_taken");

    // Still Mallory's, with Mallory's password.
    let (email, password): (String, String) =
        sqlx::query_as("SELECT email, password FROM accounts WHERE login = 'taken'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(email, "mallory@example.com");
    assert_eq!(password, "x");
}

#[tokio::test]
async fn game_accounts_are_capped_per_master() {
    let (app, pool) = test_app().await;
    let cookie = verified_master(&pool, &app, "alice@example.com").await;

    // Someone else being at the cap must not consume Alice's allowance.
    add_game_account(&pool, "mallory@example.com", "mallory1").await;

    // test_config caps at 3.
    for n in 1..=3 {
        let response = app
            .clone()
            .oneshot(post_with_cookie(
                "/api/v1/account/game-accounts",
                &cookie,
                serde_json::json!({ "login": format!("alice{n}"), "password": "game-password" }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED, "account {n} of 3");
    }

    let over = app
        .oneshot(post_with_cookie(
            "/api/v1/account/game-accounts",
            &cookie,
            serde_json::json!({ "login": "alice4", "password": "game-password" }),
        ))
        .await
        .unwrap();
    assert_eq!(over.status(), StatusCode::CONFLICT);
    assert_eq!(
        body_json(over).await["error"]["code"],
        "too_many_game_accounts"
    );

    // The refusal must not have written the row anyway.
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE email = 'alice@example.com' AND login IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn game_account_credentials_are_validated() {
    let (app, pool) = test_app().await;
    let cookie = verified_master(&pool, &app, "alice@example.com").await;

    for (login, password, why) in [
        ("ab", "game-password", "login too short"),
        ("alice bob", "game-password", "space is untypeable in the client"),
        ("alice!", "game-password", "punctuation is untypeable in the client"),
        ("alice1", "short", "password below the minimum"),
        ("alice1", "pässwörd1", "non-ASCII password is untypeable in the client"),
    ] {
        let response = app
            .clone()
            .oneshot(post_with_cookie(
                "/api/v1/account/game-accounts",
                &cookie,
                serde_json::json!({ "login": login, "password": password }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{why}");
    }

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE login IS NOT NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "no rejected request may have created a row");
}

#[tokio::test]
async fn a_master_account_starts_unverified_and_the_link_verifies_it() {
    let (app, pool) = test_app().await;

    let registered = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    let cookie = session_cookie(&registered);
    assert_eq!(body_json(registered).await["isVerified"], false);

    // The token is what the mail would have carried; mail is disabled in tests,
    // so mint the same one the handler does.
    let key = dashboard_api::auth::SigningKey::new("test-secret");
    let token = dashboard_api::auth::token::issue_verify_email(&key, "alice@example.com");

    let verified = app
        .clone()
        .oneshot(with_peer(
            Request::builder()
                .uri(format!("/api/v1/account/email/verify?token={token}"))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(verified.status(), StatusCode::NO_CONTENT);

    let is_verified: Option<i64> =
        sqlx::query_scalar("SELECT is_verified FROM accounts WHERE login IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(is_verified, Some(1));

    let me = app
        .oneshot(get_with_cookie("/api/v1/auth/me", &cookie))
        .await
        .unwrap();
    assert_eq!(body_json(me).await["isVerified"], true);
}

/// The address is the account's identity and the only record of which game
/// accounts belong to it, so there is no endpoint to change it. A session must
/// not be able to reach one by any spelling.
#[tokio::test]
async fn there_is_no_change_email_endpoint() {
    let (app, pool) = test_app().await;
    let cookie = verified_master(&pool, &app, "alice@example.com").await;

    let response = app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/account/email",
            &cookie,
            serde_json::json!({ "email": "attacker@example.com" }),
        ))
        .await
        .unwrap();
    assert!(
        response.status().is_client_error(),
        "POST /account/email answered {} — the change-email flow is supposed to be gone",
        response.status()
    );

    // And nothing moved.
    let (email,): (String,) = sqlx::query_as("SELECT email FROM accounts WHERE login IS NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(email, "alice@example.com");
}

#[tokio::test]
async fn duplicate_registration_is_rejected() {
    let (app, _pool) = test_app().await;
    let body = serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" });

    let first = app.clone().oneshot(post("/api/v1/auth/register", body.clone())).await.unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);

    // Same address in different case must still collide — addresses are
    // normalized, and the partial unique index is case-insensitive to match.
    let second = app
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "email": "ALICE@EXAMPLE.COM", "password": "correct-horse" }),
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
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
        ))
        .await
        .unwrap();

    let response = app
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "email": "alice@example.com", "password": "wrong-horse" }),
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
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
        ))
        .await
        .unwrap();

    let wrong_password = app
        .clone()
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "email": "alice@example.com", "password": "nope-nope-nope" }),
        ))
        .await
        .unwrap();
    let no_such_account = app
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "email": "nobody@example.com", "password": "nope-nope-nope" }),
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
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
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
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
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
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
        ))
        .await
        .unwrap();

    let mut saw_rate_limit = false;
    for _ in 0..10 {
        let response = app
            .clone()
            .oneshot(post(
                "/api/v1/auth/login",
                serde_json::json!({ "email": "alice@example.com", "password": "wrong-horse" }),
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
            serde_json::json!({ "email": "alice@example.com", "password": "short" }),
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
                    serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }).to_string(),
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
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
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

#[tokio::test]
async fn subdomains_of_the_site_domain_are_allowed_over_http_layer() {
    for origin in [
        "https://battlecrab.com",
        "https://www.battlecrab.com",
        "https://api.battlecrab.com",
        "https://play.battlecrab.com",
    ] {
        let (app, _pool) = test_app().await;
        let response = app.oneshot(preflight("/api/v1/auth/login", origin, "POST")).await.unwrap();
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).map(|v| v.to_str().unwrap()),
            Some(origin),
            "{origin} should be allowed",
        );
    }
}

#[tokio::test]
async fn foreign_and_lookalike_origins_are_refused_over_http_layer() {
    // `evilbattlecrab.com` is the one that a naive suffix match would let in,
    // and it is registerable by anyone.
    for origin in [
        "https://evilbattlecrab.com",
        "https://battlecrab.com.evil.example",
        "https://evil.example",
        "http://battlecrab.com",
        "null",
    ] {
        let (app, _pool) = test_app().await;
        let response = app.oneshot(preflight("/api/v1/auth/login", origin, "POST")).await.unwrap();
        assert!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none(),
            "{origin} must NOT be granted access",
        );
    }
}

/// The link-preview card has to be reachable at the exact, unhashed URL that
/// `index.html` advertises in `og:image` — a crawler follows that URL and
/// nothing else.
///
/// Skips when the SPA has not been built, matching the frontend tests: a plain
/// `cargo test` should not require a `bun run build` first.
#[tokio::test]
async fn the_link_preview_image_is_served_at_a_stable_url() {
    let dist = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/dashboard/dist/og-image.jpg");
    if !std::path::Path::new(dist).exists() {
        eprintln!("skipped: run `bun run build` in web/dashboard first");
        return;
    }

    let (app, _pool) = test_app().await;
    let response = app
        .oneshot(with_peer(
            Request::builder().uri("/og-image.jpg").body(Body::empty()).unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/jpeg",
        "a wrong content type makes some crawlers discard the image"
    );

    // It is NOT content-hashed, so the year-long immutable policy the hashed
    // bundles get would pin stale artwork that no deploy could replace.
    let cache = response.headers().get(header::CACHE_CONTROL).unwrap().to_str().unwrap();
    assert!(
        !cache.contains("immutable"),
        "og-image.jpg must not be immutably cached, got {cache:?}"
    );

    let body = response.into_body().collect().await.unwrap().to_bytes();
    // JPEG magic — proves a real image came back rather than the SPA fallback,
    // which would be a 200 full of HTML.
    assert_eq!(&body[..2], &[0xff, 0xd8], "expected a JPEG, not the index.html fallback");
}

// ---------------------------------------------------------------------------
// Admin (PLAN_DASHBOARD.md §16)
//
// The two properties everything below defends: the /admin surface is closed to
// ordinary sessions, and no admin action can ever move an access level upward.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn admin_routes_are_closed_to_ordinary_accounts() {
    let (app, pool) = test_app().await;
    let cookie = verified_master(&pool, &app, "player@example.com").await;

    // Authenticated but not an admin: 403, not 401 — the session is fine.
    let response = app
        .clone()
        .oneshot(get_with_cookie("/api/v1/admin/accounts", &cookie))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(body_json(response).await["error"]["code"], "forbidden");

    // No session at all: 401.
    let response = app
        .oneshot(with_peer(
            Request::builder().uri("/api/v1/admin/accounts").body(Body::empty()).unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_reports_admin_status() {
    let (app, pool) = test_app().await;

    let player = verified_master(&pool, &app, "player@example.com").await;
    let me = app.clone().oneshot(get_with_cookie("/api/v1/auth/me", &player)).await.unwrap();
    assert_eq!(body_json(me).await["isAdmin"], false);

    let admin = admin_master(&pool, &app, "admin@example.com").await;
    let me = app.oneshot(get_with_cookie("/api/v1/auth/me", &admin)).await.unwrap();
    assert_eq!(body_json(me).await["isAdmin"], true);
}

#[tokio::test]
async fn an_admin_lists_master_accounts_with_counts() {
    let (app, pool) = test_app().await;
    let admin = admin_master(&pool, &app, "admin@example.com").await;
    verified_master(&pool, &app, "alice@example.com").await;

    add_game_account(&pool, "alice@example.com", "alice1").await;
    add_game_account(&pool, "alice@example.com", "alice2").await;
    sqlx::query(
        "INSERT INTO characters (account_name, charId, char_name, level, lastAccess) VALUES
         ('alice1', 1, 'Shen', 42, 1), ('alice2', 2, 'Mira', 7, 2)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let listed = app
        .clone()
        .oneshot(get_with_cookie("/api/v1/admin/accounts", &admin))
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let body = body_json(listed).await;
    assert_eq!(body["total"], 2, "both masters, the admin included");

    let alice = body["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["email"] == "alice@example.com")
        .expect("alice should be listed");
    assert_eq!(alice["gameAccounts"], 2);
    assert_eq!(alice["characters"], 2);
    assert_eq!(alice["isVerified"], true);
    assert_eq!(alice["accessLevel"], 0);

    // The search box answers "which master owns login alice2".
    let found = app
        .oneshot(get_with_cookie("/api/v1/admin/accounts?q=alice2", &admin))
        .await
        .unwrap();
    let body = body_json(found).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["accounts"][0]["email"], "alice@example.com");
}

#[tokio::test]
async fn admin_account_detail_shows_game_accounts_and_characters() {
    let (app, pool) = test_app().await;
    let admin = admin_master(&pool, &app, "admin@example.com").await;
    verified_master(&pool, &app, "alice@example.com").await;
    add_game_account(&pool, "alice@example.com", "alice1").await;
    sqlx::query(
        "INSERT INTO characters (account_name, charId, char_name, level, lastAccess)
         VALUES ('alice1', 1, 'Shen', 42, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let detail = app
        .clone()
        .oneshot(get_with_cookie("/api/v1/admin/accounts/alice%40example.com", &admin))
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let body = body_json(detail).await;
    assert_eq!(body["master"]["email"], "alice@example.com");
    assert_eq!(body["gameAccounts"][0]["login"], "alice1");
    // Admin-only columns the player API must never return.
    assert_eq!(body["gameAccounts"][0]["accessLevel"], 0);
    assert!(body["gameAccounts"][0].get("lastIp").is_some());
    assert_eq!(body["characters"][0]["name"], "Shen");
    // The hash must not leak to admins either.
    assert!(body["master"].get("password").is_none());
    assert!(body["gameAccounts"][0].get("password").is_none());

    let missing = app
        .oneshot(get_with_cookie("/api/v1/admin/accounts/nobody%40example.com", &admin))
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_admin_can_ban_and_unban_a_game_account() {
    let (app, pool) = test_app().await;
    let admin = admin_master(&pool, &app, "admin@example.com").await;
    add_game_account(&pool, "alice@example.com", "alice1").await;

    let banned = app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/admin/game-accounts/alice1/access-level",
            &admin,
            serde_json::json!({ "level": -100 }),
        ))
        .await
        .unwrap();
    assert_eq!(banned.status(), StatusCode::NO_CONTENT);

    let level: i32 = sqlx::query_scalar("SELECT accessLevel FROM accounts WHERE login = 'alice1'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(level, -100, "the login server refuses accessLevel < 0 — this is the game ban");

    let unbanned = app
        .oneshot(post_with_cookie(
            "/api/v1/admin/game-accounts/alice1/access-level",
            &admin,
            serde_json::json!({ "level": 0 }),
        ))
        .await
        .unwrap();
    assert_eq!(unbanned.status(), StatusCode::NO_CONTENT);

    let level: i32 = sqlx::query_scalar("SELECT accessLevel FROM accounts WHERE login = 'alice1'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(level, 0);
}

/// The invariant the whole admin design leans on: no request, however formed,
/// may move an access level above 0. A compromised admin session must not be
/// able to mint a GM.
#[tokio::test]
async fn the_admin_api_never_raises_an_access_level() {
    let (app, pool) = test_app().await;
    let admin = admin_master(&pool, &app, "admin@example.com").await;
    verified_master(&pool, &app, "alice@example.com").await;
    add_game_account(&pool, "alice@example.com", "alice1").await;

    for path in [
        "/api/v1/admin/game-accounts/alice1/access-level",
        "/api/v1/admin/accounts/alice%40example.com/access-level",
    ] {
        let response = app
            .clone()
            .oneshot(post_with_cookie(path, &admin, serde_json::json!({ "level": 100 })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path} must refuse level > 0");
    }

    let levels: Vec<(i32,)> = sqlx::query_as(
        "SELECT accessLevel FROM accounts WHERE email = 'alice@example.com' COLLATE NOCASE",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(levels.iter().all(|(l,)| *l == 0), "nothing may have been promoted: {levels:?}");
}

/// Equal admins cannot ban each other, and an admin cannot ban themself —
/// which would otherwise be the quickest way to lock everyone out.
#[tokio::test]
async fn an_admin_cannot_touch_a_peer_or_themself() {
    let (app, pool) = test_app().await;
    let first = admin_master(&pool, &app, "first@example.com").await;
    admin_master(&pool, &app, "second@example.com").await;

    for target in ["second%40example.com", "first%40example.com"] {
        let response = app
            .clone()
            .oneshot(post_with_cookie(
                &format!("/api/v1/admin/accounts/{target}/access-level"),
                &first,
                serde_json::json!({ "level": -1 }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "banning {target} must be refused");
    }

    let levels: Vec<(i32,)> = sqlx::query_as("SELECT accessLevel FROM accounts WHERE login IS NULL")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(levels.iter().all(|(l,)| *l == 100), "both admins must be untouched: {levels:?}");
}

#[tokio::test]
async fn a_banned_master_cannot_log_in_and_its_open_sessions_die() {
    let (app, pool) = test_app().await;
    let admin = admin_master(&pool, &app, "admin@example.com").await;
    let alice = verified_master(&pool, &app, "alice@example.com").await;

    let banned = app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/admin/accounts/alice%40example.com/access-level",
            &admin,
            serde_json::json!({ "level": -1 }),
        ))
        .await
        .unwrap();
    assert_eq!(banned.status(), StatusCode::NO_CONTENT);

    // The session that was already open dies on its next request — the account
    // is re-read every time, which is what stands in for revocable sessions.
    let me = app
        .clone()
        .oneshot(get_with_cookie("/api/v1/auth/me", &alice))
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);

    // A fresh login with the *correct* password says "suspended" — after the
    // password check, so it is not an enumeration oracle.
    let login = app
        .oneshot(post(
            "/api/v1/auth/login",
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
        ))
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::FORBIDDEN);
    assert_eq!(body_json(login).await["error"]["code"], "account_banned");
}

#[tokio::test]
async fn an_admin_resets_a_game_account_password() {
    let (app, pool) = test_app().await;
    let admin = admin_master(&pool, &app, "admin@example.com").await;
    add_game_account(&pool, "alice@example.com", "alice1").await;

    let weak = app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/admin/game-accounts/alice1/password",
            &admin,
            serde_json::json!({ "password": "short" }),
        ))
        .await
        .unwrap();
    assert_eq!(weak.status(), StatusCode::BAD_REQUEST, "the game's password rules still apply");

    let reset = app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/admin/game-accounts/alice1/password",
            &admin,
            serde_json::json!({ "password": "recovered-account" }),
        ))
        .await
        .unwrap();
    assert_eq!(reset.status(), StatusCode::NO_CONTENT);

    let hash: String = sqlx::query_scalar("SELECT password FROM accounts WHERE login = 'alice1'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        hash,
        commons::crypt::hash_password("recovered-account"),
        "must be the game's own hash or the client cannot use it"
    );

    // Masters have no admin password reset — recovery must prove inbox control.
    let master = app
        .oneshot(post_with_cookie(
            "/api/v1/admin/accounts/admin%40example.com/password",
            &admin,
            serde_json::json!({ "password": "does-not-matter" }),
        ))
        .await
        .unwrap();
    assert_eq!(master.status(), StatusCode::NOT_FOUND, "no such route may exist");
}

#[tokio::test]
async fn an_admin_can_force_verify_a_master() {
    let (app, pool) = test_app().await;
    let admin = admin_master(&pool, &app, "admin@example.com").await;

    // Registered but never clicked the link.
    app.clone()
        .oneshot(post(
            "/api/v1/auth/register",
            serde_json::json!({ "email": "alice@example.com", "password": "correct-horse" }),
        ))
        .await
        .unwrap();

    let verified = app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/admin/accounts/alice%40example.com/verify",
            &admin,
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(verified.status(), StatusCode::NO_CONTENT);

    let is_verified: Option<i64> = sqlx::query_scalar(
        "SELECT is_verified FROM accounts WHERE login IS NULL AND email = 'alice@example.com'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(is_verified, Some(1));
}

/// Rows the login server auto-created carry no master email, so the master
/// list can never reach them — the game-account search is their only way in.
#[tokio::test]
async fn game_account_search_reaches_masterless_rows() {
    let (app, pool) = test_app().await;
    let admin = admin_master(&pool, &app, "admin@example.com").await;

    sqlx::query(
        "INSERT INTO accounts (login, password, email, is_verified, lastactive, accessLevel)
         VALUES ('orphan1', 'x', NULL, NULL, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let found = app
        .oneshot(get_with_cookie("/api/v1/admin/game-accounts?q=orph", &admin))
        .await
        .unwrap();
    assert_eq!(found.status(), StatusCode::OK);
    let body = body_json(found).await;
    assert_eq!(body[0]["login"], "orphan1");
    assert_eq!(body[0]["email"], serde_json::Value::Null);
}
