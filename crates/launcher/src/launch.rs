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

/// A launched game, kept so the UI can tell whether it is still running.
///
/// Without this the launcher can only ever say "Starting game…" and leave that on
/// screen forever, which reads as stuck long after the client is up.
pub struct GameProcess(platform::Handle);

impl GameProcess {
    /// Non-blocking liveness check, polled by the UI.
    pub fn is_running(&self) -> bool {
        self.0.is_running()
    }
}

/// Launches `l2.exe` pointed at `server_ip`.
///
/// The working directory must be the `system/` folder the executable lives in — the
/// client resolves its own data with relative paths and fails obscurely otherwise.
pub fn launch_game(exe: &Path, server_ip: &str) -> anyhow::Result<GameProcess> {
    if !exe.is_file() {
        bail!("game executable not found at {}", exe.display());
    }
    let workdir = exe
        .parent()
        .context("game executable has no parent directory")?;

    platform::spawn(exe, &format!("IP={server_ip}"), workdir).map(GameProcess)
}

#[cfg(windows)]
mod platform {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use anyhow::{bail, Context};
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, HANDLE, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    /// Owns the process handle returned under `SEE_MASK_NOCLOSEPROCESS`.
    ///
    /// Holding the handle keeps the *handle* valid after the process exits, so the
    /// liveness check stays correct rather than reading a recycled process id.
    pub struct Handle(HANDLE);

    impl Handle {
        pub fn is_running(&self) -> bool {
            if self.0.is_null() {
                return false;
            }
            // Zero timeout makes this a poll: WAIT_TIMEOUT means still alive,
            // WAIT_OBJECT_0 means it has exited and the handle is signalled.
            // SAFETY: `self.0` is a process handle owned by this struct.
            unsafe { WaitForSingleObject(self.0, 0) == WAIT_TIMEOUT }
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: owned handle, released exactly once. Closing it does not
                // affect the running child.
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    /// Null-terminated UTF-16, as every `*W` Win32 entry point expects.
    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    pub fn spawn(exe: &Path, params: &str, workdir: &Path) -> anyhow::Result<Handle> {
        // These must outlive the call: the struct holds borrowed pointers.
        let file = wide(exe.as_os_str());
        let params = wide(OsStr::new(params));
        let dir = wide(workdir.as_os_str());

        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        // Ask for the process handle so it is not closed behind our back; a future
        // "close the launcher once the game exits" needs it.
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

        // Ownership of the handle passes to `Handle`, which closes it on drop.
        Ok(Handle(info.hProcess))
    }
}

/// Non-Windows builds exist only so the app runs during development on macOS; there
/// is no elevation concept to honour here.
#[cfg(not(windows))]
mod platform {
    use std::path::Path;
    use std::process::Command;

    use anyhow::Context;

    pub struct Handle(std::cell::RefCell<std::process::Child>);

    impl Handle {
        pub fn is_running(&self) -> bool {
            // `try_wait` needs &mut, and it also reaps the child — hence the RefCell.
            matches!(self.0.borrow_mut().try_wait(), Ok(None))
        }
    }

    pub fn spawn(exe: &Path, params: &str, workdir: &Path) -> anyhow::Result<Handle> {
        let child = Command::new(exe)
            .arg(params)
            .current_dir(workdir)
            .spawn()
            .with_context(|| format!("failed to start {}", exe.display()))?;
        Ok(Handle(std::cell::RefCell::new(child)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_executable_is_reported_clearly() {
        // Matched by hand rather than with `expect_err`, which would need
        // `GameProcess: Debug` — a process handle has nothing useful to print.
        let err = match launch_game(Path::new("/nonexistent/system/l2.exe"), "127.0.0.1") {
            Ok(_) => panic!("a missing executable must not report success"),
            Err(e) => e,
        };
        assert!(
            err.to_string().contains("not found"),
            "unhelpful message: {err}"
        );
    }

    /// The bug this guards: the status used to say "Starting game…" forever, because
    /// nothing ever observed the process again.
    ///
    /// Exercises the non-Windows path — the Windows one is the same shape via
    /// `WaitForSingleObject` with a zero timeout, but cannot be run from here.
    #[cfg(not(windows))]
    #[test]
    fn liveness_flips_when_the_process_exits() {
        let game = platform::spawn(Path::new("/bin/sleep"), "0.3", Path::new("/"))
            .expect("failed to spawn the test process");

        assert!(game.is_running(), "should report running immediately after spawn");

        std::thread::sleep(std::time::Duration::from_millis(900));
        assert!(!game.is_running(), "should notice the process exited");
        // Repeated polling must stay stable, not flip back or block.
        assert!(!game.is_running());
    }
}
