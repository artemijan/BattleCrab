//! A binary cache of the parsed dist catalogues, for the test fixtures only.
//!
//! The suite runs under nextest, which gives every test its own process (see
//! the README for why), so the `LazyLock` catalogues in
//! `game_loop::tests::dist` are shared by exactly one test each: ~170 tests
//! re-parse the same real XML from scratch. That parse is the single largest
//! item in the suite — `SkillData` alone is 627 ms of the 1.15 s a full
//! `GameData::load_from` costs.
//!
//! [`cached`] parses once, writes the result to `target/dist-snapshots/` as
//! bincode, and lets every later process decode it instead: 627 ms → 85 ms for
//! `SkillData`. Nothing here is compiled into the server — the module is
//! `#[cfg(test)]` and the running game always parses the datapack it was
//! pointed at.
//!
//! # Staleness
//!
//! A snapshot is only valid for the bytes it was made from *and* for the type
//! layout that wrote it, so the file name carries a hash of both:
//!
//! * every file under the dist root except `data/geodata` (1.4 GB that no
//!   catalogue parses) — path, length and mtime;
//! * every source file that can change the encoding — [`LAYOUT_SOURCES`],
//!   guarded by [`tests::every_serialized_source_is_fingerprinted`] so a new
//!   `#[derive(Serialize)]` outside that scope fails the suite rather than
//!   silently decoding old bytes;
//! * [`SNAPSHOT_FORMAT`], for anything the two above cannot see.
//!
//! Deliberately *not* in the hash: the rest of the crate. Test and game-logic
//! edits are the common case, and invalidating on those would mean the cache
//! never survives the edit → build → test loop it exists for.
//!
//! Every failure path — unreadable directory, truncated file, decode error —
//! falls back to parsing. The cache can make the suite slower, never wrong.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Bump when the *meaning* of the encoded bytes changes in a way neither the
/// datapack nor [`LAYOUT_SOURCES`] can show — a bincode config change, say.
const SNAPSHOT_FORMAT: u32 = 1;

/// The sources whose contents decide how a catalogue encodes: the loaders and
/// their types, plus the files outside `data/` that carry a `Serialize` derive
/// reachable from one. Paths are relative to the crate manifest.
const LAYOUT_SOURCES: &[&str] = &[
    "src/data",
    "src/enums.rs",
    "src/model/castle.rs",
    "src/model/clan_hall.rs",
    "src/model/cursed_weapon.rs",
    "src/model/movement.rs",
    "src/model/shortcut.rs",
    "src/model/siege.rs",
    "src/model/skill",
    "src/model/stats.rs",
];

/// The 1.4 GB of geodata no catalogue parses. Walking it would cost more than
/// the parse the cache is there to skip.
const SKIPPED_DIST_SUBDIR: &str = "geodata";

/// Parse `name`'s catalogue, or decode the snapshot of an earlier parse.
///
/// `dist_root` is the datapack the caller would have parsed; `parse` is that
/// parse, called on a miss (and on any cache failure). The value is written
/// back to the cache only when it was produced by `parse`.
pub(crate) fn cached<T: Serialize + DeserializeOwned>(
    name: &str,
    dist_root: &str,
    parse: impl FnOnce() -> T,
) -> T {
    let Some(path) = snapshot_path(name, dist_root) else {
        return parse();
    };
    if let Some(value) = read(&path) {
        return value;
    }
    let value = parse();
    write(&path, &value);
    value
}

/// `target/dist-snapshots/<name>-<fingerprint>.bin`, or `None` when the
/// fingerprint could not be taken (an unreadable dist root — parse and say
/// nothing, the parse will report the same problem in its own terms).
fn snapshot_path(name: &str, dist_root: &str) -> Option<PathBuf> {
    let mut h = DefaultHasher::new();
    SNAPSHOT_FORMAT.hash(&mut h);
    h.write_u64(dist_fingerprint(dist_root)?);
    h.write_u64(*LAYOUT_FINGERPRINT);
    Some(
        target_dir()
            .join("dist-snapshots")
            .join(format!("{name}-{:016x}.bin", h.finish())),
    )
}

/// Walking 13 776 datapack files costs ~20 ms, and a process that loads three
/// catalogues would otherwise pay it three times. Every caller in the suite
/// passes the same root, so one memo covers them; a second root recomputes
/// rather than answering wrong.
fn dist_fingerprint(dist_root: &str) -> Option<u64> {
    static MEMO: std::sync::OnceLock<(String, Option<u64>)> = std::sync::OnceLock::new();
    let take = || {
        let mut h = DefaultHasher::new();
        hash_tree(&mut h, Path::new(dist_root), Some(SKIPPED_DIST_SUBDIR))?;
        Some(h.finish())
    };
    let (root, fp) = MEMO.get_or_init(|| (dist_root.to_string(), take()));
    if root == dist_root { *fp } else { take() }
}

