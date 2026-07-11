# l2r_interlude

A Rust rewrite of the L2J Mobius **Interlude Classic** Lineage 2 server —
L2J, but in Rust.

Ported 1:1 from the Java source, keeping the same wire protocol, config files,
and SQLite schema so it drops into the existing setup unchanged.

## Status

| Component | Status |
|---|---|
| Login server | ✅ Feature-complete — verified interoperating with the unmodified Java game server |
| Game server | 🚧 Playable vertical slice through G9 — login → character create → enter world, items/skills, movement + geodata, 34.9k NPC spawns, melee/magic combat, monster AI, XP/loot, death & revive. Social systems, quests, and scripting still ahead (see [PROGRESS.md](docs/PROGRESS.md)) |

## Architecture in one paragraph

One dedicated **game thread** owns all mutable world state; tokio handles the
sockets, a dedicated thread owns SQLite, and everything talks to the game
thread over channels — no locks in game logic. Game objects are stored in an
**ECS** (Entity–Component–System, via the standalone `bevy_ecs` crate): an
object is an entity whose data lives in components packed into contiguous
archetype tables, and the per-tick systems (regen, movement, AI) sweep them
as dense, cache-friendly linear scans instead of pointer-chasing a map. See
[CONCURRENCY_MODEL.md](docs/CONCURRENCY_MODEL.md) (ECS: §2.8).

## Workspace

- `crates/commons` — shared infrastructure (network core, L2 crypto, config, SQLite), reused by both servers
- `crates/loginserver` — the login server binary
- `crates/gameserver` — the game server binary

## Build & run

```sh
cargo build --release
# run from the repo root; reads dist/login/config/LoginServer.ini
./target/release/loginserver
# run with dist/game as the working directory (auto-chdir handles the repo root)
./target/release/gameserver
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
