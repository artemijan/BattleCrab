# PLAN — SeaORM 2 migration (models crate + Rust migrations + DAO layer)

Replace hand-written SQL with SeaORM 2 entities, move every table definition
into a **shared models crate**, and turn the fresh-install DDL that currently
lives in the Java repo (`dist/db_installer/sql/{sqlite,mariadb,postgresql}`)
into **Rust migrations** that any of the three binaries — and any future tool —
can run.

Status: **not started**. This document is the plan; nothing below has landed.

---

## 1. Where we are today (measured, not guessed)

| Surface | Files | `sqlx::query*` call sites |
|---|---|---|
| `gameserver/src/db.rs` (5108 lines) | 1 | 208 (60 functions; **107 of them inline in `run`**) |
| `dashboard_api/src/db/{accounts,admin,characters,mod}.rs` | 4 | 53 |
| `loginserver` (`dao.rs`, `controller.rs`, `gs_table.rs`, `gs_link/connection.rs`) | 4 | 12 |
| tests (`loginserver/tests/common`, `gameserver/tests/e2e_create`, `dashboard_api/tests/api`) | 3 | ~30, incl. **hand-written `CREATE TABLE` DDL** |

Statement mix across the workspace: 112 `SELECT`, 82 `INSERT`, 64 `UPDATE`,
45 `DELETE`, 28 `REPLACE`. **57 of the 100 dist tables** are referenced; the
other 43 have no Rust consumer yet.

Schema source of truth today: `../interlude_classic/dist/db_installer/sql/` —
**96 game tables + 4 login tables** (98 `.sql` files — `clanentry.sql` alone
defines the three `pledge_*` tables) plus 64 index statements, in three
dialects (`sqlite`, `mariadb`, `postgresql`). The `dumps/` are not seed data:
the only non-DDL statement in the whole tree is a single `INSERT INTO
gameservers` row. Per CLAUDE.md, that tree is **authoritative** — the Rust side
adapts to it, never the reverse.

Two things sit outside that tree and must not be lost:

- `docs/migrations/2026-07-21-master-accounts.sql` — the dashboard master-account
  change (`accounts.login` becomes nullable + `UNIQUE`, `is_verified` added,
  partial unique index `accounts_master_email`). It was applied by hand to the
  live DB; there is no record of it anywhere in code.
- The **live production database already exists** and has that patch applied.
  Migrations must be able to *adopt* it, not only build a DB from scratch.

Runtime: SQLite only (`commons::db::init`, JDBC-style URL, exe-relative path
resolution, WAL + `busy_timeout`). The game server runs all DB work on one
dedicated thread with a current-thread Tokio runtime, driven by
`DbCommand`/`DbEvent` channels (`CONCURRENCY_MODEL.md` §2.4). **This plan does
not change that model** — only what happens inside the thread.

---

## 2. Version decisions (verified against crates.io on 2026-07-30)

| Crate | Version | Notes |
|---|---|---|
| `sea-orm` | **2.0.0** (stable, released 2026-07-27) | needs `sqlx ^0.9`, MSRV 1.94 (local toolchain is 1.97 ✔) |
| `sea-orm-migration` | 2.0.0 | pulls `sea-orm` with `schema-sync`; `cli` feature gives the `up/down/status/fresh` CLI |
| `sea-orm-cli` | 2.0.0 | dev-time only, `cargo install sea-orm-cli` — **not** a workspace dependency |
| `sqlx` | **0.8.6 → 0.9.0** | forced: SeaORM 2 links `sqlx 0.9` |
| `sea-query` | 1.0 (transitive) | migration DSL |

Feature flags to use:

```toml
# workspace Cargo.toml
sea-orm = { version = "2.0", default-features = false, features = [
    "sqlx-sqlite", "runtime-tokio", "macros", "with-chrono",
] }
sea-orm-migration = { version = "2.0", default-features = false, features = [
    "sqlx-sqlite", "runtime-tokio", "cli",
] }
sqlx = { version = "0.9", default-features = false, features = [
    "runtime-tokio", "sqlite", "macros",
] }   # kept only until the last raw query is gone (slice 13)
```

