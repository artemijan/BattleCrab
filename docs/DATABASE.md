# Database

One SQLite file holds both the login and the game schema. The servers open it
through `commons::db`, which accepts the JDBC-style URL the `.ini` files already
carry (`jdbc:sqlite:interlude_classic.db?journal_mode=WAL&busy_timeout=5000`)
and resolves a **relative path against the executable's directory** — so the
database belongs beside the binaries, and one URL string is correct for the
login server, the game server, the dashboard and the migration tool alike.

The schema is defined by the migrations in `crates/migration`, written with
SeaORM/sea-query. They are a transcription of `dist/db_installer/sql/sqlite/**`,
which stays the authoritative source; `crates/migration/tests/dist_parity.rs`
compares the two column by column on every test run.

## Applying migrations

The binary is `l2r-migrate` (`cargo build -p migration --release` →
`target/release/l2r-migrate`).

```bash
l2r-migrate status -u jdbc:sqlite:interlude_classic.db   # what is applied
l2r-migrate up     -u jdbc:sqlite:interlude_classic.db   # apply everything pending
l2r-migrate up     -n 1 -u …                             # apply one
l2r-migrate down   -n 1 -u …                             # roll one back
```

`-u` may be omitted when `DATABASE_URL` is set. During development,
`cargo run -p migration -- status -u sqlite://./interlude_classic.db` works the
same way.

Destructive commands refuse to run without `--yes`:

```bash
l2r-migrate fresh   --yes -u …    # drop every table, rebuild from scratch
l2r-migrate refresh --yes -u …    # roll all the way back, then re-apply
l2r-migrate reset   --yes -u …    # roll all the way back
```

## Fresh install

```bash
l2r-migrate up -u jdbc:sqlite:interlude_classic.db
```

That creates all 100 tables and their indexes. Add the one row the login server
needs to know about a game server — either through the server's own
registration handshake, or by hand:

```sql
INSERT INTO gameservers (server_id, hexid, host) VALUES (1, '<hexid>', '127.0.0.1');
```

## Adopting a database that already exists

This is the normal case for an existing deployment, and it is safe:

```bash
cp interlude_classic.db interlude_classic.db.bak-$(date +%Y%m%d)
l2r-migrate status -u jdbc:sqlite:interlude_classic.db   # all "Pending"
l2r-migrate up     -u jdbc:sqlite:interlude_classic.db
```

Every baseline statement is `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT
EXISTS`, and the master-account migration checks for its own column before
touching anything. So `up` on a provisioned database creates the
`seaql_migrations` bookkeeping table, records the three migrations as applied,
and changes nothing else. `dist_parity.rs::up_is_idempotent_on_an_existing_database`
is the test that keeps it that way.

A database still on the pre-dashboard `accounts` shape (login as a NOT NULL
primary key) is the one case where `up` does rewrite a table: migration
`m20260801_000003_master_accounts` rebuilds `accounts` so `login` may be NULL —
that is what a dashboard master account is. Take the backup first.

## Adding a migration

1. Create `crates/migration/src/m<YYYYMMDD>_<NNNNNN>_<name>.rs` with a
   `Migration` struct implementing `MigrationTrait` (copy the shape of
   `m20260801_000003_master_accounts.rs`).
2. Register it in `Migrator::migrations()` in `crates/migration/src/lib.rs`.
3. Mirror the change into `dist/db_installer/sql/**` — that tree is the
   specification, and `dist_parity` fails until the two agree.
4. Regenerate the entities (below) and run `cargo nextest run -p migration -p models`.

**SQLite caveat:** before 3.35 there is no `DROP COLUMN`, and there has never
been an `ALTER COLUMN`. Anything beyond adding a column or an index means the
create-copy-drop-rename dance — `m20260801_000003_master_accounts.rs` is the
worked example, including how it stays a no-op on a database that already has
the new shape.

## Regenerating entities

Entities live in `crates/models/src/entity`, one module per table. They are
generated, not hand-written — the type fixes live in
`tools/normalize_entities.py`, so a correction survives the next regeneration:

```bash
# 1. a throwaway database with the current schema
rm -f /tmp/schema.db
for f in dist/db_installer/sql/sqlite/login/*.sql dist/db_installer/sql/sqlite/game/*.sql; do
    sqlite3 /tmp/schema.db < "$f"
done

# 2. generate, then re-type from the declared column types
cargo install sea-orm-cli --version 2.0.0        # once
sea-orm-cli generate entity -u "sqlite:///tmp/schema.db?mode=ro" -o /tmp/gen --with-serde none
python3 tools/normalize_entities.py /tmp/schema.db /tmp/gen crates/models/src/entity
```

Why the second step exists: `sea-orm-cli` infers Rust types from SQLite's
*storage classes*, so it types every integer as `i64`, `TINYINT` as `i8`,
`DECIMAL(20,0)` as `rust_decimal::Decimal`, and the `crests.data` blob as an
ignored `String`. The script re-types each field from the **declared** type
instead, and applies the handful of overrides that the DDL cannot express:

* `characters.curHp/curCp/curMp` and `pets.curHp/curMp` are declared as
  integers but hold doubles (Java writes them that way and SQLite keeps the
  fraction). Typed `i32`, they would silently truncate a wounded character's HP.
* Sixteen tables declare no primary key; SeaORM needs one, so the script
  supplies a logical key per table. This changes no schema — migrations come
  from the DDL, not from entities.
* `accounts` keys off SQLite's implicit `rowid`, because its `login` is nullable
  (a NULL login marks a dashboard master account).

## Backup and rollback

`deploy.sh` takes a copy of the database before it runs `l2r-migrate up`. To go
back a step, `l2r-migrate down -n 1`; to go back to a known-good file, stop the
services and restore the copy. Note that rolling back
`m20260801_000003_master_accounts` **drops every master account** — they have no
`login`, and the pre-dashboard schema has nowhere to put them.

## Which tables are in use

All 100 are created; the Rust server currently reads or writes 57 of them. The
rest belong to features that are not ported yet (forums, offline trade, instance
timers, …) and exist so that porting one of them needs no schema work.
