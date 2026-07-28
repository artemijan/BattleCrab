//! One error type for every handler, rendered as the envelope the SPA expects:
//! `{"error": {"code": "...", "message": "..."}}`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),

    /// Deliberately vague at the edge: never distinguish "no such account" from
    /// "wrong password" (PLAN_DASHBOARD.md §5.2).
    #[error("invalid credentials")]
    InvalidCredentials,

    #[error("authentication required")]
    Unauthorized,

    #[error("username is already taken")]
    LoginTaken,

    /// Yes, this lets someone probe which addresses are registered. So does
    /// `LoginTaken`, and the alternative — "check your inbox" for an address
    /// that already has an account — strands users who forgot they signed up.
    /// The password-reset flow stays non-enumerating; this one does not.
    #[error("an account already exists for that email address")]
    EmailTaken,

    /// The address on file has not been confirmed, and the endpoint requires it.
    #[error("please confirm your email address first")]
    EmailNotVerified,

    /// The master account already owns as many game accounts as it may.
    #[error("you can have at most {0} game accounts")]
    TooManyGameAccounts(usize),

    #[error("registration is currently disabled")]
    RegistrationDisabled,

    /// Authenticated, but not allowed to do this — the admin gate, and the
    /// peer-protection rule (an admin cannot touch an account at or above their
    /// own level).
    #[error("you don't have permission to do that")]
    Forbidden,

    /// The account exists and the password was right, but `accessLevel < 0`.
    /// Only ever returned *after* the password verified, so it is not an
    /// enumeration oracle.
    #[error("this account is suspended")]
    AccountBanned,

    #[error("not found")]
    NotFound,

    #[error("too many attempts, try again later")]
    RateLimited,

    #[error("this link is invalid or has expired")]
    InvalidToken,

    #[error("internal error")]
    Internal(#[from] anyhow_lite::Error),
}

/// Minimal stand-in so we don't pull `anyhow` in just for opaque internals.
pub mod anyhow_lite {
    #[derive(Debug, thiserror::Error)]
    #[error("{0}")]
    pub struct Error(pub String);
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        // The message is logged, never returned — it can carry SQL and paths.
        tracing::error!("database error: {e}");
        ApiError::Internal(anyhow_lite::Error("database error".into()))
    }
}

impl ApiError {
    fn parts(&self) -> (StatusCode, &'static str) {
        match self {
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "invalid_credentials"),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            ApiError::LoginTaken => (StatusCode::CONFLICT, "login_taken"),
            ApiError::EmailTaken => (StatusCode::CONFLICT, "email_taken"),
            ApiError::EmailNotVerified => (StatusCode::FORBIDDEN, "email_not_verified"),
            ApiError::TooManyGameAccounts(_) => (StatusCode::CONFLICT, "too_many_game_accounts"),
            ApiError::RegistrationDisabled => (StatusCode::FORBIDDEN, "registration_disabled"),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            ApiError::AccountBanned => (StatusCode::FORBIDDEN, "account_banned"),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
            ApiError::InvalidToken => (StatusCode::BAD_REQUEST, "invalid_token"),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = self.parts();
        let message = self.to_string();
        (
            status,
            Json(json!({ "error": { "code": code, "message": message } })),
        )
            .into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
