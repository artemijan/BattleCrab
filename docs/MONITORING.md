# Per-server monitoring and log search — technical design

Status: **P1–P3 shipped** (counters; per-server sampler and channel; `metrics.db`, poller and
admin API). P4 (log search) and P5 (UI) are planned, not built. Code comments cite this document's section numbers, so the numbering is stable — when a
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
| `packets_out` | counter | Frames written to the socket, every frame counted. The game server coalesces a tick's frames into one `write_all` and records the batch with a single atomic add of its frame count (`network/connection.rs`'s outbound arm), plus the KeyPacket written outside that batch; the login server does not batch, so it counts per `send()`. |
| `bytes_out` | counter | Wire bytes for those writes, header included. |
| `connections_accepted` | counter | Lifetime total of accepted sockets. |
| `connections_open` | gauge | Live count. Each connection task owns a `Gauge::hold()` guard: `+1` on accept, `-1` when the guard drops, so a task that panics still gives its slot back. Both sides are atomic (`fetch_add` / saturating `try_update`); a load-then-`set` would lose updates under concurrent connects/disconnects. |

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

## 3. P3 (shipped): storage

A separate file, `metrics.db`, owned by the dashboard (`crates/dashboard_api/src/monitor/store.rs`).
It's **derived data**: losing it costs 7 days of graphs and nothing else, which is why it gets
lighter treatment than the game DB (WAL with `synchronous = NORMAL`, so a crash may drop the last
few samples).

```sql
CREATE TABLE metric_sample (
  service     TEXT    NOT NULL,  -- 'game_server' | 'login_server' (matches logging's `service`)
  ts          INTEGER NOT NULL,  -- epoch ms, floor-aligned to the sample period
  started     INTEGER NOT NULL,  -- the server's sampler start: uptime, restart detection
  interval_ms INTEGER NOT NULL,  -- real elapsed; keeps rates honest across a gap
  cpu_micros, rss_bytes, heap_bytes,
  packets_in, packets_out, bytes_in, bytes_out, connections_accepted, connections_open,
  players_online, packets_handled, packets_dropped,
  tick_busy_micros_total, ticks, tick_overruns       INTEGER,  -- NULL where a service lacks it
  extra       TEXT,              -- JSON: registry series with no column yet
  PRIMARY KEY (service, ts)
) WITHOUT ROWID;
```

The column list is generated from `store::COLUMNS`, which is also the whitelist the query API builds
SQL from and the place each column's bucket aggregation is declared. Changes from the original
plan: `started`, `ticks` (the mean busy time needs Δticks) and three game-loop series
(`players_online`, `packets_handled`, `packets_dropped`). Value columns are nullable because the
login server has no tick, heap or player series.

Wide rows, not `(name, value)` tall: 7d × 17280 samples/service ≈ 30 MB for two services. Tall
rows would be ~10x the row count, and every query would need pivoting. `extra` means a *new*
counter is recorded from day one without a migration, and promoting it to a column later loses no
history. `WITHOUT ROWID` + `PRIMARY KEY (service, ts)` makes the clustered index match the query
pattern, so there's no second index to maintain on each insert. Inserts are `OR IGNORE`, so an
overlapping re-poll is harmless.

Host-level pressure goes in a separate `host_sample` table sampled by the dashboard itself
(`monitor/host.rs`): load average via `getloadavg(3)` (Linux and macOS), `MemTotal`/`MemAvailable`
from `/proc/meminfo` (NULL off Linux), and total/free disk via `statvfs(3)` on the filesystem
holding `metrics.db`. Both servers share one host, so per-service rows would repeat the same
numbers.

The schema lives in `MetricsDb::migrate()` (`CREATE TABLE IF NOT EXISTS` + `PRAGMA user_version`),
**not** the `migration` crate. That crate is wired to the game DB and its `dist_parity` test, and
this file must stay independently droppable.

Retention: an hourly `DELETE … WHERE ts < now - MetricsRetentionDays` on both tables. No
`VACUUM`/`auto_vacuum`: with a steady row count, freed pages get reused and the file plateaus, and
`auto_vacuum` would cost on every commit for no benefit here.

