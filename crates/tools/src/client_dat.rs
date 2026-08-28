//! Decrypts and re-encrypts the Lineage 2 client's `system` files.
//!
//! The client keeps its data files (`*.dat`, some `*.ini`, `*.u`, `*.int`)
//! behind a 28-byte `Lineage2VerNNN` header naming the cipher that follows.
//! This module turns a client directory into a plaintext mirror and back, so
//! the datapack we port from can be diffed against what the client actually
//! ships — `ItemName-eu.dat` and `SkillName-eu.dat` are the client-side half of
//! the item and skill tables in `dist/game/data`.
//!
//! # The formats
//!
//! **Ver111 / Ver120 / Ver121** — a constant byte XOR over everything past the
//! header. Symmetric, so one code path serves both directions, and a re-encrypt
//! reproduces the original file byte for byte.
//!
//! **Ver411 / Ver412 / Ver413 / Ver414** — raw RSA (no padding) over 128-byte
//! blocks wrapping a zlib stream, then a 20-byte footer:
//!
//! ```text
//! [28] "Lineage2Ver413" UTF-16LE
//! [ n] 128-byte RSA blocks, each decrypting to
//!          be_u32 payload_len | zero fill | payload (right-aligned, 4-byte pad)
//!      concatenated payloads = le_u32 inflated_len | zlib stream
//! [20] footer: CRC32 of everything above at [12..16], little-endian
//! ```
//!
//! # Which key, and why only 413 round-trips
//!
//! NCsoft published only the *public* exponents for its own 411/412/414 keys,
//! so those are decrypt-only — the tool reads such a file but cannot write one.
//!
//! Ver413 is the interesting case. Our client's files do **not** use NCsoft's
//! retail 413 key; they use the community "encdec" keypair, whose private
//! exponent is known, so the round trip closes. [`RETAIL_413`] is still tried
//! as a fallback on decrypt so a genuine retail client can at least be read.
//!
//! A re-encrypted RSA file is not byte-identical to the original — our deflate
//! emits an equally valid but differently-sized stream — which is why the
//! round-trip test asserts on the *plaintext*, not the ciphertext.
//!
//! # The CRC is not load-bearing
//!
//! Retail files carry a real CRC32 in the footer and we reproduce it, but the
//! client does not verify it: `ItemName-eu.dat` as shipped in our own client
//! has the all-zero footer that l2clientdat writes, and the client loads it.
//! Writing the real one costs nothing and keeps files indistinguishable from
//! retail.
//!
//! Cipher parameters are transcribed from MobiusDevelopment/l2clientdat's
//! `dist/config/cryptVersion.xml`.

use crc32fast::Hasher;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use num_bigint_dig::BigUint;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// `"Lineage2VerNNN"` encoded UTF-16LE.
pub const HEADER_LEN: usize = 28;
/// RSA modulus size — every block in and out is exactly this wide.
const RSA_BLOCK: usize = 128;
/// Plaintext bytes a single block can carry (4 go to the length prefix).
const RSA_PAYLOAD: usize = 124;
/// Trailing bytes that are *not* ciphertext.
const RSA_FOOTER: usize = 20;
/// Name of the sidecar `decrypt` writes and `encrypt` reads back.
pub const MANIFEST_NAME: &str = ".l2dat-manifest.json";
/// Manifest value for a file that carried no L2 header.
const PLAIN: &str = "plain";

const HEADER_PREFIX: &str = "Lineage2Ver";

/// An RSA crypt version. `encrypt_exp` is `None` where only NCsoft holds the
/// private exponent, which makes that version decrypt-only.
pub struct RsaKey {
    pub modulus: &'static str,
    pub decrypt_exp: &'static str,
    pub encrypt_exp: Option<&'static str>,
}