The sqlx 0.8 → 0.9 bump is cheap in feature terms (`runtime-tokio`, `sqlite`,
`macros` all still exist under those names) but is a **breaking API bump** for
the ~300 existing call sites; that is why slice 0 does it in isolation, with no
other change in the diff.

Two facts that shape the rollout:

- `sqlx-sqlite` 0.8.6 wants `libsqlite3-sys ^0.30.1`; 0.9.0 wants
  `>=0.30.1, <0.38.0`. Both unify on 0.30.1, so sqlx 0.8 and 0.9 *can*
  technically coexist — but the two `SqlitePool` types are not interchangeable and
  we would need two pools against one file. **Rejected**: bump once, keep one
  pool.
- SeaORM exposes `SqlxSqliteConnector::from_sqlx_sqlite_pool(pool)` and
  `DatabaseConnection::get_sqlite_connection_pool()`. That is the bridge that
  makes an incremental cutover possible: `commons::db` keeps building the pool
  exactly as it does now (URL parsing, exe-relative path, WAL, busy_timeout —
  all of which have tests worth keeping), wraps it into a `DatabaseConnection`,
  and any not-yet-ported raw query borrows the pool back out of it.

---

## 3. Target layout

```
crates/
  models/                     # NEW — the shared crate
    src/
      lib.rs                  # re-exports sea_orm + prelude
      entity/                 # one module per table (100)
        mod.rs
        characters.rs
        items.rs
        accounts.rs
        …
      repo/                   # table-level DAO (see §5)
        mod.rs
        accounts.rs
        characters.rs
        …
    tests/
      schema_parity.rs        # entities ↔ migrations (see §6)
  migration/                  # NEW — sea-orm-migration crate + `l2r-migrate` bin
    src/
      lib.rs                  # Migrator: Vec<Box<dyn MigrationTrait>>
      main.rs                 # cli::run_cli(Migrator).await
      m20260801_000001_baseline_login.rs
      m20260801_000002_baseline_game.rs
      m20260801_000003_master_accounts.rs
    tests/
      dist_parity.rs          # migrations ↔ dist DDL (see §6)
      fixtures/dist_sqlite/   # snapshot of dist/db_installer/sql/sqlite/**
  commons/                    # db.rs now returns DatabaseConnection
  loginserver/  gameserver/  dashboard_api/   # consumers
```

Naming: `models` (the user-facing name for the crate) with the entities under
`models::entity::*`. The migrator is a second crate because
`sea-orm-migration` drags in `clap`/`dotenvy` behind its `cli` feature and the
servers must not link that.

`gameserver/src/db.rs` (5108 lines) is split as part of the cutover:

```
gameserver/src/db/
  mod.rs          # DbCommand/DbEvent, spawn(), the thread's run loop
  boot.rs         # the ~25 unprompted boot loads
  character.rs    # load/create/store_player/delete
  clan.rs  castle.rs  olympiad.rs  boss.rs  economy.rs  moderation.rs …
```

This is a by-product, not the goal — but the command loop's 107 inline queries
are exactly the part that becomes unreadable if left alone.

---

## 4. Entities: how they get written

1. Build a throwaway SQLite DB from the authoritative DDL:
   `for f in ../interlude_classic/dist/db_installer/sql/sqlite/{login,game}/*.sql; do sqlite3 /tmp/schema.db < $f; done`
2. `sea-orm-cli generate entity -u sqlite:///tmp/schema.db -o crates/models/src/entity`
3. **Hand-audit the generated types** — this is the real work, and the generator
   is wrong in ways that matter here (§8).
4. Commit. From then on the *entities are the source of truth* for Rust; the
   generator is only ever re-run as a cross-check.

Entity conventions:

- Keep the dist column names verbatim via
  `#[sea_orm(column_name = "charId")] pub char_id: i32` — the DB is shared with
  the Java server's schema and column names are not ours to modernise.
