# Web Dashboard — Technical Design

Status: **D1–D2 implemented** on branch `feat/dashboard` (written 2026-07-21), then reworked onto
**email identity** on `feat/dashboard-email-auth` (§15). `crates/dashboard_api` and `web/dashboard`
exist and work end to end — register, log in, list characters, change password/email. §14 records
exactly what is built, where the implementation deviated from this plan, and what is still stubbed.

Scope: a public web application for the BattleCrab L2 server offering account **registration**,
**account management** (login, change password, email, view characters), and a
**landing/marketing page**. Delivered as a REST API (new Rust crate) + a React SPA.

**v1 uses exactly one database — the live game SQLite DB — and exactly two tables in it:
`accounts` (read/write, narrowly) and `characters` (read-only). No new tables.** That constraint
is the single most shaping decision in this document; §5 explains how auth works without any
storage of its own, and §7 covers what has to change when the coin shop eventually lands.

> **Amendment (2026-07-21) — §15 supersedes parts of §5.** The dashboard identity is now an
> **email address**, not a game login name, and `accounts` gained an `is_verified` column. Where
> §5.4/§5.5 say the session subject is `login`, or that a stored `accounts.email` means "verified",
> read §15 instead — those passages describe the original design, kept for the reasoning behind
> the stateless-token construction, which is unchanged.

---

## 1. Goals and non-goals

**Goals**

- Self-service account registration that produces an account the real game client can log into.
- Account management: login, change password, set/verify email, password reset.
- Read-only character list for the logged-in account (name, level, class, playtime).
- Server status (online/offline, player count).
- A landing page for the server (features, download link to the launcher, register CTA).

**Non-goals for v1**

- **The coin shop.** Deferred — it cannot be built without new storage. See §7.
- Admin/GM panel (ban, mute, item spawn). Separate concern, separate auth tier — design later.
- Forums, clan pages, rankings/ladder. Nice-to-have, additive to the same API.
- Any write path into `characters`, `items`, or other live game rows. See §3.

---

## 2. Where this fits in the existing system

The Cargo workspace at `/Users/artem/dev/l2/l2r_interlude` currently has four members:
`commons`, `loginserver`, `gameserver`, `launcher`. The dashboard API becomes a fifth.

Facts verified against the current tree (not assumed):

| Fact | Evidence |
| --- | --- |
| Passwords are `Base64(SHA1(password))` | `crates/commons/src/crypt/password.rs` — `hash_password()`, with the known-vector test `"test"` → `qUqP5cyxm6YcTAhz05Hph5gvu9M=` |
| Login and game share **one SQLite file** | `interlude_classic.db` at repo root; `crates/commons/src/db.rs` is SQLite-only, WAL, `busy_timeout` |
| `accounts` already has an `email` column | `accounts(login, password, email, created_time, lastactive, accessLevel, lastIP, lastServer, …)` |
| Account creation on-the-fly already exists | `crates/loginserver/src/dao.rs` — `INSERT INTO accounts (login, password, lastactive, accessLevel, lastIP)` |
| Password change already exists server-side | `crates/loginserver/src/controller.rs::change_password` — verifies old hash, writes new |
| Config is Java-style `.ini` with env override | `crates/commons/src/config.rs` — `PropertiesParser`, `CONFIG_<FILE>_<KEY>` env keys |

This means the dashboard is mostly **reusing** existing primitives (`commons::crypt::hash_password`,
`commons::db::init`, `commons::config::PropertiesParser`) rather than inventing new ones. There is
no schema work at all in v1.

---

## 3. The three hard constraints

These drive most of the decisions below. Violating any of them produces data corruption or
accounts that cannot log into the game.

### 3.1 The game password hash is not negotiable

The client authenticates against `accounts.password` using `Base64(SHA1(pw))`. A web-created
account **must** store that exact hash or the player cannot log in. Do not "upgrade" this column
to argon2 — that is a coordinated change across the login server and out of scope here.

With no table of its own, the dashboard also **verifies web logins against this same hash**. That
is a real security compromise, not a neutral choice; §5.2 states it plainly and lists what
compensates for it.

### 3.2 Character state is memory-first — never write it

Live character state (inventory, HP, position, buffs) lives in the game server's memory and is
flushed only by periodic autosave, logout, and shutdown. A web process writing into `characters`
or `items` for an online player **will be silently clobbered by the next autosave** — and worse,
could resurrect stale values over newer ones.

> `characters` is **strictly read-only** to this crate. The only table it writes is `accounts`,
> and only the `password` and `email` columns.

### 3.3 SQLite allows exactly one writer

The DB is WAL-mode SQLite shared with two long-running server processes. WAL permits concurrent
readers alongside one writer, and `busy_timeout` handles contention — but the dashboard must keep
writes **small, short, and rare**.

In v1 this is comfortable: the only writes are a registration, a password change, and an email
change. All are single-row, user-initiated, and rare. There is no background writer, no session
table churn, no queue polling — a direct consequence of the no-new-tables rule.

If the dashboard ever grows read-heavy (rankings, statistics), the answer is a periodic snapshot
or a read replica, not longer queries against the live DB. Flagged as an open question in §12.

---

## 4. Architecture

