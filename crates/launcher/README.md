# launcher

Windows desktop launcher for the BattleCrab game client.

Downloads `client.7z`, unpacks it, moves itself next to the installed game, and
starts `l2.exe` pointed at the server.

## What it does

1. Downloads `client.7z` from the URL baked in at build time.
2. Unpacks it into the chosen install folder.
3. Moves itself into that folder, next to the game.
4. Shows **Play**, which starts `l2.exe` with `IP=<server>`.

## Why egui

The decisive factor was runtime dependencies. Tauri would give a better-looking,
easily skinned launcher, but it needs WebView2 present on the machine. The L2
private-server audience skews toward stripped, debloated and pirated Windows
installs, exactly where WebView2 is missing — turning first run into "download a
runtime first", or an outright failure.

`eframe`/`egui` builds a single statically linked `.exe` with nothing to install.
UI performance is irrelevant here: the app is idle, then network-bound, then
CPU/disk-bound on decompression. Nothing is drawn that could be a bottleneck.

## Layout

| File | Responsibility |
| --- | --- |
| `main.rs` | Window setup, logging, `windows_subsystem` for a console-free release build |
| `app.rs` | egui shell — all UI, worker polling, no blocking work |
| `theme.rs` | Palette, backdrop, frosted-glass surfaces, progress bar, buttons |
| `assets.rs` | Embedded logo and its alpha keying |
| `install.rs` | Worker thread: download → unpack, with cancellation |
| `progress.rs` | `Phase` snapshots sent to the UI |
| `config.rs` | Build-time constants, persisted install path, locating `l2.exe` |
| `relocate.rs` | Moving the launcher into the game folder |
| `launch.rs` | Spawning `l2.exe` |

## The glass look

egui has **no backdrop blur** — there is no way to sample and blur what is behind a
widget, so real glassmorphism is not directly achievable. It is faked the way it is
faked in any renderer without a blur pass:

- The backdrop is a smooth gradient plus soft radial glows. Blur only visibly changes
  high-frequency content; against a low-frequency background, plain translucency is
  indistinguishable from a blurred one.
- Panels carry a specular sheen along the top edge. That gradient, more than the
  transparency, is what actually reads as "pane of glass".
- A hairline light stroke gives each pane a lit rim.

The window is undecorated and transparent so the rounded corners are not framed by an
opaque OS title bar — which means `app.rs` owns the title bar: drag, minimise, close.

Windows 11 could do the real thing via `DwmSetWindowAttribute` (Mica / acrylic),
blurring the actual desktop behind the window. That needs the `windows` crate and a
raw window handle, and cannot be tested from macOS — left as a future upgrade.

The logo is bright art on solid black with no alpha channel. It is keyed at load with
`alpha = max(r, g, b)`, the classic screen-blend trick: black becomes transparent,
bright pixels opaque, and the cyan glow fades smoothly instead of ending at a hard
edge. The result is premultiplied by construction, which is what egui expects.

## Looking at the UI

`cargo run -p launcher`, or render every state headlessly without a display:

```
cargo test -p launcher -- --ignored render_window
# PNGs land in crates/launcher/target/ui-render/
```

That test is ignored by default because it needs a GPU adapter. It is how the layout
was actually verified — it caught a tofu close-button glyph, an unpainted strip along
the top edge, and a failed install animating an indeterminate "still working" bar.

It also asserts the glass panel's bottom edge stays inside the window. The window is
a fixed size and cannot scroll, so a content-sized panel overflows it the moment a
message gets long — which is exactly what a failed install produces, since `{e:#}`
prints the whole `anyhow` context chain. Hence `PANEL_CONTENT_HEIGHT`: every state
reserves the same space, and the status line is one truncated row with the full text
on hover. When adding anything to the panel, re-run the render test; if it trips,
raise `PANEL_CONTENT_HEIGHT` and shrink `LOGO_WIDTH` to pay for it.

Reserving identical space in every state also stops the action button moving when an
install starts. That is deliberate, and it is why the idle panel has some empty space
at its foot.

The UI thread never blocks. All install work happens on a `std::thread` that reports
`Phase` snapshots over an `mpsc` channel and calls `Context::request_repaint()` to
wake the UI, which is otherwise asleep between frames.

## Packaging the client