- No `Relation` graph in the first pass. The Java schema has almost no real FKs
  and the queries are id-keyed; a relation graph would be invented structure.
  Add relations later, per table, only where a join is actually written.
- `DeriveActiveEnum` for string/int enums (`items.loc`, `punishments.type`,
  `punishments.affect`) is **deferred** — first cut keeps `String`/`i32` so the
  cutover diff stays a pure mechanical translation.

---

## 5. DAO: yes, but a thin one (this is the "maybe use DAO? think" answer)

A repository layer per table, wrapping single-row CRUD, would be pure ceremony —
SeaORM's `Entity::find_by_id(..).one(db)` *is* that layer already. The value of a
DAO here is narrower and real in three places, so the recommendation is a
**hybrid**:

**In `models::repo` — only queries with more than one consumer, or that encode
a shared rule.** Concretely: `accounts` (login server auth, game server play-auth,
dashboard registration/admin all read it), `characters` (game server + dashboard),
`account_data`, `gameservers`. ~8 modules, ~30 functions. Signature shape:

```rust
pub async fn find_by_login<C: ConnectionTrait>(db: &C, login: &str)
    -> Result<Option<accounts::Model>, DbErr>;
```

Generic over `C: ConnectionTrait` — not `&DatabaseConnection` — so every one of
them composes inside a `DatabaseTransaction`. Returns entity `Model`s, never
domain types: the models crate must not learn about `Player`, `Clan`, or
`PlayerSaveData`, or it becomes a second gameserver.

**In the owning crate — aggregate operations.** `store_player_tx` (29 queries
across 12 tables in one transaction), the character-load bundle, `create_character`.
These mix game structs with a Java-parity contract (`storeCharBase`,
`storeCharSub`, …) and belong next to the code that owns those structs. They are
written against entities, take `&C: ConnectionTrait`, and keep their current
transaction boundaries exactly.

**Nowhere — one-off writes.** The command loop's fire-and-forget updates
(`UpdateClanLeader`, `StoreNpcRespawn`, …) become 3-line entity calls in the
domain module for their table. Wrapping each in a repo function named after the
command adds a hop and no information.

What does *not* change: `DbCommand`/`DbEvent`, the single DB thread, the
memory-first flush policy, id allocation. The ORM lives strictly below that line.

---

## 6. Migrations

### 6.1 Shape

Three migrations to start:

| Migration | Contents |
|---|---|
| `m20260801_000001_baseline_login` | `accounts`, `account_data`, `accounts_ipauth`, `gameservers` + indexes |
| `m20260801_000002_baseline_game` | the 96 game tables + their indexes |
| `m20260801_000003_master_accounts` | the `docs/migrations/2026-07-21-master-accounts.sql` change, as Rust |

Written as explicit `Table::create()` / `Index::create()` statements —
transcribed from the dist SQLite DDL by a throwaway script, then hand-reviewed
and, more importantly, **machine-verified** (§6.3). Not
`Schema::create_table_from_entity(..)`: that renders whatever the entity says,
so a wrong entity silently produces a wrong schema and the parity test loses its
independence. The entity↔migration agreement is asserted separately, in a test.

Every table create is guarded so the migration is a no-op on a database that
already has it:

```rust
if !manager.has_table(Characters::Table.to_string()).await? {
    manager.create_table(Table::create().table(Characters::Table)…).await?;
}
```

and the master-account migration guards on
`manager.has_column("accounts", "is_verified")`. That is what makes **adopting
the live production DB** a no-risk `l2r-migrate up`: the migrator writes its
`seaql_migrations` bookkeeping rows and touches nothing else.

### 6.2 Backends

Migrations are written **backend-agnostically** (sea-query column types, no raw
SQL), so the same Rust replaces all three dialect trees in `dist/db_installer`.
Runtime wiring stays SQLite-only — nothing in this plan enables MariaDB or
Postgres, it only stops actively precluding them. Where dist uses MySQL-only
types (`MEDIUMINT`, `TINYINT`) the migration uses
`ColumnType::custom("MEDIUMINT")` so a future MariaDB run matches dist; SQLite
ignores the difference (type affinity), which the parity test confirms.

