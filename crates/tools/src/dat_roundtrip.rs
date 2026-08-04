//! Proves the client's own files survive a lap through the tools.
//!
//! ```text
//! system/  --decrypt-->  <scratch>/  --encrypt-->  system_encrypted/
//!    |                                                    |
//!    +----------------------- compare --------------------+
//! ```
//!
//! Nothing is edited in between, so every byte the client shipped should come
//! back. Anything that does not is either a packer bug or a lossy field, and
//! this is the cheapest way to find out which before a wrong `.dat` reaches a
//! running client.
//!
//! # Two gates, not one
//!
//! The obvious test is "the file hash is unchanged", and for 199 of our 201
//! files it holds exactly. It cannot be the *only* gate, because the last step
//! of writing a `.dat` is a zlib stream, and a deflate encoder is free to pick
//! among equally valid encodings of the same bytes. NCsoft's compressor and
//! `flate2` agree on almost everything and then part ways on the two largest
//! files, where a 7 MB plaintext deflates to a stream ~128 bytes shorter than
//! the client's. Nothing about the data differs — only its framing.
//!
//! So a mismatch is not a verdict, it is a question, and the answer is one
//! decryption away: [`Verdict::Equivalent`] is a file whose *plaintext* is
//! identical and whose bytes are not, and [`Verdict::Broken`] is a file that
//! genuinely changed. Only the latter fails a run. Treating every byte
//! difference as breakage would have this test crying wolf on two files
//! forever; ignoring bytes entirely would let real corruption through.

use crate::client_files::{self, Observer};
use crate::{client_dat, dat_schema::SchemaSet};
use sha2::{Digest, Sha256};
use std::path::Path;

/// How a re-encrypted file compares with the one the client shipped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Byte for byte the same file.
    Identical,
    /// Different bytes, same plaintext: only the deflate framing moved, so the
    /// client reads exactly the same data.
    Equivalent,
    /// The plaintext changed. This is the failure the round trip exists for.
    Broken(String),
    /// The file went in and never came back.
    Missing,
}

impl Verdict {
    /// Whether this file is safe to ship.
    pub fn is_ok(&self) -> bool {
        matches!(self, Verdict::Identical | Verdict::Equivalent)
    }
}

pub struct FileReport {
    pub file: String,
    /// SHA-256 of the client's original.
    pub original: String,
    /// SHA-256 of what the tools rebuilt, when there is one.
    pub rebuilt: Option<String>,
    pub original_len: u64,
    pub rebuilt_len: u64,
    pub verdict: Verdict,
}

pub struct Report {
    pub files: Vec<FileReport>,
    /// Files `decrypt` or `encrypt` refused outright, with the reason.
    pub conversion_errors: Vec<(String, String)>,
}

impl Report {
    pub fn count(&self, f: impl Fn(&Verdict) -> bool) -> usize {
        self.files.iter().filter(|r| f(&r.verdict)).count()
    }

    pub fn identical(&self) -> usize {
        self.count(|v| *v == Verdict::Identical)
    }

    pub fn equivalent(&self) -> usize {
        self.count(|v| *v == Verdict::Equivalent)
    }

    /// Every file that is not safe to ship, worst first.
    pub fn failures(&self) -> impl Iterator<Item = &FileReport> {
        self.files.iter().filter(|r| !r.verdict.is_ok())
    }

    /// The whole run passed: nothing broken, nothing lost, nothing refused.
    pub fn passed(&self) -> bool {
        self.conversion_errors.is_empty() && self.failures().count() == 0
    }
}

pub struct Config<'a> {
    /// The client's own `system`, read and never written.
    pub system_dir: &'a Path,
    /// Scratch text for the lap. Rebuilt from scratch each run.
    pub decrypted_dir: &'a Path,
    /// Where the rebuilt `system` lands, for comparison.
    pub encrypted_dir: &'a Path,
    pub chronicle: Option<&'a str>,
}

/// Run a full lap and compare what came back with what went in.
///
/// `decrypting` and `encrypting` watch the two halves; pass `&mut ()` twice for
/// a silent run.
pub fn verify(
    set: &mut SchemaSet,
    cfg: &Config,
    decrypting: &mut dyn Observer,
    encrypting: &mut dyn Observer,
) -> Result<Report, String> {
    let out = client_files::decrypt(
        set,
        &client_files::Config {
            system_dir: cfg.system_dir,
            decrypted_dir: cfg.decrypted_dir,
            chronicle: cfg.chronicle,
        },
        decrypting,
    )?;
    let mut conversion_errors: Vec<(String, String)> = out
        .failures()
        .map(|e| (e.file.clone(), format!("decrypt: {}", err(e))))
        .collect();

    // `encrypt` writes into whatever it is told is `system`, so the rebuilt
    // tree goes to `encrypted_dir` and the client's own stays untouched.
    let back = client_files::encrypt(
        set,
        &client_files::Config {
            system_dir: cfg.encrypted_dir,
            decrypted_dir: cfg.decrypted_dir,
            chronicle: cfg.chronicle,
        },
        encrypting,
    )?;
    conversion_errors.extend(
        back.failures()
            .map(|e| (e.file.clone(), format!("encrypt: {}", err(e)))),
    );

    // Only files that made the whole lap can be compared; the ones `decrypt`
    // passed over carry no `Lineage2Ver` header and were never ours to rebuild.
    let mut files = Vec::new();
    for entry in &out.entries {
        let name = &entry.file;
        let original = std::fs::read(cfg.system_dir.join(name))
            .map_err(|e| format!("{name}: cannot re-read the original: {e}"))?;
        let original_len = original.len() as u64;
        let Ok(rebuilt) = std::fs::read(cfg.encrypted_dir.join(name)) else {
            files.push(FileReport {
                file: name.clone(),
                original: sha(&original),
                rebuilt: None,
                original_len,
                rebuilt_len: 0,
                verdict: Verdict::Missing,
            });
            continue;
        };
        let verdict = if original == rebuilt {
            Verdict::Identical
        } else {
            // Bytes differ. Only the plaintext decides whether that matters,
            // and only these few files ever pay for the second decryption.
            compare_plaintext(&original, &rebuilt)
        };
        files.push(FileReport {
            file: name.clone(),
            original: sha(&original),
            rebuilt: Some(sha(&rebuilt)),
            original_len,
            rebuilt_len: rebuilt.len() as u64,
            verdict,
        });
    }

    Ok(Report {
        files,
        conversion_errors,
    })
}

