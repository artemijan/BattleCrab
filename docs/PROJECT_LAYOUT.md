# Project layout & conventions

Where things live, and where a new thing should go. If you are about to add a
file and are not sure which directory it belongs in, the answer is here.

The organising principle: **this is a port**, so the layout mirrors the Java
tree closely enough that a Java class name usually predicts the Rust module
name, and departures from it are deliberate and documented at the top of the
module that departs.

---

## 1. The workspace

| Crate | What it is |
|---|---|
| `crates/commons` | Infrastructure both servers need: network core (framing, tokio transport), L2 crypto, the `.ini` config reader, the SQLite handle, system messages, cron, shutdown signals. Java's `commons/`. |
| `crates/loginserver` | The login server binary. |
| `crates/gameserver` | The game server binary — the bulk of the port. |
| `crates/models` | SeaORM entities for every table, plus thin shared repositories. |
| `crates/migration` | The schema as SeaORM migrations, and the `l2r-migrate` binary. |
| `crates/tools` | Offline datapack/client tools and the `l2r-tools` binary ([README](../crates/tools/README.md)). |
| `crates/dashboard_api` | HTTP API behind the web dashboard ([DASHBOARD.md](DASHBOARD.md)). |
| `crates/launcher` | The player-facing updater/launcher (egui). |
| `web/dashboard` | The dashboard SPA (React + Bun + Biome). |
| `dist/` | The runtime tree — datapack, configs, SQL. **Not ours to change**; see §6. |

---

## 2. Inside `gameserver`

```
crates/gameserver/src/
├── main.rs            boot order (mirrors Java GameServer), channel wiring, shutdown
├── lib.rs             what the binary and the tests can see
├── world.rs           World — the single owner of all mutable game state
├── store.rs           the ECS: entity ↔ object-id index. `Entity` never leaves this file
├── scheduler.rs       one-shot timers, keyed by tick (Java ThreadPool.schedule)
├── session.rs         per-connection session state machine
├── db.rs              the DB thread: DbCommand in, DbEvent out
├── character.rs       character create/select/delete
├── enums.rs           shared enums that are not any one model's
│
├── config/            one module per .ini file — parse only, no behaviour
├── data/              one module per data/xml/* loader — the datapack, read-only
├── model/             the domain types: Player, Npc, Clan, Skill, Item, Siege…
├── network/           packets, cipher, connection tasks
│   ├── client_packets.rs     inbound opcode → typed struct
│   └── server_packets/       one module per outbound packet family
├── game_loop/         the tick loop and every handler and system that runs on it
├── scripts/           compiled-in quest/AI scripts (Java's data/scripts/**.java)
├── geo/               geodata: regions, LOS, pathfinding + the path worker thread
└── loginlink/         the game↔login TCP link
```

### Where does my code go?

| I am adding… | It goes in | Notes |
|---|---|---|
| A new `.ini` key | `config/<file>.rs` | Parsing only. A parsed key with no reader is this project's most-repeated failure — wire the consumer in the same change. |
| A loader for a datapack XML | `data/<name>_data.rs` | Named after the Java `data/xml/` class. Read-only after boot. |
| A domain type (a thing in the world) | `model/<thing>.rs` | Data + pure methods. No packet sends, no DB. |
| Behaviour that runs on the game thread | `game_loop/<subsystem>.rs` | Handlers, tick systems, and the rules. This is where most porting lands. |
| An outbound packet | `network/server_packets/<family>.rs` | Serialization only. |
| An inbound packet | `network/client_packets.rs` + a handler in `game_loop/` | Parse in the first, act in the second. |
| A quest or NPC script | `scripts/<name>.rs` + register in `scripts::build_registry` | Mirrors the datapack script's name. |
| A new table's row type | `crates/models/src/entity/` | See §3 — do not hand-write it. |
| A query used by more than one binary | `crates/models/src/repo/` | See §3. |
| A GM command | `game_loop/admin/<area>.rs` | Gating comes from `AdminCommands.xml`; the body is yours. |

**The split that matters most is `model/` vs `game_loop/`.** A `model` type
knows its own data and can compute from it; it cannot send a packet, touch the
database, or reach another object. Anything that coordinates two objects, or
talks to a client, is a `game_loop` function taking `&mut World`. This is what
keeps the model types testable and the borrow checker satisfied — a system
holding `&mut world.objects` and `&world.geo` is two disjoint field borrows, and
that only works if the model layer does not try to reach sideways.

---

## 3. Database code

Three layers, and the boundaries are strict:

```
crates/models/src/entity/   one module per table, GENERATED from the SQL DDL
crates/models/src/repo/     table-level queries with more than one consumer
crates/gameserver/src/db.rs the DB thread + domain aggregates (store_player, …)
```

