<p align="center">
  <img src="docs/assets/battlecrab.png" alt="L2R Battlecrab — Lineage 2 Rust server" width="360">
</p>

<h1 align="center">l2r_interlude</h1>

<p align="center">
  A Rust rewrite of the <b>L2J Mobius Interlude Classic</b> Lineage 2 server —
  L2J, but in Rust.
</p>

---

## A port, not a fork

This is a 1:1 port of [L2J Mobius](https://l2jmobius.org) Interlude Classic. The
Java server in the sibling `interlude_classic` tree is the reference
implementation, and it is the ground truth for every behavioural question.

**It is backward compatible with your existing Mobius setup:**

- **The same config files.** Every `.ini` under `dist/game/config` and
  `dist/login/config` is read with Java's `PropertiesParser` semantics, keys and
  defaults included — point this server at a Mobius config directory and it
  behaves as that config says.
- **The same datapack.** The XML, HTML and SQL under `dist/` are consumed
  unchanged. They are treated as the specification: when the port disagrees with
  the data, the port is wrong.
- **The same database schema.** Same tables, same column names (`charId`,
  `accessLevel` — verbatim, not modernised), so an existing database is adopted
  rather than migrated.
- **The same wire protocol.** Unmodified clients connect, and the Rust login
  server has been verified interoperating with the *unmodified Java game
  server*.

Environment variables override any config value using the Java convention:
`CONFIG_LOGINSERVER_URL=jdbc:sqlite:./data/l2.db`.

## Status

| Component | Status |
|---|---|
| Login server | ✅ Feature-complete, interop-verified against the unmodified Java game server |
| Game server | ✅ All milestones G0–G34 complete — world, combat, skills & effects, quests, clans, sieges, olympiad, grand bosses, instances, pets & summons, fishing, events, mail & community board, and the GM command system |

"Complete" means each milestone's gate was met and verified against the Java
server on the same database and client. Narrow behaviours deliberately skipped
inside shipped features are marked in the code and counted — **134 of them**,
enumerated in [docs/DEFERRALS.md](docs/DEFERRALS.md) and held to the code by a
test. Start at **[docs/PORTING_STATUS.md](docs/PORTING_STATUS.md)** for the full
picture of what is ported, partial, and deliberately out of scope.

## Documentation

| | |
|---|---|
| **[Porting status](docs/PORTING_STATUS.md)** | What is ported, what is partial, what never will be — one table for the whole port |
| **[Threading model](docs/THREADING_MODEL.md)** | How the server is threaded, why, and what it costs — with diagrams |
| **[Project layout](docs/PROJECT_LAYOUT.md)** | Where code lives, where new code goes, and the conventions |
| **[Progress journal](docs/PROGRESS.md)** | The dated record of what landed and what broke on the way |
| [All documentation](docs/README.md) | Index, including the database, parity checklists and the dashboard design |

## Architecture in one paragraph

One dedicated **game thread** owns all mutable world state; tokio handles the
sockets, a dedicated thread owns SQLite, a path worker owns pathfinding, and
everything talks to the game thread over channels — no locks in game logic. Game
objects live in an **ECS** (via the standalone `bevy_ecs` crate): an object is an
entity whose data sits in components packed into contiguous archetype tables, so
the per-tick systems (regen, movement, AI) sweep them as dense linear scans
instead of pointer-chasing a map. The loop runs at a 100 ms tick and warns when
one overruns. Full reasoning, diagrams and trade-offs in
[THREADING_MODEL.md](docs/THREADING_MODEL.md).

## Workspace

- `crates/commons` — shared infrastructure (network core, L2 crypto, config, SQLite), reused by both servers
- `crates/loginserver` — the login server binary
- `crates/gameserver` — the game server binary
- `crates/models` — SeaORM entities for every table, plus the shared repositories
- `crates/migration` — the schema as migrations, and the `l2r-migrate` binary
- `crates/tools` — offline datapack/client tools, and the `l2r-tools` binary
- `crates/dashboard_api` + `web/dashboard` — the web dashboard and its API
- `crates/launcher` — the player-facing updater

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

## Datapack & client tools

`l2r-tools` answers questions about the datapack by running the *server's* own
geo code over it, so a verdict from it is a verdict in game. It also unpacks and
repacks the client's `system` directory, and reconciles the strings the client
owns (NPC names, system messages) with the server's data.

```sh
cargo build --release -p tools
./target/release/l2r-tools spawn-pockets --region 20_21   # mobs buried under the floor
./target/release/l2r-tools client-dat decrypt             # system -> editable text
./target/release/l2r-tools sync-npc --dry-run             # datapack names -> client
```

All six commands are documented in
**[`crates/tools/README.md`](crates/tools/README.md)**.

## Testing

The suite runs under [cargo-nextest](https://nexte.st) — ~2,970 tests:

```sh
cargo install cargo-nextest --locked   # once
cargo nextest run                        # whole workspace
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