/// The community "encdec" 413 keypair — the one our client's files use, and
/// the only RSA version this tool can write.
const ENCDEC_413: RsaKey = RsaKey {
    modulus: concat!(
        "75B4D6DE5C016544068A1ACF125869F43D2E09FC55B8B1E289556DAF9B875763",
        "5593446288B3653DA1CE91C87BB1A5C18F16323495C55D7D72C0890A83F69BFD",
        "1FD9434EB1C02F3E4679EDFA43309319070129C267C85604D87BB65BAE205DE3",
        "707AF1D2108881ABB567C3B3D069AE67C3A4C6A3AA93D26413D4C66094AE2039",
    ),
    decrypt_exp: "1d",
    encrypt_exp: Some(concat!(
        "30b4c2d798d47086145c75063c8e841e719776e400291d7838d3e6c4405b504c6",
        "a07f8fca27f32b86643d2649d1d5f124cdd0bf272f0909dd7352fe10a77b34d8",
        "31043d9ae541f8263c6fe3d1c14c2f04e43a7253a6dda9a8c1562cbd493c1b63",
        "1a1957618ad5dfe5ca28553f746e2fc6f2db816c7db223ec91e955081c1de65",
    )),
};

/// NCsoft's retail 413 key. Decrypt-only, and tried only as a fallback — a
/// retail client's files can be read, but never rewritten.
pub const RETAIL_413: RsaKey = RsaKey {
    modulus: concat!(
        "97df398472ddf737ef0a0cd17e8d172f0fef1661a38a8ae1d6e829bc1c6e4c3c",
        "fc19292dda9ef90175e46e7394a18850b6417d03be6eea274d3ed1dde5b5d7bd",
        "e72cc0a0b71d03608655633881793a02c9a67d9ef2b45eb7c08d4be329083ce4",
        "50e68f7867b6749314d40511d09bc5744551baa86a89dc38123dc1668fd72d83",
    ),
    decrypt_exp: "35",
    encrypt_exp: None,
};

const ORIGINAL_411: RsaKey = RsaKey {
    modulus: concat!(
        "8c9d5da87b30f5d7cd9dc88c746eaac5bb180267fa11737358c4c95d9adf59dd",
        "37689f9befb251508759555d6fe0eca87bebe0a10712cf0ec245af84cd22eb4c",
        "b675e98eaf5799fca62a20a2baa4801d5d70718dcd43283b8428f1387aec6600",
        "f937bfc7bb72404d187d3a9c438f1ffce9ce365dccf754232ff6def038a41385",
    ),
    decrypt_exp: "1d",
    encrypt_exp: None,
};

const ORIGINAL_412: RsaKey = RsaKey {
    modulus: concat!(
        "a465134799cf2c45087093e7d0f0f144e6d528110c08f674730d436e40827330",
        "eccea46e70acf10cdda7d8f710e3b44dcca931812d76cd7494289bca8b73823f",
        "57efc0515b97e4a2a02612ccfa719cf7885104b06f2e7e2cc967b62e3d3b1aad",
        "b925db94cbc8cd3070a4bb13f7e202c7733a67b1b94c1ebc0afcbe1a63b448cf",
    ),
    decrypt_exp: "25",
    encrypt_exp: None,
};

const ORIGINAL_414: RsaKey = RsaKey {
    modulus: concat!(
        "ad70257b2316ce09dfaf2ebc3f63b3d673b0c98a403950e26bb87379b11e17ae",
        "d0e45af23e7171e5ec1fbc8d1ae32ffb7801b31266eef9c334b53469d4b7cbe8",
        "3284273d35a9aab49b453e7012f374496c65f8089f5d134b0eb3d1e3b22051ed",
        "5977a6dd68c4f85785dfcc9f4412c81681944fc4b8ce27caf0242deaa5762e8d",
    ),
    decrypt_exp: "25",
    encrypt_exp: None,
};

