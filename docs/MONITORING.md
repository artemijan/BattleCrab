# Monitoring and log search

How the admin dashboard gets its per-server metrics, its live client list and its log search: what
is measured, how it travels from the game and login servers to a chart, and why each part is built
the way it is.

What it covers, per server (login and game):

- **Metrics**: packets and bytes in and out, connections by stage, CPU, memory, game-thread
  pressure. Stored for 7 days at 5 s granularity.
- **Log search** over the NDJSON log files each service already writes (`docs/LOGGING.md`).
- **The live client list**: every open connection, with disconnect and IP bans.

All of it is admin-only and sits in the dashboard's admin section (`docs/DASHBOARD.md` §16).

Code comments cite this document's section numbers, so the numbering is stable. When a section
changes shape, give its number a suffix (as §6a does) rather than renumbering everything below it.

## The path of a number

```
  game server / login server (one each)             dashboard_api                  browser
 ┌─────────────────────────────────────┐          ┌──────────────────────┐
 │ call sites ─► commons::metrics      │          │                      │
 │               registry (§2)         │          │                      │
 │                    │ every 5 s      │          │                      │
 │ sampler thread ─► in-memory ring    │ since ts │ poller ─► metrics.db │  /admin/monitor/series
 │ (§4)                   │            │◄─────────│ (§3)          │      │◄──────────────────────
 │ monitor channel ◄──────┘            │          │ query API (§5)┘      │
 │ 7779 game / 7780 login              │ clients  │                      │  /admin/monitor/clients
 │   ├─ live connections (§10)         │◄─────────│ passed through,      │◄──────────────────────
 │   └─ commons::logsearch (§6)        │ logs …   │ never stored         │  /admin/logs/search
 └─────────────────────────────────────┘          └──────────────────────┘
```

1. Code on a server bumps a counter or sets a gauge in the `commons::metrics` registry (§2).
2. A sampler thread in the same process turns the registry into one `Sample` every 5 s and keeps
   the last hour of them in memory (§4).
3. The dashboard polls each server's monitor channel for the samples it doesn't have yet and writes
   them to its own `metrics.db` (§3).
4. The browser asks the dashboard for a time range, and gets it back downsampled to a few hundred
   points (§5, §6a).

The client list and log search use the same channel but store nothing: the dashboard asks the
server each time the page asks the dashboard (§6, §10).

---

## 1. The shaping decision: pull, with a ring buffer

Each server samples **itself** every 5 s into an in-memory ring. `dashboard_api` polls each server
over an internal channel on loopback or a private network (the same idea as
`loginserver::status_channel`, and the control channel DASHBOARD.md §16.6 describes) and is the
**sole writer** to a dedicated `metrics.db`.

| Alternative | Why not |
|---|---|
| Servers push to the dashboard | Puts retry and backpressure logic in the game loop; a dashboard restart becomes the game server's problem. |
| Dashboard reads counters directly every 5 s | Counters are cumulative, so the sample interval must be exact, and CPU/RSS deltas need consecutive in-process readings, not two independent processes racing a clock. |
| Servers write `log/metrics/*.ndjson`, dashboard scans them | Matches `commons::audit`'s precedent, but every chart request becomes a full scan. Downsampling is `GROUP BY`, which is SQL's job, not a log scan's. |
| Samples land in the game DB | `commons::audit`'s argument (see its module docs): three processes share one SQLite file, and 7-day pruning contends with the game servers for its lock. |

Sampling in-process buys three things:

- The interval is exact, because it is measured where the counters live.
- A bounded ring survives a dashboard restart or deploy without losing history.
- **Counter resets need no handling anywhere.** The sampler's previous reading dies with the
  process, so the first sample after a restart is naturally "counts since start".

## 2. What is measured

Both servers register the same six `commons::metrics` series (`crates/commons/src/metrics.rs`;
counters and gauges are described in `docs/LOGGING.md` §"Metrics"):

| Series | Kind | Meaning |
|---|---|---|
| `packets_in` | counter | Frames read off the socket, counted before decrypt and rate limiting: what arrived on the wire. |
| `bytes_in` | counter | Wire bytes for those frames, header included. |
| `packets_out` | counter | Frames written to the socket, every frame counted. The game server coalesces a tick's frames into one `write_all` and records the batch with a single atomic add of its frame count (`network/connection.rs`'s outbound arm), plus the KeyPacket written outside that batch. The login server does not batch, so it counts per `send()`. |
| `bytes_out` | counter | Wire bytes for those writes, header included. |
| `connections_accepted` | counter | Lifetime total of accepted sockets. |
| `connections_open` | gauge | Live count. Each connection task owns a `Gauge::hold()` guard: `+1` on accept, `-1` when the guard drops, so a task that panics still gives its slot back. Both sides are atomic (`fetch_add` / saturating `try_update`); a load-then-`set` would lose updates under concurrent connects and disconnects. |

**Where they are counted.** At the call sites (`gameserver/src/network/connection.rs`,
`loginserver/src/network/client_connection.rs`), not inside `commons::network::framing`. Those
framing helpers are shared with `gs_link`, the login↔game registration channel, and counting inside
them would mix that internal link into client traffic. The login server's `gs_link` traffic is
deliberately not counted at all: these series are about players, not inter-server plumbing.

