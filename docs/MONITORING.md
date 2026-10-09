# Per-server monitoring and log search — technical design

Status: **P1–P6 shipped** (counters; per-server sampler and channel; `metrics.db`, poller and
admin API; log search; admin UI; live client list). Code comments cite this document's section
numbers, so the numbering is stable — when a section changes shape, bump its number's suffix
rather than renumbering everything below it.

Scope: a monitoring section in the admin dashboard (`docs/DASHBOARD.md` §16), per server
(login/game): inbound/outbound packets, connected clients, memory/CPU pressure, plus log search
over the existing NDJSON files. Retention: 7 days at 5s granularity for metrics.

---

## 1. The shaping decision: pull, with a ring buffer

Each server samples **itself** every 5s into an in-memory ring; `dashboard_api` polls each server
over an internal channel on loopback or a private network (mirroring `loginserver::status_channel`,
DASHBOARD.md §16.6's "planned game-server control channel") and is the **sole writer** to a
dedicated `metrics.db`.

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
— counters/gauges, `docs/LOGGING.md` §"Metrics"):

| Series | Kind | Meaning |
|---|---|---|
| `packets_in` | counter | Frames read off the socket, counted before decrypt/rate-limit — "what arrived on the wire". |
| `bytes_in` | counter | Wire bytes for those frames, header included. |
| `packets_out` | counter | Frames written to the socket, every frame counted. The game server coalesces a tick's frames into one `write_all` and records the batch with a single atomic add of its frame count (`network/connection.rs`'s outbound arm), plus the KeyPacket written outside that batch; the login server does not batch, so it counts per `send()`. |
| `bytes_out` | counter | Wire bytes for those writes, header included. |
| `connections_accepted` | counter | Lifetime total of accepted sockets. |
| `connections_open` | gauge | Live count. Each connection task owns a `Gauge::hold()` guard: `+1` on accept, `-1` when the guard drops, so a task that panics still gives its slot back. Both sides are atomic (`fetch_add` / saturating `try_update`); a load-then-`set` would lose updates under concurrent connects/disconnects. |

### Connection stages

`connections_open` alone can't tell a player in the world from a client sitting at character
select, or a socket that never finished logging in. Each server also breaks its open connections
down by stage. These are all gauges:

| Series | Server | Meaning |
|---|---|---|
| `sessions_handshaking` | login | Before a successful `RequestAuthLogin`: key exchange, GameGuard, the login form. |
| `sessions_logged_in` | login | Past auth, at the server list. The connection closes once the client leaves for the game server. |
| `sessions_authenticating` | game | `ClientSession::Connecting` + `Authenticated`: protocol / session-key check not finished. |
| `sessions_lobby` | game | `InLobby`: character select, including players who restarted out of the world. |
| `sessions_entering` | game | `Entering`: a character picked, loading into the world. |
| `players_online` | game | `InGame` only. **Before this split it counted every game-server session**, so its history over-reports. |
| `offline_traders` | game | Unattended private stores. In the world, but they hold no connection. |

On the login server a connection holds a `metrics::LoginStage` guard that is swapped on auth, so
the two stages always sum to `connections_open`. On the game server, stage changes also happen on
DB and login-link replies, not only on network events, so `game_loop::net::refresh_session_gauges`
recounts `world.clients` once per tick. They sum to the sessions the game thread knows about.

Both servers also register `audit_written` (records the audit writer put on disk) and
`audit_blocked` (times a caller waited on a full audit queue, `docs/LOGGING.md` §"Why audit records are not allowed to drop"),
charted together as "Audit records". `audit_blocked` should stay at zero; anything else means the
never-drop audit sink is stalling server threads.

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
  sessions_handshaking, sessions_logged_in,          -- login only
  sessions_authenticating, sessions_lobby, sessions_entering,
  players_online, offline_traders, packets_handled, packets_dropped,
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