The 19 GB client packs to roughly 9.3 GB. Publish it as `client.7z` at the URL in
`.env`.

## Not done yet

- **The update flow.** Currently every install is a full re-download; there is no
  version check and no way to patch. A manifest listing per-file hashes is the
  natural shape, and was prototyped earlier, but it is not wired up.
- **Resumable downloads.** A dropped connection at 8 GB restarts from zero.
- **A free-disk-space check** before committing to a ~9 GB download plus ~19 GB of
  unpacked client.
- **Running it on Windows.** Everything here is verified by cross-compilation and
  unit tests only.

## Configuring a build

Settings live in `.env` next to `Cargo.toml`. It is **gitignored** — it points a build
at a specific deployment — so copy `.env.example` to get started:

```
LAUNCHER_CLIENT_URL=https://static.battlecrab.com/client.7z
LAUNCHER_SERVER_IP=79.137.70.1
```

`build.rs` reads it and compiles both values in. A real environment variable of the
same name wins, so CI can retarget a build without editing the file:

```
LAUNCHER_CLIENT_URL=https://staging.example.com/client.7z \
  cargo zigbuild -p launcher --target x86_64-pc-windows-gnu --release
```

With no `.env` and no environment variable, the URL falls back to
`https://REPLACE-ME.invalid/client.7z` — deliberately unroutable, so a misconfigured
release fails immediately instead of quietly downloading from somewhere unintended.

These are **not** stored in the user's config file, on purpose. A persisted URL would
mean anyone who ran an early build keeps a saved placeholder that silently overrides
every later release. The config holds only the install directory and the resolved path
to `l2.exe`.

## Finding the game after install

The Play button needs `l2.exe`, and the archive decides its own layout — it may put
`system/` at the root or nest everything under a folder of its own. So the path is
*resolved* after extraction (`config::locate_game_exe`) and stored, rather than assumed
to be `install_dir/system/l2.exe`. It checks the root, then one level of
subdirectories, case-insensitively. Not a recursive walk: scanning an unpacked 19 GB
client for one file would be slow.

Play stays hidden unless that recorded path still exists on disk, so deleting the game
folder behind the launcher's back correctly returns it to the install flow.

## Archive format

`client.7z`, decoded by `sevenz-rust2` — pure Rust, which matters because a C
dependency would break the macOS→Windows cross-build (see below).

7z is a random-access format whose index sits at the end of the file, so it cannot be
extracted from a forward-only stream. The archive is therefore staged to disk first and
unpacked from there.

Integrity comes from the per-entry CRCs the decoder verifies. There is no separate
checksum manifest.

Extraction progress is in *uncompressed* bytes — the 7z index carries the real total,
so unlike a streamed tarball the bar reflects actual progress.

## Settling into the game folder

After a successful install the launcher moves itself into the install root, next to
`system/`, so it travels with the client instead of being left in Downloads.

Windows locks a running image against writing and deletion but **not** renaming, so
`rename` succeeds on ourselves and the running process is unaffected — its image is
already mapped. That only holds within one volume. Across volumes the fallback is
copy-then-delete, where the delete necessarily fails because it is the running image;
the copy is placed and the original left behind, and the UI says so. Cleaning it up
would need a helper process outliving us, which is not worth it for a file the player
can delete.

Skipped in debug builds — otherwise `cargo run` plus an install would move
`target/debug/launcher` into the game folder, which is baffling mid-development. The
logic is covered by unit tests in `relocate.rs` that operate on ordinary files.

## Icons

There are **two** icons, and they are unrelated mechanisms — neither substitutes for
the other:

- `assets/icon.ico` is compiled into the PE resource table by `build.rs`. This is what
  Explorer shows for the *file*, and what a pinned taskbar shortcut uses.
- `assets/icon.png` is loaded at runtime via `ViewportBuilder::with_icon`. This is
  what Windows shows for the *running window* — title bar and Alt-Tab.

Both are cropped from the logo's round medallion. The full wide logo is illegible
below about 64px; the medallion keeps "L2R" readable down to 32px and degrades to a
recognisable blue-and-gold disc at 16px.

To regenerate them from `dist/images/logo2.png` (needs ImageMagick):