**When they are registered.** At boot (`register_metrics()` in each server's network module, called
from `main.rs`), so every series reads `0` from the first sample instead of being *absent* until
the first traffic. `game_loop::net::register_metrics` does the same for `packets_handled`.

### Connection stages

`connections_open` alone can't tell a player in the world from a client sitting at character
select, or from a socket that never finished logging in. Each server also breaks its open
connections down by stage. These are all gauges:

| Series | Server | Meaning |
|---|---|---|
| `sessions_handshaking` | login | Before a successful `RequestAuthLogin`: key exchange, GameGuard, the login form. |
| `sessions_logged_in` | login | Past auth, at the server list. The connection closes once the client leaves for the game server. |
| `sessions_authenticating` | game | `ClientSession::Connecting` + `Authenticated`: the protocol and session-key check is not finished. |
| `sessions_lobby` | game | `InLobby`: character select, including players who restarted out of the world. |
| `sessions_entering` | game | `Entering`: a character is picked and loading into the world. |
| `players_online` | game | `InGame` only. |
| `offline_traders` | game | Unattended private stores. They are in the world but hold no connection. |

The two servers keep these differently, because their stage changes happen in different places:

- **Login server.** A connection holds a `metrics::LoginStage` guard that is swapped on auth, so
  the two stages always sum to `connections_open`.
- **Game server.** Stage changes also happen on DB and login-link replies, not only on network
  events, so `game_loop::net::refresh_session_gauges` recounts `world.clients` once per tick. The
  stages sum to the sessions the game thread knows about.

### Other series