/// The layout half of the fingerprint — the same for every catalogue and every
/// call, so it is taken once per process.
static LAYOUT_FINGERPRINT: LazyLock<u64> = LazyLock::new(|| {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut h = DefaultHasher::new();
    for rel in LAYOUT_SOURCES {
        let path = manifest.join(rel);
        if path.is_dir() {
            hash_tree(&mut h, &path, None);
        } else {
            hash_entry(&mut h, &path);
        }
    }
    h.finish()
});

/// Fold every file under `root` (path, length, mtime) into `h`, skipping a
/// named immediate-child subdirectory. `None` when `root` cannot be walked.
///
/// Contents are deliberately not read: 100 MB of XML would cost more than the
/// parse. Length plus mtime is what every build system trusts for the same
/// job, and the datapack is a checked-out tree, not something tests rewrite
/// under themselves.
fn hash_tree(h: &mut DefaultHasher, root: &Path, skip: Option<&str>) -> Option<()> {
    let mut stack = vec![root.to_path_buf()];
    // Directory order is unspecified, so the fold has to be order-independent:
    // each entry hashes on its own and the results are summed.
    let mut acc = 0u64;
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).ok()?.flatten() {
            let path = entry.path();
            let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            if is_dir {
                if skip.is_some_and(|s| path.file_name().is_some_and(|n| n == s)) {
                    continue;
                }
                stack.push(path);
            } else {
                let mut e = DefaultHasher::new();
                hash_entry(&mut e, &path);
                acc = acc.wrapping_add(e.finish());
            }
        }
    }
    h.write_u64(acc);
    Some(())
}

fn hash_entry(h: &mut DefaultHasher, path: &Path) {
    path.hash(h);
    if let Ok(meta) = std::fs::metadata(path) {
        meta.len().hash(h);
        if let Ok(t) = meta.modified().and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| std::io::Error::other(e.to_string()))
        }) {
            t.as_nanos().hash(h);
        }
    }
}

fn read<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let bytes = std::fs::read(path).ok()?;
    bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
        .ok()
        .map(|(value, _)| value)
}

/// Write via a process-unique temporary and rename, so the 16 test processes
/// that miss together cannot read each other's half-written file.
fn write<T: Serialize>(path: &Path, value: &T) {
    let Ok(bytes) = bincode::serde::encode_to_vec(value, bincode::config::standard()) else {
        return;
    };
    let Some(dir) = path.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = dir.join(format!(
        "{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    if std::fs::write(&tmp, &bytes).is_ok() && std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// `CARGO_TARGET_DIR` when it is set (worktrees and CI override it), else the
/// workspace's own `target/`.
fn target_dir() -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The guard behind the fingerprint's narrow scope: if a type outside
    /// [`LAYOUT_SOURCES`] gains a serde derive and joins a cached catalogue,
    /// editing it would not invalidate the snapshots and tests would assert
    /// against bytes that no longer mean what they say. Failing here is the
    /// cheap version of that bug: add the file to `LAYOUT_SOURCES` (or, if it
    /// really is unreachable from a cached catalogue, note why below).
    #[test]
    fn every_serialized_source_is_fingerprinted() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let covered: Vec<PathBuf> = LAYOUT_SOURCES.iter().map(|r| manifest.join(r)).collect();
        let mut stray = Vec::new();
        let mut stack = vec![manifest.join("src")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if entry.file_type().is_ok_and(|t| t.is_dir()) {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && std::fs::read_to_string(&path).is_ok_and(|s| s.contains("serde::Serialize"))
                    && !covered.iter().any(|c| path.starts_with(c) || path == *c)
                {
                    stray.push(path);
                }
            }
        }
        assert!(
            stray.is_empty(),
            "these carry a serde derive but are outside LAYOUT_SOURCES, so editing \
             them would not invalidate a dist snapshot: {stray:#?}"
        );
    }

    /// The round trip the fixtures depend on: what a decode returns is what the
    /// parse produced. `SkillData` is the catalogue that pays for the cache and
    /// the one with the hand-rolled `#[serde(skip)]`-adjacent shapes, so it is
    /// the one worth checking end to end.
    #[test]
    fn a_snapshot_decodes_to_what_the_parse_built() {
        let parsed = crate::data::SkillData::load_from(crate::data::DIST_GAME);
        let bytes = bincode::serde::encode_to_vec(&parsed, bincode::config::standard()).unwrap();
        let (decoded, _): (crate::data::SkillData, usize) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(
            format!("{:?}", decoded.gaps()),
            format!("{:?}", parsed.gaps())
        );
        for (id, level) in [(1, 1), (1177, 5), (247, 3)] {
            assert_eq!(
                format!("{:?}", decoded.get(id, level)),
                format!("{:?}", parsed.get(id, level)),
                "skill {id} level {level}"
            );
        }
    }
}