### 6.3 Verification (the part that makes this safe)

Two tests, both cheap, both run in the normal `cargo nextest run`:

1. **`migration/tests/dist_parity.rs`** — apply `Migrator::up` to an in-memory
   SQLite; apply the snapshotted dist DDL to a second one; compare normalized
   schemas (`PRAGMA table_info` + `PRAGMA index_list` per table: name, type,
   notnull, dflt_value, pk). Any transcription slip in 100 tables fails here with
   the exact column named. The dist `.sql` files are snapshotted into
   `crates/migration/tests/fixtures/dist_sqlite/` because the Java repo is a
   sibling checkout, not a dependency.
2. **`models/tests/schema_parity.rs`** — for every entity,
   `Schema::create_table_from_entity` vs the migrated table: same column set,
   same nullability, same primary key. Catches the entity/migration drift that
   otherwise shows up as a decode error in production.

Plus, per [[l2r-verify-test-detects-bug]]: before landing each test, break one
column on purpose and confirm it fails.

### 6.4 Running them

New binary `l2r-migrate` (`crates/migration/src/main.rs`, `cli::run_cli`):

```
l2r-migrate status                     # what's applied
l2r-migrate up                         # apply pending (safe on a live DB)
l2r-migrate down -n 1                  # roll back one
l2r-migrate fresh                      # drop everything and rebuild — dev only
```

`DATABASE_URL` is taken from the environment, but the binary also accepts the
project's JDBC-style URL (`-u jdbc:sqlite:…`) through the same
`commons::db` parser, so operators use the one string they already have in
`LoginServer.ini`.

`deploy.sh` gains a step: ship `l2r-migrate`, back the DB up
(`cp interlude_classic.db …bak-$(date)`), run `l2r-migrate up`, then start the
services. First boot on a host with no DB stops being "provision it by hand from
`dist/db_installer/dumps/`".

Documentation lands in **`docs/DATABASE.md`** (fresh install, adopting an
existing DB, adding a migration, entity regeneration, backup/rollback) with a
short pointer section added to the top-level `README.md` — the user-visible
deliverable of "add instructions in a readme file how to migrate db". A draft
outline is in Appendix A.

---

## 7. Slices