- `audit_written` and `audit_blocked` (both servers): records the audit writer put on disk, and
  times a caller waited on a full audit queue (`docs/LOGGING.md` §"Why audit records are not
  allowed to drop"). They are charted together as "Audit records". `audit_blocked` should stay at
  zero; anything else means the never-drop audit sink is stalling server threads.
- `packets_handled` (game server, `game_loop::net`): compared with `packets_in`, the gap is itself
  a signal. It is packets rejected by the rate limiter or lost to a decode failure.
- Game-thread pressure (`tick_busy_micros_total`, `ticks`, `tick_overruns`) and the process
  readings (CPU, RSS, heap) are described with the sampler in §4.

## 3. Storage: `metrics.db`

A separate SQLite file owned by the dashboard (`crates/dashboard_api/src/monitor/store.rs`). It is
**derived data**: losing it costs 7 days of graphs and nothing else. That is why it gets lighter
treatment than the game DB (WAL with `synchronous = NORMAL`, so a crash may drop the last few
samples), and why it is never the game database.

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

**One column list.** The table is generated from `store::COLUMNS`. The same list is the whitelist
the query API builds SQL from, and the place each column's bucket aggregation is declared, so a
client-supplied metric name is only ever looked up there, never interpolated. Value columns are
nullable because the login server has no tick, heap or player series.

**Wide rows, not `(name, value)` tall rows.** 7 days × 17,280 samples per service per day is about
30 MB for two services. Tall rows would be roughly ten times the row count, and every query would
need pivoting.

**`extra` for series without a column.** A registry series the store has no column for goes into
the `extra` JSON, so a new counter is recorded from its first sample without a schema change, and
promoting it to a column later loses no history.

**Adding a column.** Add it to `COLUMNS`. On open, `add_missing_columns` compares `COLUMNS` with
`pragma_table_info` and runs `ALTER TABLE … ADD COLUMN` for any that are missing. This is additive
and idempotent, so it needs no `user_version` step. Older rows read NULL in the new column; if the
series was being recorded before, its value is still in their `extra`.

**Indexing and writes.** `WITHOUT ROWID` with `PRIMARY KEY (service, ts)` makes the clustered index
match the query pattern, so there is no second index to maintain on each insert. Inserts are
`OR IGNORE`, so an overlapping re-poll is harmless.

**Schema ownership.** The schema lives in `MetricsDb::migrate()` (`CREATE TABLE IF NOT EXISTS` plus
`PRAGMA user_version`), not in the `migration` crate. That crate is wired to the game DB, and this
file must stay droppable on its own.

**Retention.** An hourly `DELETE … WHERE ts < now - MetricsRetentionDays`. There is no `VACUUM` and
no `auto_vacuum`: with a steady row count, freed pages are reused and the file plateaus, while
`auto_vacuum` would cost on every commit for no benefit.

**No machine-level metrics.** Load, available memory and free disk are not collected. The dashboard
could only sample its own host, which is the wrong machine once login and game run elsewhere.
Per-machine metrics with alerting are what a host monitor (node_exporter with Prometheus/Grafana,
Netdata, the VPS provider's graphs) already does well. This store keeps to what only the servers
can see: their own processes. (Schema v2 drops the `host_sample` table from files created before
this was decided.)

### The poller

`monitor/mod.rs`. It wakes 1.5 s after each period boundary, so the servers' boundary samples
already exist, and polls each target in turn with `since <newest stored ts>`.

- **It resumes from the store, not from memory**, so a dashboard restart backfills from the
  servers' rings.
- Each poll has a 3 s timeout and an 8 MB cap. A full ring is about 720 lines of well under 1 KB,
  so anything near either limit is a wedged or misbehaving server.
- **A target whose lines name a different `service` is refused rather than stored.** That is two
  ports swapped in `MonitorTargets`, and storing it would put game traffic on the login charts.
- A failing target is logged once, when it starts failing, not on every poll.
- **Nothing here is fatal to the dashboard.** With `MonitorTargets` empty, or `metrics.db`
  unopenable, the account features still serve and `/admin/monitor` answers 503.

## 4. The sampler and the monitor channel

### The sampler

`commons::monitor` (`crates/commons/src/monitor/`). A `std::thread`, not a tokio task, so a busy
runtime can't delay a sample. Every `SampleSeconds` it reads the registry and the process's CPU and
memory, turns them into one interval `Sample`, and pushes it into a bounded ring (`RingSamples`,
default 720 = 1 hour, which covers a dashboard restart or deploy without a gap).

- **Interval values.** `commons::metrics` records each series' `Kind` at first registration
  (`metrics::readings()`). A counter's sample value is its delta since the previous sample; a
  gauge's is its reading at sample time. The baseline is taken when the sampler starts, so the
  first sample is a true rate, not a boot-time lump. A counter registered later counts from zero.
- **CPU.** `libc::getrusage(RUSAGE_SELF)` → `ru_utime + ru_stime` in microseconds, diffed per
  interval (`cpu_micros`). It works on Linux and macOS, needs no `/proc` parsing, and sums all
  threads.
- **RSS.** `/proc/self/statm` field 2 × page size on Linux. Elsewhere it is `None`, stored as SQL
  `NULL`, so local macOS development shows memory as unavailable while production (Linux) reports
  it. `sysinfo` is not used: it is a large dependency tree for one number.
- **Heap.** Game server only. `heap_bytes` is mimalloc's committed memory (`mi_process_info`, via
  `libmimalloc-sys`'s `extended` feature), passed to `monitor::spawn` as a `HeapProbe`. The login
  server runs on the system allocator and reports `null`.
- **Game-thread pressure.** `tick_busy_micros` is a gauge overwritten every tick, so a 5 s sample
  would catch 1 tick in 50. The game loop therefore also keeps two counters,
  `tick_busy_micros_total` and `ticks` (mean busy time = Δtotal / Δticks), and `tick_overruns`,
  incremented where `TICK_OVERRUN_WARN` fires. The mean hides spikes; the overrun count catches
  them.

**Alignment.** The sampler sleeps to the next period boundary rather than for a fixed period, and
stamps `ts = floor(now / period)`. Both servers therefore land in the same buckets and the phase
doesn't drift. `interval_ms` is the real monotonic elapsed time, so a stall shows up as a longer
interval instead of an inflated rate.

**Clock steps.** If the wall clock steps backwards (NTP) onto a bucket the ring already holds, that
sample is skipped without advancing the baseline, and the next one covers both intervals. Storage
is keyed on `(service, ts)` (§3), so a duplicate `ts` must never be emitted.

### The channel

One TCP port per server: game `7779`, login `7780`. One request line in, NDJSON out, then the
connection closes.

```
$ printf 'since 1759000000000\n' | nc 127.0.0.1 7779
{"cpu_micros":8120,"heap_bytes":48234496,"interval_ms":5000,"metrics":{"bytes_in":…,"packets_in":1204,…},"rss_bytes":…,"service":"game_server","ts":1759000005000}
```

| Request | Answer |
|---|---|
| `since <epoch_ms>` | Every buffered sample with `ts` strictly greater, oldest first, so a poller passing back the last `ts` it stored never gets it twice. |
| *(empty line)* | The whole ring. |
| `clients` | One `ClientRecord` line per open connection (§10). |
| `kick <id> <connectedMs>` | One `{"kicked":true\|false}` line (§10). |
| `logs streams` | One `StreamInfo` line per log stream (§6). |
| `logs search <json>` | One `Outcome` line for a `SearchRequest` (§6). |
| anything else | One `{"error":…}` line. |

- **Its own port, not an extension of `status_channel` (7778).** That channel is "connect and
  read". Making it request-driven would need a read-timeout dance to stay backward compatible.
- **Registry series sit under `metrics`**, not at the top level, so a counter can never collide
  with a fixed field. The poller maps known names to columns and the rest to `extra` (§3).
- **Requests are capped at 8 KB and 2 s**, so a silent or flooding client can't hold a task or grow
  a buffer. 8 KB fits the longest real request: a `logs search` line with a 512-byte pattern,
  JSON-escaped, plus a cursor.
- **`InternalMonitorPort = 0` disables the sampler too.** With nothing able to read the ring, there
  is no point filling it. A bind failure is logged and the server boots anyway.

### Security: the bind address is the control

The channel has no authentication, the same model as `status_channel.rs`: the network it listens on
is trusted. Anyone who can reach the port sees traffic volumes, process memory, every connected
player's address and the server's logs, and can disconnect players.

The bind rule is what keeps that safe (`commons::network::internal::check_bind`, applied to this
channel and to the login status channel). The address must be loopback (the default) or private:
`10/8`, `172.16/12`, `192.168/16`, `100.64/10` (Tailscale), `fc00::/7`. A hostname is resolved and
every address it gives must pass. `0.0.0.0`, `::` and public addresses are refused: the channel
stays off and the server logs why, but boots.

This is what lets the servers run on other machines than the dashboard (bind the machine's private
address, point `MonitorTargets` at it) without a typo in an ini ever exposing the port.

## 5. The metrics API

Admin-only (`require_admin`), under `/api/v1/admin/monitor` (`routes/monitor.rs`). Both endpoints
answer 503 `unavailable` when monitoring is off, so the SPA can tell "disabled" from "broken".

```
GET /admin/monitor/services
  → {services: [{service, address, up, lastPollMs, lastSampleTs, startedMs, uptimeSeconds, lastError}],
     pollSeconds, retentionDays}
GET /admin/monitor/series?service&from&to&metrics&maxPoints
  → {service, from, to, bucketMs, aggregation: {name: sum|max|avg|min},
     ts: [...], samples: [...], intervalMs: [...], series: {name: [...]}}
```

- **Columnar** rather than an array of points: it is what a chart consumes, and it doesn't repeat
  every key on every point.
- `from` is inclusive and `to` exclusive, both epoch ms. They default to the last hour. The range
  is capped at retention + 1 day. `maxPoints` defaults to 500, with a maximum of 2000.
- `metrics` is a comma list checked against the column whitelist (§3). An unknown name is a 400
  that lists the valid ones. An unknown `service` is a 404.
- **Aggregation.** Delta columns are `sum`med; divide by the bucket's `intervalMs` for a rate (CPU
  % = `cpu_micros / (intervalMs × 10)`). Gauges are `max`, because pressure is about the worst
  moment and an average hides it.
- `samples` is each bucket's raw sample count. A bucket with fewer than expected is a gap (the
  server or the dashboard was down), which a chart should show rather than smooth over.
