# Logging, audit and metrics

Three different questions, three different mechanisms. Most logging pain comes
from answering all three with one pipeline, so this document starts with the
split and only then gets to configuration.

| Question | Mechanism | Droppable? | Kept |
|---|---|---|---|
| *What happened to this player?* | Diagnostic log — `commons::logging` | **Yes, on purpose** | 14 days |
| *What did this account do, months ago?* | Audit records — `commons::audit` | **Never** | 180 days |
| *How is the server doing right now?* | Metrics — `commons::metrics` | n/a | one line per interval |

## Why diagnostics are allowed to drop

The game thread must never pay a `write(2)` for a log line. Every file layer
writes through `tracing_appender::non_blocking`: the line is formatted on the
calling thread, then handed to a bounded channel drained by a dedicated writer
thread. When that channel fills — which is exactly when the server is busiest —
the line is **dropped** rather than parking the caller.

That is the intended trade. A load spike should cost log fidelity, not tick
budget. The alternative is a server that stutters precisely when it is under
pressure.

Because silent loss is invisible by construction, a reporter thread publishes
the running drop count as a `WARN`. A saturated log and a quiet one must not
look the same.

**Formatting is not moved off-thread by any of this.** The lever for genuinely
hot paths is compile-time: build with `--features tracing/release_max_level_info`
and every `debug!`/`trace!` in the packet path disappears at compile time
instead of being filtered at runtime.

## Why audit records are not allowed to drop

Chat, item movements, enchants, GM commands and login activity are *records*,
not diagnostics. They are low-volume, high-value, and read months later to
answer "this player says his +12 weapon vanished". Routing them through a lossy
sink would discard exactly the evidence a busy server is most likely to be asked
for.

So the audit sink inverts every diagnostic trade: a full queue makes the caller
**wait** rather than dropping, and the writer thread is joined on shutdown.
Audit volume is a rounding error next to diagnostics, so in practice the queue
never fills; if it ever does, the metrics snapshot reports it as
`audit_blocked`.

### Why files rather than the game database

`interlude_classic.db` is opened by the login server, the game server and the
dashboard at once. SQLite allows one writer per *file*, and the connection
string sets `busy_timeout=5000` — so an audit insert contending with a
player-persistence flush does not fail fast, it parks the caller for up to five
seconds. Retention would be worse: pruning rows needs `VACUUM` to return space,
and `VACUUM` takes an exclusive lock on the whole database.

Separate NDJSON files make retention a file deletion instead.

### Records are denormalised on purpose

Each record carries `char_name` / `account` inline rather than an id to join on.
An audit record must read as it did *then*: a later rename, or a deleted
character, must not rewrite history.

## Where the files are

Relative to the datapack root (`dist/game`, `dist/login`):

```
log/
  game_server.json              → symlink to today's dated file
  game_server.2026-08-05.json   the diagnostic log, JSON lines
  game_server_error.log         → symlink; WARN+ only, plain text, never dropped
  audit/
    accounting.ndjson           → symlink to today's dated file
    chat.ndjson
    item.ndjson
    enchant.ndjson
    olympiad.ndjson
    gmaudit.ndjson
    audit.ndjson                (illegal actions — no call site yet)
```

Rotation dates the real filenames, so **tail the symlink, not a dated name** —
that is what the symlinks are for.

The error file is plain text deliberately: when something is broken you want to
read it, not parse it.

## Reading them

The console still prints human-readable, coloured output, so running the server
in a terminal is unchanged. The files are additional.

```bash
# readable one-line view of the diagnostic log
jq -r '"\(.timestamp[11:23]) \(.level) \(.target) — \(.message)"' dist/game/log/game_server.json

# only problems
jq -r 'select(.level=="WARN" or .level=="ERROR") | "\(.level) \(.message)"' dist/game/log/game_server.json

# live, with --unbuffered or jq sits on output in 4 KB chunks
tail -f dist/game/log/game_server.json | jq -r --unbuffered '"\(.level) \(.message)"'

# everything one object did, across the whole session
jq -r 'select(.oid == 268476977)' dist/game/log/game_server.json

# an account's login history
jq -r 'select(.account=="someone") | "\(.ts) \(.event)"' dist/game/log/audit/accounting.ndjson

# every GM command against a player
jq -r 'select(.target=="SomePlayer") | "\(.ts) \(.gm) \(.command)"' dist/game/log/audit/gmaudit.ndjson
```

On the deployed server, the systemd units log to the journal — startup output,
panics, and anything written before the subscriber exists:

```bash
journalctl -u l2-gameserver -f
```

## What each service has

| | diagnostics | panic hook | audit records | metrics | spans |
|---|---|---|---|---|---|
| game server | ✓ | ✓ | 6 categories | ✓ | ✓ per packet |
| login server | ✓ | ✓ | accounting (every auth attempt) | — | — |
| dashboard API | ✓ | ✓ | accounting + gmaudit (account lifecycle) | — | — |
| launcher, migration | plain `fmt()` | — | — | — | — |