A store created before a column was promoted gets it on open: `add_missing_columns` compares
`COLUMNS` with `pragma_table_info` and runs `ALTER TABLE … ADD COLUMN` for any that are missing.
This is additive and idempotent, so it doesn't need a `user_version` step. Older rows read NULL in
the new column. If the series was being recorded, its value is still in their `extra`.

Wide rows, not `(name, value)` tall: 7d × 17280 samples/service ≈ 30 MB for two services. Tall
rows would be ~10x the row count, and every query would need pivoting. `extra` means a *new*
counter is recorded from day one without a migration, and promoting it to a column later loses no
history. `WITHOUT ROWID` + `PRIMARY KEY (service, ts)` makes the clustered index match the query
pattern, so there's no second index to maintain on each insert. Inserts are `OR IGNORE`, so an
overlapping re-poll is harmless.

Machine-level metrics (load, available memory, free disk) are **not** collected here. They were,
for a while: the dashboard sampled its own host into a `host_sample` table. That was only right
while the dashboard and both servers shared one machine. Once login and game run on separate
machines, the dashboard's host is the wrong one to show. Per-machine metrics with alerting are what
a host monitor (node_exporter + Prometheus/Grafana, Netdata, the VPS provider's graphs) already
does well. This store keeps to what only the servers can see: their own processes. Schema v2 drops
`host_sample` from existing files.

The schema lives in `MetricsDb::migrate()` (`CREATE TABLE IF NOT EXISTS` + `PRAGMA user_version`),
**not** the `migration` crate. That crate is wired to the game DB, and this file must stay
independently droppable.

Retention: an hourly `DELETE … WHERE ts < now - MetricsRetentionDays`. No
`VACUUM`/`auto_vacuum`: with a steady row count, freed pages get reused and the file plateaus, and
`auto_vacuum` would cost on every commit for no benefit here.

**The poller** (`monitor/mod.rs`) wakes 1.5 s after each period boundary, so the servers' boundary
samples already exist. Each wake polls each target in turn with
`since <newest stored ts>`. Resuming from the store rather than from memory means a dashboard
restart backfills from the servers' rings. Each poll has a 3 s timeout and an 8 MB cap. A target
whose lines name a different `service` is refused rather than stored, because that is two ports
swapped in `MonitorTargets`, and storing it would put game traffic on the login charts. A failing
target is logged once, when it starts failing, not on every poll. Nothing here is fatal to the
dashboard: with `MonitorTargets` empty, or `metrics.db` unopenable, the account features still
serve and `/admin/monitor` answers 503.

## 4. P2 (shipped): CPU, memory, and the sampler

`commons::monitor` (`crates/commons/src/monitor/`): a `std::thread` (not
tokio, so a busy runtime can't delay a sample) that every
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

**The channel**: a new port per server (game `7779`, login `7780`), not an extension of
`status_channel`. Making 7778 request-driven would need a read-timeout dance to stay backward
compatible with "connect and read". One line in, NDJSON out, close. It uses the same security model
as `status_channel.rs`: the bind address is the control, and the network behind it is trusted.

**Bind rule** (`commons::network::internal::check_bind`, applied to this channel and to the login
status channel): the address must be loopback (the default) or private — `10/8`, `172.16/12`,
`192.168/16`, `100.64/10` (Tailscale), `fc00::/7`. A hostname is resolved and every address it
gives must pass. `0.0.0.0`, `::` and public addresses are refused: the channel stays off and the
server logs why, but boots. That is what lets the servers sit on other machines than the dashboard
(bind the machine's private address, point `MonitorTargets` at it) without a typo in an ini ever
exposing an unauthenticated port that can list players' IPs, read the logs and kick.

```
$ printf 'since 1759000000000\n' | nc 127.0.0.1 7779
{"cpu_micros":8120,"heap_bytes":48234496,"interval_ms":5000,"metrics":{"bytes_in":…,"packets_in":1204,…},"rss_bytes":…,"service":"game_server","ts":1759000005000}
```

- `since <epoch_ms>` returns every buffered sample with `ts` strictly greater, oldest first, so a
  poller passing back the last `ts` it stored never gets it twice. An empty line returns the
  whole ring.
- Registry series sit under `metrics`, not at the top level, so a counter can never collide with a
  fixed field. The P3 poller maps known names to columns and the rest to `extra`.
- `clients` and `kick` serve the Audit page (§10); `logs streams` and `logs search <json>` serve log
  search (§6).
- Anything else gets one `{"error":…}` line. Requests are capped at 8 KB (a `logs search` line with
  a 512-byte pattern, JSON-escaped, plus a cursor) and 2 s, so a silent or flooding client can't
  hold a task or grow a buffer.
- `InternalMonitorPort = 0` disables the sampler too: with nothing able to read the ring, there's no
  point filling it. A bind failure is logged and the server boots anyway.

## 5. P3 (shipped): API surface

Admin-only (`require_admin`), under `/api/v1/admin/monitor` (`routes/monitor.rs`). Both
endpoints answer 503 `unavailable` when monitoring is off, so the SPA can tell "disabled" from
"broken".

```
GET /admin/monitor/services
  → {services: [{service, address, up, lastPollMs, lastSampleTs, startedMs, uptimeSeconds, lastError}],
     pollSeconds, retentionDays}
GET /admin/monitor/series?service&from&to&metrics&maxPoints
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
  moment and an average hides it.
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

## 6. P4 (shipped): log search

Admin-only (`require_admin`, DASHBOARD.md §16.2), under `/api/v1/admin/logs` (`routes/logs.rs`).

**Each service searches its own files, on its own machine.** The scanner is
`commons::logsearch`. The game and login servers run it when the dashboard asks over their monitor
channel (§4): `logs streams` answers one `StreamInfo` line per stream, and `logs search <json>`
takes a `SearchRequest` and answers one `Outcome` line, which the dashboard passes on as it came.
The dashboard searches only its own logs, in-process (`dashboard_api::logsearch`). Nothing reads
another process's directory, so the servers can run on other machines, and the mapping from
service to files lives with the service that writes them (each server builds its `Source` from its
own `Logging.ini` at boot), not in the dashboard's config.

A request is validated twice by the same function, `SearchRequest::validate`: by the dashboard,
for a clear 400, and by the server, which does not take the asker's word for it. The dashboard
sends its own budget (`LogSearchMaxBytes`, `LogSearchTimeoutMs`); the server caps it at 1 GB and
30 s, since the scan costs its machine. Its reply timeout is that deadline plus 5 s.

```
GET /admin/logs/streams
  → {streams: [{service, stream, files, oldest, newestEnd}],       only streams that have files
     sources: [{service, up, error}]}       one per MonitorTargets server; a down one lists nothing
GET /admin/logs/search?service&stream&from&to&q&regex&level&limit&cursor
  → {service, stream, from, to, hits: [{file, offset, ts, line}], cursor, stopped, truncated,
     scannedBytes, filesScanned, skippedOversized}
```

**No client-supplied paths, by construction.** `service` must be `dashboard_api` or a
`MonitorTargets` service (else 404). `stream` must parse into a closed enum: `diagnostic`, `error`,
or `audit:<category>` for one of `commons::audit::Category::ALL`. Each service's directories are
resolved once at boot from its datapack's own `Logging.ini` (the dashboard's audit directory is
`dashboard_api::logsearch::DASHBOARD_AUDIT_DIR`, which `main` also uses), and the search enumerates
that directory.
The cursor carries a filename, but it's only ever *compared* against that enumeration, never joined
onto a path, so traversal can't be expressed (tested with `../` and absolute names).

The scan (`logsearch::search`):

1. **Files by date.** Rotation dates the filenames (`<prefix>.<YYYY-MM-DD[-HH]>.<suffix>`, UTC).
   Files whose span lies wholly outside the range are skipped without being opened. The undated
   `<prefix>.<suffix>` is the "latest" symlink and is skipped, unless it's a real file, which is
   what `Rotation = never` writes.
2. **Reverse chunked read**, newest first (`logsearch::reverse`). The scan reads 256 KB backwards
   from EOF (or from the cursor) and reassembles lines across chunk boundaries before matching.
   Every line comes back whole and exactly once, so a pattern can't straddle a boundary; a test
   runs every chunk size from 1 byte up. A line over 1 MB is skipped and counted
   (`skippedOversized`) rather than buffered. Each file is read only up to its length at open,
   because lines appended mid-scan are newer than anything being paged through.
3. **Cheap checks before parsing.** The timestamp (`"timestamp":"` / `"ts":"` / the leading token
   of a plain line) and the level are found by byte search, not by a JSON parse. An escaped `\"`
   can't fake either key. Order per line: time window → level floor → pattern → JSON parse (only
   for lines that will be returned). Once lines are 5 s older than `from` (the slack allows for
   writer reordering), the whole search stops: everything further back is older still.
4. **Bounds**, all enforced: `LogSearchMaxBytes` (256 MB) and `LogSearchTimeoutMs` (3 s) per
   request; `limit` 1..=500 (default 100), and at most 8 MB of matching lines per response
   (`MAX_HIT_BYTES`, stopping like `limit`); `LogSearchConcurrency` (2) via a semaphore on the
   dashboard, covering local and remote searches alike, plus 2 per server. A search over the cap
   gets a 429 rather than a queue; a server that can't answer is a 502 naming it. Hitting the byte or time bound returns
   `truncated: true` and a cursor; hitting `limit` returns `stopped: "limit"` and a cursor. Either
   way the cursor resumes exactly after the last line consumed (tested: no gaps, no repeats,
   including resuming a truncated scan to completion).
5. `spawn_blocking`, holding the permit for the scan's duration.
6. JSON lines come back as pass-through `serde_json::Value` (span fields intact). `_error.log`
   lines come back as raw strings, and so does any JSON-stream line that fails to parse.

`q` is literal and case-insensitive by default (`regex::escape`). With `regex=true` it's taken as
written. `regex` is linear-time with no backtracking, and `size_limit`/`dfa_size_limit` (1 MB) stop
a pattern from costing memory instead. A `level` floor is refused on audit streams, which have no
level. A plain `_error.log` line without a stamp (a panic's continuation) is kept, because it
belongs to the entry above it.

Measured on a 191 MB, 1M-line synthetic day (release build, M-series laptop): a full-day literal
search or a `level=error` search both take ~130 ms (~1.5 GB/s). A last-hour search stops after
8 MB in 6 ms. The 256 MB default budget is therefore a bit over a day of dense logging per request.

**Audit logs contain chat and IPs.** Every search, including each cursor page, writes a `gmaudit`
record (`event: "log_search"`, admin, service, stream, `q`, regex flag, range) *before* the scan,
so a search that is cut short is still on record.

FTS5 was considered and rejected for v1: it needs a second writer tailing files and roughly
doubles storage, while rotation already provides the time index that makes the bounded scan fast.
It remains the escape hatch if scans measure slow in practice.

## 6a. P5 (shipped): the admin UI

Two pages under the dashboard's admin section (`web/dashboard`), reached from a new tab row
(`components/AdminNav.tsx`: Accounts / Monitoring / Audit / Logs, Audit added by §10). The header keeps its single "Admin"
entry.

**`/admin/monitor`** (`pages/Monitoring.tsx`): a card per server (up/down, uptime or last sample,
last poll error), then one tab per server, and 1h/6h/24h/7d ranges.
- **Game server:** packets/s, bandwidth, open connections and players, new connections/s, CPU %
  of one core, RSS and heap, mean tick time, and ticks over 50 ms.
- **Login server:** the same minus the game-only series.

Rates are computed client-side as each `sum` bucket over its own `intervalMs` (§5). Every chart
refetches about once per bucket, never faster than the poll interval.

**Charts are hand-rolled SVG** (`components/LineChart.tsx`, arithmetic in `lib/chart.ts`), the
answer to §9 q4: no chart dependency, consistent with DASHBOARD.md §8.2.
- **Rendering:** drawn at the container's measured pixel width (ResizeObserver) rather than
  through a scaled viewBox, so strokes and labels stay crisp. Static: no transitions or filters,
  within the mobile GPU budget.
- **Axes:** the x axis is the *requested* range, so downtime at the edges shows as empty space.
  The y axis starts at 0, since every series is a rate, count or size, and a floating baseline
  turns noise into cliffs.
- **Gaps:** a line breaks at missing buckets (`null`, or a time jump > 1.5 buckets) instead of
  bridging an outage, and a lone sample renders as a dot.
- **Interaction:** hover or touch snaps a crosshair to the nearest bucket, but only within one
  bucket of the pointer, so a gap reads as a gap.
- **Colour:** a theme-aware `--chart-*` palette, blue and amber first (distinguishable with
  red-green colour blindness).

**`/admin/logs`** (`pages/Logs.tsx`): the form has server, log (stream), time range, minimum
level (disabled for audit streams, which have none) and text or regex. It also states up front
that every search is recorded in the GM audit log.
- **Paging:** results are an infinite list over the API cursor. A page cut short by the scan
  budget shows an amber note and **Keep searching**, never anything that reads as the end of the
  results.
- **Rows:** a JSON line shows its time, level, target and message, with the remaining fields
  inline; it expands to the full pretty-printed JSON. An `_error.log` line shows as raw monospace
  text.
- **No silent refetches:** searches never refetch in the background (each request is an audit
  record).

**Also fixed while building this:** `build.ts` now sets `publicPath: "/"`. Bun's default emits
relative asset URLs (`./index-x.js`), so a hard refresh or direct link to any *nested* route
(`/admin/accounts/:email` already, and now `/admin/monitor`) requested `/admin/index-x.js`. The
SPA fallback returned `index.html` for it, and the page rendered blank. This happened on both the
Rust host and Cloudflare. The new UI tests caught it, since they open nested routes directly.

Tests: `tests/chart.test.ts` (ticks, gap-breaking paths, rates over real intervals, formatting)
and `tests/monitoring.test.ts` (Playwright against the built bundle with a stubbed API). The
Playwright tests cover drawing a data hole as two line runs, keeping game-only charts off the
login tab, the 503 "disabled" message, the search request carrying the chosen filters, "Load
older" sending the cursor back, the truncation note, audit streams never sending `level`, and
the current admin tab, including account detail pages.

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

**Shipped in P4**, also in `Dashboard.ini`: `LogSearchEnabled` (`True`; `False` makes
`/admin/logs` answer 503), `LogSearchMaxBytes` (256 MB), `LogSearchTimeoutMs` (3000) and
`LogSearchConcurrency` (2). P4 first shipped `LogSearchRoots` (`service=datapack_root` pairs the
dashboard read files from); it went when each server started searching its own files, and an old
`Dashboard.ini` that still sets it is ignored.

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
| **P3** | `metrics.db`, poller, pruner, `services`/`series` endpoints (a `host` endpoint shipped too, retired in schema v2, §3) | **Shipped** |
| **P4** | Log search: registry, reverse scanner, endpoints, gmaudit record | **Shipped** |
| **P5** | UI: charts + log viewer | **Shipped** (§6a) |
| **P6** | Live client list: `clients` channel request, endpoint, Audit page | **Shipped** (§10) |

## 9. Open questions, recorded rather than decided

1. **Raw retention vs. rollups.** Plan keeps raw 5s samples for the full 7 days (~30 MB). The
   alternative — raw for 24h + 1-minute rollups for the rest of the week — is ~1/12 the storage and
   makes long-range queries trivial, at the cost of losing 5s resolution past a day. Revisit if
   `metrics.db` size or query latency becomes a problem.
2. **Log search assumes co-location.** *Resolved:* each server searches its own files when asked
   over its monitor channel (§6), so the dashboard reads no other machine's disk.
3. **Gaps when the dashboard is down.** The server-side ring (§4) covers deploys; a longer outage
   leaves a hole in the chart — the honest rendering, not an interpolated lie.
4. **Frontend charting.** *Decided: hand-rolled SVG* (§6a). No chart library in `web/dashboard`,
   in keeping with `docs/DASHBOARD.md` §8.2's habit of minimizing dependencies, and static SVG
   stays inside the mobile GPU budget.
5. **Login and game on separate machines.** *Resolved* for a trusted private network: every
   channel between the dashboard and the servers may bind a private-network address, and
   nothing wider (§4's bind rule). Metrics, the client list, kicks, status and log search then
   all work across machines with configuration alone. Machine-level metrics (load, memory,
   disk) are left to an external host monitor (§3). The deploy scripts take a host per service
   (`LOGIN_HOST`, `GAME_HOST`, `DASHBOARD_HOST`), tell co-located services apart by
   `/etc/machine-id`, and write every address above from that topology (`deploy-lib.sh`). A
   split needs PostgreSQL (`docs/DATABASE.md`), which the scripts check: SQLite is one machine's
   file.

## 10. P6 (shipped): the live client list (Audit page)

Every connection open right now, on both servers, with who it is and how far it got. Unlike §3–§5
nothing is stored: the dashboard asks each server whenever the page asks the dashboard.

**On the channel**, a second request: `clients` returns one `ClientRecord` line per open
connection and closes (`commons::monitor::clients`). The record is camelCase, unlike a sample line,
because the dashboard passes it to the browser unchanged:

```
$ printf 'clients\n' | nc 127.0.0.1 7779
{"service":"game_server","id":12,"ip":"203.0.113.9","port":51234,"connectedMs":…,"stage":"in_game","account":"alice","character":"Hero","hwid":null,"traffic":{"packetsIn":…,"bytesIn":…,"packetsOut":…,"bytesOut":…,"lastPacketMs":…},"details":{…}}
```

A server answers through the `Provider` it installs with `clients::set_provider` (a list function
and a kick function, see "Disconnect and IP bans" below). It's a slot rather than a
`monitor::spawn` argument because the game server starts its monitor before its game loop's
channel exists. A server with no provider gets an error line, and so does a provider that doesn't
answer within 2 s.

- **Game server.** The sessions belong to the game thread, so the provider sends
  `GameEvent::Monitor(MonitorRequest)` into the unified event channel, and the game thread answers in
  the same drain (`game_loop::net::clients`). It costs nothing while nobody is looking, and it
  adds no lock for the game thread to take.
- **Login server.** Each connection task holds a `clients::Registration`, which adds a row to a
  registry and removes it on drop, so a panicking task leaves nothing behind (the `LoginStage`
  reasoning). The session updates the row on auth and on `PlayOk`.

**Per-connection traffic** is `ConnectionStats`: atomics bumped at the same call sites as the
server-wide `packets_in`/`bytes_in`/`packets_out`/`bytes_out` (§2), so the two always agree. It
also records the last inbound frame's time and the connect time. On the game server it lives on
`OutboundTx`, the one per-connection handle every session state already carries. On the login
server, `send` reaches it through a task-local set by `Registration::scope`, so the 14 send sites
don't each take another argument.

**Stages**, the same split as the §2 gauges:

| `stage` | Server | Shown as |
|---|---|---|
| `handshaking` | login | Logging in |
| `logged_in` | login | Server list |
| `joining_game` | login | Joining game (`PlayOk` sent; the client is about to leave for the game server) |
| `authenticating` | game | Authenticating (`Connecting` + `Authenticated`) |
| `lobby` | game | Lobby |
| `entering` | game | Entering world |
| `in_game` | game | In game |

**`details`** carries server-specific extras. On the game server: the protocol version, the
`RequestHardWareInfo` report (CPU, GPU and driver, Windows version), the lobby's character list,
and for an in-world character roughly what `//charinfo` shows: level, class, race, clan, access
level, hero/noble, PvP/PK, reputation, HP/MP/CP and position. On the login server: the access
level, the last server and the server the client is joining.

**HWID** is the MAC address from `RequestHardWareInfo`, the same key HWID punishments match on
(G31). The stock Interlude client never sends that packet, so expect `null` unless a protection
layer adds it. The login protocol has no hardware fingerprint at all. A client's own MAC can't be
seen from the server's side of a TCP connection, so there's no other source for it.

**API**: `GET /admin/monitor/clients` → `{nowMs, clients: [...], sources: [{service, up, error}]}`.
It's admin-only, and 503 when monitoring is off, like the other `/admin/monitor` routes. All
targets are asked in parallel, each with the 3 s poll timeout. A target that fails, or answers as
another service (ports swapped in `MonitorTargets`, as in §3), is reported in `sources` and
contributes no rows. `nowMs` is the dashboard's clock, so the page measures durations against the
clock that stamped `connectedMs` rather than the viewer's. **No gmaudit record per request**: the
page refreshes every 5 s, and a line per refresh would bury the GM audit log. Log search (§6)
audits because each search is a deliberate act; opening this page is closer to opening
Monitoring.

**`/admin/audit`** (`pages/Audit.tsx`, arithmetic in `lib/audit.ts`):
- **Table:** server, status, account, character, IP, connected, last packet, HWID. Every column
  sorts, and IPv4 sorts numerically.
- **Filters:** search (IP, account, character or HWID), a server switch, and status chips with
  counts.
- **Shared addresses:** an IP or HWID shared by several *game-server* connections gets a "×N"
  marker that filters the list down to them. Login rows are left out of the count, because every
  player passes through the login server on the way in.
- **Details:** a row opens a dialog with the connection (address, connect time, last packet,
  traffic and average rate, protocol), the account (with a "find owner" link to
  `/admin?q=<account>`, which the Accounts page now reads), the character, the lobby list, the
  hardware report, and other connections from the same IP. The dialog follows the client across
  refreshes. Once the client is gone, it says so and keeps the last data seen.
- **Missing servers:** a server that didn't answer is named above the table, so its players don't
  look disconnected.

Class and race names come from `lib/classes.ts`. The server sends ids only, as it does for the
account page's characters.

Tests: `commons` (request grammar, a provider's records stamped with the service, the timeout),
`game_loop::tests::monitor_clients_tests` (stages, lobby list, hardware, character),
`loginserver::clients` (register, stage changes, outbound counted only inside the scope, unlisted
on drop), `dashboard_api` `clients_are_merged_across_servers_and_a_bad_target_is_reported`, and
on the frontend `tests/audit-lib.test.ts` and `tests/audit.test.ts` (Playwright).

### Disconnect and IP bans

The details dialog has two actions, **Disconnect** and **Disconnect and ban IP…**, and the page
ends with a **Banned IPs** section listing every ban, with add, edit and remove.

**Disconnect** is a third channel request, `kick <id> <connectedMs>`, answered with one
`{"kicked":true|false}` line. `false` means no such connection is open. The connect time is part of
the name because ids restart with the process: a list from before a restart must not kick
whoever holds that id now. On the game server the request goes into the game loop
(`MonitorRequest::Kick`) and takes the flood protector's kick path (`helpers::kick_client`):
an in-world character is saved and despawned, and anything earlier just loses its session. On the
login server the registry row carries a `Notify`; the connection task selects on it, sends
`LoginFail(ACCESS_FAILED)` and hangs up, freeing the account like any other disconnect.

This makes the channel able to change something, not just read. The control is unchanged: the
bind address, which may be loopback or a private network and nothing wider (§4). Anyone who can
reach the port can disconnect players.

**IP bans** live in the `ip_bans` table (`ip`, `expires_at` in epoch ms with `0` for permanent,
`reason`, `banned_by`, `created_at`; migration `m20261007_000001_ip_bans`). It replaced the
boot-time `banned_ip.cfg` and the login server's in-memory list, both gone, so every ban survives
a restart. **Both servers read it on every new connection**, so an edit needs no reload. The game
server checks it in each accepted connection's task, before the game thread hears of the
connection. On either server, a database error lets the client through rather than refusing
everyone. Matching is Java's `isBannedAddress` rule, kept in one place in
`models::repo::ip_bans::covering`: a ban covers its address, and trailing `.0` octets make a range,
so `10.1.2.0` is 10.1.2.\* and `10.0.0.0` is all of 10.\*. Expired rows stay in the table and are
shown as expired until someone removes them.

Besides the dashboard, the servers write it themselves, with `banned_by` naming the server. They
use `ban_at_least`, which never shortens a ban already in force, so a 15-minute automatic ban
can't cut a permanent one down:

- **Login server** (`login_server`): `LoginTryBeforeBan` wrong passwords from one address ban it for
  `LoginBlockAfterBan`. The counter itself stays in memory. A game server's `RequestTempBan` bans
  the reported address until the account's ban ends. Java passed that end timestamp as a
  *duration*, which banned the address for decades; the list shows its end date, so it now gets
  the real one.
- **Game server** (`game_server`): the **authentication deadline** (`game_loop::net::auth_guard`,
  configured in `Security.ini`). A game connection that hasn't authenticated within
  `UnauthenticatedTimeout` (5 s) is dropped. If it never sent `AuthLogin`, that counts as a strike
  against its address. `UnauthenticatedStrikesBeforeBan` (5) strikes within
  `UnauthenticatedStrikeWindow` (10 min) ban the address for `UnauthenticatedBanMinutes` (0 means
  permanent). A client that did send its credentials and is still waiting on the login server is
  dropped without a strike, so a login-server outage can't ban real players.

The Audit page shows these as "Login server (automatic)" and "Game server (automatic)".

A ban refuses new connections only. Connections already open stay open, which is why "Disconnect
and ban" and the add form's "disconnect everyone it covers now" also kick, on both servers, every
connection the ban covers.

**API** (admin-only, every change recorded in gmaudit with the acting admin):
- `POST /admin/monitor/clients/disconnect` `{service, id, connectedMs, ip?, account?}` → 204, or
  404 when the connection already left, or 502 when the server didn't answer. `ip` and `account`
  are only for the audit record.
- `GET /admin/ip-bans` → `{nowMs, bans: [{ip, expiresAt, reason, bannedBy, createdAt}]}`, newest
  first. `expiresAt` is `null` for a permanent ban.
- `POST /admin/ip-bans` `{ip, expiresAt?, reason?, disconnect?}` → 201
  `{ban, disconnected, disconnectErrors}`. It replaces any ban already on that address. With
  `disconnect`, it fetches a fresh client list and kicks every covered connection. A server that
  couldn't be asked lands in `disconnectErrors`, and so does monitoring being off; the ban is
  stored either way.
- `PUT /admin/ip-bans/{ip}` `{ip, expiresAt?, reason?}` rewrites a ban, including its address.
  It returns 404 for no such ban, and 400 when the new address already has its own ban.
- `DELETE /admin/ip-bans/{ip}` → 204, or 404.

The address must parse as an IP and is stored normalized. A timed ban must end in the future, and
the reason is capped at 255 characters. The page computes expiry times from the dashboard's clock
(`nowMs`), because the login server compares against that clock and not the viewer's.

Tests: `commons` (the `kick` grammar and answers), `monitor_clients_tests`
(`a_kick_closes_only_the_connection_it_names`), `loginserver::clients` (a kick for the wrong
connect time is refused, and the right one is delivered), loginserver `tests/auth.rs`
(`an_ip_bans_row_refuses_the_address_until_it_expires`, and the failed-password and temp bans
landing in `ip_bans`), `models/tests/ip_bans.rs` (an automatic ban never shortens one in force),
gameserver `tests/handshake.rs`
(`a_banned_address_is_refused_on_accept`) and `game_loop::tests::auth_guard_tests` (the
deadline, the strike exemptions, the ban on the fifth strike), `dashboard_api` (disconnect, ban with
disconnect across two servers, list/edit/lift, validation), and on the frontend
`tests/audit-lib.test.ts` (coverage rule, scope, expiry) and `tests/audit.test.ts` (the three
flows against a stubbed API).
