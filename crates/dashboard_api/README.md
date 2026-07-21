# dashboard_api

Web dashboard for the BattleCrab server: account registration, login, password/email management,
and a read-only character list. Serves its own React SPA.

Design and rationale: [`docs/PLAN_DASHBOARD.md`](../../docs/PLAN_DASHBOARD.md).

## Running it

```bash
# 1. Set the secret — the server refuses to start without it.
export DIST_GAME_CONFIG_DASHBOARD_SESSIONSECRET="$(openssl rand -hex 32)"

# 2. Build the frontend. Optional for `cargo run` — a debug build reads dist/
#    from disk at runtime, and without it you get a "frontend not built"
#    placeholder while the API works normally. Required for a release build,
#    which embeds dist/ into the binary.
cd web/dashboard && bun install && bun run build && cd -

# 3. Run from the repo root, so the relative DB path in Dashboard.ini resolves.
cargo run -p dashboard_api
```

`cargo build` never requires Bun: `build.rs` creates the (gitignored) `web/dashboard/dist`
directory if it is absent, because `#[derive(RustEmbed)]` fails to *compile* against a missing
folder. It emits a `cargo:warning` pointing here when the directory is empty.

Config lives in `dist/game/config/Dashboard.ini`. Every key can be overridden with an environment
variable named `DIST_GAME_CONFIG_DASHBOARD_<KEY>` — the prefix comes from the *file path*, so
moving the ini renames the variables.

### The database path is the thing that goes wrong

`URL` is relative to the **working directory**, and the database is gitignored — so it exists in
your main checkout but *not* in a fresh git worktree. Start the server from the wrong directory and
you are pointing at a file that isn't there.

The server now refuses to start in both failure modes, naming the absolute path it tried:

- **File missing** — it will not create one. Run from the directory holding the real
  `interlude_classic.db`, or set an absolute path:
  `DIST_GAME_CONFIG_DASHBOARD_URL="jdbc:sqlite:/abs/path/interlude_classic.db?journal_mode=WAL&busy_timeout=5000"`.
- **File present but not the game DB** (no `accounts`/`characters`) — typically a 4 KB empty file
  left behind by an earlier run from the wrong directory. Delete it and fix the path.

Both used to be silent: `commons::db::init` opens with `create_if_missing(true)`, so a wrong path
produced an empty database and every request then failed with `no such table: characters` — a
stream of 500s at runtime instead of one clear error at boot.

### Frontend dev loop

```bash
cargo run -p dashboard_api            # terminal 1 — API on :8080
cd web/dashboard && bun run dev       # terminal 2 — SPA on :3000, proxies /api to :8080
```

The proxy keeps everything same-origin so the session cookie behaves exactly as in production.

## Three things to know before changing this crate

1. **The password hash is not ours to choose.** `accounts.password` is `Base64(SHA1(pw))` because
   that is what the game client verifies. Changing it produces accounts nobody can log in with.
   `commons::crypt::hash_password` is the only correct way to write that column.

2. **`characters` is read-only.** Live character state is memory-first in the game server and
   flushed on autosave/logout/shutdown; any write here is silently clobbered or resurrects stale
   data. There are deliberately no write helpers in `db::characters`.

3. **There is no session table.** Sessions are HMAC-signed cookies and reset/verify links are
   HMAC-signed tokens, both signed over the account's *current* password hash — which is what makes
   a password change invalidate every outstanding session and reset link. If you add storage, say
   so in the design doc first; the no-new-tables rule is a deliberate constraint, not an oversight.

## Tests

```bash
cargo test -p dashboard_api
```

31 unit tests plus 14 integration tests that drive the real axum router against an in-memory SQLite
DB using the shipped `accounts`/`characters` DDL. The most important one is
`register_stores_the_hash_the_game_client_expects` — it is the acceptance test for the whole
design.