- `uptimeSeconds` comes from the newest sample's `started` and is null while the target is down.

**Downsampling happens on the server, and it is load-bearing.** 7 days at 5 s is 120,960 points per
series. The bucket width is `ceil(range / (maxPoints − 1))`, rounded up to whole 5 s periods so a
bucket never splits a sample. Buckets are epoch-aligned, so a range touches one more bucket than
`range / width`, and the `− 1` is what keeps the count ≤ `maxPoints` (a test sweeps a 7-day range
across start offsets).

**The live view is polling.** The page requests `series?from=<last bucket ts>` every 5 s. That
bucket is still filling, so the client replaces it rather than appending. There is no SSE or
WebSocket: data only arrives every 5 s anyway, and keeping the API stateless behind the Cloudflare
Tunnel is worth more than shaving that latency.

## 6. Log search

Admin-only (`require_admin`, DASHBOARD.md §16.2), under `/api/v1/admin/logs` (`routes/logs.rs`).

```
GET /admin/logs/streams
  → {streams: [{service, stream, files, oldest, newestEnd}],       only streams that have files
     sources: [{service, up, error}]}       one per MonitorTargets server; a down one lists nothing
GET /admin/logs/search?service&stream&from&to&q&regex&level&limit&cursor
  → {service, stream, from, to, hits: [{file, offset, ts, line}], cursor, stopped, truncated,
     scannedBytes, filesScanned, skippedOversized}
```

### Each service searches its own files

The scanner is `commons::logsearch`, and it runs on the machine that wrote the logs:

- The game and login servers run it when the dashboard asks over their monitor channel (§4).
  `logs streams` answers one `StreamInfo` line per stream. `logs search <json>` takes a
  `SearchRequest` and answers one `Outcome` line, which the dashboard passes on as it came.
- The dashboard searches only its own logs, in-process (`dashboard_api::logsearch`).

Nothing reads another process's directory, so the servers can run on other machines. The mapping
from service to files also lives with the service that writes them: each server builds its `Source`
from its own `Logging.ini` at boot, and the dashboard's config names no log directories.

**A request is validated twice by the same function**, `SearchRequest::validate`: by the dashboard,
for a clear 400, and by the server, which does not take the asker's word for it. The dashboard
sends its own budget (`LogSearchMaxBytes`, `LogSearchTimeoutMs`); the server caps it at 1 GB and
30 s, since the scan costs its machine. The dashboard waits for the reply for that deadline plus
5 s.

### No client-supplied paths, by construction

- `service` must be `dashboard_api` or a `MonitorTargets` service, else 404.
- `stream` must parse into a closed enum: `diagnostic`, `error`, or `audit:<category>` for one of
  `commons::audit::Category::ALL`.
- Each service's directories are resolved once at boot from its datapack's own `Logging.ini` (the
  dashboard's audit directory is `dashboard_api::logsearch::DASHBOARD_AUDIT_DIR`, which `main` also
  uses), and a search enumerates that directory.
- The cursor carries a filename, but it is only ever *compared* against that enumeration, never
  joined onto a path, so traversal can't be expressed (tested with `../` and absolute names).

### The scan

