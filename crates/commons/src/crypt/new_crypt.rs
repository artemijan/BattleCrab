//! Port of `loginserver/crypt/NewCrypt.java` (+ `BlowfishEngine.java`).
//!
//! The Java engine is standard Blowfish ECB except 32-bit words are packed
//! little-endian (`bits32ToBytes` writes LSB first, buffers are LE), which is
//! exactly `blowfish::Blowfish<byteorder::LE>`. All offsets/sizes below
//! operate on a slice starting at the packet payload (every Java call site
//! passes offset 0).

use std::slice;

use blowfish::Blowfish;
use blowfish::cipher::KeyInit;
use cipher::{Block, BlockCipherDecrypt, BlockCipherEncrypt};

type BlowfishLe = Blowfish<byteorder::LE>;

pub struct NewCrypt {
    cipher: BlowfishLe,
}

impl NewCrypt {
    pub fn new(key: &[u8]) -> Self {
        Self {
            cipher: BlowfishLe::new_from_slice(key).expect("invalid blowfish key length"),
        }
    }

    /// Encrypts `data` in-place using 8-byte ECB blocks.
    ///
    /// # Safety
    ///
    /// This function uses an `unsafe` transmute-style pointer cast to view `data`
    /// as a slice of `Block<BlowfishLe>`. This is sound because:
    /// 1. `Block<BlowfishLe>` is a `#[repr(transparent)]` wrapper over `GenericArray<u8, U8>`,
    ///    which has the exact same memory layout and alignment as an 8-byte slice (`[u8; 8]`).
    /// 2. `data.as_mut_ptr()` yields a valid, mutable, and properly aligned byte pointer.
    /// 3. The slice length is strictly limited to `data.len() / 8`, guaranteeing that the
    ///    constructed slice does not read or write past the allocation boundary of `data`.
    /// 4. Any remaining tail bytes (`data.len() % 8`) are safely ignored and left unencrypted.
    pub fn crypt(&self, data: &mut [u8]) {
        assert_eq!(
            data.len() % 8,
            0,
            "Data length must be a multiple of 8 bytes"
        );
        let num_blocks = data.len() / 8;
        // SAFETY: `Block<BlowfishLe>` is repr(transparent) over [u8; 8].
        // We only cast whole 8-byte blocks up to `num_blocks`.
        let blocks = unsafe {
            slice::from_raw_parts_mut(data.as_mut_ptr() as *mut Block<BlowfishLe>, num_blocks)
        };
        self.cipher.encrypt_blocks(blocks);
    }

    /// Decrypts `data` in-place using 8-byte ECB blocks.
    ///
    /// # Safety
    ///
    /// Identical safety guarantees as [`crypt`]: reinterprets whole 8-byte chunks of
    /// `data` as a contiguous slice of `Block<BlowfishLe>` for maximum batch processing speed.
    pub fn decrypt(&self, data: &mut [u8]) {
        assert_eq!(
            data.len() % 8,
            0,
            "Data length must be a multiple of 8 bytes"
        );
        let num_blocks = data.len() / 8;
        // SAFETY: `Block<BlowfishLe>` is repr(transparent) over [u8; 8].
        // We only cast whole 8-byte blocks up to `num_blocks`.
        let blocks = unsafe {
            slice::from_raw_parts_mut(data.as_mut_ptr() as *mut Block<BlowfishLe>, num_blocks)
        };
        self.cipher.decrypt_blocks(blocks);
    }

    /// `NewCrypt.verifyChecksum` — XOR of all LE i32 words except the last,
    /// compared against the last word.
    pub fn verify_checksum(data: &[u8]) -> bool {
        let size = data.len();
        if (size & 3) != 0 || size <= 4 {
            return false;
        }
        let mut checksum: i32 = 0;
        let count = size - 4;
        let mut i = 0;
        while i < count {
            checksum ^= read_i32_le(data, i);
            i += 4;
        }
        read_i32_le(data, i) == checksum
    }

    /// `NewCrypt.appendChecksum` — writes the XOR checksum into the last word.
    pub fn append_checksum(data: &mut [u8]) {
        let size = data.len();
        let mut checksum: i32 = 0;
        let count = size - 4;
        let mut i = 0;
        while i < count {
            checksum ^= read_i32_le(data, i);
            i += 4;
        }
        write_i32_le(data, i, checksum);
    }

    /// `NewCrypt.encXORPass` — the XOR pass applied to the very first server
    /// packet (`Init`), keyed with a random int whose final state is stored in
    /// the last 8 bytes.
    pub fn enc_xor_pass(data: &mut [u8], key: i32) {
        let stop = data.len() as isize - 8;
        let mut pos: isize = 4;
        let mut ecx = key;
        while pos < stop {
            let edx = read_i32_le(data, pos as usize);
            ecx = ecx.wrapping_add(edx);
            write_i32_le(data, pos as usize, edx ^ ecx);
            pos += 4;
        }
        write_i32_le(data, pos as usize, ecx);
    }
}

fn read_i32_le(data: &[u8], index: usize) -> i32 {
    i32::from_le_bytes(data[index..index + 4].try_into().unwrap())
}

fn write_i32_le(data: &mut [u8], index: usize, value: i32) {
    data[index..index + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_roundtrip() {
        let mut data = vec![0u8; 16];
        data[..12].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        NewCrypt::append_checksum(&mut data);
        assert!(NewCrypt::verify_checksum(&data));
    }

    #[test]
    fn blowfish_roundtrip() {
        let crypt = NewCrypt::new(b"_;5.]94-31==-%xT!^[$\0");
        let plain: Vec<u8> = (0u8..32).collect();
        let mut data = plain.clone();
        crypt.crypt(&mut data);
        assert_ne!(data, plain);
        crypt.decrypt(&mut data);
        assert_eq!(data, plain);
    }
}
