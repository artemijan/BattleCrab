//! Raw ("RSA/ECB/nopadding") RSA keypair for the GS↔LS link
//! (`GameServerTable.initRSAKeys`: 512-bit, F4). The modulus travels to the
//! game server as `BigInteger.toByteArray()` — big-endian two's complement,
//! so a leading 0x00 appears when the top bit is set.

use num_bigint_dig::BigUint;
use rsa::traits::{PrivateKeyParts, PublicKeyParts};
use rsa::RsaPrivateKey;

pub struct RawRsaKeyPair {
    n: BigUint,
    d: BigUint,
    block_size: usize,
}

impl RawRsaKeyPair {
    pub fn generate(bits: usize) -> Self {
        let key = RsaPrivateKey::new(&mut rand::thread_rng(), bits).expect("RSA keygen failed");
        Self { n: key.n().clone(), d: key.d().clone(), block_size: bits / 8 }
    }

    pub fn from_parts(n: BigUint, d: BigUint, bits: usize) -> Self {
        Self { n, d, block_size: bits / 8 }
    }

    pub fn modulus(&self) -> &BigUint {
        &self.n
    }

    /// `BigInteger.toByteArray()` of the modulus.
    pub fn modulus_java_bytes(&self) -> Vec<u8> {
        let mut bytes = self.n.to_bytes_be();
        if bytes[0] & 0x80 != 0 {
            bytes.insert(0, 0);
        }
        bytes
    }

    /// Raw block decrypt: `block^d mod n`, output zero-padded to block size.
    pub fn decrypt_raw(&self, block: &[u8]) -> Vec<u8> {
        let c = BigUint::from_bytes_be(block);
        let m = c.modpow(&self.d, &self.n);
        let bytes = m.to_bytes_be();
        let mut out = vec![0u8; self.block_size.max(bytes.len())];
        let start = out.len() - bytes.len();
        out[start..].copy_from_slice(&bytes);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsa512_roundtrip_with_zero_stripping() {
        let pair = RawRsaKeyPair::generate(512);
        // Simulate the GS side: RSA-encrypt a 16-byte blowfish key in a
        // 64-byte block, then the LS decrypts and strips leading zeros.
        let mut key = [0u8; 16];
        key[0] = 0x5a; // ensure no accidental leading zero in the key itself
        commons_fill(&mut key[1..]);
        let mut plain = vec![0u8; 64];
        plain[48..].copy_from_slice(&key);
        let m = BigUint::from_bytes_be(&plain);
        let c = m.modpow(&BigUint::from(65537u32), pair.modulus());
        let mut block = vec![0u8; 64];
        let cb = c.to_bytes_be();
        block[64 - cb.len()..].copy_from_slice(&cb);

        let decrypted = pair.decrypt_raw(&block);
        let stripped: Vec<u8> = {
            let first = decrypted.iter().position(|&b| b != 0).unwrap_or(decrypted.len());
            decrypted[first..].to_vec()
        };
        assert_eq!(stripped, key);
    }

    fn commons_fill(buf: &mut [u8]) {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(37).wrapping_add(11);
        }
    }
}