**The poller** (`monitor/mod.rs`) wakes 1.5 s after each period boundary, so the servers' boundary
samples already exist. Each wake takes one host reading and then polls each target in turn with
`since <newest stored ts>`. Resuming from the store rather than from memory means a dashboard
restart backfills from the servers' rings. Each poll has a 3 s timeout and an 8 MB cap. A target
whose lines name a different `service` is refused rather than stored, because that is two ports
swapped in `MonitorTargets`, and storing it would put game traffic on the login charts. A failing
target is logged once, when it starts failing, not on every poll. Nothing here is fatal to the
dashboard: with `MonitorTargets` empty, or `metrics.db` unopenable, the account features still
serve and `/admin/monitor` answers 503.

## 4. P2 (shipped): CPU, memory, and the sampler

`commons::monitor` (`crates/commons/src/monitor/`): a `std::thread` (like
`metrics::spawn_reporter`, not tokio, so a busy runtime can't delay a sample) that every
`SampleSeconds` reads the registry and the process's CPU/RSS, turns them into one interval
`Sample`, and pushes it into a bounded ring (`RingSamples`, default 720 = 1 hour, which covers a
dashboard restart/deploy without a gap).

- **Interval values**: `commons::metrics` now records each series' `Kind` at first registration
  (`metrics::readings()`). A counter's sample value is its delta since the previous sample; a
  gauge's is its reading at sample time. The baseline is taken when the sampler starts, so the
  first sample is a true rate, not a boot-time lump. A counter registered later counts from zero.
- **CPU**: `libc::getrusage(RUSAGE_SELF)` → `ru_utime + ru_stime` in microseconds, diffed per
  interval (`cpu_micros`). Works on Linux and macOS, no `/proc` parsing, and sums all threads.
- **RSS**: `/proc/self/statm` field 2 × page size on Linux. `None` elsewhere, stored as SQL `NULL`,
  so local macOS dev shows memory as unavailable while prod (Linux) reports it. No `sysinfo`: it
  is a large dependency tree for one number.
- **Game-thread pressure**: `tick_busy_micros` is a gauge overwritten every tick, so a 5s sample
  catches 1 tick in 50. The game loop now also keeps `tick_busy_micros_total` and `ticks`
  counters (mean busy = Δtotal / Δticks) and `tick_overruns`, incremented where
  `TICK_OVERRUN_WARN` already fires. The mean hides spikes; the overrun count catches them.
- **Heap**: game server only. `heap_bytes` is mimalloc's committed memory (`mi_process_info`,
  via `libmimalloc-sys`'s `extended` feature), passed to `monitor::spawn` as a `HeapProbe`. The
  login server runs on the system allocator and reports `null`.

**Alignment**: the sampler sleeps to the next period boundary rather than for a fixed period, and
stamps `ts = floor(now / period)`, so both servers land in the same buckets and the phase doesn't
drift. `interval_ms` is the real monotonic elapsed time, so a stall shows up as a longer interval
instead of an inflated rate. If the wall clock steps backwards (NTP) onto a bucket the ring already
holds, that sample is skipped without advancing the baseline; the next one covers both intervals.
P3 keys storage on `(service, ts)`, so a duplicate `ts` must never be emitted.

**The channel**: a new loopback port per server (game `7779`, login `7780`), not an extension of
`status_channel`. Making 7778 request-driven would need a read-timeout dance to stay backward
compatible with "connect and read". One line in, NDJSON out, close. It uses the same security model
as `status_channel.rs`: the loopback bind is the control.

```
$ printf 'since 1759000000000\n' | nc 127.0.0.1 7779
{"cpu_micros":8120,"heap_bytes":48234496,"interval_ms":5000,"metrics":{"bytes_in":…,"packets_in":1204,…},"rss_bytes":…,"service":"game_server","ts":1759000005000}
```

- `since <epoch_ms>` returns every buffered sample with `ts` strictly greater, oldest first, so a
  poller passing back the last `ts` it stored never gets it twice. An empty line returns the
  whole ring.
- Registry series sit under `metrics`, not at the top level, so a counter can never collide with a
  fixed field. The P3 poller maps known names to columns and the rest to `extra`.
- Anything else gets one `{"error":…}` line. Requests are capped at 64 bytes and 2 s, so a silent
  or flooding client can't hold a task or grow a buffer.
- `InternalMonitorPort = 0` disables the sampler too: with nothing able to read the ring, there's no
  point filling it. A bind failure is logged and the server boots anyway.

## 5. P3 (shipped): API surface

Admin-only (`require_admin`), under `/api/v1/admin/monitor` (`routes/monitor.rs`). All three
endpoints answer 503 `unavailable` when monitoring is off, so the SPA can tell "disabled" from
"broken".

```
GET /admin/monitor/services
  → {services: [{service, address, up, lastPollMs, lastSampleTs, startedMs, uptimeSeconds, lastError}],
     pollSeconds, retentionDays}
GET /admin/monitor/series?service&from&to&metrics&maxPoints
GET /admin/monitor/host?from&to&maxPoints
  → {service, from, to, bucketMs, aggregation: {name: sum|max|avg|min},
     ts: [...], samples: [...], intervalMs: [...], series: {name: [...]}}
```

- **Columnar** rather than an array of points: it's what a chart library consumes, and it doesn't
  repeat every key on every point.
- `from` is inclusive and `to` exclusive, both epoch ms. They default to the last hour. The range
  is capped at retention + 1 day. `maxPoints` defaults to 500, with a maximum of 2000.
- `metrics` is a comma list checked against the column whitelist. An unknown name is a 400 that
  lists the valid ones. An unknown `service` is a 404.
- **Aggregation**: delta columns are `sum`med; divide by the bucket's `intervalMs` for a rate (CPU
  % = `cpu_micros / (intervalMs × 10)`). Gauges are `max`, because pressure is about the worst
  moment and an average hides it. For host series, load is `avg`, and available memory and free
  disk are `min`.
