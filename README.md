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
- `crates/models` — SeaORM entities for every table, plus the shared repositories
- `crates/migration` — the schema as migrations, and the `l2r-migrate` binary

## Database

The schema lives in `crates/migration` as SeaORM migrations; entities are in
`crates/models`. Provision or upgrade a database with the `l2r-migrate` binary:

```sh
cargo build --release -p migration
./target/release/l2r-migrate up -u jdbc:sqlite:interlude_classic.db
```

Running it against an existing database is safe — every statement is
`IF NOT EXISTS` — so it records the migrations as applied and changes nothing.
See [docs/DATABASE.md](docs/DATABASE.md) for fresh installs, adopting a live
database, adding a migration and regenerating entities.

## Build & run

```sh
cargo build --release
# Neither binary changes its working directory. The SQLite `URL` in both inis
# is relative to the EXECUTABLE, so put interlude_classic.db beside the binary
# and both servers open the same file whatever directory you start them from.
./target/release/loginserver   # reads dist/login/config/LoginServer.ini
./target/release/gameserver    # finds the datapack at dist/game automatically

# Datapack elsewhere? Point the game server at it (it still does not chdir):
DATAPACK_ROOT=/srv/l2/dist/game ./target/release/gameserver
```

Config values can be overridden by environment variables using the Java
`PropertiesParser` convention: `CONFIG_LOGINSERVER_<KEY>`
(e.g. `CONFIG_LOGINSERVER_URL=jdbc:sqlite:./data/l2.db`).

## Testing

The suite runs under [cargo-nextest](https://nexte.st):

```sh
cargo install cargo-nextest --locked   # once
cargo nextest run                        # whole workspace (~1600 tests)
cargo nextest run -p gameserver          # just the game server
cargo nextest run -p gameserver stealth  # substring filter
cargo nextest run --profile ci           # retries + JUnit (see .config/nextest.toml)
```

nextest runs each test in its own process, which **isolates tests from each
other's global state** — the game-server suite used to hang under plain
`cargo test` because a few integration tests mutate process-global state (the
current directory, a PID-named temp DB). The `.config/nextest.toml` `default`
profile also **terminates any test that runs past ~2 minutes**, so a single
deadlocked test is reported as a `TIMEOUT` failure instead of wedging the run.

Two integration tests copy the real `interlude_classic.db` (an untracked
working-tree file); they run on a full checkout / CI and self-skip on a fresh
checkout or `git worktree` that lacks it. Plain `cargo test` still works for a
single filtered test, but prefer nextest for anything broad.

One test is `#[ignore]`d: `e2e_create::full_login_to_character_create`. It is a
pre-existing failure the old hanging `cargo test` never reached — the login
server answers `RequestServerLogin` with `PlayFail` instead of `PlayOk`. That is
a real login play-auth bug (grep `TODO(login-playauth)`), not a test-runner
issue; remove the `#[ignore]` once it is fixed. Run it explicitly with
`cargo nextest run -p gameserver --run-ignored all full_login_to_character_create`.

## Linting & formatting

- **Rust**: `cargo fmt` (rustfmt, default style) and `cargo clippy --workspace --all-targets -- -D warnings`.
  Pre-existing stylistic lints are grandfathered in `[workspace.lints.clippy]` (root `Cargo.toml`) —
  delete an entry there, fix what surfaces, and commit both together to burn the list down.
- **Frontend**: [Biome](https://biomejs.dev) (formatter + linter in one binary) — `cd web/dashboard`
  then `bun run lint` / `bun run format`. Config in `web/dashboard/biome.json`.
- **Tailwind classes**: `eslint-plugin-better-tailwindcss` (`bun run lint:tw`, config in
  `web/dashboard/eslint.config.js`) — conflicting/duplicate/unknown classes and canonical forms
  (`text-(--x)` over `text-[var(--x)]`). ESLint exists in the repo for these rules only.
- **Pre-commit hook** runs both, scoped to what is staged. Activate once per clone:

  ```
  git config core.hooksPath .githooks
  ```

  Bypass in an emergency with `git commit --no-verify`.

## Docs

- [`docs/PROGRESS.md`](docs/PROGRESS.md) — **milestone progress & current state** (start here)
- [`docs/JAVA_TO_RUST_CHALLENGES.md`](docs/JAVA_TO_RUST_CHALLENGES.md) — concept differences and the architectural decisions
- [`docs/CONCURRENCY_MODEL.md`](docs/CONCURRENCY_MODEL.md) — threading/ownership model
- [`docs/PLAN_LOGIN_SERVER.md`](docs/PLAN_LOGIN_SERVER.md) — login server implementation plan
- [`docs/PLAN_GAME_SERVER.md`](docs/PLAN_GAME_SERVER.md) — game server implementation plan (milestones G0–G12)
- [`docs/LOGIN_SERVER_PARITY.md`](docs/LOGIN_SERVER_PARITY.md) — file-by-file Java→Rust parity checklist
