//! Port of `loginserver/network/ScrambledKeyPair.java` plus the raw
//! ("RSA/ECB/nopadding") decrypt used by `RequestAuthLogin`.

use num_bigint_dig::BigUint;
use rsa::RsaPrivateKey;
use rsa::traits::{PrivateKeyParts, PublicKeyParts};

pub struct ScrambledKeyPair {
    n: BigUint,
    d: BigUint,
    scrambled_modulus: [u8; 0x80],
}

impl ScrambledKeyPair {
    /// Generates a fresh RSA-1024 pair (Java: `KeyPairGenerator` with F4).
    pub fn generate() -> Self {
        let key = RsaPrivateKey::new(&mut rand::thread_rng(), 1024).expect("RSA keygen failed");
        Self::from_parts(key.n().clone(), key.d().clone())
    }

    pub fn from_parts(n: BigUint, d: BigUint) -> Self {
        let scrambled_modulus = scramble_modulus(&n);
        Self {
            n,
            d,
            scrambled_modulus,
        }
    }

    pub fn scrambled_modulus(&self) -> &[u8; 0x80] {
        &self.scrambled_modulus
    }

    /// Raw RSA: `block^d mod n`, no padding — the credential block decrypt.
    /// Input and output are 128-byte big-endian blocks.
    pub fn decrypt_raw(&self, block: &[u8; 0x80]) -> [u8; 0x80] {
        let c = BigUint::from_bytes_be(block);
        let m = c.modpow(&self.d, &self.n);
        let bytes = m.to_bytes_be();
        let mut out = [0u8; 0x80];
        out[0x80 - bytes.len()..].copy_from_slice(&bytes);
        out
    }
}

/// The four scramble transforms, 1:1 with the Java code.
fn scramble_modulus(modulus: &BigUint) -> [u8; 0x80] {
    let bytes = modulus.to_bytes_be();
    assert_eq!(bytes.len(), 0x80, "expected 1024-bit modulus");
    let mut m: [u8; 0x80] = bytes.try_into().unwrap();

    // Step 1: 0x4d-0x50 <-> 0x00-0x04.
    for i in 0..4 {
        m.swap(i, 0x4d + i);
    }
    // Step 2: xor first 0x40 bytes with last 0x40 bytes.
    for i in 0..0x40 {
        m[i] ^= m[0x40 + i];
    }
    // Step 3: xor bytes 0x0d-0x10 with bytes 0x34-0x38.
    for i in 0..4 {
        m[0x0d + i] ^= m[0x34 + i];
    }
    // Step 4: xor last 0x40 bytes with first 0x40 bytes.
    for i in 0..0x40 {
        m[0x40 + i] ^= m[i];
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint_dig::BigUint;

    #[test]
    fn raw_rsa_roundtrip() {
        let pair = ScrambledKeyPair::generate();
        // Encrypt with the public exponent, decrypt with d.
        let e = BigUint::from(65537u32);
        let mut plain = [0u8; 0x80];
        plain[0x5e..0x6c].copy_from_slice(b"testaccount\0\0\0");
        let m = BigUint::from_bytes_be(&plain);
        let c = m.modpow(&e, &pair.n);
        let mut block = [0u8; 0x80];
        let cb = c.to_bytes_be();
        block[0x80 - cb.len()..].copy_from_slice(&cb);
        assert_eq!(pair.decrypt_raw(&block), plain);
    }
}
