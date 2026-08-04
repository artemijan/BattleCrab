//! Session cookie: `login|expiry|signature`, HMAC-signed, no server-side state.
//!
//! The signature covers the account's *current* password hash, so changing the
//! password invalidates every outstanding session — the one revocation case
//! that actually matters after a compromise (DASHBOARD.md §5.3).

use axum::http::HeaderMap;
use axum::http::header::{COOKIE, SET_COOKIE};

use super::{SigningKey, now_unix};

pub const COOKIE_NAME: &str = "bc_session";

/// Builds the `Set-Cookie` value for a fresh session.
pub fn issue(
    key: &SigningKey,
    login: &str,
    password_hash: &str,
    ttl_days: i64,
    secure: bool,
) -> String {
    let expiry = now_unix() + ttl_days * 86_400;
    let value = encode(key, login, password_hash, expiry);
    let max_age = ttl_days * 86_400;
    let secure_flag = if secure { "; Secure" } else { "" };
    format!("{COOKIE_NAME}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure_flag}")
}

/// Expires the cookie client-side. Nothing server-side to delete.
pub fn clear(secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    format!("{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure_flag}")
}

fn encode(key: &SigningKey, login: &str, password_hash: &str, expiry: i64) -> String {
    let expiry_s = expiry.to_string();
    let sig = key.sign(&[
        login.as_bytes(),
        expiry_s.as_bytes(),
        password_hash.as_bytes(),
    ]);
    format!("{login}|{expiry_s}|{sig}")
}

/// Returns the login the cookie authenticates, if the signature is valid, the
/// cookie has not expired, and the password hash still matches.
pub fn authenticate(key: &SigningKey, raw: &str, current_password_hash: &str) -> Option<String> {
    let mut parts = raw.splitn(3, '|');
    let login = parts.next()?;
    let expiry_s = parts.next()?;
    let sig = parts.next()?;

    let expiry: i64 = expiry_s.parse().ok()?;
    if expiry <= now_unix() {
        return None;
    }
    if !key.verify(
        &[
            login.as_bytes(),
            expiry_s.as_bytes(),
            current_password_hash.as_bytes(),
        ],
        sig,
    ) {
        return None;
    }
    Some(login.to_string())
}

/// Pulls our cookie out of the request headers, ignoring any others present.
pub fn extract(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(COOKIE)?.to_str().ok()?;
    raw.split(';')
        .filter_map(|c| c.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE_NAME)
        .map(|(_, value)| value.to_string())
}

/// The login a `Set-Cookie` header would authenticate — test/debug helper.
pub fn header_value(set_cookie: &HeaderMap) -> Option<String> {
    let raw = set_cookie.get(SET_COOKIE)?.to_str().ok()?;
    raw.split(';')
        .next()
        .and_then(|c| c.split_once('='))
        .map(|(_, v)| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "qUqP5cyxm6YcTAhz05Hph5gvu9M=";

    fn key() -> SigningKey {
        SigningKey::new("test-secret")
    }

    fn value_of(set_cookie: &str) -> String {
        set_cookie
            .split(';')
            .next()
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .to_string()
    }

    #[test]
    fn issued_cookie_authenticates() {
        let raw = value_of(&issue(&key(), "alice", HASH, 7, true));
        assert_eq!(authenticate(&key(), &raw, HASH).as_deref(), Some("alice"));
    }

    #[test]
    fn password_change_invalidates_the_session() {
        let raw = value_of(&issue(&key(), "alice", HASH, 7, true));
        // Same cookie, account now has a different hash.
        assert!(authenticate(&key(), &raw, "SomeOtherHashValue=").is_none());
    }

    #[test]
    fn expired_cookie_is_rejected() {
        let raw = value_of(&issue(&key(), "alice", HASH, -1, true));
        assert!(authenticate(&key(), &raw, HASH).is_none());
    }

    #[test]
    fn tampered_login_is_rejected() {
        let raw = value_of(&issue(&key(), "alice", HASH, 7, true));
        let forged = raw.replacen("alice", "admin", 1);
        assert!(authenticate(&key(), &forged, HASH).is_none());
    }

    #[test]
    fn garbage_is_rejected_without_panicking() {
        for raw in ["", "|", "a|b", "a|b|c", "a|notanumber|c"] {
            assert!(authenticate(&key(), raw, HASH).is_none());
        }
    }

    #[test]
    fn extract_finds_our_cookie_among_others() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            "theme=dark; bc_session=abc; other=1".parse().unwrap(),
        );
        assert_eq!(extract(&headers).as_deref(), Some("abc"));
    }

    #[test]
    fn extract_returns_none_when_absent() {
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, "theme=dark".parse().unwrap());
        assert!(extract(&headers).is_none());
    }

    #[test]
    fn secure_flag_is_omitted_for_plain_http_dev() {
        assert!(issue(&key(), "a", HASH, 7, true).contains("; Secure"));
        assert!(!issue(&key(), "a", HASH, 7, false).contains("; Secure"));
    }
}
