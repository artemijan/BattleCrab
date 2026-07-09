# Implementation Plan — Login Server

**Status: PROPOSED (awaiting approval).** First implementation phase of the Rust
rewrite. Builds the reusable network/crypto/db foundation (`commons`) plus the
complete login server. Architecture per
[CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md); decisions per
[JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md).

## 1. Goal and definition of done

A headless Rust login server that is a **drop-in replacement** for the Java one:

1. A real Interlude client (protocol rev `0x0106` / `0xc621`) can connect, log
   in against the SQLite `accounts` table, see the server list, and get `PlayOk`.
2. The **existing Java game server registers and interoperates with it
   unchanged** (same GS-link protocol, same `hexid`, same session-key handoff) —
   this gives us end-to-end verification long before the Rust game server exists.
3. Same config files (`LoginServer.ini`, `banned_ip.cfg`), same DB schema, same
   logged behavior. Excluded by prior decisions: Swing GUI, MariaDB/Postgres,
   `tools/` GUIs.

## 2. What the Java login server consists of (port inventory)

~7,650 lines (minus ~700 of GUI). Three protocol surfaces:

| Surface | Transport | Crypto | Packets |
|---|---|---|---|
| Client ↔ LS (port 2106) | Async-mmocore, 2-byte LE length framing | Blowfish (per-client key; first packet under a static key with XOR pass), +checksum; RSA-1024 "nopadding" for credentials | 8 client / 13 server packets |
| GS ↔ LS (port 9014) | `GameServerListener`/`GameServerThread`, **blocking** sockets, same framing | Static Blowfish `_;5.]94-31==-%xT!^[$\0` → RSA key exchange → session Blowfish key | 12 GS→LS / 7 LS→GS packets |
| DB | JDBC | — | `accounts`, `accounts_ipauth`, `gameservers` tables |

Core logic classes: `LoginController` (RSA/Blowfish key caches, auth,
session keys, failed-login IP bans, authed-client map, 5-min purge task),
`GameServerTable` (registered servers from DB + `servername.xml`, `hexid`),
`SessionKey` (2×2 ints matched between LS and GS), `FloodProtectedListener`
(per-IP connection rate on the GS port), flow state machine
(`CONNECTED → AUTHED_GG → AUTHED_LOGIN`).

Login flow (must match byte-for-byte):
`Init` (session id, scrambled RSA modulus, Blowfish key) → `AuthGameGuard` →
`GGAuth` → `RequestAuthLogin` (RSA block; SHA-1+Base64 password check;
auto-create optional) → `LoginOk`/`LoginFail` → `RequestServerList` →
`ServerList` → `RequestServerLogin` → `PlayOk` (session key half 2) →
client connects to GS → GS asks LS `PlayerAuthRequest` → `PlayerAuthResponse`.

### Crypto notes (the risky bits, all verified in source)

- **L2 Blowfish is not standard-byte-order Blowfish**: `crypt/BlowfishEngine`
  reads blocks little-endian. The `blowfish` Rust crate is big-endian per spec —
  we either byte-swap around it or port the engine (~350 lines). Plan: **wrap
  the crate with LE conversion**, verify against Java test vectors; port the
  engine only if vectors disagree.
- **First server packet special case** (`LoginEncryption.encrypt`): XOR pass
  with random key + static Blowfish key + `_static=false` flip. Sizes: pad to
  8, +8 checksum block (`encryptedSize`).
