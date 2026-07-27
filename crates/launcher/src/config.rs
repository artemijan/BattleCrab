//! Persisted launcher settings.
//!
//! Stored next to the launcher's own data (`%APPDATA%\BattleCrab\launcher.json` on
//! Windows) rather than in the install directory, so wiping the game client does not
//! lose the user's chosen install path.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// URL of the packaged client, baked in at build time from `LAUNCHER_CLIENT_URL` (see
/// `bake_in_config` in `build.rs`, and `.env`).
///
/// Compiled in rather than persisted deliberately: it is a property of the build, not
/// a user setting. Storing it in the config file would mean a user who ran an early
/// build keeps a saved placeholder that silently overrides every later release.
pub const CLIENT_URL: &str = env!("LAUNCHER_CLIENT_URL");

/// Server address passed to `l2.exe`, baked in from `LAUNCHER_SERVER_IP`.
pub const SERVER_IP: &str = env!("LAUNCHER_SERVER_IP");

/// The executable the Play button starts.
pub const GAME_EXE: &str = "l2.exe";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Where the archive is unpacked.
    pub install_dir: PathBuf,
    /// Resolved path to `l2.exe`, recorded after a successful install.
    ///
    /// Stored rather than derived because the archive decides its own layout: it may
    /// unpack `system/l2.exe` at the root, or nest everything under a folder of its
    /// own. Guessing `install_dir/system/l2.exe` would leave Play permanently
    /// unavailable for the nested case.
    pub game_exe: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            install_dir: default_install_dir(),
            game_exe: None,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|e| {
                tracing::warn!(
                    "config at {} is malformed ({e}); using defaults",
                    path.display()
                );
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// A client counts as installed only if the executable is actually on disk. The
    /// recorded path alone is not trusted — the user may have deleted the folder
    /// behind our back, or moved the install elsewhere.
    pub fn is_installed(&self) -> bool {
        self.game_exe.as_deref().is_some_and(Path::is_file)
    }

    /// Re-resolves [`Config::game_exe`] by looking for the game under `install_dir`.
    pub fn refresh_game_exe(&mut self) {
        self.game_exe = locate_game_exe(&self.install_dir);
    }
}

/// Finds `system/l2.exe` beneath `root`.
///
/// Checks `root/system/l2.exe` first, then one level of subdirectories, because a 7z
/// archive commonly wraps everything in a single top-level folder. Deliberately not a
/// recursive walk: a full scan of an unpacked 19 GB client to find one file would be
/// slow, and anything deeper than one level is not a layout worth guessing at.
///
/// Matching is case-insensitive — Windows filesystems are, and archives are
/// inconsistent about `System` versus `system`.
pub fn locate_game_exe(root: &Path) -> Option<PathBuf> {
    if let Some(found) = game_exe_in(root) {
        return Some(found);
    }
    std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .find_map(|e| game_exe_in(&e.path()))
}

/// `dir/system/l2.exe`, case-insensitively on both components.
fn game_exe_in(dir: &Path) -> Option<PathBuf> {
    let system = find_child(dir, "system")?;
    let exe = find_child(&system, GAME_EXE)?;
    exe.is_file().then_some(exe)
}

fn find_child(dir: &Path, name: &str) -> Option<PathBuf> {
    // Try the exact name first: one stat beats listing a directory that, for the
    // client root, holds thousands of entries.
    let direct = dir.join(name);
    if direct.exists() {
        return Some(direct);
    }
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        e.file_name()
            .to_str()
            .is_some_and(|n| n.eq_ignore_ascii_case(name))
            .then(|| e.path())
    })
}

/// The launcher's own data directory (`%APPDATA%\\BattleCrab` on Windows) — shared
/// by the config file and the single-instance lock.
pub fn app_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "BattleCrab")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| Path::new(".").to_path_buf())
}

fn config_path() -> PathBuf {
    app_dir().join("launcher.json")
}

/// Default install location. Deliberately *not* under Program Files — writing there
/// needs elevation, and the game itself writes into its own directory at runtime.
fn default_install_dir() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|d| d.home_dir().to_path_buf().into())
        .unwrap_or_else(|| PathBuf::from("."))
        .join("BattleCrab")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("launcher-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_exe(dir: &Path, rel: &str) -> PathBuf {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"MZ").unwrap();
        path
    }

    #[test]
    fn finds_the_exe_at_the_install_root() {
        let root = scratch("root");
        let exe = make_exe(&root, "system/l2.exe");
        assert_eq!(locate_game_exe(&root), Some(exe));
    }

    /// A 7z commonly wraps everything in one top-level folder. Assuming
    /// `install_dir/system/l2.exe` would leave Play permanently unavailable here.
    #[test]
    fn finds_the_exe_nested_one_level_down() {
        let root = scratch("nested");
        let exe = make_exe(&root, "L2_Client/system/l2.exe");
        assert_eq!(locate_game_exe(&root), Some(exe));
    }

    /// Archives are inconsistent about `System` vs `system`, and Windows does not
    /// care — so neither can this.
    ///
    /// Compared by resolved identity, not by string: on a case-insensitive
    /// filesystem (macOS, Windows) the `exists()` fast path returns the *requested*
    /// casing rather than the on-disk casing. The path still opens the right file,
    /// which is all that matters.
    #[test]
    fn matching_is_case_insensitive() {
        let root = scratch("case");
        let exe = make_exe(&root, "System/L2.exe");

        let found = locate_game_exe(&root).expect("should find the exe whatever the casing");
        assert!(found.is_file());
        assert_eq!(
            found.canonicalize().unwrap(),
            exe.canonicalize().unwrap(),
            "located path must resolve to the same file"
        );
    }

    #[test]
    fn reports_nothing_when_absent() {
        let root = scratch("absent");
        make_exe(&root, "system/other.exe");
        assert_eq!(locate_game_exe(&root), None);
    }

    /// Guards the Play button: a recorded path whose file has since been deleted must
    /// not count as installed.
    #[test]
    fn not_installed_when_the_recorded_exe_is_gone() {
        let root = scratch("stale");
        let exe = make_exe(&root, "system/l2.exe");
        let mut config = Config {
            install_dir: root,
            game_exe: Some(exe.clone()),
        };
        assert!(config.is_installed());

        std::fs::remove_file(&exe).unwrap();
        assert!(
            !config.is_installed(),
            "deleted client must not count as installed"
        );

        config.refresh_game_exe();
        assert_eq!(config.game_exe, None);
    }
}