```
                    ┌───────────────────────────────┐
  browser ────────► │  dashboard_api  (Rust/axum)   │
   (SPA)            │  • REST /api/v1/*             │
                    │  • serves embedded SPA assets │
                    │  • stateless: no session store│
                    └───────────────┬───────────────┘
                                    │ sqlx (SQLite, WAL) — one pool
                                    ▼
                    ┌───────────────────────────────┐
                    │      interlude_classic.db     │
                    │   accounts    (r/w: pw, email)│
                    │   characters  (READ-ONLY)     │
                    └───────────────┬───────────────┘
                                    │
                                    ▼
                       loginserver       gameserver
```

Account creation, login, and character listing go **directly against the live game DB**. There is
no sync job, no mirrored copy, no second database. An account created by the web is the same row
the login server reads, immediately — which is the entire point of the direct-access approach.

Deliberately **no direct network link** between `dashboard_api` and the game/login servers in v1.
The database is the integration point. This keeps the dashboard deployable and restartable
independently, and means a dashboard outage cannot take the game down.

(A later slice may add a small internal HTTP endpoint on the game server for live player count and
"kick/reload" actions — see §12.)

---

## 5. Backend

### 5.1 Stack

| Concern | Choice | Why |
| --- | --- | --- |
| HTTP framework | **axum** (0.8 line) | Tokio-native, tower middleware ecosystem, already a tokio workspace |
| Async runtime | **tokio** (workspace dep) | Already pinned in `[workspace.dependencies]` |
| DB | **sqlx 0.8**, SQLite, **one pool** | Already a workspace dep with the `sqlite` feature; `commons::db::init` builds it from the same JDBC-style URL the other servers use |
| Middleware | **tower-http** | CORS, compression, tracing, `ServeDir`/embedded assets for the SPA |
| Serialization | **serde / serde_json** | Already used by `launcher` |
| Validation | Hand-written in `routes::validate_*` (planned: `validator` derive — see §14) | The rules encode game-client compatibility, not generic shapes |
| Session auth | **signed cookie**, no server-side store | See §5.3 — there is no table to put sessions in |
| Signing / tokens | **hmac + sha2** | Cookie signatures and stateless email/reset tokens (§5.4) |
| Rate limiting | In-process `auth::ratelimit` (planned: `tower_governor` — see §14) | Load-bearing, not optional (§5.2); needs per-IP **and** per-account keys |
| Email | **lettre** (SMTP) | Verification + password reset |
| Errors | **thiserror** (workspace) + an `IntoResponse` impl | Matches existing crates' style |
| Logging | **tracing** / **tracing-subscriber** (workspace) | Consistent with the rest of the workspace |
| API schema | **utoipa** + `utoipa-swagger-ui` | Generates OpenAPI → feeds the typed FE client (§8) |

Versions above reflect what is current as of writing; pin exact versions at implementation time
and check for breaking releases (axum in particular has moved fast).

Note there is **no argon2 dependency** — with only `accounts` available there is nowhere to store a
second hash. See below.

### 5.2 Password handling — one hash, and what that costs

Registration and password change write `accounts.password = commons::crypt::hash_password(pw)`,
i.e. `Base64(SHA1(pw))`. Web login verifies against that same value. One hash, one column, no new
storage — and any account created in-game can log into the web immediately with no "claim" flow,
which removes a whole feature that a dual-hash design would have needed.

**State the downside honestly**: unsalted SHA-1 is fast and unsalted, so if the DB leaks, the whole
account table is crackable offline at enormous speed, and identical passwords across accounts are
visible as identical hashes. This is already true of the game server today — the web dashboard does
not introduce the weakness — but it does **widen the exposure**, because a public HTTPS login form
is far more reachable than the game protocol.

Compensating controls, all of which should be treated as required, not optional:

- **Aggressive rate limiting** on `/auth/login`, per-IP *and* per-account, with exponential backoff
  and a lockout threshold. This is the primary defence against online guessing.
- **Minimum password length enforced at registration** (the practical mitigation for weak hashes).
- **TLS everywhere**, HSTS, and never logging credentials.
- **Generic failure messages** — never distinguish "no such account" from "wrong password".

The upgrade path, when it is wanted: add a `web_credentials` table with argon2, write both hashes
on registration and password change, verify argon2 for web and leave SHA-1 for the game. That is a
one-table change and is exactly what §7's storage decision should reconsider. Until then, this is a
**deliberate, bounded** acceptance of the existing hash — not an oversight.

### 5.3 Sessions — signed cookies, because there is no session table

Server-side sessions need storage. With no table available, use a **stateless signed cookie**:
HTTP-only, `Secure`, `SameSite=Lax`, containing `login` + issued-at + expiry, HMAC-signed with
`SessionSecret` from config (`tower-cookies`' private/signed jar, or `axum-extra`'s `SignedCookieJar`).

Tradeoffs, accepted knowingly:

- **Not individually revocable.** There is no server-side record to delete, so "log out everywhere"
  and instant ban-kick are not possible. Mitigations: keep the lifetime short (e.g. 7 days), and
  bind the signature to the account's current password hash (§5.4's trick) so that **changing the
  password invalidates every existing session** — which covers the case that actually matters
  after a compromise.
- **Rotating `SessionSecret` logs everyone out.** Acceptable; document it in the runbook.

Logout clears the cookie client-side. That is honest for this design — it ends the session on that
browser, and password change is the tool for ending them everywhere.