- **RSA "nopadding"**: raw modexp on a 128-byte block (no PKCS#1). Use
  `num-bigint-dig` `modpow` directly (the `rsa` crate enforces padding).
  Modulus scrambling (`ScrambledKeyPair`) is 4 array transforms — port 1:1.
- **Password hash**: `SHA-1(password)` → Base64 — `sha1` + `base64` crates.
- **GameGuard constants, protocol rev, packet layouts**: copy verbatim.

## 3. Workspace layout

```
l2r_interlude/
├── Cargo.toml            # workspace
├── docs/                 # (this file, model docs)
├── crates/
│   ├── commons/          # ← reused by gameserver later
│   │   ├── network/      #    framing, Client, queues, traits (see §4)
│   │   ├── crypt/        #    L2 blowfish wrapper, checksum, xor-pass, rsa raw
│   │   ├── config/       #    .ini/.properties parser (PropertiesParser port)
│   │   ├── db/           #    sqlx SQLite pool + helpers
│   │   └── util/         #    Rnd, hexid, string utils
│   └── loginserver/      # binary crate
│       ├── main.rs           # bootstrap (mirrors LoginServer.java order)
│       ├── controller.rs     # LoginController state + actor task
│       ├── gs_table.rs       # GameServerTable
│       ├── session.rs        # SessionKey, AccountInfo, enums
│       ├── network/
│       │   ├── client_packets/   # 8 files, same names as Java
│       │   ├── server_packets/   # 13 files
│       │   └── encryption.rs     # LoginEncryption port
│       └── gs_link/
│           ├── listener.rs       # GameServerListener + flood protection
│           ├── thread.rs         # GameServerThread → per-GS tokio task
│           └── packets/          # gs/ls packets, same names
└── dist/                 # config templates, sql, servername.xml (copied)
```

Java file → Rust file is 1:1 wherever the language allows (per project goal);
the packet classes become structs implementing `read`/`run` or `write` traits.

## 4. `commons/network` — the reusable core (replaces Async-mmocore)

Designed so the game server later changes **only** the packet enum, the cipher,
and the executor side. tokio-based:

- **`Framing`**: length-prefixed codec, 2-byte LE header including itself
  (`HEADER_SIZE = 2`), max-size guard.
- **`trait PacketCipher`**: `decrypt(&mut self, buf: &mut [u8]) -> bool` /
  `encrypt(&mut self, buf: &mut BytesMut) -> bool` (stateful — login flips off
  the static key; game XOR cipher rolls its key). Login and game provide impls.
- **`Connection` task pair** per socket: read half (frame → decrypt → raw
  packet bytes → `handler`), write half (bounded `mpsc<Bytes>` — serialize +
  encrypt before queueing or after, decided by profiling; start simple:
  encrypt in the write task).
- **`trait PacketBuffer`** read/write helpers with L2 conventions: LE ints,
  `f64`, UTF-16LE null-terminated strings, byte arrays — port of
  `ReadableBuffer`/`WritableBuffer` surface.
- **Lifecycle hooks**: `on_connected` (server pushes `Init`), `on_disconnected`
  (cleanup message to state owner), close-with-final-packet (Java
  `close(packet)` semantics: clear queue, send one, disconnect).
- **Acceptor**: bind, optional `ConnectionFilter` (IP bans / flood rules),
  client-id assignment.
- Deliberately deferred to the GS phase (documented as stubs): drop-policy
  (`DropPackets` threshold), buffer pooling, `Network.ini` knobs — login
  traffic never needs them.

## 5. Login server concurrency shape

Per CONCURRENCY_MODEL open question #3: **pure tokio, no game thread.** The
mutable state (`authed clients`, session keys, failed-login counters, IP bans,
GS table entries) lives in **one `LoginController` actor task** owning a plain
struct; connection tasks and GS-link tasks talk to it via
`mpsc<ControllerMsg>` + `oneshot` replies. This keeps the "single owner, no
locks" invariant from the concurrency model at login scale, and packet handler
code stays sequential and 1:1 with the Java `run()` bodies.

- CPU work (RSA decrypt of credentials — ~1 ms) runs on the connection task,
  not the controller, mirroring Java (it runs on the pool there).
- DB access: sqlx async SQLite pool, called from the controller task's message
  handlers (login rate makes blocking-the-actor-on-awaits acceptable; the
  gameserver's stricter DB-thread pattern is not needed here).
- Timers: `LoginController.purge` (5 min), scheduled restart, temp-ban expiry —
  `tokio::time::interval` inside the controller task's `select!`.
- GS link: `GameServerThread` becomes one tokio task per connected GS
  (read loop + write queue), flood-protection check on accept.

## 6. Milestones

Ordered so every step is verifiable against real counterparts.

- **M0 — Scaffold.** Workspace, crates, CI-less `cargo build`, `tracing`
  logging, config loader reading the *existing* `LoginServer.ini` keys, SQLite
  pool opening the existing schema (apply `dist/db_installer` SQL for
  `accounts`/`gameservers`). ✔ = builds, loads config, connects DB.
- **M1 — Crypto parity.** `commons/crypt`: L2-Blowfish, checksum, XOR pass, raw
  RSA + modulus scramble, SHA-1+Base64. ✔ = **golden-vector tests**: a tiny Java
  harness (run once against `loginserver/crypt` classes) dumps input/output
  vectors to JSON; Rust tests must match exactly. This de-risks everything else.
- **M2 — Client handshake.** `commons/network` + `Init` → `AuthGameGuard` →
  `GGAuth`. ✔ = real Interlude client reaches the login/password screen with no
  protocol error (client visibly accepts `Init` and sends GG auth).
- **M3 — Authentication.** `RequestAuthLogin` (both credential layouts),
  `LoginController` actor, account lookup/auto-create, failed-login bans,
  `banned_ip.cfg`, `LoginOk`/`LoginFail`/`AccountKicked`. ✔ = client logs in
  against SQLite; wrong password / banned IP paths behave like Java.
- **M4 — GS link + server list.** GS listener, key exchange, `GameServerAuth`
  (hexid match against `gameservers` table), `ServerList`,
  `RequestServerLogin`/`PlayOk`, `PlayerAuthRequest`/`PlayerAuthResponse`,
  `PlayerInGame`/`PlayerLogout`, `ServerStatus`, kick. ✔ = **the unmodified Java
  game server registers with the Rust login server, and a real client logs in
  end-to-end into the Java GS.** This is the phase gate.
- **M5 — Long tail.** Remaining GS-link packets (`ChangePassword`,
  `RequestTempBan`, `PlayerTracert`, `RequestCharacters`/`ReplyCharacters` —
  chars-per-server counts in `ServerList`), `RequestCmdLogin` config path,
  scheduled restart, graceful shutdown, Dockerfile (mirror `login.Dockerfile`).
  ✔ = feature-parity checklist against the Java file list, item by item.

Suggested review points: after M1 (vectors), after M4 (interop demo).

## 7. Testing strategy

1. **Golden vectors (M1)** — the only trustworthy way to port crypto.
2. **Packet snapshot tests** — serialize each server packet with fixed inputs;
   compare hex against captures from the Java server (log-based or a 20-line
   Java dump harness).
3. **Interop integration (M2–M4)** — real client + real Java GS against the
   Rust LS; this substitutes for the absent Java test suite.
4. **Actor unit tests** — controller state machine (double login, ALREADY_ON_GS
   kick path, ban expiry) driven by messages, no sockets.

## 8. Dependencies (initial Cargo set)

`tokio` (net, sync, time, macros), `bytes`, `sqlx` (sqlite, runtime-tokio),
`blowfish`/`cipher` (wrapped LE), `sha1`, `base64`, `num-bigint-dig` (raw RSA),
`rand`, `tracing` + `tracing-subscriber`, `thiserror`, `quick-xml`
(servername.xml), `ctrlc` or tokio signal. No framework crates; the point is a
thin, auditable stack.

## 9. Explicitly out of scope (this phase)

- Game server (next phase — reuses `commons/*`; its client-side cipher, packet
  enum, and the game-thread executor are the only new network pieces).
- GUI (decision #10), MariaDB/Postgres (decision #9), `tools/` ports,
  `accounts_ipauth` enforcement beyond parity, DatabaseBackup (SQLite: file
  copy on shutdown — trivial, included in M5 only if config enables it).

## 10. Risks

| Risk | Mitigation |
|---|---|
| Blowfish byte-order subtleties brick the handshake | M1 golden vectors before any networking |
| Client-observable framing differences (padding, checksum sizes) | `encryptedSize` logic ported 1:1 + snapshot tests |
| GS-link protocol drift (Java GS is the peer) | Test against the real Java GS from M4 day one; keep `PROTOCOL_REV 0x0106` |
| Actor round-trips complicate handler code | Handlers run *inside* the controller task (like packet `run()` on one thread); connection tasks only do I/O + RSA |
