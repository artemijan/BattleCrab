//! Starting the game client.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context};

/// Launches `l2.exe` pointed at `server_ip`.
///
/// The working directory must be the `system/` folder the executable lives in — the
/// client resolves its own data with relative paths and fails obscurely otherwise.
/// No `start` shell wrapper is involved: spawning directly keeps the child handle,
/// which a later "close launcher when the game exits" behaviour will need.
pub fn launch_game(exe: &Path, server_ip: &str) -> anyhow::Result<()> {
    if !exe.is_file() {
        bail!("game executable not found at {}", exe.display());
    }
    let workdir = exe
        .parent()
        .context("game executable has no parent directory")?;

    Command::new(exe)
        .arg(format!("IP={server_ip}"))
        .current_dir(workdir)
        .spawn()
        .with_context(|| format!("failed to start {}", exe.display()))?;

    Ok(())
}
