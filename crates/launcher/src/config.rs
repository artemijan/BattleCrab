//! Persisted launcher settings.
//!
//! Stored next to the launcher's own data (`%APPDATA%\BattleCrab\launcher.json` on
//! Windows) rather than in the install directory, so wiping the game client does not
//! lose the user's chosen install path.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Where the packaged client lives, baked in at build time from
/// `LAUNCHER_BASE_URL`. See `bake_in_config` in `build.rs`.
///
/// Compiled in rather than persisted deliberately: it is a property of the build, not
/// a user setting. Storing it in the config file would mean a user who ran an early
/// build keeps a saved placeholder that silently overrides every later release.
pub const BASE_URL: &str = env!("LAUNCHER_BASE_URL");

/// Server address passed to `l2.exe`, baked in from `LAUNCHER_SERVER_IP`.
pub const SERVER_IP: &str = env!("LAUNCHER_SERVER_IP");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Root the client is unpacked into. `system/l2.exe` is resolved beneath it.
    pub install_dir: PathBuf,
    /// Version string of the currently installed client, if any. `None` means "not
    /// installed" and drives the UI into the install flow.
    pub installed_version: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self { install_dir: default_install_dir(), installed_version: None }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|e| {
                tracing::warn!("config at {} is malformed ({e}); using defaults", path.display());
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

    /// Path to the game executable for the current install directory.
    pub fn game_exe(&self) -> PathBuf {
        self.install_dir.join("system").join("l2.exe")
    }

    /// A client counts as installed only if the executable is actually on disk —
    /// the recorded version alone is not trusted, since the user may have deleted
    /// the folder behind our back.
    pub fn is_installed(&self) -> bool {
        self.installed_version.is_some() && self.game_exe().is_file()
    }
}

fn app_dir() -> PathBuf {
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