Each slice is one PR, compiles, passes `cargo nextest run` + `cargo clippy
--workspace --all-targets -- -D warnings`, and changes **no behaviour**. Order is
chosen so the riskiest surface (the game server's persistence) moves last, over
the smallest steps, behind the existing persistence tests.

| # | Slice | Scope | Gate |
|---|---|---|---|
| 0 | sqlx 0.8 → 0.9 | mechanical API fixes across all ~300 call sites; nothing else in the diff | suite green |
| 1 | `crates/models` skeleton + login entities | 4 tables, hand-audited | crate builds, unit test round-trips `accounts` |
| 2 | `crates/migration` + login baseline + `l2r-migrate` + `docs/DATABASE.md` | + `dist_parity` test for the 4 login tables | `l2r-migrate up` on a copy of prod is a no-op |
| 3 | login server cutover | 12 call sites, `dao.rs` → `models::repo::accounts`; `tests/common` builds its DB with `Migrator::up` instead of hand-written DDL | `m5_parity`, `auth` tests green |
| 4 | game entities + game baseline | 96 tables; the big mechanical diff, no consumer changes | both parity tests green |
| 5 | `commons::db` → `DatabaseConnection` | keep URL parsing/exe-relative/WAL tests; consumers borrow the pool back via `get_sqlite_connection_pool()` — no query changes | full suite green |
| 6 | dashboard cutover | 53 sites; the two admin list queries keep raw SQL via `raw_sql!` (§8) | `dashboard_api/tests/api.rs` green, its DDL constants deleted |
| 7 | game: characters + items | `load_characters`, `create_character`, `store_player_tx`, `load_items` — the transaction shape is preserved verbatim | `char_persistence`, `e2e_create` green |
| 8 | game: the boot loads | ~25 `load_*` fns → `boot.rs` | census tests ([[l2r-census-tests]]) unchanged |
| 9 | game: clans, crests, recruit, subpledges | | |
| 10 | game: castle, siege, manor, clan halls, residence functions | | |
| 11 | game: olympiad, heroes, bosses, npc respawns, cursed weapons | | |
| 12 | game: economy + social (mail, auctions, lottery, MDT, premium, buffer schemes, favorites, punishments, shortcuts, macros, variables, quests, skills) | | |
| 13 | cleanup | drop `sqlx` from every crate manifest, delete the last DDL string in tests, `deploy.sh` runs `l2r-migrate up`, update `README.md` / `CONCURRENCY_MODEL.md` / `PROGRESS.md` | `grep -r 'sqlx::query' crates` → 0 hits |

Slices 7–12 are independent of each other and can land in any order once 4–5 are
in.

**Estimate:** slice 0 ≈ 0.5 day; 1–3 ≈ 1.5 days; 4 ≈ 2 days (100 tables, mostly
script + review); 5–6 ≈ 1.5 days; 7 ≈ 1.5 days; 8–12 ≈ 3–4 days; 13 ≈ 0.5 day.
**≈ 11–12 focused days.**

---

## 8. Gotchas (the ones that will actually bite)

1. **Floats stored in integer columns.** `characters.curHp/curCp/curMp` are
   declared `MEDIUMINT` in dist but the code binds and reads `f64` (Java stores
   doubles; SQLite's type affinity keeps a fractional value as REAL). Same for
   `character_summons.curHp/curMp` (`int`) and `pets.curHp/curMp` (`int`) —
   while `npc_respawns.currentHp/currentMp` really are `double`. The generated
   entity will say `i32` for the first group and **decode-fail at runtime on any
   character with fractional HP**. Entity field types must follow *how the code
   reads the column*, not the DDL. Add a round-trip test that saves `1234.5` HP.
2. **SQLite integer widths.** SeaORM 2 maps both `Integer` and `BigInteger` to
   `i64` for SQLite by default (`--big-integer-type=i32` at generation time, or
   hand-fix). The codebase is `i32`-heavy (`object_id`, `char_id`, item ids) and
   silently widening them would ripple into every packet writer.
3. **`REPLACE INTO` and `ON CONFLICT` upserts** (28 sites) →
   `Entity::insert(..).on_conflict(OnConflict::column(..).update_columns(..))`.
   Verify each one against the dist table's actual unique index — a couple of
   the current upserts rely on an index that exists only because the dist DDL
   declares it (e.g. `character_reco_bonus.charId`).
4. **Booleans are `TINYINT`.** `nobless`, `is_verified`, `online` are 0/1 ints;
   map as `i8`/`i32` and convert at the domain boundary, not as `bool` in the
   entity (a NULL `is_verified` is meaningful — it distinguishes a game account
   from a master account).
5. **Dynamic ORDER BY + correlated subqueries** in `dashboard_api/src/db/admin.rs`
   (`list_masters`: two correlated counts, `LIKE … ESCAPE`, `COLLATE NOCASE`).
   Translating that into `QuerySelect` would be a rewrite, not a port. Keep it as
   SQL through SeaORM's `raw_sql!` macro (parameterised, injection-safe), and
   note it as deliberate. Same for `list_masters`' shared `WHERE` constant.
6. **`COLLATE NOCASE`** appears in the master-account unique index and in
   dashboard lookups. sea-query can emit collations, but confirm the emitted
   partial index (`WHERE login IS NULL`) matches byte-for-byte — the parity test
   covers exactly this.
7. **`DELETE FROM items WHERE owner_id=?` + N inserts** in `store_player_tx`:
   use `insert_many` (SeaORM 2 reworked it; `last_insert_id` is now
   `Option<Value>`) and keep the delete+insert shape rather than "improving" it
   to an upsert — the Java parity contract is a full rewrite of the owned set.
8. **`ExprTrait` must be in scope** for `.eq()`/`.like()` on `Expr` in
   sea-query 1.0 (`use sea_orm::ExprTrait;`). Minor, but it is the first
   compile error everyone hits.
9. **Generated entity code vs `-D warnings`.** The pre-commit hook runs clippy
   with `-D warnings` over `--all-targets`; generated modules may trip lints
   (e.g. `enum_variant_names`). Prefer fixing to blanket `#![allow]` at the crate
   root, and if a grandfathered allow is unavoidable, add it to the workspace
   lints table with a comment, per the existing convention.
10. **Never let SeaORM touch the schema at runtime.** `schema-sync` /
    `get_schema_registry().sync()` is a genuinely useful 2.0 feature and is
    exactly wrong here: the DB is shared with the Java server's schema and the
    dist DDL is authoritative. Schema changes happen *only* through migrations.
11. **`seaql_migrations` on the live DB.** It will be created on first
    `l2r-migrate up`. Confirm the Java server (which the operator may still run
    against the same file) ignores unknown tables — it does, but verify before
    the first production run.
12. **Two tests copy the real `interlude_classic.db`** (`e2e_create.rs`, and the
    README notes a second). Once migrations exist those should build a fresh DB
    with `Migrator::up` instead; that also un-skips them on a clean checkout.

---

## 9. Open decisions

Defaults below are what the plan assumes; each is cheap to flip before slice 4.

| # | Decision | Assumed | Alternative |
|---|---|---|---|
| A | Baseline covers **all 100** dist tables, or only the 57 the Rust touches | all 100 — a fresh install then matches what the Java installer produces, and later milestones (forums, offline trade, instances' `character_instance_time`, …) need no schema work | 57 now, add tables as milestones need them: ~35% less transcription, but "fresh install" stops meaning "the dist schema" |
| B | Migrations backend-agnostic | yes (§6.2) — it costs almost nothing at write time and retires all three dialect trees | SQLite-only, raw `Statement`s where convenient |
| C | Raw SQL escape hatch allowed | yes, for the ≤5 analytical queries in `dashboard_api` (§8.5), each with a comment saying why | 100% entity API, accepting a rewrite of those queries |
| D | `models` crate name | `models` (`models::entity::*`, `models::repo::*`) | `entity` + `migration`, the SeaORM book's convention |
| E | Enum columns (`items.loc`, `punishments.type`) | `String`/`i32` in the first cut | `DeriveActiveEnum` immediately — better types, but mixes a refactor into a mechanical port |

---

## Appendix A — `docs/DATABASE.md` outline

```
# Database

## Layout                  one SQLite file next to the binaries; WAL; the URL
                           in LoginServer.ini / GameServer.ini resolves
                           relative to the *executable*
## Fresh install           cargo run -p migration -- up   (or ./l2r-migrate up)
                           → 100 tables, then the single seed `gameservers` row
## Existing database       back up, `l2r-migrate status`, `l2r-migrate up`
                           (every baseline create is has_table-guarded, so an
                           existing DB is only bookkept, never rewritten)
## Adding a migration      sea-orm-cli migrate generate <name>
                           → crates/migration/src/m<date>_<name>.rs
                           → register in Migrator::migrations()
                           → run the dist_parity + schema_parity tests
                           SQLite caveat: no DROP/ALTER COLUMN before 3.35 —
                           use the create-copy-rename dance
## Regenerating entities   build a temp DB from migrations, `sea-orm-cli
                           generate entity`, diff against crates/models/src/entity
                           (a non-empty diff is a bug in one of the two)
## Rollback / recovery     l2r-migrate down -n 1; restore from the deploy backup
## Deploy                  deploy.sh backs up + runs `up` before starting units
```

## Appendix B — the reference the migrations replace

`../interlude_classic/dist/db_installer/sql/` — 96 game + 4 login tables ×
3 dialects. The plan snapshots the **sqlite** tree into
`crates/migration/tests/fixtures/dist_sqlite/` as the parity oracle. The Java
tree stays read-only and authoritative; if the two ever disagree, the migration
is wrong.