`logsearch::search`:

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
4. **Bounds**, all enforced:
   - `LogSearchMaxBytes` (256 MB) and `LogSearchTimeoutMs` (3 s) per request.
   - `limit` 1..=500 (default 100), and at most 8 MB of matching lines per response
     (`MAX_HIT_BYTES`, stopping like `limit`).
   - `LogSearchConcurrency` (2) via a semaphore on the dashboard, covering local and remote
     searches alike, plus 2 per server. A search over the cap gets a 429 rather than a queue. A
     server that can't answer is a 502 naming it.
   - Hitting the byte or time bound returns `truncated: true` and a cursor. Hitting `limit` returns
     `stopped: "limit"` and a cursor. Either way the cursor resumes exactly after the last line
     consumed (tested: no gaps, no repeats, including resuming a truncated scan to completion).
5. **Off the async runtime.** The scan runs in `spawn_blocking`, holding the permit for its
   duration.
6. **Lines pass through.** JSON lines come back as `serde_json::Value` with span fields intact.
   `_error.log` lines come back as raw strings, and so does any JSON-stream line that fails to
   parse.

### Matching

`q` is literal and case-insensitive by default (`regex::escape`). With `regex=true` it is taken as
written. The `regex` crate is linear-time with no backtracking, and `size_limit` /
`dfa_size_limit` (1 MB) stop a pattern from costing memory instead. A `level` floor is refused on
audit streams, which have no level. A plain `_error.log` line without a stamp (a panic's
continuation) is kept, because it belongs to the entry above it.

### Cost

Measured on a 191 MB, 1M-line synthetic day (release build, M-series laptop): a full-day literal
search and a `level=error` search each take about 130 ms (about 1.5 GB/s). A last-hour search stops
after 8 MB in 6 ms. The 256 MB default budget is therefore a bit over a day of dense logging per
request.

**Why a bounded scan and not an index.** FTS5 was considered and rejected: it needs a second writer
tailing the files and roughly doubles storage, while rotation already provides the time index that
makes the bounded scan fast. It remains the escape hatch if scans measure slow in practice.

### Searches are audited

Audit logs contain chat and IPs. Every search, including each cursor page, writes a `gmaudit`
record (`event: "log_search"`, admin, service, stream, `q`, regex flag, range) *before* the scan,
so a search that is cut short is still on record.

## 6a. The admin UI

Pages under the dashboard's admin section (`web/dashboard`), reached from a tab row
(`components/AdminNav.tsx`: Accounts / Monitoring / Audit / Logs). The header keeps its single
"Admin" entry. The Audit page is described in §10.

### `/admin/monitor`

`pages/Monitoring.tsx`: a card per server (up or down, uptime or last sample, last poll error),
then one tab per server, and 1h / 6h / 24h / 7d ranges.

- **Game server:** packets/s, bandwidth, open connections and players, new connections/s, CPU % of
  one core, RSS and heap, mean tick time, and ticks over 50 ms.
- **Login server:** the same minus the game-only series.

Rates are computed client-side as each `sum` bucket over its own `intervalMs` (§5). Every chart
refetches about once per bucket, never faster than the poll interval.

**Charts are hand-rolled SVG** (`components/LineChart.tsx`, arithmetic in `lib/chart.ts`). There is
no chart dependency, consistent with DASHBOARD.md §8.2 (§9 item 4).

- **Rendering:** drawn at the container's measured pixel width (ResizeObserver) rather than through
  a scaled viewBox, so strokes and labels stay crisp. Static: no transitions or filters, within the
  mobile GPU budget.
- **Axes:** the x axis is the *requested* range, so downtime at the edges shows as empty space. The
  y axis starts at 0, since every series is a rate, count or size, and a floating baseline turns
  noise into cliffs.
- **Gaps:** a line breaks at missing buckets (`null`, or a time jump > 1.5 buckets) instead of
  bridging an outage, and a lone sample renders as a dot.
- **Interaction:** hover or touch snaps a crosshair to the nearest bucket, but only within one
  bucket of the pointer, so a gap reads as a gap.
- **Colour:** a theme-aware `--chart-*` palette, blue and amber first (distinguishable with
  red-green colour blindness).

### `/admin/logs`

`pages/Logs.tsx`: the form has server, log (stream), time range, minimum level (disabled for audit
streams, which have none) and text or regex. It states up front that every search is recorded in
the GM audit log.

- **Paging:** results are an infinite list over the API cursor. A page cut short by the scan budget
  shows an amber note and **Keep searching**, never anything that reads as the end of the results.
- **Rows:** a JSON line shows its time, level, target and message, with the remaining fields
  inline; it expands to the full pretty-printed JSON. An `_error.log` line shows as raw monospace
  text.
- **No silent refetches:** searches never refetch in the background, because each request is an
  audit record.

### Nested routes need an absolute asset path

`build.ts` sets `publicPath: "/"`. Bun's default emits relative asset URLs (`./index-x.js`), so a
hard refresh or direct link to a *nested* route (`/admin/monitor`, `/admin/accounts/:email`) would
request `/admin/index-x.js`, the SPA fallback would return `index.html` for it, and the page would
render blank, on both the Rust host and Cloudflare. The UI tests open nested routes directly, which
guards this.

