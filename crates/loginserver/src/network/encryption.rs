//! Port of `loginserver/network/LoginEncryption.java`.
//!
//! Every packet is Blowfish-encrypted with the client's key, except the very
//! first server packet (`Init`), which uses an XOR pass plus a well-known
//! static key — after which the cipher switches to the per-client key.

use commons::crypt::NewCrypt;
use commons::util::rnd;

const STATIC_BLOWFISH_KEY: [u8; 16] = [
    0x6b, 0x60, 0xcb, 0x5b, 0x82, 0xce, 0x90, 0xb1, 0xcc, 0x2b, 0x6c, 0x55, 0x6c, 0x6c, 0x6c, 0x6c,
];

pub struct LoginEncryption {
    crypt: NewCrypt,
    static_mode: bool,
}

impl LoginEncryption {
    pub fn new(key: &[u8]) -> Self {
        Self {
            crypt: NewCrypt::new(key),
            static_mode: true,
        }
    }

    /// Decrypts in place and validates the checksum. `false` = corrupt packet
    /// (Java closes the connection).
    pub fn decrypt(&self, data: &mut [u8]) -> bool {
        if !data.len().is_multiple_of(8) {
            return false;
        }
        self.crypt.decrypt(data);
        NewCrypt::verify_checksum(data)
    }

    /// `encryptedSize` from Java: reserve for checksum (or the XOR key int),
    /// pad to the 8-byte block, plus one extra block.
    fn encrypted_size(&self, data_size: usize) -> usize {
        let mut size = data_size + if self.static_mode { 8 } else { 4 };
        size += 8 - (size % 8);
        size += 8;
        size
    }

    /// Consumes the packet body and returns the encrypted payload.
    pub fn encrypt(&mut self, body: Vec<u8>) -> Vec<u8> {
        let mut data = body;
        data.resize(self.encrypted_size(data.len()), 0);
        if self.static_mode {
            NewCrypt::enc_xor_pass(&mut data, rnd::next_int());
            NewCrypt::new(&STATIC_BLOWFISH_KEY).crypt(&mut data);
            self.static_mode = false;
        } else {
            NewCrypt::append_checksum(&mut data);
            self.crypt.crypt(&mut data);
        }
        data
    }
}
