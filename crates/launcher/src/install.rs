//! The install worker: fetch manifest → download chunks → verify → unpack.
//!
//! Runs on a plain `std::thread`, not a tokio runtime. The work is a linear sequence
//! of blocking I/O with no concurrency to exploit, so async would add a runtime and
//! buy nothing. All progress reaches the UI over [`crate::progress`].

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context};
use sha2::{Digest, Sha256};

use crate::manifest::{Chunk, Manifest};
use crate::progress::{Phase, ProgressReader, Reporter};

/// Must match the `--long=27` used when packaging. zstd refuses to decode a frame
/// whose window exceeds the decoder's limit, and the default limit is well below
/// 128 MB — omitting this fails on real archives while test fixtures pass.
const ZSTD_WINDOW_LOG_MAX: u32 = 27;

/// Cooperative cancellation. Checked between chunks and inside the copy loops, so a
/// cancel lands within a few megabytes rather than at the end of a 9 GB download.
#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    fn check(&self) -> anyhow::Result<()> {
        if self.is_cancelled() {
            bail!("cancelled");
        }
        Ok(())
    }
}

pub struct InstallRequest {
    pub base_url: String,
    pub install_dir: PathBuf,
    pub cancel: Cancel,
}

/// Runs a full install. Reports terminal state ([`Phase::Ready`] or
/// [`Phase::Failed`]) before returning, so the UI never needs to poll the thread.
pub fn run(req: InstallRequest, reporter: Reporter) {
    match install(&req, &reporter) {
        Ok(version) => {
            tracing::info!("install complete, version {version}");
            reporter.send(Phase::Ready);
        }
        Err(e) if req.cancel.is_cancelled() => {
            tracing::info!("install cancelled: {e}");
            reporter.send(Phase::Failed("Cancelled".into()));
        }
        Err(e) => {
            // `{e:#}` prints the whole anyhow context chain, which is the difference
            // between "install failed" and "install failed: connect timed out".
            tracing::error!("install failed: {e:#}");
            reporter.send(Phase::Failed(format!("{e:#}")));
        }
    }
}

/// Returns the installed version tag on success.
fn install(req: &InstallRequest, reporter: &Reporter) -> anyhow::Result<String> {
    reporter.send(Phase::CheckingManifest);

    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("battlecrab-launcher/", env!("CARGO_PKG_VERSION")))
        // No overall timeout: a multi-gigabyte download legitimately runs for hours.
        // Only the connect phase is bounded.
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()?;

    let manifest = fetch_manifest(&client, &req.base_url)?;
    tracing::info!(
        "manifest version {} — {} chunk(s), {} bytes",
        manifest.version,
        manifest.chunks.len(),
        manifest.total_size()
    );

    std::fs::create_dir_all(&req.install_dir)
        .with_context(|| format!("cannot create {}", req.install_dir.display()))?;

    // Archives are staged here rather than streamed straight into the extractor so
    // the hash can be verified before anything touches the install directory, and so
    // a failed run can later resume instead of restarting.
    let staging = req.install_dir.join(".launcher-cache");
    std::fs::create_dir_all(&staging)?;

    for chunk in &manifest.chunks {
        req.cancel.check()?;
        let archive = download_chunk(&client, req, chunk, &staging, reporter)?;
        req.cancel.check()?;
        extract_chunk(&archive, &req.install_dir, req, reporter)?;
        // Freeing ~9 GB immediately matters more than the resume it would enable;
        // revisit when chunking makes individual archives small.
        let _ = std::fs::remove_file(&archive);
    }

    let _ = std::fs::remove_dir_all(&staging);
    Ok(manifest.version)
}

fn fetch_manifest(client: &reqwest::blocking::Client, base_url: &str) -> anyhow::Result<Manifest> {
    let url = format!("{}/manifest.json", base_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .send()
        .with_context(|| format!("requesting {url}"))?
        .error_for_status()
        .with_context(|| format!("bad status from {url}"))?;
    resp.json().context("manifest.json is not valid JSON")
}

/// Streams one chunk to disk, verifying its SHA-256 as it goes.
fn download_chunk(
    client: &reqwest::blocking::Client,
    req: &InstallRequest,
    chunk: &Chunk,
    staging: &Path,
    reporter: &Reporter,
) -> anyhow::Result<PathBuf> {
    let url = format!("{}/{}", req.base_url.trim_end_matches('/'), chunk.path);
    let filename = Path::new(&chunk.path)
        .file_name()
        .context("chunk path has no file name")?;
    let dest = staging.join(filename);

    let mut resp = client
        .get(&url)
        .send()
        .with_context(|| format!("requesting {url}"))?
        .error_for_status()
        .with_context(|| format!("bad status from {url}"))?;

    // Prefer the manifest's size over Content-Length: it is what the hash was
    // computed over, and it is known before the request starts.
    let total = if chunk.size > 0 { Some(chunk.size) } else { resp.content_length() };

    let mut out = BufWriter::new(
        File::create(&dest).with_context(|| format!("cannot write {}", dest.display()))?,
    );
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut done: u64 = 0;
    let mut last_reported = 0u64;

    loop {
        req.cancel.check()?;
        let n = resp.read(&mut buf).context("connection dropped mid-download")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])?;
        done += n as u64;
        if done - last_reported >= 1 << 22 {
            last_reported = done;
            reporter.send(Phase::Downloading { done, total });
        }
    }
    out.flush()?;
    reporter.send(Phase::Downloading { done, total });

    let got = hex::encode(hasher.finalize());
    // An empty hash in the manifest means "unverified" — useful while iterating on
    // packaging, but it should never be empty in production.
    if !chunk.sha256.is_empty() && !got.eq_ignore_ascii_case(&chunk.sha256) {
        let _ = std::fs::remove_file(&dest);
        bail!("checksum mismatch for {}: expected {}, got {got}", chunk.path, chunk.sha256);
    }

    Ok(dest)
}

/// Unpacks a `.tar.zst` archive into `install_dir`.
fn extract_chunk(
    archive: &Path,
    install_dir: &Path,
    req: &InstallRequest,
    reporter: &Reporter,
) -> anyhow::Result<()> {
    let file = File::open(archive)?;
    let compressed_size = file.metadata()?.len();

    // The bar tracks *compressed* bytes consumed: the uncompressed total is not in
    // the archive header, so this is the only figure known up front.
    let counted = ProgressReader::new(
        BufReader::with_capacity(1 << 20, file),
        compressed_size,
        reporter.clone(),
    );

    let mut decoder = zstd::stream::Decoder::new(counted)?;
    decoder
        .window_log_max(ZSTD_WINDOW_LOG_MAX)
        .context("failed to raise zstd window limit")?;

    let mut tar = tar::Archive::new(decoder);
    tar.set_overwrite(true);

    for entry in tar.entries()? {
        req.cancel.check()?;
        let mut entry = entry?;
        // `unpack_in` refuses paths escaping the destination (`..`, absolute paths),
        // so a malicious or malformed archive cannot write outside install_dir.
        entry
            .unpack_in(install_dir)
            .with_context(|| format!("unpacking {:?}", entry.path().ok()))?;
    }

    reporter.send(Phase::Extracting { done: compressed_size, total: compressed_size });
    Ok(())
}
