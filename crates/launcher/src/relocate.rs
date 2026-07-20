//! Moving the launcher into the game folder once the client is installed.
//!
//! Players download the launcher to wherever their browser puts things and run it
//! from there. After an install it belongs next to the game — in the install root,
//! alongside `system/` — so it travels with the client and is where they will look
//! for it.
//!
//! ## Moving a running executable
//!
//! Windows locks a running image against *writing and deletion*, but not against
//! renaming. So `rename` succeeds even on ourselves, and the running process carries
//! on unaffected — its image is already mapped.
//!
//! That only holds within a volume. Across volumes `rename` fails, and the fallback
//! is copy-then-delete — where the delete cannot succeed, because that is the running
//! image. In that case the copy is left in place and the original stays behind. The
//! next run happens from wherever the player launches, and the stale original is
//! harmless; cleaning it up would need a helper process outliving us, which is not
//! worth it for a file the player can delete.

use std::path::{Path, PathBuf};

use anyhow::Context;

#[derive(Debug, PartialEq, Eq)]
pub enum Relocation {
    /// Already in the install root; nothing to do.
    AlreadyInPlace,
    /// Renamed into place. The running process is unaffected.
    Moved(PathBuf),
    /// Copied across a volume boundary; the original could not be removed.
    CopiedLeftOriginal(PathBuf),
}

/// Moves `current_exe` into `install_dir`.
///
/// Split from [`relocate_self`] so it can be tested with ordinary files — a test
/// cannot meaningfully move the test binary itself.
pub fn relocate_exe(current_exe: &Path, install_dir: &Path) -> anyhow::Result<Relocation> {
    let file_name = current_exe
        .file_name()
        .context("executable path has no file name")?;
    let target = install_dir.join(file_name);

    // Compare canonicalised parents: the install dir may be given with symlinks or a
    // trailing component that differs textually from `current_exe`'s parent while
    // naming the same directory.
    let current_parent = current_exe
        .parent()
        .context("executable path has no parent")?;
    if same_dir(current_parent, install_dir) {
        return Ok(Relocation::AlreadyInPlace);
    }

    std::fs::create_dir_all(install_dir)
        .with_context(|| format!("cannot create {}", install_dir.display()))?;

    // Renaming a running image is allowed on Windows and is atomic within a volume.
    if std::fs::rename(current_exe, &target).is_ok() {
        return Ok(Relocation::Moved(target));
    }

    // Cross-volume: copy, then try to remove the original. The removal is expected to
    // fail on Windows precisely because it is the running image, so it is not an
    // error — just a leftover.
    std::fs::copy(current_exe, &target)
        .with_context(|| format!("cannot copy launcher to {}", target.display()))?;

    match std::fs::remove_file(current_exe) {
        Ok(()) => Ok(Relocation::Moved(target)),
        Err(e) => {
            tracing::debug!("left original launcher at {}: {e}", current_exe.display());
            Ok(Relocation::CopiedLeftOriginal(target))
        }
    }
}

/// [`relocate_exe`] applied to the running executable.
///
/// Skipped in debug builds: `cargo run` followed by an install would otherwise move
/// `target/debug/launcher` into the game folder, which is bewildering during
/// development. The logic itself is covered by the unit tests below.
pub fn relocate_self(install_dir: &Path) -> anyhow::Result<Relocation> {
    if cfg!(debug_assertions) {
        tracing::info!("debug build: skipping relocation into {}", install_dir.display());
        return Ok(Relocation::AlreadyInPlace);
    }
    let current = std::env::current_exe().context("cannot determine current executable")?;
    relocate_exe(&current, install_dir)
}

/// True when both paths name the same directory, resolving symlinks where possible.
fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        // A directory that does not exist yet cannot be where we already are.
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique scratch directory; avoids pulling in a temp-dir dependency.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("launcher-relocate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_exe(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"MZ fake executable").unwrap();
        path
    }

    #[test]
    fn moves_into_the_install_root() {
        let root = scratch("move");
        let downloads = root.join("Downloads");
        let install = root.join("BattleCrab");
        std::fs::create_dir_all(&downloads).unwrap();
        let exe = write_exe(&downloads, "launcher.exe");

        let outcome = relocate_exe(&exe, &install).unwrap();

        assert_eq!(outcome, Relocation::Moved(install.join("launcher.exe")));
        assert!(install.join("launcher.exe").is_file(), "launcher should be in the game folder");
        assert!(!exe.exists(), "original should be gone after a same-volume move");
    }

    #[test]
    fn creates_the_install_dir_if_absent() {
        let root = scratch("create");
        let exe = write_exe(&root, "launcher.exe");
        let install = root.join("nested/game/dir");

        relocate_exe(&exe, &install).unwrap();

        assert!(install.join("launcher.exe").is_file());
    }

    /// The common case on a second run: relocating again must be a no-op rather than
    /// copying the launcher onto itself.
    #[test]
    fn already_in_place_is_a_no_op() {
        let install = scratch("inplace");
        let exe = write_exe(&install, "launcher.exe");

        let outcome = relocate_exe(&exe, &install).unwrap();

        assert_eq!(outcome, Relocation::AlreadyInPlace);
        assert!(exe.is_file(), "launcher must survive a no-op relocation");
    }

    /// A trailing `.` names the same directory but differs textually — the parent
    /// comparison has to resolve that, or the launcher copies onto itself.
    #[test]
    fn already_in_place_survives_a_non_canonical_path() {
        let install = scratch("noncanon");
        let exe = write_exe(&install, "launcher.exe");

        let outcome = relocate_exe(&exe, &install.join(".")).unwrap();

        assert_eq!(outcome, Relocation::AlreadyInPlace);
        assert!(exe.is_file());
    }
}
