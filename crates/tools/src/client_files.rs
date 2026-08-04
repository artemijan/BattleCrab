//! One-directory view of the client's editable data files.
//!
//! `system_decrypted/` holds *text* for everything, under the original
//! filenames, and `system/` holds what the client reads. There is no binary
//! halfway directory: the shape of a file on the way out is decided per type.
//!
//! ```text
//! *.ini, *.int   cipher only          — deciphering already yields text
//! *.dat          cipher + schema      — deciphered bytes are records, not text
//! ```
//!
//! Only those three extensions are touched. The client's executables,
//! libraries, `.u` packages and the rest are left exactly where they are.
//!
//! # Why a manifest is not optional here
//!
//! Both directions need facts the files themselves do not carry. Which cipher
//! a file used is not derivable from its name (`.ini` appears under Ver111,
//! Ver413 *and* unencrypted), and once a `.dat` has become text there is
//! nothing in it to say which schema produced it — or, for the handful no
//! schema fits, that it is raw bytes rather than text at all. `decrypt`
//! records all of that in [`MANIFEST_NAME`]; `encrypt` reads it back and
//! refuses anything it cannot place.

use crate::dat_schema::{Layout, SchemaSet};
use crate::{client_dat, dat_pack, dat_text};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Written into the decrypted directory, read back when packing.
pub const MANIFEST_NAME: &str = ".l2client-manifest.json";

/// The only extensions this tool converts.
const EXTENSIONS: [&str; 3] = ["ini", "int", "dat"];

/// How a file was turned into what sits in `system_decrypted`.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Form {
    /// Deciphering produced text directly (`.ini`, `.int`).
    Cipher,
    /// Deciphered bytes were walked with a schema into text (`.dat`).
    Schema,
    /// A `.dat` no schema fits. The deciphered *bytes* are on disk instead of
    /// text, so the file still round-trips even though it cannot be edited.
    Raw,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Record {
    /// `Lineage2Ver` code, e.g. `413`.
    pub version: String,
    pub form: Form,
    /// `chronicle/version` of the layout that unpacked it, when `form` is
    /// [`Form::Schema`] — reused verbatim on the way back so packing cannot
    /// silently pick a different one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
}

pub struct Config<'a> {
    pub system_dir: &'a Path,
    pub decrypted_dir: &'a Path,
    /// Chronicle to force, or `None` to try every layout and keep whichever
    /// consumes the file exactly.
    pub chronicle: Option<&'a str>,
}

pub struct Entry {
    pub file: String,
    pub detail: String,
    pub error: Option<String>,
}

pub struct Report {
    pub entries: Vec<Entry>,
    /// Files whose extension is in scope but which carry no `Lineage2Ver`
    /// header, so there is nothing to decipher.
    pub skipped: Vec<String>,
}

impl Report {
    pub fn ok_count(&self) -> usize {
        self.entries.iter().filter(|e| e.error.is_none()).count()
    }

    pub fn failures(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| e.error.is_some())
    }
}

/// Watches a run file by file, so a caller can draw a progress bar.
///
/// Deliberately push-only and infallible: a display that cannot keep up, or a
/// terminal that has gone away, must never be able to fail a conversion. The
/// no-op implementation on `()` is what keeps the library silent by default.
pub trait Observer {
    /// Once, before the first file.
    fn begin(&mut self, total: usize) {
        let _ = total;
    }
    /// About to convert `file`, with `done` files already behind it.
    fn step(&mut self, done: usize, total: usize, file: &str) {
        let _ = (done, total, file);
    }
    /// Once, after the last file.
    fn finish(&mut self) {}
}

impl Observer for () {}

fn in_scope(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

fn list(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && in_scope(p))
        .collect();
    files.sort();
    Ok(files)
}

/// `system` -> `system_decrypted`, as text wherever text is possible.
pub fn decrypt(
    set: &mut SchemaSet,
    cfg: &Config,
    obs: &mut dyn Observer,
) -> Result<Report, String> {
    let files = list(cfg.system_dir)?;
    std::fs::create_dir_all(cfg.decrypted_dir)
        .map_err(|e| format!("cannot create {}: {e}", cfg.decrypted_dir.display()))?;

    let enums = set.enums.clone();
    let mut manifest: BTreeMap<String, Record> = BTreeMap::new();
    let mut entries = Vec::new();
    let mut skipped = Vec::new();

    let total = files.len();
    obs.begin(total);
    for (done, path) in files.into_iter().enumerate() {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        obs.step(done, total, &name);
        let raw = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                entries.push(Entry {
                    file: name,
                    detail: String::new(),
                    error: Some(format!("read: {e}")),
                });
                continue;
            }
        };
        let Some(version) = client_dat::read_version(&raw) else {
            skipped.push(name);
            continue;
        };
        let plain = match client_dat::decrypt(&raw, &version) {
            Ok(p) => p,
            Err(e) => {
                entries.push(Entry {
                    file: name,
                    detail: format!("Ver{version}"),
                    error: Some(e),
                });
                continue;
            }
        };

        let is_dat = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("dat"));

        // `.ini` and `.int` are text the moment they are deciphered.
        let (bytes, record, detail) = if !is_dat {
            (
                plain,
                Record {
                    version: version.clone(),
                    form: Form::Cipher,
                    layout: None,
                },
                format!("Ver{version}, text"),
            )
        } else {
            match unpack(set, &name, &plain, cfg.chronicle, &enums) {
                Some((text, label)) => (
                    text.into_bytes(),
                    Record {
                        version: version.clone(),
                        form: Form::Schema,
                        layout: Some(label.clone()),
                    },
                    format!("Ver{version}, {label}"),
                ),
                // No layout fits. Keep the deciphered bytes so the file still
                // round-trips; it just cannot be edited as text.
                None => (
                    plain,
                    Record {
                        version: version.clone(),
                        form: Form::Raw,
                        layout: None,
                    },
                    format!("Ver{version}, raw bytes (no schema fits)"),
                ),
            }
        };

        let error = std::fs::write(cfg.decrypted_dir.join(&name), &bytes)
            .err()
            .map(|e| format!("write: {e}"));
        if error.is_none() {
            manifest.insert(name.clone(), record);
        }
        entries.push(Entry {
            file: name,
            detail,
            error,
        });
    }

    obs.finish();

    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(cfg.decrypted_dir.join(MANIFEST_NAME), text + "\n")
        .map_err(|e| format!("cannot write manifest: {e}"))?;

    Ok(Report { entries, skipped })
}

