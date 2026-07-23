//! The install worker: download `client.7z`, unpack it, done.
//!
//! Runs on a plain `std::thread`, not a tokio runtime. The work is a linear sequence
//! of blocking I/O with no concurrency to exploit, so async would add a runtime and
//! buy nothing. All progress reaches the UI over [`crate::progress`].

use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context};
use sevenz_rust2::{ArchiveReader, Password};

use crate::progress::{Phase, Reporter};

/// Report at most this often, in bytes moved. Reporting per read floods the channel
/// and costs more than the copy itself.
const REPORT_INTERVAL: u64 = 4 * 1024 * 1024;

/// Cooperative cancellation. Checked inside the copy loops and between archive
/// entries, so a cancel lands within a few megabytes rather than at the end of a
/// multi-gigabyte download.
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
    pub client_url: String,
    pub install_dir: PathBuf,
    pub cancel: Cancel,
}

/// Runs a full install, reporting terminal state ([`Phase::Ready`] or
/// [`Phase::Failed`]) before returning so the UI never needs to poll the thread.
pub fn run(req: InstallRequest, reporter: Reporter) {
    match install(&req, &reporter) {
        Ok(()) => {
            tracing::info!("install complete");
            reporter.send(Phase::Ready);
        }
        Err(e) if req.cancel.is_cancelled() => {
            tracing::info!("install cancelled: {e}");
            reporter.send(Phase::Failed("Cancelled".into()));
        }
        Err(e) => {
            // `{e:#}` prints the whole anyhow context chain — the difference between
            // "install failed" and "install failed: connect timed out".
            tracing::error!("install failed: {e:#}");
            reporter.send(Phase::Failed(format!("{e:#}")));
        }
    }
}

fn install(req: &InstallRequest, reporter: &Reporter) -> anyhow::Result<()> {
    reporter.send(Phase::Connecting);

    std::fs::create_dir_all(&req.install_dir)
        .with_context(|| format!("cannot create {}", req.install_dir.display()))?;

    // The archive is staged rather than streamed straight into the extractor: 7z is
    // a random-access format whose index lives at the end of the file, so it cannot
    // be extracted from a forward-only stream at all.
    let staging = req.install_dir.join(".launcher-cache");
    std::fs::create_dir_all(&staging)?;
    let archive = staging.join("client.7z");

    download(&req.client_url, &archive, req, reporter)?;
    req.cancel.check()?;
    extract(&archive, &req.install_dir, req, reporter)?;

    // Frees ~9 GB straight away. Keeping it would allow a re-extract without
    // re-downloading, but not at that price.
    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::remove_dir(&staging);
    Ok(())
}

/// Streams the archive to disk.
fn download(
    url: &str,
    dest: &Path,
    req: &InstallRequest,
    reporter: &Reporter,
) -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("battlecrab-launcher/", env!("CARGO_PKG_VERSION")))
        // No overall timeout: a multi-gigabyte download legitimately runs for hours.
        // Only the connect phase is bounded.
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()?;

    let mut resp = client
        .get(url)
        .send()
        .with_context(|| format!("requesting {url}"))?
        .error_for_status()
        .with_context(|| format!("bad status from {url}"))?;

    let total = resp.content_length();
    tracing::info!("downloading {url} ({})", describe(total));

    let mut out = BufWriter::new(
        File::create(dest).with_context(|| format!("cannot write {}", dest.display()))?,
    );
    let mut buf = vec![0u8; 1 << 20];
    let mut done: u64 = 0;
    let mut last_reported = 0u64;

    loop {
        req.cancel.check()?;
        let n = resp
            .read(&mut buf)
            .context("connection dropped mid-download")?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        done += n as u64;
        if done - last_reported >= REPORT_INTERVAL {
            last_reported = done;
            reporter.send(Phase::Downloading { done, total });
        }
    }
    out.flush()?;
    reporter.send(Phase::Downloading { done, total });

    // A truncated transfer that still returned 200 would otherwise surface later as a
    // confusing "malformed archive".
    if let Some(total) = total {
        if done != total {
            bail!("download incomplete: got {done} of {total} bytes");
        }
    }
    Ok(())
}

/// Unpacks the 7z archive into `install_dir`.
///
/// Progress is in *uncompressed* bytes, which the archive index knows up front — so
/// unlike a streamed tarball this bar is honest about the real total.
///
/// Per-entry CRCs are verified by the decoder, which is where integrity checking
/// comes from now that there is no manifest carrying hashes.
fn extract(
    archive: &Path,
    install_dir: &Path,
    req: &InstallRequest,
    reporter: &Reporter,
) -> anyhow::Result<()> {
    let mut reader = ArchiveReader::open(archive, Password::empty())
        .with_context(|| format!("cannot open {}", archive.display()))?;

    let total: u64 = reader
        .archive()
        .files
        .iter()
        .filter(|f| f.has_stream && !f.is_directory)
        .map(|f| f.size)
        .sum();
    tracing::info!("unpacking {} bytes", total);
    reporter.send(Phase::Extracting { done: 0, total });

    let mut done: u64 = 0;
    let mut last_reported = 0u64;
    let cancel = req.cancel.clone();

    reader
        .for_each_entries(|entry, stream| {
            if cancel.is_cancelled() {
                // Ends iteration; `install` turns this into the cancelled path.
                return Ok(false);
            }

            let Some(dest) = safe_join(install_dir, &entry.name) else {
                tracing::warn!("skipping entry outside the install dir: {}", entry.name);
                return Ok(true);
            };

            if entry.is_directory {
                std::fs::create_dir_all(&dest)?;
                return Ok(true);
            }
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut out = BufWriter::new(File::create(&dest)?);
            let mut buf = vec![0u8; 1 << 20];
            loop {
                if cancel.is_cancelled() {
                    return Ok(false);
                }
                let n = stream.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
                done += n as u64;
                if done - last_reported >= REPORT_INTERVAL {
                    last_reported = done;
                    reporter.send(Phase::Extracting { done, total });
                }
            }
            out.flush()?;
            Ok(true)
        })
        .map_err(|e| anyhow::anyhow!("unpacking failed: {e}"))?;

    req.cancel.check()?;
    reporter.send(Phase::Extracting { done: total, total });
    Ok(())
}

