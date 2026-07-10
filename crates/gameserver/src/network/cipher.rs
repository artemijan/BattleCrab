//! Port of `gameserver/network/Encryption.java` — the game client's rolling XOR
//! cipher (not Blowfish, despite the config name). Verified byte-for-byte
//! against the Java class by golden vectors (`tests/cipher_vectors.rs`).
//!
//! Framing is identical to login (2-byte LE length header); only the packet
//! **body** is (de/en)crypted, with no checksum or padding.
//!
//! The 16-byte key is `[8 random bytes | 8 static bytes]`; the server sends the
//! client only the first 8 (the static tail is hard-coded in the client). Both
//! sides then roll bytes 8..12 of the key forward by the number of bytes
//! processed, independently per direction.

/// The static tail (bytes 8..16) of every game cipher key (`BlowFishKeygen`).
pub const KEY_STATIC_TAIL: [u8; 8] = [0xc8, 0x27, 0x93, 0x01, 0xa1, 0x6c, 0x31, 0x97];

pub struct Encryption {
    in_key: [u8; 16],
    out_key: [u8; 16],
    /// Java `_isEnabled`: the very first `encrypt` call is a pass-through that
    /// only flips this flag (so `KeyPacket` goes out in the clear); every call
    /// after that transforms. `decrypt` is a no-op while this is false.
    enabled: bool,
}

impl Encryption {
    pub fn new() -> Self {
        Self { in_key: [0; 16], out_key: [0; 16], enabled: false }
    }

    /// Java `setKey`: both directions start from the same key.
    pub fn set_key(&mut self, key: &[u8; 16]) {
        self.in_key = *key;
        self.out_key = *key;
    }

    /// Build the full 16-byte key from the 8 random bytes and the static tail.
    pub fn key_from_random(random8: &[u8; 8]) -> [u8; 16] {
        let mut key = [0u8; 16];
        key[..8].copy_from_slice(random8);
        key[8..].copy_from_slice(&KEY_STATIC_TAIL);
        key
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Encrypt `data` in place. The first call is a pass-through (see `enabled`).
    pub fn encrypt(&mut self, data: &mut [u8]) {
        if !self.enabled {
            self.enabled = true;
            return;
        }
        let mut prev = 0u8;
        for (i, b) in data.iter_mut().enumerate() {
            let e = *b ^ self.out_key[i & 0x0f] ^ prev;
            *b = e;
            prev = e;
        }
        shift_key(&mut self.out_key, data.len());
    }

    /// Decrypt `data` in place. No-op until the cipher is enabled.
    pub fn decrypt(&mut self, data: &mut [u8]) {
        if !self.enabled {
            return;
        }
        let mut prev = 0u8;
        for (i, b) in data.iter_mut().enumerate() {
            let enc = *b;
            *b = enc ^ self.in_key[i & 0x0f] ^ prev;
            prev = enc;
        }
        shift_key(&mut self.in_key, data.len());
    }
}

impl Default for Encryption {
    fn default() -> Self {
        Self::new()
    }
}

/// Java key shift: bytes 8..12 form a little-endian u32 incremented by the
/// number of bytes just processed.
fn shift_key(key: &mut [u8; 16], size: usize) {
    let v = u32::from_le_bytes([key[8], key[9], key[10], key[11]]).wrapping_add(size as u32);
    key[8..12].copy_from_slice(&v.to_le_bytes());
}
