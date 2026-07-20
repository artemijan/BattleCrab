//! Embeds the Windows icon resource into the executable.
//!
//! This is what Explorer, the pinned taskbar entry and Alt-Tab read. It is separate
//! from the icon set at runtime via `ViewportBuilder::with_icon` — that one only
//! affects the live window, and a binary with no resource icon still shows the
//! generic default in Explorer. Both are needed; neither substitutes for the other.
//!
//! Rust ships no resource compiler, so this drives LLVM's. The resource object must
//! be built for the *target* machine, not the host: linking an arm64 resource into an
//! x64 binary fails with `machine type arm64 conflicts with x64`.
//!
//! If no resource compiler is present the build still succeeds, just without a file
//! icon — failing here would block anyone building the rest of the workspace without
//! LLVM installed.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Used when `LAUNCHER_BASE_URL` is not set. Deliberately not a working URL: a
/// release built without the real bucket should fail loudly at the manifest fetch,
/// not silently download from somewhere unintended.
const FALLBACK_BASE_URL: &str = "https://pub-REPLACE-ME.r2.dev";
const FALLBACK_SERVER_IP: &str = "79.137.70.1";

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    // Must run before the non-Windows early return below, or host builds and tests
    // would have no value for `env!` to read and would fail to compile.
    bake_in_config();

    // Target, not host: cross-compiling from macOS must still embed this.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let icon = manifest_dir.join("assets/icon.ico");
    if !icon.is_file() {
        warn("assets/icon.ico missing");
        return;
    }

    // `1` is the resource id. Windows shows the lowest-numbered ICON resource as the
    // application icon, so this must stay first if more are ever added.
    let rc_path = out_dir.join("icon.rc");
    let rc = format!("1 ICON \"{}\"\n", icon.display().to_string().replace('\\', "\\\\"));
    std::fs::write(&rc_path, rc).expect("failed to write icon.rc");

    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let obj = out_dir.join("icon.o");

    let built = match arch.as_str() {
        // `windres` names the machine with a BFD format string.
        "x86_64" => windres("pe-x86-64", &rc_path, &obj),
        "x86" => windres("pe-i386", &rc_path, &obj),
        // llvm-windres has no ARM64 BFD name ("Unsupported architecture in target
        // 'pe-aarch64'"), so ARM64 goes the two-step MSVC-style route instead.
        "aarch64" => rc_then_cvtres("ARM64", &rc_path, &out_dir, &obj),
        other => {
            warn(&format!("no resource-compiler recipe for target arch '{other}'"));
            false
        }
    };

    if built {
        // Linking the object is what puts the resource into the PE image.
        println!("cargo:rustc-link-arg-bins={}", obj.display());
    }
}

/// Bakes the download location and server address into the binary.
///
/// These are build-time properties, not user settings — a given build talks to a
/// given deployment. Keeping them out of the persisted config also avoids the stale
/// -config trap, where a user who ran an early build keeps a saved placeholder URL
/// that silently overrides the one compiled in.
///
/// ```text
/// LAUNCHER_BASE_URL=https://pub-xxxx.r2.dev cargo build --release
/// ```
fn bake_in_config() {
    for (var, fallback) in [
        ("LAUNCHER_BASE_URL", FALLBACK_BASE_URL),
        ("LAUNCHER_SERVER_IP", FALLBACK_SERVER_IP),
    ] {
        println!("cargo:rerun-if-env-changed={var}");
        let value = std::env::var(var).unwrap_or_else(|_| fallback.to_string());
        // Trailing slashes would produce `…r2.dev//manifest.json`; the install code
        // trims them too, but normalising once here keeps logs readable.
        let value = value.trim().trim_end_matches('/').to_string();
        if value.is_empty() {
            panic!("{var} is set but empty");
        }
        println!("cargo:rustc-env={var}={value}");
    }
}

/// GNU-style single step: `.rc` straight to a COFF object.
fn windres(bfd_target: &str, rc_path: &Path, obj: &Path) -> bool {
    let Some(tool) = which(&["llvm-windres", "windres", "x86_64-w64-mingw32-windres"]) else {
        warn("no windres found; install LLVM to embed the file icon");
        return false;
    };
    run(
        &tool,
        &[
            &format!("--target={bfd_target}"),
            "--input-format=rc",
            "--output-format=coff",
            &rc_path.display().to_string(),
            &obj.display().to_string(),
        ],
    )
}

/// MSVC-style two step: `.rc` to `.res`, then `.res` to a COFF object for `machine`.
///
/// Both tools take MSVC-style `/FLAG` options, which on Unix collides with absolute
/// paths — `/Users/…/icon.res` is parsed as an option and the tool fails with
/// "Exactly one input file should be provided". Everything is therefore run from
/// `out_dir` with bare relative filenames.
fn rc_then_cvtres(machine: &str, rc_path: &Path, out_dir: &Path, obj: &Path) -> bool {
    let (Some(rc_tool), Some(cvt)) = (which(&["llvm-rc"]), which(&["llvm-cvtres"])) else {
        warn("llvm-rc/llvm-cvtres not found; install LLVM to embed the file icon");
        return false;
    };
    let rc_name = file_name(rc_path);
    let obj_name = file_name(obj);
    run_in(out_dir, &rc_tool, &["-FO", "icon.res", &rc_name])
        && run_in(
            out_dir,
            &cvt,
            &[&format!("/machine:{machine}"), &format!("/out:{obj_name}"), "icon.res"],
        )
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

fn run(tool: &str, args: &[&str]) -> bool {
    run_impl(Command::new(tool).args(args), tool)
}

fn run_in(dir: &Path, tool: &str, args: &[&str]) -> bool {
    run_impl(Command::new(tool).args(args).current_dir(dir), tool)
}

fn run_impl(cmd: &mut Command, tool: &str) -> bool {
    match cmd.status() {
        Ok(s) if s.success() => true,
        Ok(s) => {
            warn(&format!("{tool} failed ({s})"));
            false
        }
        Err(e) => {
            warn(&format!("could not run {tool} ({e})"));
            false
        }
    }
}

/// Homebrew keeps LLVM off `PATH` by default, so the well-known locations are tried
/// after the bare names.
fn which(names: &[&str]) -> Option<String> {
    const PREFIXES: &[&str] = &["/opt/homebrew/opt/llvm/bin", "/usr/local/opt/llvm/bin"];
    for name in names {
        if Command::new(name).arg("--version").output().is_ok() {
            return Some((*name).to_string());
        }
        for prefix in PREFIXES {
            let path = Path::new(prefix).join(name);
            if path.is_file() {
                return Some(path.display().to_string());
            }
        }
    }
    None
}

fn warn(msg: &str) {
    println!("cargo:warning={msg}; building without a file icon");
}
