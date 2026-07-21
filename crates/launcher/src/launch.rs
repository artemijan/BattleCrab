//! Starting the game client.
//!
//! ## Why this is not just `Command::spawn`
//!
//! `std::process::Command` goes through `CreateProcess`, which **cannot elevate**. If
//! the target executable's manifest asks for administrator rights, `CreateProcess`
//! fails outright with `ERROR_ELEVATION_REQUIRED` (os error 740) — which is exactly
//! what `l2.exe` does:
//!
//! ```text
//! failed to start …\system\l2.exe: The requested operation requires elevation. (os error 740)
//! ```
//!
//! Double-clicking in Explorer works because the shell uses `ShellExecuteEx`, which
//! recognises the elevation request and shows the UAC prompt. So this module does the
//! same. The default verb is used rather than `runas`: with the default, Windows
//! elevates only if the executable actually asks to, so a client that does not need
//! administrator rights starts without a prompt.
//!
//! Passing the path and arguments as separate fields also sidesteps quoting entirely,
//! which matters — install paths contain spaces (`…\BattleCrab\The Game\system`).

use std::path::Path;

use anyhow::{bail, Context};

/// Launches `l2.exe` pointed at `server_ip`.
///
/// The working directory must be the `system/` folder the executable lives in — the
/// client resolves its own data with relative paths and fails obscurely otherwise.
pub fn launch_game(exe: &Path, server_ip: &str) -> anyhow::Result<()> {
    if !exe.is_file() {
        bail!("game executable not found at {}", exe.display());
    }
    let workdir = exe
        .parent()
        .context("game executable has no parent directory")?;

    platform::spawn(exe, &format!("IP={server_ip}"), workdir)
}

#[cfg(windows)]
mod platform {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use anyhow::{bail, Context};
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    /// Null-terminated UTF-16, as every `*W` Win32 entry point expects.
    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    pub fn spawn(exe: &Path, params: &str, workdir: &Path) -> anyhow::Result<()> {
        // These must outlive the call: the struct holds borrowed pointers.
        let file = wide(exe.as_os_str());
        let params = wide(OsStr::new(params));
        let dir = wide(workdir.as_os_str());

        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        // Asking for the handle is what makes ShellExecuteEx report a real failure
        // rather than returning before the process is created. It is closed below;
        // a future "close the launcher once the game exits" would keep it instead.
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        // Null verb = the default action, i.e. what double-clicking does. `runas`
        // would force a UAC prompt even for a client that does not need one.
        info.lpVerb = std::ptr::null();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.lpDirectory = dir.as_ptr();
        info.nShow = SW_SHOWNORMAL;

        // SAFETY: `info` is zeroed, `cbSize` is set, and every pointer field refers to
        // a null-terminated buffer that outlives the call.
        let ok = unsafe { ShellExecuteExW(&mut info) };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            // The user clicked "No" on the UAC prompt. Not an error worth a stack of
            // Win32 jargon.
            if err.raw_os_error() == Some(ERROR_CANCELLED as i32) {
                bail!("Launch cancelled — the game needs permission to start.");
            }
            return Err(err).with_context(|| format!("failed to start {}", exe.display()));
        }

        // Nothing tracks the child, so release our handle immediately. Closing it
        // does not affect the running process.
        if !info.hProcess.is_null() {
            // SAFETY: a valid handle returned under SEE_MASK_NOCLOSEPROCESS.
            unsafe { CloseHandle(info.hProcess) };
        }
        Ok(())
    }
}

/// Non-Windows builds exist only so the app runs during development on macOS; there
/// is no elevation concept to honour here.
#[cfg(not(windows))]
mod platform {
    use std::path::Path;
    use std::process::Command;

    use anyhow::Context;

    pub fn spawn(exe: &Path, params: &str, workdir: &Path) -> anyhow::Result<()> {
        Command::new(exe)
            .arg(params)
            .current_dir(workdir)
            .spawn()
            .with_context(|| format!("failed to start {}", exe.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_executable_is_reported_clearly() {
        let err = launch_game(Path::new("/nonexistent/system/l2.exe"), "127.0.0.1")
            .expect_err("a missing executable must not report success");
        assert!(
            err.to_string().contains("not found"),
            "unhelpful message: {err}"
        );
    }
}