/// Try every candidate layout, keeping the first that consumes the file
/// exactly. Returns the text and the label of the layout that won.
fn unpack(
    set: &mut SchemaSet,
    name: &str,
    plain: &[u8],
    chronicle: Option<&str>,
    enums: &std::collections::HashMap<String, std::collections::HashMap<i64, String>>,
) -> Option<(String, String)> {
    let candidates: Vec<(String, Layout)> = match chronicle {
        Some(c) => set
            .layout_for(c, name)
            .map(|l| vec![(format!("{c}/{}", l.version), l)])
            .unwrap_or_default(),
        None => set.candidates(name),
    };
    for (label, layout) in candidates {
        let outcome = dat_text::read(plain, &layout, enums, false);
        if outcome.exact() {
            return Some((outcome.text, label));
        }
    }
    None
}

/// `system_decrypted` -> `system`, re-applying whatever each file needs.
pub fn encrypt(
    set: &mut SchemaSet,
    cfg: &Config,
    obs: &mut dyn Observer,
) -> Result<Report, String> {
    let manifest_path = cfg.decrypted_dir.join(MANIFEST_NAME);
    let manifest: BTreeMap<String, Record> =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).map_err(|e| {
            format!(
                "cannot read {} ({e}) — run `client-dat decrypt` first",
                manifest_path.display()
            )
        })?)
        .map_err(|e| format!("cannot parse {}: {e}", manifest_path.display()))?;

    std::fs::create_dir_all(cfg.system_dir)
        .map_err(|e| format!("cannot create {}: {e}", cfg.system_dir.display()))?;

    let enums = set.enums.clone();
    let mut entries = Vec::new();
    let mut skipped = Vec::new();

    let files = list(cfg.decrypted_dir)?;
    let total = files.len();
    obs.begin(total);
    for (done, path) in files.into_iter().enumerate() {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        obs.step(done, total, &name);
        let Some(record) = manifest.get(&name) else {
            // Not something decrypt produced; saying so beats guessing a
            // cipher and writing a file the client cannot read.
            skipped.push(name);
            continue;
        };
        let content = match std::fs::read(&path) {
            Ok(c) => c,
            Err(e) => {
                entries.push(Entry {
                    file: name,
                    detail: String::new(),
                    error: Some(format!("read: {e}")),
                });
                continue;
            }
        };

        let plain = match record.form {
            Form::Cipher | Form::Raw => Ok(content),
            Form::Schema => pack(set, &name, &content, record, &enums),
        };
        let result = plain.and_then(|p| {
            client_dat::encrypt(&p, &record.version).and_then(|bytes| {
                std::fs::write(cfg.system_dir.join(&name), &bytes)
                    .map_err(|e| format!("write: {e}"))
            })
        });

        entries.push(Entry {
            file: name,
            detail: format!("Ver{}, {:?}", record.version, record.form),
            error: result.err(),
        });
    }
    obs.finish();

    Ok(Report { entries, skipped })
}

/// Rebuild a `.dat`'s bytes from its text, using the layout that produced it,
/// and prove the result before it is accepted.
fn pack(
    set: &mut SchemaSet,
    name: &str,
    content: &[u8],
    record: &Record,
    enums: &std::collections::HashMap<String, std::collections::HashMap<i64, String>>,
) -> Result<Vec<u8>, String> {
    let text = String::from_utf8(content.to_vec())
        .map_err(|_| "file is not valid UTF-8 text".to_string())?;

    // Prefer the exact layout `decrypt` used; fall back to searching so an
    // edited tree still packs if the manifest lost the label.
    let mut candidates = set.candidates(name);
    if let Some(want) = &record.layout {
        candidates.sort_by_key(|(label, _)| label != want);
    }

    // Report the *first* candidate's failure, not the last. Candidates lead
    // with the layout that unpacked this file, so its error is the one worth
    // reading; a later chronicle's layout failing on some unrelated field only
    // sends the reader hunting through the wrong schema.
    let mut first: Option<String> = None;
    for (_label, layout) in candidates {
        let bytes = match dat_pack::pack(&text, &layout) {
            Ok(b) => b,
            Err(e) => {
                first.get_or_insert(e);
                continue;
            }
        };
        // Re-read what we are about to write: it must reproduce the text it
        // came from, or the `.dat` would go to the client subtly wrong.
        let back = dat_text::read(&bytes, &layout, enums, false);
        if !back.exact() {
            first.get_or_insert_with(|| {
                format!(
                    "the packed bytes do not re-read cleanly: {}",
                    back.summary()
                )
            });
            continue;
        }
        match dat_pack::diff_text(&back.text, &text) {
            None => return Ok(bytes),
            Some(why) => {
                first.get_or_insert(format!("the packed bytes re-read differently: {why}"));
            }
        }
    }
    Err(first.unwrap_or_else(|| "no layout packed this file".to_string()))
}
