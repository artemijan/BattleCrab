//! Shared GS↔LS link crypto (`GameServerThread`/`LoginServerThread`): the
//! static Blowfish key, the payload encrypt/decrypt (`[data + 4-byte checksum +
//! pad-to-8]`, Blowfish ECB — no XOR pass, no static/session switch), and the
//! RSA-`nopadding` public-key encrypt used by the game server to ship its
//! session Blowfish key. Used by both the login server (LS side) and the game
//! server (GS side), so it lives here.

use num_bigint_dig::BigUint;

use super::NewCrypt;

/// Initial GS-link Blowfish key (`_;v.]05-31!|+-%xT!^[$` + NUL).
pub const GS_STATIC_BLOWFISH_KEY: &[u8] = b"_;v.]05-31!|+-%xT!^[$\x00";

/// Serialize an outbound GS-link packet: append the checksum slot, pad the body
/// to a multiple of 8, write the checksum, then Blowfish-encrypt.
pub fn gs_encrypt(crypt: &NewCrypt, mut body: Vec<u8>) -> Vec<u8> {
    body.extend_from_slice(&[0u8; 4]); // reserved for checksum
    while body.len() % 8 != 0 {
        body.push(0);
    }
    NewCrypt::append_checksum(&mut body);
    crypt.crypt(&mut body);
    body
}

/// Decrypt + checksum-verify an inbound GS-link payload in place.
pub fn gs_decrypt(crypt: &NewCrypt, data: &mut [u8]) -> bool {
    if data.len() % 8 != 0 {
        return false;
    }
    crypt.decrypt(data);
    NewCrypt::verify_checksum(data)
}

/// The LS public modulus as the game server receives it in `InitLS`
/// (`new BigInteger(bytes)`), for RSA-encrypting the session Blowfish key with
/// `Cipher "RSA/ECB/nopadding"` (public exponent F4 = 65537).
pub struct RsaPublicModulus {
    n: BigUint,
    block_size: usize,
}

impl RsaPublicModulus {
    /// Parse the modulus bytes from `InitLS`. Java builds a *signed*
    /// `BigInteger`; the value is positive, so a leading 0x00 sign byte is
    /// harmless for the unsigned interpretation.
    pub fn from_java_bytes(bytes: &[u8]) -> Self {
        let n = BigUint::from_bytes_be(bytes);
        let block_size = (n.bits() + 7) / 8;
        Self { n, block_size }
    }

    /// `data^F4 mod n`, big-endian, left-padded to the modulus byte length —
    /// exactly Java's `RSA/ECB/nopadding` ENCRYPT of `data`.
    pub fn encrypt_raw(&self, data: &[u8]) -> Vec<u8> {
        let m = BigUint::from_bytes_be(data);
        let c = m.modpow(&BigUint::from(65537u32), &self.n);
        let cb = c.to_bytes_be();
        let mut out = vec![0u8; self.block_size.max(cb.len())];
        let start = out.len() - cb.len();
        out[start..].copy_from_slice(&cb);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypt::RawRsaKeyPair;

    #[test]
    fn gs_roundtrip() {
        let crypt = NewCrypt::new(GS_STATIC_BLOWFISH_KEY);
        let body = vec![0x00, 1, 2, 3, 4];
        let mut encrypted = gs_encrypt(&crypt, body.clone());
        assert_eq!(encrypted.len() % 8, 0);
        assert!(gs_decrypt(&crypt, &mut encrypted));
        assert_eq!(&encrypted[..body.len()], &body[..]);
    }

    #[test]
    fn rsa_encrypt_matches_ls_decrypt() {
        // GS side encrypts the blowfish key with the modulus; LS side decrypts.
        let pair = RawRsaKeyPair::generate(512);
        let pubmod = RsaPublicModulus::from_java_bytes(&pair.modulus_java_bytes());
        let key = crate::util::generate_hex(40); // non-zero bytes, like Java
        let block = pubmod.encrypt_raw(&key);
        let decrypted = pair.decrypt_raw(&block);
        let first = decrypted.iter().position(|&b| b != 0).unwrap_or(decrypted.len());
        assert_eq!(&decrypted[first..], &key[..]);
    }
}
