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

## Build notes

TLS goes through rustls, which pulls `aws-lc-rs` — that needs CMake and NASM on the
Windows build machine. See the comment in `Cargo.toml` for why the SChannel route is
currently closed and what the escape hatch is.
