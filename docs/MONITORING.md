# Per-server monitoring and log search — technical design

Status: **P1 shipped** (wire-traffic counters on both servers). P2–P5 below are planned, not
built. Code comments cite this document's section numbers, so the numbering is stable — when a
section changes shape, bump its number's suffix rather than renumbering everything below it.

Scope: a monitoring section in the admin dashboard (`docs/DASHBOARD.md` §16), per server
(login/game): inbound/outbound packets, connected clients, memory/CPU pressure, plus log search
over the existing NDJSON files. Retention: 7 days at 5s granularity for metrics.

---

## 1. The shaping decision: pull, with a ring buffer

Each server samples **itself** every 5s into an in-memory ring; `dashboard_api` polls each server
over a loopback channel (mirroring `loginserver::status_channel`, DASHBOARD.md §16.6's "planned
game-server control channel") and is the **sole writer** to a dedicated `metrics.db`.

| Alternative | Why not |
|---|---|
| Servers push to dashboard | Puts retry/backpressure logic in the game loop; a dashboard restart becomes the game server's problem. |
| Dashboard reads counters directly every 5s | Counters are cumulative — the sample interval must be exact, and CPU/RSS deltas need consecutive in-process readings, not two independent processes racing a clock. |
| Servers write `log/metrics/*.ndjson`, dashboard scans them | Matches `commons::audit`'s precedent, but every chart request becomes a full scan; downsampling is `GROUP BY`, which is SQL's job, not a log scan's. |
| Samples land in the game DB | `commons::audit`'s argument verbatim (see its module docs): three processes share one SQLite file, and 7-day pruning wants `VACUUM`, which takes an exclusive lock on the whole database. |

Sampling in-process buys three things: the interval is exact, a bounded ring survives a dashboard
deploy without losing history, and **counter resets need no handling anywhere** — the sampler's
`prev` dies with the process, so the first sample after a restart is naturally "counts since
start".

## 2. P1 (shipped): the counters themselves

Both servers now register the same six `commons::metrics` series (`crates/commons/src/metrics.rs`
— counters/gauges, snapshotted to one JSON log line per `MetricsIntervalSeconds`, same mechanism
`docs/LOGGING.md` §"Metrics" already documents):

| Series | Kind | Meaning |
|---|---|---|
| `packets_in` | counter | Frames read off the socket, counted before decrypt/rate-limit — "what arrived on the wire". |
| `bytes_in` | counter | Wire bytes for those frames, header included. |
| `packets_out` | counter | Frames written to the socket. Game server counts per *coalesced batch write* (one add per `write_all`, not per broadcast recipient — see `network/connection.rs`'s outbound arm); login server counts per `send()` call, since it does not batch. |
| `bytes_out` | counter | Wire bytes for those writes, header included. |
| `connections_accepted` | counter | Lifetime total of accepted sockets. |
| `connections_open` | gauge | Live count, `+1`/`-1` via the new `Gauge::incr`/`decr` (atomic `fetch_add`/saturating `fetch_update`, not load-then-`set` — the latter loses updates under concurrent connects/disconnects). |

`packets_in` vs. `game_loop::net::packets_handled` (game server only) is itself a signal: the gap
between them is packets rejected by the rate limiter or lost to a decode failure.

Instrumented at the call sites (`gameserver/src/network/connection.rs`,
`loginserver/src/network/client_connection.rs`), not inside `commons::network::framing` — those
helpers are shared with `gs_link` (the login↔game registration channel), and counting inside them
would conflate that internal link with client traffic. The login server's `gs_link` traffic is
deliberately **not** counted here for the same reason: this section is about players, not
inter-server plumbing.

Registered at boot (`register_metrics()` in each server's network module, called from `main.rs`)
so every series reads `0` from the first snapshot instead of being *absent* until first traffic —
the same reasoning `game_loop::net::register_metrics` already documented for `packets_handled`.

## 3. Storage (planned — P3)

Separate file, `metrics.db`, owned by the dashboard. It is **derived data** — losing it costs 7
days of graphs and nothing else — which justifies lighter treatment than the game DB gets.

```sql
CREATE TABLE metric_sample (
  service      TEXT    NOT NULL,   -- 'game_server' | 'login_server' (matches logging's `service`)
  ts           INTEGER NOT NULL,   -- epoch ms, floor-aligned to 5s
  interval_ms  INTEGER NOT NULL,   -- real elapsed; keeps rates honest across a gap
  packets_in   INTEGER NOT NULL,   -- deltas, not cumulative
  packets_out  INTEGER NOT NULL,
  bytes_in     INTEGER NOT NULL,
  bytes_out    INTEGER NOT NULL,
  connections_accepted INTEGER NOT NULL,
  connections_open     INTEGER NOT NULL,  -- gauge, instantaneous
  cpu_micros   INTEGER NOT NULL,   -- process CPU consumed during the interval
  rss_bytes    INTEGER,            -- NULL where unavailable (macOS dev — see §4)
  heap_bytes   INTEGER,            -- mimalloc, game only
  tick_busy_micros_total INTEGER,  -- game only
  tick_overruns          INTEGER,  -- game only
  extra        TEXT,               -- JSON: registry metrics with no column yet
  PRIMARY KEY (service, ts)
) WITHOUT ROWID;
```

Wide rows, not `(name, value)` tall: 7d × 17280 samples/service ≈ 30 MB for two services; tall
would be ~10x the rows and every query would need pivoting. `extra` means a *new* counter is still
recorded without a migration; promoting it to a column is optional and later. `WITHOUT ROWID` +
`PRIMARY KEY (service, ts)` makes the clustered index the query pattern, so there's no second
index to maintain per insert.

Host-level pressure (`/proc/loadavg`, `MemAvailable`, disk free) goes in a separate `host_sample`
table sampled by the dashboard itself — both servers share one host, and putting it in
`metric_sample` would duplicate identical numbers per row.

Schema lives in a small `metrics_db::migrate()` inside `dashboard_api`
(`CREATE TABLE IF NOT EXISTS` + `PRAGMA user_version`), **not** the `migration` crate — that crate
is wired to the game DB and its `dist_parity` test; keeping the metrics DB independently droppable.

Retention: an hourly `DELETE FROM metric_sample WHERE ts < ?`. No `VACUUM`/`auto_vacuum` — steady
row count means freed pages get reused and the file plateaus, and `auto_vacuum` costs on every
commit for no benefit here.

## 4. CPU, memory, and the sampler (planned — P2)

`commons::monitor` (new): a `std::thread` (mirrors `metrics::spawn_reporter`, not tokio) that every
5s snapshots the registry, reads process CPU/RSS, diffs against `prev`, and pushes a `Sample` into
a bounded ring (720 entries = 1 hour — covers a dashboard restart/deploy without a gap).

- **CPU**: `libc::getrusage(RUSAGE_SELF)` → `ru_utime + ru_stime` in microseconds. Portable
  Linux/macOS, no `/proc` parsing, and on Linux already sums all threads.
- **RSS**: `/proc/self/statm` field 2 × page size on Linux; `None` on macOS, stored as SQL `NULL`
  — local dev shows memory as unavailable while prod (Linux) works. Not pulling in `sysinfo` for
  parity; it's a large dependency tree for one number.
- **Game-thread pressure**: `tick_busy_micros` is a gauge overwritten 50×/sample window, so
  sampling it directly catches 1 tick in 50. Needs `tick_busy_micros_total` + `ticks` counters
  (mean = ΔTotal/Δticks) and a `tick_overruns` counter incremented where `TICK_OVERRUN_WARN`
  already fires in `game_loop/mod.rs` — mean hides spikes, overruns catch them.
- **Heap**: mimalloc process stats, game server only.

**Alignment matters**: stamp each sample at `floor(now / 5s)` so both servers land in the same
bucket — otherwise a combined chart interleaves two series 2.5s apart.

The channel: a new loopback port per server (not an extension of `status_channel` — making 7778
request-driven would need a read-timeout dance to stay backward compatible with "connect and
read"). One line in, NDJSON out, close, same security model as `status_channel.rs` (loopback bind
is the control; widening it publishes the same things that channel's doc comment warns about):

```
$ printf 'since 1759000000000\n' | nc 127.0.0.1 7779
{"service":"game_server","ts":1759000005000,"interval_ms":5000,"packets_in":1204,…}
```

## 5. API surface (planned — P3)

```
GET /admin/monitor/services   → [{service, up, lastSampleTs, uptimeSeconds}]
GET /admin/monitor/series?service&from&to&metrics&maxPoints
GET /admin/monitor/host?from&to&maxPoints
```

Server-side downsampling is load-bearing, not an optimization: 7 days at 5s is 120,960 points,
which kills the browser. `series` picks a bucket from `(to - from) / maxPoints` and returns ≤ ~500
points via `GROUP BY ts / bucket_ms` (`SUM` for delta columns, `AVG`/`MAX` for gauges).

Live view = poll `series?from=lastTs` every 5s. No SSE/WebSocket in v1 — data only arrives every
5s anyway, and keeping the API stateless behind the Cloudflare Tunnel is worth more than shaving
that latency.

## 6. Log search (planned — P4)

Admin-only (`require_admin`, DASHBOARD.md §16.2).

**No client-supplied paths, by construction.** The request names a `service` enum
(`game`/`login`/`dashboard`) and a `stream` enum (`diagnostic`/`error`/`audit:<category>`); the
server maps those to a directory + filename prefix from config and enumerates with `read_dir`. A
client string never reaches the filesystem, so traversal is unrepresentable rather than merely
defended against.

The scan:

1. Rotation dates the filenames (`docs/LOGGING.md` §"Where the files are") — filter candidate
   files by that date against the query range before opening anything.
2. Reverse chunked scan, newest first — 256 KB backwards from EOF, split on `\n`, processed in
   reverse.
3. Byte-level prefilter (`memchr`, or the compiled `regex`) on the raw line before `serde_json`
   ever runs — most lines are rejected without touching the parser.
4. Bounds, all enforced: `max_bytes_scanned` (~256 MB), a wall-clock deadline (~3s), `limit ≤ 500`,
   a concurrency semaphore of 2. Hitting a bound returns `truncated: true` plus a resumable cursor,
   never a partial lie.
5. `spawn_blocking` — this is blocking file I/O and must not sit on an axum worker.
6. Each line comes back as a pass-through `serde_json::Value`, not a fixed DTO — span fields are
   the point of the JSON format (`docs/LOGGING.md` §"Correlation spans") and a DTO would drop them.
   `*_error.log` files are plain text, so the line type is `Structured(Value) | Raw(String)`.

`regex` is already a dependency and is linear-time with no backtracking; `RegexBuilder::size_limit`
caps it so it can't become a CPU bomb.

**Audit logs contain chat and IPs.** Every search writes a `gmaudit` record — reading player chat
is exactly the kind of action DASHBOARD.md §16.5 says needs attribution.

FTS5 was considered and rejected for v1: it needs a second writer tailing files, roughly doubles
storage, and rotation already gives the time index that makes the bounded scan fast. It's the
named escape hatch if scans measure slow in practice.

```
GET /admin/logs/streams   → (service, stream) pairs + date coverage
GET /admin/logs/search?service&stream&from&to&q&regex&level&limit&cursor
```

## 7. Configuration (planned)

`Dashboard.ini`: `MetricsDatabaseUrl`, `MonitorTargets`
(`game_server=127.0.0.1:7779,login_server=127.0.0.1:7780`), `MetricsPollSeconds` (5),
`MetricsRetentionDays` (7), `LogSearchRoots`, `LogSearchMaxBytes`, `LogSearchTimeoutMs`,
`LogSearchConcurrency`.

⚠ `MetricsDatabaseUrl` must default to an absolute or datapack-relative path — a relative SQLite
path resolves against the *executable's* directory, not the cwd (the same trap documented for the
dev database in dashboard-api's local-dev notes).

`LoginServer.ini` / game `Network.ini`: `InternalMonitorPort` (0 disables),
`InternalMonitorBindAddress`, `MonitorServiceName`, `MonitorSampleSeconds`, `MonitorRingSamples`.
Both `deploy.sh` and `deploy-dashboard.sh` write config on every deploy, so both need updating when
this lands.

## 8. Phasing

| | Scope | Status |
|---|---|---|
| **P1** | Packet/byte/connection counters on both servers; login server's first metrics at all | **Shipped** |
| **P2** | `commons::monitor`: sampler, ring, loopback channel; wired into both `main.rs` | Planned |
| **P3** | `metrics.db`, poller, pruner, `services`/`series`/`host` endpoints | Planned |
| **P4** | Log search: registry, reverse scanner, endpoints, gmaudit record | Planned |
| **P5** | UI: charts + log viewer | Planned |

## 9. Open questions, recorded rather than decided

1. **Raw retention vs. rollups.** Plan keeps raw 5s samples for the full 7 days (~30 MB). The
   alternative — raw for 24h + 1-minute rollups for the rest of the week — is ~1/12 the storage and
   makes long-range queries trivial, at the cost of losing 5s resolution past a day. Revisit if
   `metrics.db` size or query latency becomes a problem.
2. **Log search assumes co-location.** It reads files on the local disk `dashboard_api` runs on.
   Both deploy scripts target one host today; if that changes, log search needs the monitor channel
   to carry log lines too.
3. **Gaps when the dashboard is down.** The server-side ring (§4) covers deploys; a longer outage
   leaves a hole in the chart — the honest rendering, not an interpolated lie.
4. **Frontend charting.** No chart library in `web/dashboard` today, and `docs/DASHBOARD.md` §8.2
   shows a deliberate habit of minimizing dependencies. Hand-rolled SVG line charts fit that habit
   and dodge the mobile GPU budget (`dashboard-local-dev` notes); a library is the faster path.
   Decide before P5.