- `samples` is each bucket's raw sample count. A bucket with fewer than expected is a gap (server or
  dashboard down), which a chart should show rather than smooth over.
- `uptimeSeconds` comes from the newest sample's `started` and is null while the target is down.

Server-side downsampling is load-bearing, not an optimization: 7 days at 5 s is 120,960 points per
series. The bucket width is `ceil(range / (maxPoints − 1))`, rounded up to whole 5 s periods.
Buckets are epoch-aligned, so a range touches one more bucket than `range / width`, and the `− 1`
is what keeps the count ≤ `maxPoints` (a test sweeps a 7-day range across start offsets).

Live view = poll `series?from=<last bucket ts>` every 5 s. That bucket is still filling, so the
client replaces it rather than appending. There's no SSE/WebSocket in v1: data only arrives every
5 s anyway, and keeping the API stateless behind the Cloudflare Tunnel is worth more than shaving
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

## 7. Configuration

**Shipped in P3**, in `Dashboard.ini`: `MonitorTargets`
(`game_server=127.0.0.1:7779,login_server=127.0.0.1:7780`; empty disables, a malformed entry
disables with an error at boot), `MetricsDatabase` (`metrics.db`), `MetricsPollSeconds` (5) and
`MetricsRetentionDays` (7). The code defaults equal the shipped values. That matters because
`deploy-dashboard.sh` seeds `Dashboard.ini` only when the remote has none, so an existing remote
file never receives the new keys and runs on the defaults.

⚠ `MetricsDatabase` is a plain path, and a relative one resolves against the *executable's*
directory, exactly like the game database's `URL`, not the cwd. In prod that's next to
`interlude_classic.db`; under `cargo run` it's `target/debug/metrics.db`. The absolute path is
logged at boot. Both deploy scripts already exclude `*.db` from their rsyncs.

Planned for P4: `LogSearchRoots`, `LogSearchMaxBytes`, `LogSearchTimeoutMs`,
`LogSearchConcurrency`.

**Shipped in P2**: a new `config/Monitor.ini` in each datapack (`dist/game/`, `dist/login/`), read
by `commons::monitor::MonitorConfig`, following the same pattern as `Logging.ini`. The keys are
`InternalMonitorBindAddress` (127.0.0.1), `InternalMonitorPort` (game 7779, login 7780; 0
disables), `SampleSeconds` (5) and `RingSamples` (720). The service name isn't configurable: it's
the same `game_server`/`login_server` string logging uses, passed by each `main.rs`. `deploy.sh`
rsyncs `dist/{game,login}/` whole, so the new file deploys with no script change.

## 8. Phasing

| | Scope | Status |
|---|---|---|
| **P1** | Packet/byte/connection counters on both servers; login server's first metrics at all | **Shipped** |
| **P2** | `commons::monitor`: sampler, ring, loopback channel; wired into both `main.rs` | **Shipped** |
| **P3** | `metrics.db`, poller, pruner, `services`/`series`/`host` endpoints | **Shipped** |
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