## 7. Configuration

### On each server: `config/Monitor.ini`

One per datapack (`dist/game/`, `dist/login/`), read by `commons::monitor::MonitorConfig`,
following the same pattern as `Logging.ini`.

| Key | Default | Meaning |
|---|---|---|
| `InternalMonitorBindAddress` | `127.0.0.1` | Where the channel listens. Loopback or a private address only (§4). |
| `InternalMonitorPort` | game `7779`, login `7780` | `0` disables the channel and the sampler. |
| `SampleSeconds` | `5` | The sample period. Keep it the same on both servers, so they share buckets. |
| `RingSamples` | `720` | How many samples the ring keeps: how long the dashboard can be away without a gap. |

The service name isn't configurable. It is the same `game_server` / `login_server` string logging
uses, passed by each `main.rs`. `deploy.sh` rsyncs `dist/{game,login}/` whole, so this file deploys
with the datapack.

### On the dashboard: `Dashboard.ini`

| Key | Default | Meaning |
|---|---|---|
| `MonitorTargets` | `game_server=127.0.0.1:7779,login_server=127.0.0.1:7780` | `service=host:port` pairs. Empty disables monitoring; a malformed entry disables it with an error at boot. |
| `MetricsDatabase` | `metrics.db` | Path of the sample store (§3). |
| `MetricsPollSeconds` | `5` | How often the poller runs. |
| `MetricsRetentionDays` | `7` | How long samples are kept. |
| `LogSearchEnabled` | `True` | `False` makes `/admin/logs` answer 503. |
| `LogSearchMaxBytes` | 256 MB | Bytes one search request may read. |
| `LogSearchTimeoutMs` | `3000` | Time one search request may take. |
| `LogSearchConcurrency` | `2` | Searches the dashboard runs at once; more get a 429. |

- **The code defaults equal the values in the distributed ini.** `deploy-dashboard.sh` seeds
  `Dashboard.ini` only when the remote has none, so an existing remote file never receives keys
  added later and runs on the defaults.
- ⚠ **`MetricsDatabase` is a plain path, and a relative one resolves against the *executable's*
  directory**, exactly like the game database's `URL`, not the working directory. In production
  that is next to `interlude_classic.db`; under `cargo run` it is `target/debug/metrics.db`. The
  absolute path is logged at boot. Both deploy scripts exclude `*.db` from their rsyncs.
- There is no key naming log directories: each service finds its own (§6). An old `Dashboard.ini`
  that still sets `LogSearchRoots` is ignored.

## 8. Where the code lives

| Piece | Location |
|---|---|
| Metric registry (counters, gauges, `Kind`) | `crates/commons/src/metrics.rs` |
| Wire traffic counters | `crates/commons/src/network/traffic.rs`, `crates/loginserver/src/metrics.rs`, and the call sites named in §2 |
| Sampler, ring, `MonitorConfig` | `crates/commons/src/monitor/mod.rs` |
| CPU and RSS readings | `crates/commons/src/monitor/process.rs` |
| Monitor channel | `crates/commons/src/monitor/channel.rs` |
| Bind rule | `crates/commons/src/network/internal.rs` |
| Client records, `Provider`, `ConnectionStats` | `crates/commons/src/monitor/clients.rs` |
| Game server's client list and kick | `crates/gameserver/src/game_loop/net/clients.rs` |
| Login server's client registry | `crates/loginserver/src/clients.rs` |
| Log scanner | `crates/commons/src/logsearch/` |
| Poller, target state, channel requests | `crates/dashboard_api/src/monitor/mod.rs` |
| Sample store | `crates/dashboard_api/src/monitor/store.rs` |
| Sample line parsing | `crates/dashboard_api/src/monitor/wire.rs` |
| Dashboard's own log search | `crates/dashboard_api/src/logsearch.rs` |
| HTTP routes | `crates/dashboard_api/src/routes/{monitor,logs,ip_bans}.rs` |
| IP ban table and matching | `models::repo::ip_bans`, migration `m20261007_000001_ip_bans` |
| Pages | `web/dashboard/src/pages/{Monitoring,Logs,Audit}.tsx` |
| Charts | `web/dashboard/src/components/LineChart.tsx`, `lib/chart.ts` |

### Tests

- **Sampler and channel:** `commons` (request grammar, a provider's records stamped with the
  service, the timeout, the `kick` grammar and answers).
- **Log search:** `commons::logsearch` tests (every chunk size, cursor resume, traversal attempts).
- **Client list and kick:** `game_loop::tests::monitor_clients_tests` (stages, lobby list,
  hardware, character, `a_kick_closes_only_the_connection_it_names`) and `loginserver::clients`
  (register, stage changes, outbound counted only inside the scope, unlisted on drop, a kick for
  the wrong connect time refused and the right one delivered).
- **IP bans:** loginserver `tests/auth.rs` (`an_ip_bans_row_refuses_the_address_until_it_expires`,
  and the failed-password and temp bans landing in `ip_bans`), `models/tests/ip_bans.rs` (an
  automatic ban never shortens one in force), gameserver `tests/handshake.rs`
  (`a_banned_address_is_refused_on_accept`) and `game_loop::tests::auth_guard_tests` (the deadline,
  the strike exemptions, the ban on the fifth strike).