```
magick dist/images/logo2.png -crop 500x500+450+40 +repage \
  \( -size 500x500 xc:black -fill white -draw "circle 250,250 250,6" \) \
  -alpha off -compose CopyOpacity -composite -resize 512x512 /tmp/icon_src.png
magick /tmp/icon_src.png -resize 256x256 crates/launcher/assets/icon.png
magick /tmp/icon_src.png -define icon:auto-resize=256,128,64,48,32,16 \
  crates/launcher/assets/icon.ico
```

`build.rs` needs a resource compiler, which Rust does not ship. It uses LLVM's
(`brew install llvm`) and picks a route per target architecture:

| Arch | Route |
| --- | --- |
| x86_64 / x86 | `llvm-windres --target=pe-x86-64` (or `pe-i386`) — one step to COFF |
| aarch64 | `llvm-rc` then `llvm-cvtres /machine:ARM64` — `llvm-windres` has no ARM64 BFD name |

Two traps, both already paid for:

- The resource object must be built for the **target** machine. Building it for the
  host gives `lld-link: error: machine type arm64 conflicts with x64`.
- `llvm-rc` and `llvm-cvtres` take MSVC-style `/FLAG` options, which collide with
  Unix absolute paths — `/Users/…/icon.res` parses as an option and fails with
  "Exactly one input file should be provided". `build.rs` runs them from `OUT_DIR`
  with bare relative filenames.

If no resource compiler is found the build still succeeds, with a warning and no file
icon, so the workspace stays buildable without LLVM.

Verify a built binary really has it:

```
llvm-readobj --coff-resources target/x86_64-pc-windows-gnu/release/launcher.exe
# expect: Total Number of Resources: 7, Type: ICON, Type: GROUP_ICON
```

## Building the Windows binary from macOS

Cross-compiles from an Apple Silicon Mac with no Windows machine involved:

```
brew install zig                          # once
cargo install cargo-zigbuild              # once
rustup target add x86_64-pc-windows-gnu   # once

cargo zigbuild -p launcher --target x86_64-pc-windows-gnu --release
# -> target/x86_64-pc-windows-gnu/release/launcher.exe
```

Verified to produce `PE32+ executable (GUI) x86-64` — "GUI" meaning the console is
correctly suppressed. It has **not** been run on Windows yet; a clean cross-link is
not proof it works at runtime, particularly for `wgpu` picking a DX12 adapter.

`zig` supplies the C cross-toolchain, which is needed because `zstd-sys` compiles C.
The `ignoring deprecated linker optimization setting '1'` warning during linking is
harmless.

The binary is ~27 MB because release builds keep debug symbols. To strip:

```
RUSTFLAGS="-C strip=symbols" cargo zigbuild -p launcher \
  --target x86_64-pc-windows-gnu --release
```

Not set in `[profile.release]` because Cargo profiles are workspace-wide and that
would change how the game and login servers are built too.

### Windows on ARM

Ship the x64 binary — it covers ARM machines too, via the x64 emulation built into
Windows 11 on ARM.

A native ARM64 build also works, if it is ever wanted:

```
rustup target add aarch64-pc-windows-gnullvm
cargo zigbuild -p launcher --target aarch64-pc-windows-gnullvm --release
```

Produces `PE32+ executable (GUI) Aarch64`, 14 MB. Note `gnullvm`, not `gnu` — there
is no `aarch64-pc-windows-gnu`, since mingw does not target ARM64.

It is not shipped because the payoff is small: `l2.exe` is a 32-bit x86 binary and
runs emulated on ARM regardless, so a native launcher only hands off to an emulated
game. The one genuine gain is zstd decompression, which is CPU-bound and slower under
emulation. That does not currently outweigh maintaining a second binary for an
audience that is overwhelmingly x64 desktops.

### Why this is possible at all

Only because TLS is `native-tls` (SChannel on Windows) rather than rustls. rustls
pulls `aws-lc-rs`, which needs CMake and NASM and does not cross-compile easily —
with it in the tree there is no Windows build from macOS at all. That is also why
`reqwest` is pinned to 0.12; see the comment in `Cargo.toml`.

Keep the dependency tree free of C code and this keeps working. If something pulls
in a `-sys` crate, expect to fight the linker.

### The alternative

A GitHub Actions `windows-latest` runner builds natively, which sidesteps
cross-compilation entirely and can produce release artifacts. Worth adding when the
launcher starts shipping to players; the local path above is for iterating.
