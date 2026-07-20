# launcher

Windows desktop launcher and updater for the BattleCrab game client.

Downloads the packaged client from Cloudflare R2, unpacks it, and starts `l2.exe`
pointed at the game server.

## Status

Skeleton. The install pipeline is wired end to end and the UI is functional but
deliberately unstyled — the visual design is still to be specified.

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
| `install.rs` | Worker thread: manifest → download → SHA-256 verify → unpack |
| `progress.rs` | `Phase` messages and the counting reader driving the unpack bar |
| `manifest.rs` | Remote `manifest.json` model |
| `config.rs` | Persisted install path / base URL / server IP |
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

The client is distributed as zstd-compressed tarballs:

```
tar -cf - client/ | zstd -19 --long=27 -T0 -o client.tar.zst
```

`--long=27` is a 128 MB window. The decoder **must** be told to allow it or the
frame is rejected — see `ZSTD_WINDOW_LOG_MAX` in `install.rs`. This fails only on
real archives; small test fixtures decode fine without it.

The 19 GB client currently packs to ~9.3 GB, which fits R2's 10 GB free-tier cap —
but leaves no room to stage a new version alongside the old one. Splitting into
per-directory chunks (`textures`, `sounds`, `maps`, `system`) is the intended fix:
`manifest.json` already lists chunks as an array, so only the packaging side needs
to change. It also buys resumable downloads and per-chunk updates.

## Not done yet

- The update flow. `manifest.rs` carries the version and per-chunk hashes needed for
  it, but nothing compares them against a local record yet.
- Resumable downloads (HTTP range requests).
- Free-disk-space check before starting a ~9 GB download.
- Visual design.

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