- **Dashboard API:** `crates/dashboard_api/tests/api.rs` (series and bucketing, log search,
  `clients_are_merged_across_servers_and_a_bad_target_is_reported`, disconnect, ban with disconnect
  across two servers, list/edit/lift, validation).
- **Frontend:** `tests/chart.test.ts` (ticks, gap-breaking paths, rates over real intervals,
  formatting), `tests/audit-lib.test.ts` (coverage rule, scope, expiry), and two Playwright suites
  against the built bundle with a stubbed API:
  - `tests/monitoring.test.ts`: a data hole drawn as two line runs, game-only charts kept off the
    login tab, the 503 "disabled" message, the search request carrying the chosen filters, "Load
    older" sending the cursor back, the truncation note, audit streams never sending `level`, and
    the current admin tab, including account detail pages.
  - `tests/audit.test.ts`: the client list and the disconnect and ban flows.

## 9. Trade-offs and known limits

1. **Raw retention, no rollups.** Raw 5 s samples are kept for the full 7 days (about 30 MB). The
   alternative, raw for 24 h plus 1-minute rollups for the rest of the week, is about 1/12 the
   storage and makes long-range queries trivial, at the cost of losing 5 s resolution past a day.
   Revisit if `metrics.db` size or query latency becomes a problem.
2. **Log search does not assume co-location.** Each server searches its own files when asked over
   its monitor channel (§6), so the dashboard reads no other machine's disk.
3. **Gaps when the dashboard is down.** The server-side ring (§4) covers a deploy. A longer outage
   leaves a hole in the chart, which is the honest rendering rather than an interpolated one.
4. **No chart library.** Charts are hand-rolled SVG (§6a), in keeping with `docs/DASHBOARD.md`
   §8.2's habit of minimizing dependencies, and static SVG stays inside the mobile GPU budget.
