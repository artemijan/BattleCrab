# l2r_interlude

A Rust rewrite of the L2J Mobius **Interlude Classic** Lineage 2 server —
L2J, but in Rust.

Ported 1:1 from the Java source, keeping the same wire protocol, config files,
and SQLite schema so it drops into the existing setup unchanged.

## Status

| Component | Status |
|---|---|
| Login server | ✅ Feature-complete — verified interoperating with the unmodified Java game server |
| Game server | ⏳ Not started |

## Workspace

- `crates/commons` — shared infrastructure (network core, L2 crypto, config, SQLite), reused by both servers
- `crates/loginserver` — the login server binary

## Build & run

```sh
cargo build --release
# run from the repo root; reads dist/login/config/LoginServer.ini
./target/release/loginserver
```

Config values can be overridden by environment variables using the Java
`PropertiesParser` convention: `CONFIG_LOGINSERVER_<KEY>`
(e.g. `CONFIG_LOGINSERVER_URL=jdbc:sqlite:./data/l2.db`).

## Docs

- [`docs/PROGRESS.md`](docs/PROGRESS.md) — **milestone progress & current state** (start here)
- [`docs/JAVA_TO_RUST_CHALLENGES.md`](docs/JAVA_TO_RUST_CHALLENGES.md) — concept differences and the architectural decisions
- [`docs/CONCURRENCY_MODEL.md`](docs/CONCURRENCY_MODEL.md) — threading/ownership model
- [`docs/PLAN_LOGIN_SERVER.md`](docs/PLAN_LOGIN_SERVER.md) — login server implementation plan
- [`docs/PLAN_GAME_SERVER.md`](docs/PLAN_GAME_SERVER.md) — game server implementation plan (milestones G0–G12)
- [`docs/LOGIN_SERVER_PARITY.md`](docs/LOGIN_SERVER_PARITY.md) — file-by-file Java→Rust parity checklist