/// Decide whether two differing files still say the same thing.
fn compare_plaintext(original: &[u8], rebuilt: &[u8]) -> Verdict {
    let Some(version) = client_dat::read_version(original) else {
        return Verdict::Broken("the original has no Lineage2Ver header".to_string());
    };
    let Some(rebuilt_version) = client_dat::read_version(rebuilt) else {
        return Verdict::Broken("the rebuilt file has no Lineage2Ver header".to_string());
    };
    if version != rebuilt_version {
        return Verdict::Broken(format!(
            "Ver{version} went in, Ver{rebuilt_version} came out"
        ));
    }
    let a = match client_dat::decrypt(original, &version) {
        Ok(a) => a,
        Err(e) => return Verdict::Broken(format!("the original no longer decrypts: {e}")),
    };
    let b = match client_dat::decrypt(rebuilt, &version) {
        Ok(b) => b,
        Err(e) => return Verdict::Broken(format!("the rebuilt file does not decrypt: {e}")),
    };
    if a == b {
        return Verdict::Equivalent;
    }
    if a.len() != b.len() {
        return Verdict::Broken(format!("plaintext is {} bytes, was {}", b.len(), a.len()));
    }
    let at = a
        .iter()
        .zip(b.iter())
        .position(|(x, y)| x != y)
        .unwrap_or_default();
    Verdict::Broken(format!("plaintext differs from byte {at}"))
}

fn sha(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn err(entry: &client_files::Entry) -> &str {
    entry.error.as_deref().unwrap_or("unknown error")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only the two safe verdicts pass; a plaintext change and a lost file do
    /// not. This is the whole point of grading mismatches instead of failing
    /// on any byte difference.
    #[test]
    fn only_identical_and_equivalent_are_shippable() {
        assert!(Verdict::Identical.is_ok());
        assert!(Verdict::Equivalent.is_ok());
        assert!(!Verdict::Broken("changed".to_string()).is_ok());
        assert!(!Verdict::Missing.is_ok());
    }

    /// A file whose bytes differ but whose plaintext does not is the deflate
    /// framing case, and it must not read as breakage.
    #[test]
    fn differing_bytes_with_the_same_plaintext_are_equivalent() {
        let plain = b"the client's own bytes".repeat(64);
        let a = client_dat::encrypt(&plain, "413").expect("encrypts");
        let mut b = a.clone();
        // Corrupt the CRC footer only: `decrypt` recovers the same plaintext,
        // so this stands in for a stream that was framed differently.
        let last = b.len() - 1;
        b[last] ^= 0xFF;
        assert_ne!(a, b);
        assert_eq!(compare_plaintext(&a, &b), Verdict::Equivalent);
    }

    /// A real change has to be caught, whatever the framing.
    #[test]
    fn a_changed_plaintext_is_broken() {
        let a = client_dat::encrypt(b"one payload", "413").expect("encrypts");
        let b = client_dat::encrypt(b"another one", "413").expect("encrypts");
        assert!(matches!(compare_plaintext(&a, &b), Verdict::Broken(_)));
    }

    fn report_of(verdicts: Vec<Verdict>) -> Report {
        Report {
            files: verdicts
                .into_iter()
                .enumerate()
                .map(|(i, verdict)| FileReport {
                    file: format!("f{i}.dat"),
                    original: String::new(),
                    rebuilt: None,
                    original_len: 0,
                    rebuilt_len: 0,
                    verdict,
                })
                .collect(),
            conversion_errors: Vec::new(),
        }
    }

    /// A run passes on identical *and* equivalent files, and on nothing else.
    /// Getting this wrong in either direction is the whole risk: too strict and
    /// the check cries wolf on every pristine client, too loose and it stops
    /// being a check at all.
    #[test]
    fn a_run_passes_only_when_every_file_survived() {
        assert!(report_of(vec![Verdict::Identical, Verdict::Equivalent]).passed());
        assert!(!report_of(vec![Verdict::Identical, Verdict::Missing]).passed());
        assert!(!report_of(vec![Verdict::Broken("plaintext differs".to_string())]).passed());

        // A file that never converted fails the run even if what did survive
        // came back clean.
        let mut refused = report_of(vec![Verdict::Identical]);
        refused
            .conversion_errors
            .push(("f0.dat".to_string(), "encrypt: nope".to_string()));
        assert!(!refused.passed());
    }

    #[test]
    fn sha_is_the_usual_hex_digest() {
        assert_eq!(
            sha(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