CSRF: `SameSite=Lax` plus requiring a custom header (e.g. `X-Requested-With`) on all mutating
requests. If a stricter posture is wanted, add the double-submit token pattern — also stateless.

### 5.4 Email verification and password reset — stateless tokens

Both flows normally need a `tokens` table. Neither does here.

**Password reset.** The emailed token is `HMAC(SessionSecret, login ‖ expiry ‖ current_password_hash)`
plus the plaintext `login` and `expiry`. Verification recomputes the HMAC using the account's
*current* hash. This gives single-use semantics for free: the moment the password changes, the hash
changes, and the token no longer validates. Expiry caps the window (e.g. 1 hour). This is the same
construction Django uses for its reset links.

**Email verification.** Issue a token over `subject ‖ new_email ‖ expiry` and **only write the
address when the link is clicked**.

> Superseded in part by §15: the "a stored address is verified by construction" trick is gone,
> because registration now stores the address up front — it *is* the login. An explicit
> `accounts.is_verified` column records proof instead. The token construction is unchanged.

One consequence to accept: a user who requests a change and never clicks the link keeps their old
email, with nothing in the UI showing "pending". Re-request is the remedy. Cheap, and no storage.

### 5.5 Data access rules

One `SqlitePool` on `AppState`, built with `commons::db::init` (it already parses the JDBC-style
URL and sets WAL + `busy_timeout`).

**`accounts` — read/write, narrowly**

- `INSERT` on registration, mirroring `loginserver/src/dao.rs`'s column set.
- `UPDATE` on `password` and `email` **only**.
- **Never** write `accessLevel` (privilege escalation), `lastIP`/`pcIp`/`hop*` (the login server
  owns those), `lastServer`, or `lastactive`.
- Registration must handle the race with in-game auto-creation: rely on a unique constraint and
  treat a violation as "taken", rather than a check-then-insert. (Since §15, `login` is a UNIQUE
  index rather than the primary key, and master accounts are made unique by the
  `accounts_master_email` partial index.)

**`characters` — read-only**

- `SELECT` for the character list, scoped to `account_name = <session login>`.
- Enforce it structurally: put these queries in a module with no write helpers, and expose them
  through a narrow projection type rather than a row struct mirroring the whole table.

**Everything else in the DB: untouched.** No new tables, no schema migrations, no
`dist/db_installer` changes. The crate ships without a `migrations/` directory in v1.

### 5.6 What must never leave the API

Never expose: `password` (the hash), `accessLevel`, `lastIP`/`pcIp`/`hop*`, other accounts'
characters, or character coordinates and inventory. The character list projection should be an
explicit allowlist of columns — `char_name`, `level`, `classid`, `race`, `sex`, `onlinetime`,
`lastAccess`, `online` — never `SELECT *`.

---

## 6. API surface (v1)

Prefix everything with `/api/v1`. JSON in, JSON out. Errors use a consistent envelope
(`{"error": {"code": "...", "message": "..."}}`) with proper status codes.

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `POST` | `/auth/register` | — | Create an `accounts` row with the SHA-1 hash |
| `POST` | `/auth/login` | — | Verify hash, set signed cookie |
| `POST` | `/auth/logout` | cookie | Clear the cookie |
| `GET`  | `/auth/me` | cookie | Current account summary |
| `POST` | `/auth/forgot-password` | — | Email a stateless reset token (always 200, never leak existence) |
| `POST` | `/auth/reset-password` | token | Verify token, rewrite hash (invalidates sessions) |
| `POST` | `/account/password` | cookie | Change password (requires current) |
| `POST` | `/account/email` | cookie | Request email change → verification mail |
| `GET`  | `/account/email/verify` | token | Consume token, write `accounts.email` |
| `GET`  | `/account/characters` | cookie | Character list (read-only projection) |
| `GET`  | `/server/status` | — | Online/offline + player count |

No `/auth/claim` — unnecessary, since web and game share one hash (§5.2). No `/shop/*` — deferred
(§7).

Rate-limit `register`, `login`, and `forgot-password` aggressively (per-IP and per-account).

---

## 7. Deferred: the coin shop

The shop is **out of v1** because it cannot be built inside the two-table constraint. Recording
this now so the eventual decision is informed rather than rediscovered.

What it minimally requires:

1. **A wallet + ledger** — balance per account, and an append-only audit of every change. Money
   state without an audit trail is not operable when a player disputes a purchase.
2. **A delivery queue** — the game server must grant items through its *normal in-game item-add
   path* (the one quest rewards use), so inventory limits, weight, and client packets are honored.
   Writing items directly into the DB violates §3.2 and will be clobbered by autosave.
3. **Idempotency** — an order key that makes a retried delivery a no-op rather than a double grant.

That is three tables' worth of state, so when the shop lands, the single-DB decision must be
revisited. Two options, in rough order of preference:

- **Add the tables to the game DB.** The delivery queue genuinely belongs there — the game server
  must read it — and this keeps one file, one backup, and lets a purchase debit the wallet and
  enqueue the delivery in **one atomic transaction**. Cost: schema additions to the game DB, and
  more write traffic against its single writer (§3.3).
- **A second `dashboard.db` for web-only state.** Keeps game schema pristine, but the wallet and
  the queue then live in different files, and SQLite has no cross-file transactions — so the
  purchase needs an outbox and a background pusher to avoid either double-granting or debiting
  without delivering. Meaningfully more machinery.

