//! Single-instance guard.
//!
//! Two launchers running at once can race each other over the same install
//! directory — two workers unpacking into one folder, or one reinstalling while the
//! other launches the half-replaced client. So the launcher refuses to start twice.
//!
//! ## Why an OS file lock, not a PID file or a named mutex
//!
//! The guard is an advisory lock (`std::fs::File::try_lock`) on a file in the
//! launcher's own config directory. The OS releases the lock when the process dies —
//! *however* it dies — so a crashed launcher never leaves a stale guard behind the
//! way a PID file would. And unlike a Windows named mutex it is portable, which
//! keeps the development build on macOS behaving like the release build on Windows.
//!
//! The lock file itself is never deleted; an empty file in the config dir is not
//! worth the delete-vs-lock race that cleaning it up would introduce.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use crate::config;

/// Holds the OS lock for the lifetime of the process. Dropping it (or dying) lets
/// the next launcher start. `None` inside means the guard could not be established
/// and the launcher is running unguarded — see [`acquire`].
pub struct InstanceLock {
    _file: Option<File>,
}

/// Claims the single-instance lock. `None` means another launcher is already
/// running and this process should exit.
///
/// Fails *open*: if the lock file cannot even be created or locked for some
/// environmental reason (unwritable config dir, exotic filesystem without lock
/// support), the launcher starts anyway. Refusing to run at all would be a worse
/// failure than tolerating a hypothetical second instance.
pub fn acquire() -> Option<InstanceLock> {
    let path = lock_path();
    match try_acquire_at(&path) {
        Ok(Some(lock)) => Some(lock),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(
                "could not use single-instance lock at {}: {e:#}; continuing unguarded",
                path.display()
            );
            Some(InstanceLock { _file: None })
        }
    }
}

/// `Ok(None)` means another live process holds the lock.
fn try_acquire_at(path: &Path) -> anyhow::Result<Option<InstanceLock>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Not `create_new`: the file persists across runs by design. `truncate(false)`
    // because only the lock matters, never the contents.
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(InstanceLock { _file: Some(file) })),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) => Err(e.into()),
    }
}

fn lock_path() -> PathBuf {
    config::app_dir().join("launcher.lock")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// File locks are per open handle on every platform std supports (`flock` on
    /// Unix, `LockFileEx` on Windows), so a second handle within one process is
    /// refused exactly like a second process would be — which makes the contended
    /// case testable without spawning anything.
    #[test]
    fn second_acquire_is_refused_until_the_first_is_dropped() {
        let path = std::env::temp_dir().join(format!("launcher-lock-test-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let first = try_acquire_at(&path)
            .expect("locking a fresh temp file should not error")
            .expect("first acquire should win the lock");

        assert!(
            try_acquire_at(&path)
                .expect("contended try_lock is WouldBlock, not an error")
                .is_none(),
            "second acquire must be refused while the first lock lives"
        );

        drop(first);
        assert!(
            try_acquire_at(&path).unwrap().is_some(),
            "lock must be reclaimable once released"
        );
    }
}