/// How a given `Lineage2VerNNN` is enciphered.
pub enum Cipher {
    /// A constant byte XOR over everything past the header.
    Xor(u8),
    Rsa(&'static RsaKey),
}

/// Resolve a header version (`"413"`) to its cipher, or `None` if unknown.
pub fn cipher_for(version: &str) -> Option<Cipher> {
    Some(match version {
        "111" => Cipher::Xor(172),
        "120" | "121" => Cipher::Xor(230),
        "411" => Cipher::Rsa(&ORIGINAL_411),
        "412" => Cipher::Rsa(&ORIGINAL_412),
        "413" => Cipher::Rsa(&ENCDEC_413),
        "414" => Cipher::Rsa(&ORIGINAL_414),
        _ => return None,
    })
}

/// The version named in a file's header, or `None` when it carries none.
pub fn read_version(data: &[u8]) -> Option<String> {
    if data.len() < HEADER_LEN {
        return None;
    }
    let units: Vec<u16> = data[..HEADER_LEN]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .collect();
    let text = String::from_utf16(&units).ok()?;
    text.strip_prefix(HEADER_PREFIX).map(str::to_owned)
}

fn header_bytes(version: &str) -> Vec<u8> {
    format!("{HEADER_PREFIX}{version}")
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Bytes a block pads with so its payload ends on a 4-byte boundary. Java
/// spells this `(-size & 1) + (-size & 2)`, which is the same value.
fn align_pad(len: usize) -> usize {
    (4 - (len % 4)) % 4
}

// --- ciphers ----------------------------------------------------------------

fn xor_apply(body: &[u8], key: u8) -> Vec<u8> {
    body.iter().map(|b| b ^ key).collect()
}

fn to_block(v: &BigUint) -> Result<[u8; RSA_BLOCK], String> {
    let raw = v.to_bytes_be();
    if raw.len() > RSA_BLOCK {
        return Err(format!(
            "block overflowed the modulus ({} bytes)",
            raw.len()
        ));
    }
    let mut block = [0u8; RSA_BLOCK];
    block[RSA_BLOCK - raw.len()..].copy_from_slice(&raw);
    Ok(block)
}

fn rsa_decrypt_with(data: &[u8], key: &RsaKey) -> Result<Vec<u8>, String> {
    let modulus = BigUint::parse_bytes(key.modulus.as_bytes(), 16)
        .ok_or_else(|| "bad modulus literal".to_string())?;
    let exponent = BigUint::parse_bytes(key.decrypt_exp.as_bytes(), 16)
        .ok_or_else(|| "bad exponent literal".to_string())?;

    let end = data.len().saturating_sub(RSA_FOOTER);
    let body = &data[HEADER_LEN.min(end)..end];
    // Only whole blocks are ciphertext; a ragged tail is padding the client
    // ignores too.
    let body = &body[..body.len() - body.len() % RSA_BLOCK];

    let mut out = Vec::with_capacity(body.len());
    for cipher in body.as_chunks::<RSA_BLOCK>().0 {
        let chunk = to_block(&BigUint::from_bytes_be(cipher).modpow(&exponent, &modulus))?;
        let size = i32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        // A wrong key yields uniformly random blocks, so this is what rejects
        // it — the length is the only self-describing field a block has.
        if size < 0 || size as usize > RSA_PAYLOAD {
            return Err(format!("implausible block length {size} (wrong key?)"));
        }
        let size = size as usize;
        let start = RSA_BLOCK - size - align_pad(size);
        out.extend_from_slice(&chunk[start..start + size]);
    }

    if out.len() < 4 {
        return Err("no payload recovered".to_string());
    }
    let declared = u32::from_le_bytes([out[0], out[1], out[2], out[3]]) as usize;
    let mut plain = Vec::with_capacity(declared);
    ZlibDecoder::new(&out[4..])
        .read_to_end(&mut plain)
        .map_err(|e| format!("inflate failed: {e} (wrong key?)"))?;
    if plain.len() != declared {
        return Err(format!(
            "length mismatch: header says {declared}, inflated {}",
            plain.len()
        ));
    }
    Ok(plain)
}

fn rsa_encrypt(plain: &[u8], version: &str, key: &RsaKey) -> Result<Vec<u8>, String> {
    let exp = key.encrypt_exp.ok_or_else(|| {
        format!(
            "no private exponent is known for Ver{version}; only Ver413 and the \
             XOR versions can be written"
        )
    })?;
    let modulus = BigUint::parse_bytes(key.modulus.as_bytes(), 16)
        .ok_or_else(|| "bad modulus literal".to_string())?;
    let exponent = BigUint::parse_bytes(exp.as_bytes(), 16)
        .ok_or_else(|| "bad exponent literal".to_string())?;

    // The inflated length goes in front of the zlib stream, little-endian;
    // `decrypt` checks it to prove the whole chain came out right.
    let mut payload = (plain.len() as u32).to_le_bytes().to_vec();
    let mut encoder = ZlibEncoder::new(payload, Compression::new(6));
    encoder
        .write_all(plain)
        .map_err(|e| format!("deflate failed: {e}"))?;
    payload = encoder
        .finish()
        .map_err(|e| format!("deflate failed: {e}"))?;

    let mut out = header_bytes(version);
    for piece in payload.chunks(RSA_PAYLOAD) {
        let mut block = [0u8; RSA_BLOCK];
        block[..4].copy_from_slice(&(piece.len() as u32).to_be_bytes());
        let start = RSA_BLOCK - piece.len() - align_pad(piece.len());
        block[start..start + piece.len()].copy_from_slice(piece);
        let cipher = BigUint::from_bytes_be(&block).modpow(&exponent, &modulus);
        out.extend_from_slice(&to_block(&cipher)?);
    }

    let mut hasher = Hasher::new();
    hasher.update(&out);
    let mut footer = [0u8; RSA_FOOTER];
    footer[12..16].copy_from_slice(&hasher.finalize().to_le_bytes());
    out.extend_from_slice(&footer);
    Ok(out)
}

/// Recover the plaintext of a file whose header named `version`.
pub fn decrypt(data: &[u8], version: &str) -> Result<Vec<u8>, String> {
    match cipher_for(version) {
        Some(Cipher::Xor(key)) => Ok(xor_apply(&data[HEADER_LEN.min(data.len())..], key)),
        Some(Cipher::Rsa(key)) => rsa_decrypt_with(data, key).or_else(|err| {
            // A retail client's 413 files use a different key than ours; fall
            // back before giving up, but report the primary key's failure.
            if version == "413" {
                rsa_decrypt_with(data, &RETAIL_413).map_err(|_| err)
            } else {
                Err(err)
            }
        }),
        None => Err(format!("unsupported crypt version Ver{version}")),
    }
}

/// Re-encipher plaintext back into the `version` the client expects.
pub fn encrypt(plain: &[u8], version: &str) -> Result<Vec<u8>, String> {
    match cipher_for(version) {
        Some(Cipher::Xor(key)) => {
            let mut out = header_bytes(version);
            out.extend_from_slice(&xor_apply(plain, key));
            Ok(out)
        }
        Some(Cipher::Rsa(key)) => rsa_encrypt(plain, version, key),
        None => Err(format!("unsupported crypt version Ver{version}")),
    }
}

// --- directory driver -------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Decrypt,
    Encrypt,
}

/// What to convert. Nothing here is read from the environment.
pub struct Config<'a> {
    pub mode: Mode,
    pub in_dir: &'a Path,
    pub out_dir: &'a Path,
    /// Also mirror files that carry no L2 header. Off by default: the client's
    /// executables and libraries are ~200 MB of bytes no one edits, and they
    /// stay valid in place.
    pub include_plain: bool,
}

