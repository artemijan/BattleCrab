//! Stateless auth. There is no session table and no token table — v1 writes
//! nothing but `accounts.password` / `accounts.email` (PLAN_DASHBOARD.md §5).
//!
//! Sessions are signed cookies; reset/verify links are signed tokens. Both are
//! HMAC-SHA256 over the `DASHBOARD_SESSION_SECRET` signing key.

pub mod cookie;
pub mod ratelimit;
pub mod token;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// Signing key derived from config. Cloneable so handlers can hold it in state.
#[derive(Clone)]
pub struct SigningKey(Vec<u8>);

impl SigningKey {
    pub fn new(secret: &str) -> Self {
        Self(secret.as_bytes().to_vec())
    }

    fn mac(&self, parts: &[&[u8]]) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(&self.0).expect("hmac accepts any key length");
        // Length-prefix each part so ("ab","c") and ("a","bc") can't collide.
        for part in parts {
            mac.update(&(part.len() as u64).to_be_bytes());
            mac.update(part);
        }
        mac.finalize().into_bytes().to_vec()
    }

    pub fn sign(&self, parts: &[&[u8]]) -> String {
        URL_SAFE_NO_PAD.encode(self.mac(parts))
    }

    /// Constant-time verification — a timing-variable compare here leaks the
    /// signature one byte at a time.
    pub fn verify(&self, parts: &[&[u8]], signature: &str) -> bool {
        let Ok(provided) = URL_SAFE_NO_PAD.decode(signature) else {
            return false;
        };
        let expected = self.mac(parts);
        expected.ct_eq(&provided).into()
    }
}

/// Verify a plaintext password against the stored game hash.
///
/// This is `Base64(SHA1(pw))` — the scheme the game client requires (§3.1).
/// Compared in constant time even though the hash is public-ish, because the
/// comparison runs on attacker-supplied input on every login.
pub fn verify_password(plaintext: &str, stored_hash: &str) -> bool {
    let candidate = commons::crypt::hash_password(plaintext);
    candidate.as_bytes().ct_eq(stored_hash.as_bytes()).into()
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_against_the_game_hash() {
        // Same vector commons::crypt asserts, reached through our wrapper.
        assert!(verify_password("test", "qUqP5cyxm6YcTAhz05Hph5gvu9M="));
        assert!(!verify_password("Test", "qUqP5cyxm6YcTAhz05Hph5gvu9M="));
    }

    #[test]
    fn signature_roundtrips_and_rejects_tampering() {
        let key = SigningKey::new("secret");
        let sig = key.sign(&[b"alice", b"42"]);
        assert!(key.verify(&[b"alice", b"42"], &sig));
        assert!(!key.verify(&[b"alice", b"43"], &sig));
        assert!(!key.verify(&[b"bob", b"42"], &sig));
    }

    #[test]
    fn length_prefixing_prevents_field_confusion() {
        let key = SigningKey::new("secret");
        // Without length prefixes these two would produce the same MAC.
        assert_ne!(key.sign(&[b"ab", b"c"]), key.sign(&[b"a", b"bc"]));
    }

    #[test]
    fn a_different_key_does_not_verify() {
        let a = SigningKey::new("secret-a");
        let b = SigningKey::new("secret-b");
        let sig = a.sign(&[b"alice"]);
        assert!(!b.verify(&[b"alice"], &sig));
    }
}
