//! Stateless one-time links (PLAN_DASHBOARD.md §5.4).
//!
//! Password reset signs over the account's *current* password hash, which buys
//! single-use semantics with no storage: the moment the password changes, the
//! hash changes and every outstanding reset link stops verifying.
//!
//! Email verification signs over the pending address and is only honored by the
//! handler that writes `accounts.email` — so an address in that column is
//! verified by construction, replacing an `email_verified` column we have
//! nowhere to store.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

use super::{now_unix, SigningKey};

pub const RESET_TTL_SECS: i64 = 3600;
pub const VERIFY_EMAIL_TTL_SECS: i64 = 24 * 3600;

const PURPOSE_RESET: &str = "reset";
const PURPOSE_VERIFY: &str = "verify";

/// `purpose|login|payload|expiry|signature`, base64url'd as one opaque string.
fn encode(key: &SigningKey, purpose: &str, login: &str, payload: &str, ttl: i64) -> String {
    let expiry = (now_unix() + ttl).to_string();
    let sig = key.sign(&[
        purpose.as_bytes(),
        login.as_bytes(),
        payload.as_bytes(),
        expiry.as_bytes(),
    ]);
    let joined = format!("{purpose}|{login}|{payload}|{expiry}|{sig}");
    URL_SAFE_NO_PAD.encode(joined)
}

/// Returns `(login, payload)` for a structurally valid, unexpired, correctly
/// signed token of the requested purpose. Callers check the payload themselves.
fn decode(key: &SigningKey, want_purpose: &str, raw: &str) -> Option<(String, String)> {
    let decoded = URL_SAFE_NO_PAD.decode(raw).ok()?;
    let joined = String::from_utf8(decoded).ok()?;
    let mut parts = joined.splitn(5, '|');

    let purpose = parts.next()?;
    let login = parts.next()?;
    let payload = parts.next()?;
    let expiry_s = parts.next()?;
    let sig = parts.next()?;

    if purpose != want_purpose {
        return None;
    }
    if expiry_s.parse::<i64>().ok()? <= now_unix() {
        return None;
    }
    if !key.verify(
        &[purpose.as_bytes(), login.as_bytes(), payload.as_bytes(), expiry_s.as_bytes()],
        sig,
    ) {
        return None;
    }
    Some((login.to_string(), payload.to_string()))
}

/// Reset link. `current_password_hash` is what makes it single-use.
pub fn issue_reset(key: &SigningKey, login: &str, current_password_hash: &str) -> String {
    encode(key, PURPOSE_RESET, login, current_password_hash, RESET_TTL_SECS)
}

/// Returns the login iff the token is valid *and* the account's password has
/// not changed since it was issued.
pub fn verify_reset(key: &SigningKey, raw: &str, current_password_hash: &str) -> Option<String> {
    let (login, payload) = decode(key, PURPOSE_RESET, raw)?;
    (payload == current_password_hash).then_some(login)
}

/// Email confirmation link.
///
/// Subject and payload are the *same* address. There is no pending-address
/// flow: an account's address is its identity and cannot be changed from the
/// dashboard, so a token naming two addresses is not something this system
/// issues.
pub fn issue_verify_email(key: &SigningKey, email: &str) -> String {
    encode(key, PURPOSE_VERIFY, email, email, VERIFY_EMAIL_TTL_SECS)
}

/// Returns the address to confirm, iff the token is valid **and** names a
/// single address.
///
/// The equality check is load-bearing rather than cosmetic: the removed
/// change-email flow issued `(old, new)` tokens, and any of those still sitting
/// in an inbox would otherwise keep working and move an account after the
/// feature was withdrawn.
pub fn verify_email(key: &SigningKey, raw: &str) -> Option<String> {
    let (subject, payload) = decode(key, PURPOSE_VERIFY, raw)?;
    (payload == subject).then_some(subject)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "qUqP5cyxm6YcTAhz05Hph5gvu9M=";

    fn key() -> SigningKey {
        SigningKey::new("test-secret")
    }

    #[test]
    fn reset_token_roundtrips() {
        let t = issue_reset(&key(), "alice", HASH);
        assert_eq!(verify_reset(&key(), &t, HASH).as_deref(), Some("alice"));
    }

    #[test]
    fn reset_token_is_single_use_via_the_password_hash() {
        let t = issue_reset(&key(), "alice", HASH);
        // Password was changed (by using this very link, or any other way).
        assert!(verify_reset(&key(), &t, "NewHashAfterReset=").is_none());
    }

    #[test]
    fn a_verify_token_cannot_be_used_as_a_reset_token() {
        let t = issue_verify_email(&key(), "a@b.c");
        assert!(verify_reset(&key(), &t, HASH).is_none());
    }

    #[test]
    fn a_reset_token_cannot_be_used_as_a_verify_token() {
        let t = issue_reset(&key(), "alice", HASH);
        assert!(verify_email(&key(), &t).is_none());
    }

    #[test]
    fn verify_email_roundtrips_the_address() {
        let t = issue_verify_email(&key(), "alice@example.com");
        assert_eq!(verify_email(&key(), &t).as_deref(), Some("alice@example.com"));
    }

    /// A leftover change-email link from before that feature was removed is
    /// correctly signed and unexpired — the *only* thing that stops it moving
    /// an account is this equality check.
    #[test]
    fn a_token_naming_two_addresses_is_refused() {
        let forged = encode(
            &key(),
            PURPOSE_VERIFY,
            "alice@example.com",
            "attacker@example.com",
            VERIFY_EMAIL_TTL_SECS,
        );
        assert!(verify_email(&key(), &forged).is_none());
    }

    #[test]
    fn tokens_from_another_key_are_rejected() {
        let t = issue_reset(&SigningKey::new("other"), "alice", HASH);
        assert!(verify_reset(&key(), &t, HASH).is_none());
    }

    #[test]
    fn garbage_is_rejected_without_panicking() {
        for raw in ["", "!!!!", "YWJj", "YXxifGN8ZA"] {
            assert!(verify_reset(&key(), raw, HASH).is_none());
            assert!(verify_email(&key(), raw).is_none());
        }
    }
}