- **Entities are generated, not written.** They come from the DDL in
  `dist/db_installer/sql/**` via sea-orm-codegen and a normalizer script; see
  [DATABASE.md](DATABASE.md) for the regeneration procedure. Column names stay
  verbatim — `charId`, `accessLevel` — because the schema is shared with the
  Java server and is not ours to modernise.
- **`repo/` is for queries with two or more consumers**, or one that encodes a
  rule worth naming once ("a temporary ban masks the access level"). Single-table
  CRUD does *not* belong there: `Entity::find_by_id(..).one(db)` is already that
  layer and wrapping it adds a hop and no information. Every function is generic
  over `C: ConnectionTrait`, so it composes inside a transaction.
- **Domain aggregates stay out of `models`.** `store_player` and the
  character-load bundle mix game structs with a Java-parity contract, so they
  live in the crate that owns those structs. `models` must never learn what a
  `Player` is.
- **Nothing outside `db.rs` performs I/O.** Game code sends a `DbCommand` and
  handles a `DbEvent` next tick. See [THREADING_MODEL.md](THREADING_MODEL.md) §4.

A schema change is therefore three coordinated edits: a migration in
`crates/migration`, a regenerated entity, and the reader/writer in `db.rs`. A
column persists only once **both** ends carry it — a migration alone changes
nothing observable.

---

## 4. Conventions

### Module headers carry the Java source

Every module opens with a `//!` header naming the Java class it ports and any
deliberate departure. This is the single most useful convention in the repo:

```rust
//! `org.l2jmobius.gameserver.model.World` — the single owner of all mutable
//! game state. Exactly one thread (the game thread) ever touches it, so it
//! holds no locks (CONCURRENCY_MODEL §2, challenge #2).
```

### Record every skipped behaviour at the site

When a port intentionally skips part of the Java behaviour, leave a
`TODO(G<N>): …` comment **at the exact spot**, naming what the Java source does
("Java also fires `EVT_FORGET_OBJECT` at the AI here"). Never silently drop a
Java side effect — that is how parity bugs like the missing
`TargetUnselected`-on-visibility-drop happen.

These markers are load-bearing, not litter: they are counted by
`deferral_markers_match_the_recorded_inventory`, whose expected list is
**empty** as of 2026-08-07. Adding a gap without recording it there fails the
build; so does closing one without taking it off.

### `PLAN_*.md` in a comment means a retired plan

Comments cite plan documents (`PLAN_G19_SYMBOLS.md`) that no longer exist as
files — they were deleted once their work shipped. The name still identifies
*which* plan, and [PORTING_STATUS.md](PORTING_STATUS.md#retired-plans) lists all
172 with the `git show` command to read one. Do not add new such references;
point at the code or at PROGRESS.md instead.

### Java side effects hide in overrides

When porting a call, check the `Player`/`Creature` override chain, not just the
method named at the call site. `Player.setTarget(null)` broadcasts
`TargetUnselected` with `includeSelf` — nothing at the call site says so.

### Tests

- Unit tests live in the module they test (`#[cfg(test)] mod tests`).
- Game-loop behaviour tests live in `game_loop/tests/<subsystem>_tests.rs`.
- Cross-crate and end-to-end tests live in each crate's `tests/`.
- **Run with `cargo nextest run`**, not `cargo test` — see the README for why
  (process isolation; plain `cargo test` hangs on globals).
- **Verify the test detects the bug**: disable the fix and confirm the test
  fails. A test that passes both ways tests nothing.

### Naming

Rust module names follow the Java class they port, snake_cased
(`SkillTreeData` → `data/skill_tree_data.rs`, `AttackableAI` → `game_loop/npc_ai.rs`).
Where a Rust module deliberately merges or splits Java classes, the header says
which ones.

---

## 5. Formatting & lint

`cargo fmt` (default rustfmt) and
`cargo clippy --workspace --all-targets -- -D warnings`. Pre-existing stylistic
lints are grandfathered in `[workspace.lints.clippy]` at the workspace root —
delete an entry, fix what surfaces, commit both together to burn the list down.

Frontend is Biome (`web/dashboard`), plus an ESLint config that exists solely
for the Tailwind class rules. The pre-commit hook runs both, scoped to what is
staged; activate it once per clone with `git config core.hooksPath .githooks`.

---

## 6. `dist/` is the specification

The XML, SQL and `.ini` under `dist/` define real, retail-faithful behaviour and
are **treated as 100% correct**. When the port behaves differently from what the
data implies, the bug is in the port. Do not edit the datapack to match the
code, and do not write off a datapack value as wrong — the Elven Ruins
"to village" bug looked like a bad `respawn.xml` and was a missing `RespawnZone`
port.

The Java tree in the sibling `interlude_classic` repository is the reference
implementation to port *from*; work happens here.
