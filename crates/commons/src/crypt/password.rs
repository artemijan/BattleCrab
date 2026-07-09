//! Account password hash: `Base64(SHA-1(password))`, as in
//! `LoginController.retriveAccountInfo`.

use base64::Engine;
use sha1::{Digest, Sha1};

pub fn hash_password(password: &str) -> String {
    let digest = Sha1::digest(password.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(digest)
}

#[cfg(test)]
mod tests {
    #[test]
    fn known_hash() {
        // Java: Base64(SHA("test")) — well-known vector.
        assert_eq!(super::hash_password("test"), "qUqP5cyxm6YcTAhz05Hph5gvu9M=");
    }
}