5. **Login and game on separate machines.** Supported on a trusted private network: every channel
   between the dashboard and the servers may bind a private-network address, and nothing wider
   (§4's bind rule). Metrics, the client list, kicks, status and log search then all work across
   machines with configuration alone. Machine-level metrics (load, memory, disk) are left to an
   external host monitor (§3). The deploy scripts take a host per service (`LOGIN_HOST`,
   `GAME_HOST`, `DASHBOARD_HOST`), tell co-located services apart by `/etc/machine-id`, and write
   every address above from that topology (`deploy-lib.sh`). A split needs PostgreSQL
   (`docs/DATABASE.md`), which the scripts check: SQLite is one machine's file.

## 10. The live client list (Audit page)

Every connection open right now, on both servers, with who it is and how far it got. Unlike §3–§5
nothing is stored: the dashboard asks each server whenever the page asks the dashboard.

### On the channel

`clients` returns one `ClientRecord` line per open connection and closes
(`commons::monitor::clients`). The record is camelCase, unlike a sample line, because the dashboard
passes it to the browser unchanged:

```
$ printf 'clients\n' | nc 127.0.0.1 7779
{"service":"game_server","id":12,"ip":"203.0.113.9","port":51234,"connectedMs":…,"stage":"in_game","account":"alice","character":"Hero","hwid":null,"traffic":{"packetsIn":…,"bytesIn":…,"packetsOut":…,"bytesOut":…,"lastPacketMs":…},"details":{…}}
```

A server answers through the `Provider` it installs with `clients::set_provider`: a list function
and a kick function. It is a slot rather than a `monitor::spawn` argument because the game server
starts its monitor before its game loop's channel exists. A server with no provider gets an error
line, and so does a provider that doesn't answer within 2 s.

- **Game server.** The sessions belong to the game thread, so the provider sends
  `GameEvent::Monitor(MonitorRequest)` into the unified event channel, and the game thread answers
  in the same drain (`game_loop::net::clients`). It costs nothing while nobody is looking, and it
  adds no lock for the game thread to take.
- **Login server.** Each connection task holds a `clients::Registration`, which adds a row to a
  registry and removes it on drop, so a panicking task leaves nothing behind (the same reasoning as
  `LoginStage`, §2). The session updates the row on auth and on `PlayOk`.

**Per-connection traffic** is `ConnectionStats`: atomics bumped at the same call sites as the
server-wide `packets_in` / `bytes_in` / `packets_out` / `bytes_out` (§2), so the two always agree.
It also records the last inbound frame's time and the connect time. On the game server it lives on
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
seen from the server's side of a TCP connection, so there is no other source for it.

### The API

`GET /admin/monitor/clients` → `{nowMs, clients: [...], sources: [{service, up, error}]}`.
Admin-only, and 503 when monitoring is off, like the other `/admin/monitor` routes.

- All targets are asked in parallel, each with the 3 s poll timeout.
- A target that fails, or answers as another service (ports swapped in `MonitorTargets`, as in
  §3), is reported in `sources` and contributes no rows.
- `nowMs` is the dashboard's clock, so the page measures durations against the clock that stamped
  `connectedMs` rather than the viewer's.
- **No gmaudit record per request.** The page refreshes every 5 s, and a line per refresh would
  bury the GM audit log. Log search (§6) audits because each search is a deliberate act; opening
  this page is closer to opening Monitoring.

### The page

`/admin/audit` (`pages/Audit.tsx`, arithmetic in `lib/audit.ts`):

- **Table:** server, status, account, character, IP, connected, last packet, HWID. Every column
  sorts, and IPv4 sorts numerically.
- **Filters:** search (IP, account, character or HWID), a server switch, and status chips with
  counts.
- **Shared addresses:** an IP or HWID shared by several *game-server* connections gets a "×N"
  marker that filters the list down to them. Login rows are left out of the count, because every
  player passes through the login server on the way in.
- **Details:** a row opens a dialog with the connection (address, connect time, last packet,
  traffic and average rate, protocol), the account (with a "find owner" link to
  `/admin?q=<account>`, which the Accounts page reads), the character, the lobby list, the hardware
  report, and other connections from the same IP. The dialog follows the client across refreshes.
  Once the client is gone, it says so and keeps the last data seen.
- **Missing servers:** a server that didn't answer is named above the table, so its players don't
  look disconnected.

Class and race names come from `lib/classes.ts`. The server sends ids only, as it does for the
account page's characters.

### Disconnect and IP bans

The details dialog has two actions, **Disconnect** and **Disconnect and ban IP…**, and the page
ends with a **Banned IPs** section listing every ban, with add, edit and remove.

#### Disconnect

A channel request, `kick <id> <connectedMs>`, answered with one `{"kicked":true|false}` line.
`false` means no such connection is open.

- **The connect time is part of the name** because ids restart with the process: a list from before
  a restart must not kick whoever holds that id now.
- **Game server.** The request goes into the game loop (`MonitorRequest::Kick`) and takes the flood
  protector's kick path (`helpers::kick_client`): an in-world character is saved and despawned, and
  anything earlier just loses its session.
- **Login server.** The registry row carries a `Notify`. The connection task selects on it, sends
  `LoginFail(ACCESS_FAILED)` and hangs up, freeing the account like any other disconnect.

This makes the channel able to change something, not just read. The control is unchanged: the bind
address, which may be loopback or a private network and nothing wider (§4). Anyone who can reach
the port can disconnect players.

#### IP bans

Bans live in the `ip_bans` table (`ip`, `expires_at` in epoch ms with `0` for permanent, `reason`,
`banned_by`, `created_at`; migration `m20261007_000001_ip_bans`). There is no boot-time
`banned_ip.cfg` and no in-memory list on the login server, so every ban survives a restart.

- **Both servers read the table on every new connection**, so an edit needs no reload. The game
  server checks it in each accepted connection's task, before the game thread hears of the
  connection.
- **A database error lets the client through** on either server, rather than refusing everyone.
- **Matching** is Java's `isBannedAddress` rule, kept in one place in
  `models::repo::ip_bans::covering`: a ban covers its address, and trailing `.0` octets make a
  range, so `10.1.2.0` is 10.1.2.\* and `10.0.0.0` is all of 10.\*.
- **Expired rows stay in the table** and are shown as expired until someone removes them.
- **A ban refuses new connections only.** Connections already open stay open, which is why
  "Disconnect and ban" and the add form's "disconnect everyone it covers now" also kick, on both
  servers, every connection the ban covers.

Besides the dashboard, the servers write bans themselves, with `banned_by` naming the server. They
use `ban_at_least`, which never shortens a ban already in force, so a 15-minute automatic ban can't
cut a permanent one down. The Audit page shows these as "Login server (automatic)" and "Game server
(automatic)".

- **Login server** (`login_server`): `LoginTryBeforeBan` wrong passwords from one address ban it
  for `LoginBlockAfterBan`. The counter itself stays in memory. A game server's `RequestTempBan`
  bans the reported address until the account's ban ends. (Java passed that end timestamp as a
  *duration*, which banned the address for decades. The list shows the end date, so here it is the
  real one.)
- **Game server** (`game_server`): the **authentication deadline** (`game_loop::net::auth_guard`,
  configured in `Security.ini`). A game connection that hasn't authenticated within
  `UnauthenticatedTimeout` (5 s) is dropped. If it never sent `AuthLogin`, that counts as a strike
  against its address. `UnauthenticatedStrikesBeforeBan` (5) strikes within
  `UnauthenticatedStrikeWindow` (10 min) ban the address for `UnauthenticatedBanMinutes` (0 means
  permanent). A client that did send its credentials and is still waiting on the login server is
  dropped without a strike, so a login-server outage can't ban real players.

#### API

Admin-only, and every change is recorded in gmaudit with the acting admin.

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
- `PUT /admin/ip-bans/{ip}` `{ip, expiresAt?, reason?}` rewrites a ban, including its address. It
  returns 404 for no such ban, and 400 when the new address already has its own ban.
- `DELETE /admin/ip-bans/{ip}` → 204, or 404.

The address must parse as an IP and is stored normalized. A timed ban must end in the future, and
the reason is capped at 255 characters. The page computes expiry times from the dashboard's clock
(`nowMs`), because the login server compares against that clock and not the viewer's.