/// One file the run enciphered, or tried to.
pub struct Entry {
    pub rel: String,
    pub version: String,
    pub error: Option<String>,
}

pub struct Report {
    pub entries: Vec<Entry>,
    /// Unencrypted files mirrored verbatim (only with `include_plain`).
    pub copied: Vec<String>,
    /// Unencrypted files left where they were.
    pub skipped: Vec<String>,
    /// Files `encrypt` could not match to a crypt version, so did not pack.
    /// Without a manifest entry *and* without an existing file to sniff there
    /// is nothing to go on — and quietly leaving an edited file out of the
    /// client is the one failure worth being loud about.
    pub unresolved: Vec<String>,
    /// Set when the manifest could not be read or written; the run still
    /// happened, falling back to sniffing headers.
    pub manifest_error: Option<String>,
}

impl Report {
    pub fn failures(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| e.error.is_some())
    }

    pub fn converted(&self) -> usize {
        self.entries.iter().filter(|e| e.error.is_none()).count()
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, String)>) -> std::io::Result<()> {
    let mut items: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    items.sort_by_key(std::fs::DirEntry::path);
    for item in items {
        let path = item.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if rel != MANIFEST_NAME {
                out.push((path, rel));
            }
        }
    }
    Ok(())
}

/// Convert every recognised file under `in_dir` into `out_dir`.
///
/// Which cipher a file used is not derivable from its name — `.ini` files
/// appear under Ver111, Ver413 *and* no encryption at all — so `decrypt`
/// records each file's header in [`MANIFEST_NAME`] and `encrypt` reads it back.
/// Without a manifest, `encrypt` sniffs the header of the file it is about to
/// overwrite, which covers the ordinary "unpack a client, edit, repack" loop.
pub fn run(cfg: &Config) -> Result<Report, String> {
    let mut files = Vec::new();
    walk(cfg.in_dir, cfg.in_dir, &mut files)
        .map_err(|e| format!("cannot read {}: {e}", cfg.in_dir.display()))?;
    std::fs::create_dir_all(cfg.out_dir)
        .map_err(|e| format!("cannot create {}: {e}", cfg.out_dir.display()))?;

    let mut manifest_error = None;
    let manifest = if cfg.mode == Mode::Encrypt {
        match read_manifest(&cfg.in_dir.join(MANIFEST_NAME)) {
            Ok(m) => m,
            Err(e) => {
                manifest_error = e;
                BTreeMap::new()
            }
        }
    } else {
        BTreeMap::new()
    };

    // Decide per file what to do before doing any of it, so the expensive
    // parallel pass is pure crypto.
    let (mut work, mut copied, mut skipped, mut unresolved) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (src, rel) in files {
        let dst = cfg.out_dir.join(&rel);
        match cfg.mode {
            // A file's own header says all there is to know.
            Mode::Decrypt => match sniff(&src) {
                Some(v) => work.push((src, dst, rel, v)),
                None if cfg.include_plain => copied.push((src, dst, rel)),
                None => skipped.push(rel),
            },
            // Plaintext carries no header, so the version has to come from the
            // manifest, or failing that from the file we are about to replace.
            Mode::Encrypt => match manifest.get(&rel).map(String::as_str) {
                Some(PLAIN) if cfg.include_plain => copied.push((src, dst, rel)),
                Some(PLAIN) => skipped.push(rel),
                Some(v) => {
                    let v = v.to_owned();
                    work.push((src, dst, rel, v));
                }
                None => match sniff(&dst) {
                    Some(v) => work.push((src, dst, rel, v)),
                    None => unresolved.push(rel),
                },
            },
        }
    }

    for (src, dst, rel) in copied.iter() {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::copy(src, dst).map_err(|e| format!("copying {rel}: {e}"))?;
    }

    let mode = cfg.mode;
    let entries: Vec<Entry> = work
        .into_par_iter()
        .map(|(src, dst, rel, version)| {
            let error = convert_one(mode, &src, &dst, &version).err();
            Entry {
                rel,
                version,
                error,
            }
        })
        .collect();

    let copied: Vec<String> = copied.into_iter().map(|(_, _, rel)| rel).collect();
    if cfg.mode == Mode::Decrypt
        && let Err(e) = write_manifest(cfg, &entries, &copied)
    {
        manifest_error = Some(e);
    }

    Ok(Report {
        entries,
        copied,
        skipped,
        unresolved,
        manifest_error,
    })
}