The atomicity argument makes the first option better *if* the queue must be shared, which it must.
Worth also re-examining `character_premium_items` at that point: it already exists with nearly the
right shape, but its Java consumer (the in-client premium-item window) is unported in Rust, so
reusing it means porting that UI flow too.

**Adding argon2 (§5.2) should ride along with whichever storage decision is made** — once one new
table is acceptable, a second for `web_credentials` is nearly free, and it retires the most
significant security compromise in this design.

---

## 8. Frontend

### 8.1 Stack

| Concern | Choice | Why |
| --- | --- | --- |
| Runtime, package manager, bundler, test runner | **Bun** — `bun install`, `bun build`, `bun test`, `bun --hot` dev server | One binary for the entire JS toolchain. No Node, no Vite, no separate bundler config. `bun.lock` is committed |
| Framework | **React 19 + TypeScript** | As requested. Bun transpiles TSX natively — no `tsc` in the build path (still run `tsc --noEmit` in CI for type checking) |
| Routing | **React Router** (data router) or **TanStack Router** | TanStack Router if fully-typed routes/search params are wanted; React Router if familiarity matters more. Either is fine — pick one and don't mix |
| Server state | **TanStack Query** | Caching, retries, invalidation for every API call. Do not hand-roll fetch state |
| Forms | **react-hook-form + zod** | Registration/login validation; zod schemas shared with the generated API types |
| Styling | **Tailwind CSS** (via `bun-plugin-tailwind`) + **shadcn/ui** | Copy-in components, no runtime dep, easy to theme dark/fantasy for an L2 server. See §8.2 for the shadcn caveat under Bun |
| API client | **openapi-typescript** (+ `openapi-fetch`) generated from the utoipa spec | Types stay in sync with the Rust DTOs automatically; a schema drift becomes a compile error |
| Unit tests | **`bun test`** + **Testing Library** (with `happy-dom`) | Bun's runner is Jest/Vitest-API-compatible, needs no extra dependency, and starts fast |
| E2E tests | **Playwright** for the register→login→character-list happy path | Caveat: Playwright's own runner still drives a Node process for the browser harness. Install it with Bun and invoke via `bun x playwright`; this is the one place Node appears, confined to CI/E2E — not the app or its build |
| Lint/format | **ESLint** + **Prettier**, invoked via `bun x` | Both run fine under Bun. If a single faster tool is preferred later, **Biome** replaces both — an independent decision, not required by Bun |

### 8.2 Deliberate non-choices

**No Vite.** `bun build` handles this app's entire build: HTML entrypoints, TSX transpilation,
CSS, asset hashing, code splitting, minification, and sourcemaps, with `bun --hot` providing the
dev server and hot reload. For a login-gated SPA plus a few marketing pages there is nothing Vite
does here that Bun does not, and dropping it removes a config file, a dependency tree, and a
second tool from CI.

Three caveats to go in with eyes open — none blocking, all worth knowing before D1:

1. **shadcn/ui's CLI assumes Vite or Next.** Its `init` writes framework-specific path aliases and
   config. The components themselves are plain copy-in React + Tailwind and work fine; expect to
   set up `components.json`/`tsconfig` path aliases by hand once, or copy components in directly
   and skip the CLI. One-time cost, not ongoing.
2. **Tailwind goes through `bun-plugin-tailwind`** rather than Tailwind's Vite plugin. Supported
   and documented, but it is a `bunfig.toml` plugin entry rather than the path most Tailwind docs
   assume.
3. **The plugin ecosystem is smaller.** If a future need turns up that only exists as a Vite
   plugin, the escape hatch is cheap: Bun runs Vite fine (`bun x vite`), so switching back is a
   day's work, not an architecture change.

**No Next.js.** Next.js would add an SSR JavaScript runtime to deploy and monitor for very little
gain: this app is behind a login and has nothing to SEO except the landing page. A static SPA
served by the Rust binary is one process instead of two.

The landing page *does* benefit from SEO. Two options:

1. **Recommended for v1** — keep the landing page in the same SPA and accept CSR with proper
   `<meta>`/OpenGraph tags in the HTML entrypoint. Note that dropping Vite also drops the
   off-the-shelf pre-render plugins, so if real static HTML for marketing routes becomes a
   requirement, the cheap answer is a small `bun run prerender` script that renders those few
   routes with `renderToString` at build time — a dozen lines, not a framework.
2. Keep marketing separate on the existing static site and mount the dashboard at `/account`.

Note: `/Users/artem/dev/l2/interlude_web` is an existing static Next.js/Chakra marketing site
(index/about/downloads, no API, no auth, last touched mid-2024). It is **not** a starting point
for this work — but its copy and assets are worth lifting into whichever option is chosen.

---

## 9. Repo layout and serving

```
l2r_interlude/
  crates/
    commons/          # reused: crypt::hash_password, db::init, config
    loginserver/
    gameserver/
    launcher/
    dashboard_api/    # NEW
      src/
        main.rs
        config.rs        # .ini via commons PropertiesParser, env override
        state.rs         # AppState { pool, config, mailer, session_key }
        error.rs         # ApiError → IntoResponse
        auth/            # cookie signing, stateless tokens, password verify
        routes/          # one module per resource group
        db/
          accounts.rs    # the only module with writes
          characters.rs  # read-only projections
        mail/
                         # note: no migrations/ — v1 adds no tables
  web/
    dashboard/        # NEW — React SPA, Bun for everything
      src/
      index.html      # bun build entrypoint
      package.json    # scripts run with `bun run <script>`
      bunfig.toml     # bun-plugin-tailwind registration
      bun.lock        # committed; CI installs with `bun install --frozen-lockfile`
      dist/           # `bun build` output, embedded into the binary
```

