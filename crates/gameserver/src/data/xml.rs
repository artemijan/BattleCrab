//! The attribute readers every datapack XML loader in this module opens with.
//!
//! Each loader used to carry its own copy — 26 identical bodies for the string
//! read alone, half of them as a `let attr = |key|` closure over the current
//! element and half as a private `fn attr_str`. They are here instead.
//!
//! What is deliberately *not* here: the loaders whose readers only look alike.
//! `npc_data` unescapes XML entities (`unescape_value`) and `route_data`
//! matches attribute names case-insensitively. Those differences decide what a
//! datapack file means, so each keeps its own reader. Strict-vs-lossy and
//! trim-vs-not are differences too, but they are *arguments* — [`attr_strict`]
//! and [`attr_i32_trimmed`] — not separate readers.
//!
//! Loaders that want the short `attr(b"id")` form keep a closure over the
//! current element; the closure body is one call to a reader here, which is the
//! point. `pledge_skill_tree` used to be listed as strict-on-purpose and is
//! not any more: every one of its call sites either parses a number or compares
//! against a literal, so strict and lossy cannot produce different answers.

use quick_xml::events::BytesStart;
use std::path::{Path, PathBuf};

/// Every `.xml` file directly inside `dir`, **sorted by path**.
///
/// Sorted because `read_dir` order is filesystem-dependent while the load order
/// is not free: the loaders that keep the first entry for a duplicate id (Java's
/// `putIfAbsent`) would otherwise pick a different one per machine. Every
/// caller sorted already; doing it here is what makes them one function.
///
/// A missing or unreadable directory yields an empty list — the datapack ships
/// some of these directories only on certain dists, and a loader with nothing
/// to read is not an error.
pub(crate) fn xml_files_in(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("xml"))
        .collect();
    paths.sort();
    paths
}

/// Every `.xml` file under `dir` **recursively**, sorted by path.
///
/// The [`xml_files_in`] counterpart for the datapack trees that nest by region
/// (`spawns/`, `teleporters/`). Same sort rationale, and unreadable
/// subdirectories are skipped rather than failing the load.
pub(crate) fn xml_files_under(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("xml") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir.as_ref(), &mut out);
    out.sort();
    out
}

/// The raw text of attribute `key`, or `None` when the element does not carry
/// it. Invalid UTF-8 is replaced rather than rejected (`from_utf8_lossy`):
/// the datapack is authored as UTF-8, and a mangled character in one name is
/// not worth dropping the whole entry over.
///
/// Entity references are **not** expanded — see the module note.
pub(crate) fn attr_str(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

/// The raw text of attribute `key`, named as a `&str` and read **strictly**:
/// invalid UTF-8 gives `None`, so the caller drops the entry rather than
/// storing a mangled one. The counterpart to [`attr_str`] for the loaders that
/// would rather lose an entry than keep a corrupt one.
pub(crate) fn attr_strict(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key.as_bytes())
        .and_then(|a| String::from_utf8(a.value.into_owned()).ok())
}

/// Attribute `key` parsed as `i32` — `None` if absent *or* unparseable, which
/// every caller treats the same way: fall back to a default, or skip the entry.
pub(crate) fn attr_i32(e: &BytesStart, key: &[u8]) -> Option<i32> {
    attr_str(e, key).and_then(|v| v.parse().ok())
}

/// Attribute `key` parsed as `i32` after trimming. Kept apart from
/// [`attr_i32`] on purpose: `instances.xml` and `ItemAuction.xml` pad some
/// numeric attributes with whitespace, and a plain `parse()` rejects those —
/// while trimming everywhere would quietly start accepting padded values in
/// files that have never contained any.
pub(crate) fn attr_i32_trimmed(e: &BytesStart, key: &[u8]) -> Option<i32> {
    attr_str(e, key).and_then(|v| v.trim().parse().ok())
}

/// Attribute `key` parsed as `i64` — the [`attr_i32`] counterpart for counts and
/// timestamps that outgrow 32 bits.
pub(crate) fn attr_i64(e: &BytesStart, key: &[u8]) -> Option<i64> {
    attr_str(e, key).and_then(|v| v.parse().ok())
}

/// Attribute `key` parsed as `f64` — chances, multipliers and stat values.
pub(crate) fn attr_f64(e: &BytesStart, key: &[u8]) -> Option<f64> {
    attr_str(e, key).and_then(|v| v.parse().ok())
}
