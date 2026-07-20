//! Remote manifest describing the packaged client.
//!
//! Fetching a manifest rather than hard-coding one archive URL is what makes the
//! later update flow possible: an update becomes "diff the local record against the
//! remote manifest and re-fetch the chunks whose hashes changed". The initial
//! install is just the degenerate case where nothing is present locally.

use serde::{Deserialize, Serialize};

/// `manifest.json` at the root of the R2 bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Opaque version tag; compared against [`crate::config::Config::installed_version`].
    pub version: String,
    /// Archives making up a full install, applied in order.
    ///
    /// Today this is a single `client.tar.zst`. Splitting the client into
    /// per-directory chunks later needs no launcher change beyond the packaging
    /// side emitting more entries here — which is the point of listing them.
    pub chunks: Vec<Chunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Path relative to the bucket base URL, e.g. `chunks/textures.tar.zst`.
    pub path: String,
    /// Compressed size in bytes. Drives the download bar and lets us check free
    /// disk space before starting.
    pub size: u64,
    /// SHA-256 of the compressed archive, hex-encoded. Verified after download so a
    /// truncated or corrupted transfer fails loudly instead of producing a broken
    /// install that only surfaces as a crash in-game.
    pub sha256: String,
}

impl Manifest {
    pub fn total_size(&self) -> u64 {
        self.chunks.iter().map(|c| c.size).sum()
    }
}