/// The crypt version of the file at `path`, reading only its header.
fn sniff(path: &Path) -> Option<String> {
    use std::io::Read as _;
    let mut head = [0u8; HEADER_LEN];
    let mut file = std::fs::File::open(path).ok()?;
    file.read_exact(&mut head).ok()?;
    read_version(&head)
}

fn convert_one(mode: Mode, src: &Path, dst: &Path, version: &str) -> Result<(), String> {
    let data = std::fs::read(src).map_err(|e| format!("read: {e}"))?;
    let out = match mode {
        Mode::Decrypt => decrypt(&data, version)?,
        Mode::Encrypt => encrypt(&data, version)?,
    };
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    std::fs::write(dst, out).map_err(|e| format!("write: {e}"))
}

fn read_manifest(path: &Path) -> Result<BTreeMap<String, String>, Option<String>> {
    // A missing manifest is the documented fallback path, not an error.
    if !path.is_file() {
        return Err(None);
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| Some(format!("cannot read {}: {e}", path.display())))?;
    serde_json::from_str(&text).map_err(|e| Some(format!("cannot parse {}: {e}", path.display())))
}

/// Record what each file in `out_dir` was, so `encrypt` can put it back. Only
/// files that actually landed there are listed — a plain file that was skipped
/// is not in the mirror, so naming it would only mislead the next run.
fn write_manifest(cfg: &Config, entries: &[Entry], copied: &[String]) -> Result<(), String> {
    let mut map: BTreeMap<&str, &str> = entries
        .iter()
        .filter(|e| e.error.is_none())
        .map(|e| (e.rel.as_str(), e.version.as_str()))
        .collect();
    map.extend(copied.iter().map(|rel| (rel.as_str(), PLAIN)));
    let text = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
    std::fs::write(cfg.out_dir.join(MANIFEST_NAME), text + "\n")
        .map_err(|e| format!("cannot write manifest: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_pad_matches_javas_bit_trick() {
        for size in 0i32..=124 {
            let java = (-size & 1) + (-size & 2);
            assert_eq!(align_pad(size as usize), java as usize, "size {size}");
        }
    }

    #[test]
    fn header_round_trips() {
        let bytes = header_bytes("413");
        assert_eq!(bytes.len(), HEADER_LEN);
        assert_eq!(read_version(&bytes).as_deref(), Some("413"));
        assert_eq!(read_version(b"not an l2 file at all......."), None);
    }

    /// The whole point of the tool: whatever comes out of `decrypt` must go
    /// back in through `encrypt` unchanged.
    #[test]
    fn rsa_413_round_trips() {
        for plain in [
            b"".to_vec(),
            b"x".to_vec(),
            b"a slab of client text".repeat(500),
            (0u8..=255).cycle().take(300_000).collect::<Vec<u8>>(),
        ] {
            let packed = encrypt(&plain, "413").expect("encrypt");
            assert_eq!(read_version(&packed).as_deref(), Some("413"));
            assert_eq!(decrypt(&packed, "413").expect("decrypt"), plain);
        }
    }

    #[test]
    fn xor_round_trips_and_is_byte_exact() {
        let plain = b"[Public]\nObject=(Name=DXAudio)\n".repeat(40);
        let packed = encrypt(&plain, "111").expect("encrypt");
        assert_eq!(decrypt(&packed, "111").expect("decrypt"), plain);
        // XOR is symmetric, so unlike RSA the ciphertext is reproducible.
        assert_eq!(encrypt(&plain, "111").expect("again"), packed);
    }

    #[test]
    fn footer_carries_a_crc_over_everything_before_it() {
        let packed = encrypt(b"payload", "413").expect("encrypt");
        let (body, footer) = packed.split_at(packed.len() - RSA_FOOTER);
        let mut hasher = Hasher::new();
        hasher.update(body);
        assert_eq!(footer[12..16], hasher.finalize().to_le_bytes());
    }

    #[test]
    fn a_wrong_key_is_rejected_rather_than_returning_garbage() {
        let packed = encrypt(b"payload", "413").expect("encrypt");
        // 414 is a different modulus entirely; nothing plausible can come out.
        assert!(decrypt(&packed, "414").is_err());
        assert!(decrypt(&packed, "999").is_err());
    }

    #[test]
    fn decrypt_only_versions_refuse_to_write() {
        let err = encrypt(b"payload", "414").expect_err("414 has no private exponent");
        assert!(err.contains("no private exponent"), "{err}");
    }
}