The gaps are deliberate, not oversights:

- **Metrics on login and dashboard.** Both are request/response services whose
  load is visible from the outside; the game server is the one with a tick
  budget to protect. Add counters there when there is a question they answer.
- **Spans outside the game server.** The game server's span exists because a
  packet is a unit of work with no other identity. HTTP requests already have
  one, and axum carries it.
- **Launcher and migration** still use a bare `tracing_subscriber::fmt()`. A
  desktop app and a one-shot CLI have no rotation or retention problem to
  solve; giving them a datapack-relative log directory would be worse than
  leaving them printing to stdout.

**The dashboard writes to its own audit directory** (`log/audit-dashboard`),
not the game server's. Both resolve against `dist/game`, so sharing one
directory would put two processes on the same NDJSON files — interleaved
appends, and two independent retention sweeps deleting each other's rotated
files. Any future service that audits needs the same treatment.

## Correlation spans

Packet handling runs inside a `packet` span carrying `client_id`, `oid` and
`opcode`, so every log line emitted while handling a packet inherits them. That
turns "what happened to this player" into one `jq` query instead of
reconstructing it from interleaved lines.

The span fields are `i32`s only, deliberately. This is the game thread and the
span is built per packet — a `char_name` field would mean a `String` clone per
packet. Names live on the audit records, which carry them already; `oid` is the
join key between the two.

## Configuration

### `config/Logging.ini` — the sink

Verbosity, rotation, retention, buffer depth, the audit sink and the metrics
interval. `RUST_LOG` overrides `Level` entirely, which is the intended way to
debug a running server without editing the datapack:

```bash
RUST_LOG=debug cargo run -p gameserver
RUST_LOG=info,gameserver::game_loop::net=debug cargo run -p gameserver
```

`FileFormat = plain` swaps the JSON diagnostic file for human-readable text if
you would rather `less` it than query it. You lose machine querying and log
shipper compatibility.

### `config/General.ini` — which categories record

Ported from the Java server, same key names and same `False` defaults. These are
operator decisions about disk and retention, not features:

| Key | Effect |
|---|---|
| `LogChat` | chat records |
| `LogItems` | item ownership and count changes |
| `LogItemsSmallLog` | narrow to adena and equippable items |
| `LogItemsIdsOnly` + `LogItemsIdsList` | narrow to specific item ids |
| `LogItemEnchants` | item enchant attempts and outcomes |
| `LogSkillEnchants` | skill enchant attempts and outcomes |
| `GMAudit` | every GM command, its target and its arguments |

The small-log and id-list keys are **overrides**, not filters: with either set,
those items are recorded even when `LogItems` is off. That is Java's shape, and
the whole predicate lives in `GeneralConfig::should_log_item` so the call sites
cannot drift apart.

### How item records are collected

Gains have one choke point (`add_inventory_item_tracked`). Losses have ~43, and
every one of them holds a `&mut Inventory` borrow that a `World`-aware call
cannot coexist with. So the removal methods *note* what left on the inventory
itself, and `items::drain_item_audit` turns those notes into records once per
tick, where the config gate, the item names and the owning player are all
reachable — plus once more on disconnect, since after the session is torn down
the per-tick drain would never see them.

The noted amount is what **actually** left, not what was requested: asking to
remove more than the player holds removes only what is there, and a negative
count means "all of it". A record claiming 500 adena moved when only 100 existed
would be worse than no record.

Accounting and olympiad records are always written when the sink is on. Who
logged in, and who won a match, are not debugging aids — Java has no config
switch for them either.

## Metrics

Counters and gauges, emitted as one structured `metrics` log event per interval
(`MetricsIntervalSeconds`). One event rather than one per metric, so a single
line is a complete picture at that instant, and it lands in the JSON log where a
shipper or `jq` can graph it with no endpoint to secure.

```rust
metrics::counter("packets_handled").incr();
metrics::gauge("players_online").set(n);
```

Looking a metric up by name takes the registry lock, so hot paths hold the
handle in a `OnceLock` rather than re-fetching per event — after the first call
it is a relaxed atomic add and nothing else.

Currently registered: `packets_handled`, `players_online`, plus `audit_written`
and `audit_blocked` on every snapshot.

## Deployment

`deploy.sh` writes systemd units with `StandardOutput=journal`. The servers own
their own rotating files, so the unit only catches stdout/stderr. **Do not add a
file redirect there** — that is what produced the unbounded log this replaced.

Rotation is handled in-process rather than by logrotate on purpose: with
`StandardOutput=append:` systemd holds the fd, so logrotate needs `copytruncate`,
which races with in-flight writes and silently loses lines.

## Gaps

- **The `audit` (illegal actions) category has no call site**: no
  `IllegalPlayerActionTask` equivalent is ported. The category is kept so the
  filename and the Java category stay aligned for when it lands.
- **No dashboard UI for metrics.** The counters exist and are emitted; wiring
  them into the React dashboard is not done.
