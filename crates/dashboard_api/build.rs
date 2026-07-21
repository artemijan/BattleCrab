//! Guarantees `web/dashboard/dist` exists before `rust-embed` looks at it.
//!
//! The SPA build output is gitignored, so it is absent on any fresh checkout.
//! `#[derive(RustEmbed)]` treats a missing folder as a *compile* error, which
//! meant `cargo build` failed for anyone who had not run `bun run build` first
//! — including on the other crates in the workspace.
//!
//! This script only creates the directory. It deliberately does **not** invoke
//! Bun: shelling out to a JS toolchain from `build.rs` would make `cargo build`
//! fail on a machine without Bun, which is the very problem being fixed here.
//! An empty directory compiles fine, and `web::serve_spa` already answers with
//! a "frontend not built" message, so a Rust-only checkout builds and runs.
//!
//! Producing a real frontend stays an explicit step, ordered in CI/Docker:
//!
//!     cd web/dashboard && bun install && bun run build
//!     cargo build --release -p dashboard_api

use std::path::PathBuf;

fn main() {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("cargo always sets CARGO_MANIFEST_DIR");
    let dist = PathBuf::from(manifest_dir).join("../../web/dashboard/dist");

    if let Err(e) = std::fs::create_dir_all(&dist) {
        panic!("cannot create {} for the embedded SPA: {e}", dist.display());
    }

    // Re-run when the built frontend changes so a release build re-embeds it
    // instead of baking in a stale bundle.
    println!("cargo:rerun-if-changed={}", dist.display());

    let is_empty = std::fs::read_dir(&dist)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(true);

    if is_empty {
        println!(
            "cargo:warning=web/dashboard/dist is empty — dashboard_api will serve a \
             'frontend not built' placeholder. Run `bun install && bun run build` in \
             web/dashboard to build the SPA."
        );
    }
}