Crate name `dashboard_api` (underscored) as requested — the squashed names of the existing crates
are echoes of the Java package names and there is no reason to extend that convention here.

**Deployment has since moved cross-origin.** The SPA is served from
`https://battlecrab.com` and the API from `https://api.battlecrab.com`, so the
same-origin assumption below no longer holds in production. Consequences, all
implemented:

- CORS restricted to `battlecrab.com` and its subdomains over HTTPS
  (`AllowedOrigins`, implemented in `cors::OriginPolicy`). A wildcard is
  impossible once credentials are involved, and the match is per DNS label so
  lookalikes like `evilbattlecrab.com` are refused.
- `SiteBaseUrl` is separate from `PublicBaseUrl`, because reset and verification
  links are *frontend* routes.
- The SPA's API base is substituted at build time (`API_BASE_URL`), defaulting
  to the production subdomain.
- The session cookie stays `SameSite=Lax`: the two hosts are cross-origin but
  same-*site*, so it is still sent. A different registrable domain would require
  `SameSite=None`.

The embedded-SPA path below still works and is still how `cargo run` serves the
app locally; it is simply not the production topology any more.

**Serving**: embed `web/dashboard/dist` into the binary with `rust-embed` and serve it from axum
with an SPA fallback (unknown paths → `index.html`). One binary, one port, no CORS, no separate
static host, cookies trivially same-origin. In development, run `bun --hot` (Bun's dev server) with
a proxy to the API port for hot reload.

**Build order matters**: a *release* build embeds `web/dashboard/dist` at compile time, so the SPA
must be built first to end up in the binary. That ordering is an explicit step in CI/Docker, not a
`build.rs` that shells out to Bun — invoking a JS toolchain from `build.rs` would make `cargo build`
fail on a machine without Bun, breaking the other crates' developer experience for no benefit.

There is one thing `build.rs` **must** do, though, and its absence was a real bug: `dist/` is
gitignored, and `#[derive(RustEmbed)]` treats a *missing* folder as a compile error — so a fresh
checkout of `main` could not build `dashboard_api` at all, and therefore could not build the
workspace. `crates/dashboard_api/build.rs` now creates the directory, and only that (it never runs
Bun), so:

- A Rust-only checkout compiles and runs; `web::serve_spa` answers with a "frontend not built"
  message and the API works normally.
- `cargo:warning` points at `bun run build` when the directory is empty.
- Debug builds read `dist/` from the filesystem at runtime, so building the frontend afterwards
  needs no recompile.

Verified end to end: with `dist/` absent the workspace builds and the API serves; after
`bun run build` the debug server serves the real SPA with no rebuild; and a release binary still
serves it with `dist/` moved away — which is what proves the assets are genuinely embedded.

**Docker**: multi-stage — an `oven/bun` stage runs `bun install --frozen-lockfile && bun run build`,
then the Rust builder stage copies `dist/` in before `cargo build --release`. The final image
carries no JS toolchain at all; it is the single Rust binary with the assets embedded.

**Deployment note**: because it opens the same SQLite file, `dashboard_api` must run on the **same
host/volume** as the game and login servers. SQLite over a network filesystem is not safe. This is
the main operational constraint the direct-DB-access approach imposes — worth confirming against
the existing helm/Docker topology (`/Users/artem/dev/l2/interlude_helm`) before D1.

---

## 10. Configuration

Follow the existing pattern: `dist/game/config/Dashboard.ini`, read by
`commons::config::PropertiesParser`, which already supports env-var override — that is how secrets
get injected in Docker/helm without committing them.

**The override prefix comes from the config file's path, not the crate name**, so it is
`DIST_GAME_CONFIG_DASHBOARD_<KEY>` (e.g. `DIST_GAME_CONFIG_DASHBOARD_SESSIONSECRET`). Moving the
ini file renames every variable.

Keys: `BindAddress`, `Port`, `PublicBaseUrl`, `URL`, `MaximumDatabaseConnections`, `SessionSecret`,
`SessionTtlDays`, `RegistrationEnabled`, `MinPasswordLength`, `MaxPasswordLength`,
`MaxLoginLength`, `LoginRateLimit`, `LoginRateWindowSecs`, and (once D3 lands)
`SmtpHost/User/Password/From`.

`URL` and `MaximumDatabaseConnections` deliberately reuse `LoginServer.ini`'s key names so the
value can be copied across verbatim. It must point at the *same* `interlude_classic.db` the
login/game servers use.

**This is the setting that actually goes wrong.** `URL` is relative to the working directory and
the database is gitignored, so it exists in the main checkout but not in a fresh git worktree —
and `commons::db::init` opens with `create_if_missing(true)`. A wrong path therefore did not fail:
it created an empty database, and every request 500'd with `no such table: characters` forever.
That happened in practice.

Two startup checks now make it loud instead (`db::sqlite_path`, `db::missing_tables`), both naming
the absolute path that was opened:

1. **File missing** → refuse to start; `dashboard_api` never creates a database.
2. **File present but lacking `accounts`/`characters`** → refuse to start, and say it is probably
   an empty file from an earlier run with the wrong working directory.

The server also logs the resolved absolute path at boot, not the raw URL.

**Secrets are environment-only.** `Dashboard.ini` is committed, so it holds no secret at all:
`DASHBOARD_SESSION_SECRET` is read solely from the environment (its own variable, not the
path-derived override), and the server refuses to boot if it is unset or under 32 characters. A
`SessionSecret` key found in the ini is ignored and logged as an error. The same rule applies to
the SMTP password when D3 lands.
`SessionSecret` must be stable across restarts — a generated-per-boot key logs every user out on
every deploy.

---

## 11. Suggested slices

Each slice should end compiling, tested, and independently deployable.

| Slice | Content |
| --- | --- |
| **D1** | Crate skeleton: axum server, config, DB pool, health endpoint, error envelope, tracing. Bun SPA skeleton (`bun build` + `bun --hot`), embedded serving, and the CI build order from §9. |
| **D2** | Registration + login + logout + `/auth/me`. Signed cookies, rate limiting. **Verify end-to-end that a web-registered account logs into the real game client** — this is the acceptance test for the whole design, and it should be done on day one of D2, not at the end. |
| **D3** | Email: change-email with verification, forgot/reset password, change password. Stateless tokens (§5.4). |
| **D4** | Character list + server status. Read-only projections. |
| **D5** | Landing page content + styling pass. |

D1–D4 is a complete, useful product: register, manage, and see your characters. The shop (§7) is a
separate future effort gated on a storage decision.

---

## 12. Open questions

1. **Is single-hash auth acceptable for launch?** (§5.2) It is the direct consequence of the
   two-table constraint. If not, the smallest fix is one `web_credentials` table with argon2 — which
   is worth deciding *before* D2 rather than migrating users afterwards.
2. **Session revocation.** Signed cookies cannot be individually revoked (§5.3). If instant
   ban-kick from the web matters, that also needs a table (or a very short cookie lifetime plus a
   revocation check against something already in `accounts`).
3. **Live server status**: reading `characters.online` gives a stale-ish count and says nothing
   about whether the process is actually up. A tiny internal HTTP endpoint on the game server
   would be accurate. Worth it, or is the DB good enough for v1?
4. **Multi-server**: schema has `accounts.lastServer` and the login server supports several game
   servers. Does the dashboard need to be server-aware (character list per server), or is there
   exactly one game server for the foreseeable future?
5. **Username/password rules**: the game client imposes its own limits (`VARCHAR(45)`, and the
   client's login box has a practical max). Registration validation must match the client's
   accepted charset/length or players will create accounts they cannot type into the client.
   Needs verification against the client before D2 ships.
6. **Read-heavy features later** (rankings, statistics, who's-online lists): querying the live
   SQLite file will contend with the game server. Snapshot/replica strategy needed before that
   lands — worth deciding early if rankings are wanted soon.
7. **GDPR-ish concerns**: storing emails implies a deletion path. Trivial to add now, annoying to
   retrofit.

---

## 13. Summary of recommendations

- New workspace crate **`crates/dashboard_api`**: axum + sqlx, reusing `commons` for hashing, DB,
  and config. **One SQLite pool against the live game DB.**
- **Two tables only**: `accounts` (write `password`/`email` only) and `characters` (read-only).
  No new tables, no migrations, no schema changes anywhere.
- **One password hash** — the game's SHA-1/Base64, used for web login too. A knowing compromise
  (§5.2) that removes the need for a dual-hash and an account-claim flow; it makes rate limiting
  load-bearing rather than optional.
- **Stateless auth**: signed cookies for sessions, HMAC tokens for reset and email verification —
  the reset token is bound to the current password hash, so it is single-use for free.
- New **`web/dashboard`**: React 19 + TypeScript with **Bun for the entire JS toolchain** —
  `bun install`, `bun build`, `bun test`, `bun --hot`. No Vite, no Node; embedded into the Rust
  binary and served same-origin.
- **The coin shop is deferred** (§7) — it needs a wallet, a queue, and idempotency, so it is the
  moment to revisit both the single-DB decision and argon2, together.
- Ship **D1–D5**; treat "a web-registered account logs into the real client" as D2's first task.

---

## 14. Implementation status (D1–D2)

Built on `feat/dashboard`. This section is the honest diff between the plan above and the code, so
the next person doesn't trust a spec the implementation has already moved past.

### Working end to end

- `crates/dashboard_api` — axum 0.8, one sqlx SQLite pool, `accounts` (narrow writes) +
  `characters` (read-only projection), signed-cookie sessions, HMAC reset/verify tokens, per-IP and
  per-account login throttling, embedded SPA with client-route fallback.
- `web/dashboard` — React 19 + TS on Bun. Landing, login, register, and account pages; glass
  surfaces, blue/yellow palette, light/dark toggle, staggered entrance animations.
- **Rust tests**: unit + HTTP-level integration against a real SQLite schema (25 integration cases
  after the §15 rework, including the game-account-cannot-sign-in guard).
- Verified against a running server: registering `Smoke`/`correct-horse` stored
  `NstYn3QVe0WBGmkMWLQ0CV9I6fo=`, which equals an independently computed
  `base64(sha1("correct-horse"))`. **The D2 acceptance criterion — a web-created account is
  byte-identical to one the game client can use — holds.**

### Deviations from the plan above

| Plan said | Built instead | Why |
| --- | --- | --- |
| Env override prefix `CONFIG_DASHBOARD_<KEY>` | **`DIST_GAME_CONFIG_DASHBOARD_<KEY>`** | `PropertiesParser` derives the prefix from the config file's *path*, not a fixed string. The plan was simply wrong; moving the ini file changes the variable names. |
| Config key `DatabaseUrl` | **`URL`** + `MaximumDatabaseConnections` | Matches `LoginServer.ini` verbatim, so the value can be copied across without translation. |
| `tower_governor` for rate limiting | Small in-process `RateLimiter` | governor keys on IP; we need per-IP **and** per-account. ~50 lines with tests beat bending a dep. Caveat: per-instance, so it assumes the single-instance deployment §9 already requires. |
| `validator` derive | Hand-written `validate_login` / `validate_password` / `validate_email` | The rules are client-compatibility rules (ASCII-only, length caps), not generic ones, and each needs a comment explaining the *game* constraint behind it. |
| shadcn/ui + react-hook-form + zod | Hand-built `Panel`/`Button`/`Field`/`Alert`, plain controlled inputs | Four forms did not justify three dependencies, and shadcn's CLI assumes Vite/Next (§8.2 caveat 1). Revisit if the surface grows. |
| CSRF as a note | Implemented as middleware (`csrf.rs`) | The API client sends `X-Requested-With`; the server now *requires* it on non-GET. Covered by a test that mimics a cross-site form post. |

### Not yet built

- ~~**Email sending.**~~ **Done.** `crates/dashboard_api/src/mail.rs` sends both messages over SMTP
  (Amazon SES) with `lettre` + rustls — rustls specifically, because the deploy cross-compiles with
  `cargo-zigbuild` and an OpenSSL dependency would need a cross-built C toolchain.

  Email is **disabled unless SmtpHost + username + password are all set**, in which case links are
  logged instead of sent; that keeps local development working without an SES account, and the
  server warns loudly at boot. Credentials are environment-only (`DASHBOARD_SMTP_USERNAME` /
  `DASHBOARD_SMTP_PASSWORD`) for the same reason as the session key.

  Two deliberate asymmetries in error handling: `change-email` returns 500 when delivery fails (the
  caller is authenticated and chose the address, so telling them beats claiming "check your inbox"),
  while `forgot-password` still returns 202 and only logs — its response must not reveal whether an
  address exists. Both verified against real SES.

  STARTTLS vs implicit TLS is derived from the port (465/2465 wrapped, 25/587/2587 upgraded) rather
  than configured, since choosing wrong either hangs or fails the handshake.
- **`utoipa` / OpenAPI + generated TS client.** `web/dashboard/src/lib/api.ts` is hand-written and
  its types are maintained by hand against the Rust DTOs. That drift risk is real and is exactly
  what the generator was meant to remove.
- **Frontend tests are thin.** `web/dashboard/tests/header-responsive.test.ts` (`bun test`) drives
  real Chrome and asserts the header does not overflow at 320–480px — added after the theme toggle
  was found escaping the header at 360px, and verified to fail on the pre-fix markup. Nothing else
  covers the UI; there is no test of the register/login *flows* through the browser yet.
  Requires a built `dist/` and Google Chrome; skips with a message when either is absent.
- ~~**`/auth/reset-password` and `/account/email/verify` have no UI routes.**~~ **Done.** The SPA
  now has `/forgot-password`, `/reset-password` and `/verify-email`, plus the "Forgot your
  password?" link on the login form — without which the flow was unreachable, since nothing in the
  UI could trigger a reset email at all. All three are public and deliberately outside
  `RedirectIfAuthed`: the links are opened from an inbox, sometimes in another browser, and
  bouncing a signed-in user to /account would swallow the token.
- **Class names.** The character list shows level and race; `classId` is returned but not resolved
  to a name, which needs the datapack's class table.
- Landing page "Download client" button is disabled pending a real launcher URL.
- **`og:image` is not set.** Link previews show no artwork. The logo is content-hashed at build
  time, so wiring it into a static `<meta>` needs either a build step that rewrites the tag or a
  stable unhashed copy — decide which before the site is shared anywhere social.

### Branding assets

`web/dashboard/assets/` holds what the frontend ships:

| File | Use | Notes |
| --- | --- | --- |
| `logo.webp` (761w, 141 KB) + `logo-420.webp` (59 KB) | Landing hero, via `srcset` | Converted from `dist/images/logo2.png` (1.0 MB PNG) at cwebp `-q 75`, trimmed to its solid alpha bounds. q75 is visually indistinguishable from the source at display size. The source PNG has a real alpha channel, so it sits on both themes without a plate. |
| `favicon.svg` (2.1 KB) | Favicon **and** the header brand mark | An "L2R" wordmark in the logo's gold-on-blue. |
| `apple-touch-icon.png` (180², 6 KB) | iOS home screen | Rendered from the same SVG, squared and opaque — iOS applies its own mask, and transparent corners render black. |

**Why the header mark is not the crab medallion.** It was measured: cropped out of the artwork and
drawn at 36px the medallion is unreadable mush, and a favicon renders at 16px. The flat `L2R` mark
keeps the logo's palette and lettering at sizes where the full artwork cannot survive; the artwork
itself gets the hero, where it has room. The header `<img>` and the favicon reference the *same*
file, so the tab icon and the brand mark cannot drift apart.

Its letterforms are geometry rather than `<text>`, so rendering never depends on the viewer's
fonts. Bun content-hashes all of these at build time, which is what makes the immutable
`Cache-Control` in `crates/dashboard_api/src/web.rs` safe for them.

### Operational notes

- **SMTP is environment-only — all of it**, not just the credentials:
  `DASHBOARD_SMTP_HOST`, `_PORT`, `_FROM`, `_USERNAME`, `_PASSWORD`. There are
  no `Smtp*` keys in `Dashboard.ini`; the server logs an error naming any it
  finds, because they are ignored. The host/port/from moved out of the ini
  after the deploy script's `sed "s|^SmtpHost.*|...|"` silently no-oped against
  a remote ini that predated those keys — sed exits 0 when it matches nothing,
  so the deploy reported success while mail stayed disabled. A key that does
  not exist cannot be missed.
- Values in the systemd `EnvironmentFile` must be **single-quoted**: systemd
  treats an unquoted `#` as a comment, which truncates any secret containing
  one. The failure is nasty — the value stays non-empty, so email reports as
  *enabled* and then fails authentication.

- `SessionSecret` is empty in the committed `Dashboard.ini` and the server **refuses to start**
  without it — deliberate, since a generated-per-boot key silently logs everyone out on each deploy.
- Cookies are marked `Secure` only when `PublicBaseUrl` is `https://`; the server warns loudly at
  boot when it isn't.
- `dist/` must be built (`bun run build`) before `cargo build --release`, because `rust-embed` reads
  it at compile time. In debug it reads from disk, so `cargo run` works without a built frontend.

---

## 15. Master accounts — email identity (2026-07-21)

Supersedes the parts of §5 that treat the game login name as the dashboard identity.

### 15.1 The problem

The original design gave each player exactly one row in `accounts`, used as both the game login and
the website login. That forecloses the thing players actually want: several game accounts (to park
characters, to mule, to play alongside a friend on one household) under one person. It also makes
the website login name a game-client constraint — ASCII alphanumerics, 45 chars — for no reason the
user can see.

### 15.2 The model

One table, two kinds of row, told apart by `login`:

| | `login` | `email` | `is_verified` |
|---|---|---|---|
| **Master account** (dashboard identity) | `NULL` | the address, unique among masters | `0` or `1` |
| **Game account** (typed into the client) | the login name | copy of its master's address | `NULL` |

The address is the link between them. A master account **cannot log into the game**: every
login-server query is `WHERE login = ?`, and no NULL row matches that — so the model needs no new
guard on the game side, which is why `login IS NULL` was chosen over, say, an `is_master` flag.

`is_verified` is deliberately three-valued. `NULL` means "not an identity, question doesn't apply",
which is what makes a game account's row self-describing rather than only meaningful in contrast to
its master.

### 15.3 Schema

`login` becomes nullable, so it can no longer be the primary key. It is a `UNIQUE` index instead;
the table has no primary key, which is fine because every query keys on `login` or `email`.

Master-account uniqueness is a *partial* constraint — unique on `email` **where `login IS NULL`**,
since game accounts deliberately share their master's address. sqlite and postgresql express this
as a partial unique index (`accounts_master_email`). **MariaDB cannot**, and enforces it only in
the application; the note in `dist/db_installer/sql/mariadb/login/accounts.sql` records this. The
dashboard runs on SQLite today, so nothing depends on the MariaDB gap right now — but a future
MariaDB deployment would need a `BEFORE INSERT` trigger or a generated-column index.

### 15.4 Sessions and tokens

The cookie and one-time tokens are unchanged in construction (§5.3, §5.4); only the **subject**
changed, from the login name to the address. `Account::subject()` is the single place that decides
this, so nothing else has to know.

`current_account` resolves the cookie through `find_master_by_email`, whose `login IS NULL`
predicate is load-bearing: without it a game account sharing the address would satisfy the lookup,
and a leaked sub-account password would open the owner's dashboard. The test
`a_game_account_cannot_sign_into_the_dashboard` fails if that predicate is removed.

### 15.5 Verification

Registration now stores the address immediately — it is the login — so §5.4's "a stored address is
verified by construction" no longer holds, and `is_verified` records proof explicitly.

An unverified account **can still sign in**, carrying `isVerified: false` so the SPA can nag.
Blocking login would strand any user whose verification mail bounced, with no authenticated way to
ask for another; `/auth/resend-verification` is that path, and it requires a session.

One handler serves both links, distinguished by whether the token's payload differs from its
subject: `(email, email)` confirms, `(old, new)` moves the account. A move rewrites the master row
*and every game account under it* in one transaction — they are joined by the address itself, so a
partial update would orphan the lot.

### 15.6 Not yet built

Creating game accounts from the dashboard. The schema, `accounts::create_game_account`, and the
`GET /account/game-accounts` listing are in place; the create endpoint and its UI are not. When
added, it must validate the login with `validate_login` (the game-client rules still apply to
*that* name) and hash with the same `commons::crypt::hash_password`.