/// Joins an archive-supplied path onto `root`, refusing anything that escapes it.
///
/// A malicious or malformed archive can carry `../` components or absolute paths;
/// without this check ("zip slip") an entry could be written anywhere the process can
/// write. Backslashes are normalised because 7z archives built on Windows use them.
fn safe_join(root: &Path, name: &str) -> Option<PathBuf> {
    use std::path::Component;

    let normalised = name.replace('\\', "/");
    let mut out = root.to_path_buf();
    for component in Path::new(&normalised).components() {
        match component {
            Component::Normal(part) => out.push(part),
            // Ignore no-ops; reject anything that climbs or re-roots.
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (out != root).then_some(out)
}

fn describe(total: Option<u64>) -> String {
    match total {
        Some(t) => format!("{t} bytes"),
        None => "size unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_accepts_normal_paths() {
        let root = Path::new("/game");
        assert_eq!(
            safe_join(root, "system/l2.exe"),
            Some(PathBuf::from("/game/system/l2.exe"))
        );
        // 7z archives written on Windows use backslashes.
        assert_eq!(
            safe_join(root, "system\\l2.exe"),
            Some(PathBuf::from("/game/system/l2.exe"))
        );
    }

    /// Zip-slip: without this the entry would be written outside the install dir.
    #[test]
    fn safe_join_rejects_escapes() {
        let root = Path::new("/game");
        assert_eq!(safe_join(root, "../evil.exe"), None);
        assert_eq!(safe_join(root, "system/../../evil.exe"), None);
        assert_eq!(safe_join(root, "/etc/passwd"), None);
        assert_eq!(safe_join(root, "..\\evil.exe"), None);
    }

    #[test]
    fn safe_join_rejects_empty_and_self() {
        assert_eq!(safe_join(Path::new("/game"), "."), None);
        assert_eq!(safe_join(Path::new("/game"), ""), None);
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("launcher-install-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Builds a real 7z archive laid out like the client, unpacks it through the
    /// production path, and checks the result.
    ///
    /// This is the only test that exercises the actual 7z decoder rather than our
    /// path handling around it — everything else could pass with extraction
    /// completely broken.
    #[test]
    fn extracts_a_real_archive_and_the_game_is_then_findable() {
        let root = scratch("roundtrip");
        let src = root.join("src");
        // Nested under a top-level folder, which is how 7z archives usually arrive.
        std::fs::create_dir_all(src.join("L2_Client/system")).unwrap();
        std::fs::write(src.join("L2_Client/system/l2.exe"), b"MZ fake game").unwrap();
        std::fs::write(src.join("L2_Client/system/l2.ini"), b"[Game]\n").unwrap();

        let archive = root.join("client.7z");
        sevenz_rust2::compress_to_path(&src, &archive).expect("failed to build test archive");

        let install_dir = root.join("install");
        std::fs::create_dir_all(&install_dir).unwrap();
        let (tx, _rx) = std::sync::mpsc::channel();
        let req = InstallRequest {
            client_url: String::new(),
            install_dir: install_dir.clone(),
            cancel: Cancel::default(),
        };

        extract(&archive, &install_dir, &req, &Reporter::new(tx, None)).expect("extraction failed");

        let exe = install_dir.join("L2_Client/system/l2.exe");
        assert!(exe.is_file(), "l2.exe should have been unpacked");
        assert_eq!(
            std::fs::read(&exe).unwrap(),
            b"MZ fake game",
            "contents must survive"
        );

        // The whole point of unpacking: the Play button must light up afterwards.
        assert_eq!(
            crate::config::locate_game_exe(&install_dir),
            Some(exe),
            "the launcher must find the game it just installed"
        );
    }

    /// Cancelling must stop promptly rather than unpacking the remaining gigabytes.
    #[test]
    fn extraction_stops_when_cancelled() {
        let root = scratch("cancel");
        let src = root.join("src");
        std::fs::create_dir_all(src.join("system")).unwrap();
        for i in 0..8 {
            std::fs::write(src.join(format!("system/file{i}.dat")), vec![b'x'; 4096]).unwrap();
        }
        let archive = root.join("client.7z");
        sevenz_rust2::compress_to_path(&src, &archive).unwrap();

        let install_dir = root.join("install");
        let cancel = Cancel::default();
        cancel.cancel();
        let (tx, _rx) = std::sync::mpsc::channel();
        let req = InstallRequest {
            client_url: String::new(),
            install_dir: install_dir.clone(),
            cancel,
        };

        let result = extract(&archive, &install_dir, &req, &Reporter::new(tx, None));
        assert!(
            result.is_err(),
            "a cancelled extraction must not report success"
        );
    }
}
