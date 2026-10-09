/**
 * `/admin/monitor` — per-server traffic, connections and pressure
 * (docs/MONITORING.md §5).
 *
 * One tab per server. Charts are derived client-side from the
 * API's bucketed series: `sum` series become per-second rates over each
 * bucket's real `intervalMs`, so a bucket that lost a sample does not read as
 * a dip. The current range refetches on the server's poll interval; a longer
 * range refetches less often, since a 5 s change is invisible at a week's
 * resolution.
 */
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { AdminNav } from "../components/AdminNav";
import { type ChartLine, LineChart } from "../components/LineChart";
import { Alert, Panel, Spinner, cx } from "../components/ui";
import {
  ApiError,
  api,
  type MonitorSeries,
  type MonitorService,
  type MonitorServiceStatus,
} from "../lib/api";
import {
  cpuPercent,
  formatBytes,
  formatCount,
  formatDuration,
  perSecond,
  ratio,
} from "../lib/chart";

const RANGES = [
  { label: "1h", ms: 3_600_000 },
  { label: "6h", ms: 6 * 3_600_000 },
  { label: "24h", ms: 24 * 3_600_000 },
  { label: "7d", ms: 7 * 24 * 3_600_000 },
] as const;

const TABS: Array<{ id: MonitorService; label: string }> = [
  { id: "game_server", label: "Game server" },
  { id: "login_server", label: "Login server" },
];

/** Points per chart. Charts are ~600-900 px wide; more would be sub-pixel. */
const MAX_POINTS = 360;

const C1 = "var(--chart-1)";
const C2 = "var(--chart-2)";
const C3 = "var(--chart-3)";
const C4 = "var(--chart-4)";
const C5 = "var(--chart-5)";

const perSec = (f: (v: number) => string) => (v: number) => `${f(v)}/s`;
const percent = (v: number) => `${formatCount(v)}%`;
const ms = (v: number) => `${formatCount(v)} ms`;

function errorText(error: unknown): string {
  if (error instanceof ApiError && error.code === "unavailable") {
    return "Server monitoring is disabled on this dashboard (MonitorTargets is empty).";
  }
  return error instanceof ApiError ? error.message : "Something went wrong.";
}

export function Monitoring() {
  const [tab, setTab] = useState<MonitorService>("game_server");
  const [rangeMs, setRangeMs] = useState<number>(RANGES[0].ms);

  const services = useQuery({
    queryKey: ["admin", "monitor", "services"],
    queryFn: api.admin.monitor.services,
    refetchInterval: 5_000,
  });
  const pollMs = (services.data?.pollSeconds ?? 5) * 1000;
  // Roughly one refetch per bucket, never faster than the server polls.
  const refetchMs = Math.max(pollMs, Math.round(rangeMs / MAX_POINTS));

  const series = useQuery({
    queryKey: ["admin", "monitor", tab, rangeMs],
    queryFn: () => {
      const to = Date.now();
      const from = to - rangeMs;
      return api.admin.monitor.series(tab, from, to, MAX_POINTS);
    },
    refetchInterval: refetchMs,
    placeholderData: (previous, previousQuery) =>
      // Keep the old chart while the same tab loads a new range; a different
      // tab's data under this tab's titles would be wrong, not just stale.
      previousQuery?.queryKey[2] === tab ? previous : undefined,
  });

  return (
    <div className="space-y-5 pb-6">
      <AdminNav />
      <section className="animate-rise">
        <h1 className="text-3xl font-black tracking-tight">Monitoring</h1>
        <p className="mt-1.5 text-(--text-muted)">
          Traffic, connections and resource pressure per server, sampled every{" "}
          {services.data?.pollSeconds ?? 5} seconds and kept for {services.data?.retentionDays ?? 7}{" "}
          days.
        </p>
      </section>

      {services.isError && <Alert kind="error">{errorText(services.error)}</Alert>}

      {/* The status cards double as the server tabs. */}
      <ServiceCards
        statuses={services.data?.services}
        checking={services.isPending}
        selected={tab}
        onSelect={setTab}
      />

      <fieldset className="flex justify-end gap-1" aria-label="Time range">
        {RANGES.map((r) => (
          <button
            key={r.label}
            type="button"
            aria-pressed={rangeMs === r.ms}
            onClick={() => setRangeMs(r.ms)}
            className={cx(
              "rounded-lg px-2.5 py-1 text-xs font-semibold transition-colors",
              rangeMs === r.ms
                ? "bg-(--surface-strong) text-(--text)"
                : "text-(--text-muted) hover:text-(--text)",
            )}
          >
            {r.label}
          </button>
        ))}
      </fieldset>

      {series.isPending ? (
        <Panel className="flex items-center gap-3 p-6 text-sm text-(--text-muted)">
          <Spinner /> Loading…
        </Panel>
      ) : series.isError ? (
        <Alert kind="error">{errorText(series.error)}</Alert>
      ) : (
        <ServiceCharts data={series.data} game={tab === "game_server"} />
      )}
    </div>
  );
}

/**
 * One card per server, rendered from `TABS` rather than from the status
 * response so the tabs exist while the status is loading or unavailable.
 */
function ServiceCards({
  statuses,
  checking,
  selected,
  onSelect,
}: {
  statuses: MonitorServiceStatus[] | undefined;
  checking: boolean;
  selected: MonitorService;
  onSelect: (tab: MonitorService) => void;
}) {
  return (
    <div role="tablist" aria-label="Server" className="grid gap-3 sm:grid-cols-2">
      {TABS.map((t) => {
        const s = statuses?.find((status) => status.service === t.id);
        const active = selected === t.id;
        return (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={active}
            onClick={() => onSelect(t.id)}
            className={cx(
              "glass-strong glass-sheen block w-full touch-manipulation rounded-2xl p-4 text-left",
              "transition-[border-color,box-shadow,transform] duration-200",
              active ? "aura-card" : "opacity-80 hover:-translate-y-0.5 hover:opacity-100",
            )}
          >
            <div className="flex items-center justify-between gap-2">
              <p className={cx("font-semibold", active && "text-brand-600 dark:text-brand-100")}>
                {t.label}
              </p>
              {s ? (
                <span
                  className={cx(
                    "inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[11px] font-medium",
                    s.up
                      ? "bg-emerald-500/15 text-emerald-600 dark:text-emerald-300"
                      : "bg-red-500/15 text-red-600 dark:text-red-300",
                  )}
                >
                  <span
                    aria-hidden
                    className={cx("size-1.5 rounded-full", s.up ? "bg-emerald-500" : "bg-red-500")}
                  />
                  {s.up ? "Up" : "Down"}
                </span>
              ) : (
                checking && <Spinner className="size-3.5 text-(--text-faint)" />
              )}
            </div>
            <p className="mt-1 text-sm text-(--text-muted)">
              {!s
                ? checking
                  ? "Checking…"
                  : "Status unavailable"
                : s.up && s.uptimeSeconds !== null
                  ? `Up ${formatDuration(s.uptimeSeconds)}`
                  : s.lastSampleTs
                    ? `Last sample ${new Date(s.lastSampleTs).toLocaleString()}`
                    : "No samples yet"}
              {s && <span className="break-all text-(--text-faint)"> · {s.address}</span>}
            </p>
            {s && !s.up && s.lastError && (
              <p
                className="mt-1 truncate text-xs text-red-600 dark:text-red-300"
                title={s.lastError}
              >
                {s.lastError}
              </p>
            )}
          </button>
        );
      })}
    </div>
  );
}

function chartProps(data: MonitorSeries) {
  return { ts: data.ts, from: data.from, to: data.to, bucketMs: data.bucketMs };
}

function col(data: MonitorSeries, name: string): Array<number | null> {
  return data.series[name] ?? data.ts.map(() => null);
}

function ServiceCharts({ data, game }: { data: MonitorSeries; game: boolean }) {
  const interval = data.intervalMs;
  const rate = (name: string) => perSecond(col(data, name), interval);
  const p = chartProps(data);

  const open: ChartLine = {
    label: "Open",
    values: col(data, "connections_open"),
    color: C1,
    description: game
      ? "TCP connections to the game server right now, in any stage. The stages beside it add up to this."
      : "TCP connections to the login server right now. Handshaking + Logged in add up to this.",
  };
  // Where each open connection is in its lifecycle. A client goes
  // login server → (closes) → game server, so a player who just pressed
  // "Play" shows up on the game server's chart, not here.
  const connections: ChartLine[] = game
    ? [
        open,
        {
          label: "Logging in",
          values: col(data, "sessions_authenticating"),
          color: C2,
          description:
            "Connected to the game server but not yet past the protocol and session-key check with the login server. Should be momentary; a steady count here is stalled or non-game clients.",
        },
        {
          label: "Character select",
          values: col(data, "sessions_lobby"),
          color: C4,
          description:
            "Authenticated accounts sitting at the character-selection screen, including players who restarted out of the world. No character is in the world yet.",
        },
        {
          label: "Entering world",
          values: col(data, "sessions_entering"),
          color: C5,
          description:
            "A character was picked and is being loaded into the world. Should be momentary.",
        },
        {
          label: "In game",
          values: col(data, "players_online"),
          color: C3,
          description: "Characters actually in the world with a live connection.",
        },
      ]
    : [
        open,
        {
          label: "Handshaking",
          values: col(data, "sessions_handshaking"),
          color: C2,
          description:
            "Connected to the login server but not logged in yet: key exchange, GameGuard, or the login form.",
        },
        {
          label: "Logged in",
          values: col(data, "sessions_logged_in"),
          color: C3,
          description:
            "Logged in and looking at the server list. The connection closes once the client moves on to the game server, or after 5 minutes at most.",
        },
      ];

  return (
    <div className="grid gap-3 lg:grid-cols-2">
      <LineChart
        title="Packets"
        {...p}
        format={perSec(formatCount)}
        lines={[
          {
            label: "In",
            values: rate("packets_in"),
            color: C1,
            description:
              "Client packets received per second, counted off the socket before decryption or rate limiting.",
          },
          {
            label: "Out",
            values: rate("packets_out"),
            color: C2,
            description: game
              ? "Socket writes per second. The game server batches a tick's packets for one client into one write, so this is lower than the packets actually sent."
              : "Packets sent to clients per second.",
          },
        ]}
      />
      <LineChart
        title="Bandwidth"
        {...p}
        format={perSec(formatBytes)}
        lines={[
          {
            label: "In",
            values: rate("bytes_in"),
            color: C1,
            description: "Bytes received from clients per second, packet headers included.",
          },
          {
            label: "Out",
            values: rate("bytes_out"),
            color: C2,
            description: "Bytes sent to clients per second, packet headers included.",
          },
        ]}
      />
      <LineChart title="Connections" {...p} format={formatCount} lines={connections} />
      {/* Its own chart: a rate of a few per second on the open-connections
          axis is a flat line along the bottom. A spike here with flat open
          connections is a connect flood, which is exactly what to spot. */}
      <LineChart
        title="New connections"
        {...p}
        format={perSec(formatCount)}
        lines={[
          {
            label: "Accepted",
            values: rate("connections_accepted"),
            color: C3,
            description:
              "New TCP connections accepted per second. A spike with flat open connections is a connect flood.",
          },
        ]}
      />
      {game && (
        <LineChart
          title="Players in world"
          {...p}
          format={formatCount}
          lines={[
            {
              label: "In game",
              values: col(data, "players_online"),
              color: C3,
              description: "Characters in the world with a live connection.",
            },
            {
              label: "Offline shops",
              values: col(data, "offline_traders"),
              color: C4,
              description:
                "Characters left in the world as unattended private stores after their owner disconnected. They hold no connection.",
            },
          ]}
        />
      )}
      <LineChart
        title="CPU (% of one core)"
        {...p}
        format={percent}
        lines={[
          {
            label: "CPU",
            values: cpuPercent(col(data, "cpu_micros"), interval),
            color: C1,
            description:
              "Process CPU time (user + system) as a share of one core. Above 100% means more than one core busy.",
          },
        ]}
      />
      <LineChart
        title="Memory (peak)"
        {...p}
        format={formatBytes}
        note="Memory is reported on Linux only."
        lines={[
          {
            label: "RSS",
            values: col(data, "rss_bytes"),
            color: C1,
            description:
              "Resident memory of the process: everything in RAM, including code, stacks and mapped data files. Peak per bucket.",
          },
          ...(game
            ? [
                {
                  label: "Heap",
                  values: col(data, "heap_bytes"),
                  color: C2,
                  description:
                    "Memory the allocator has committed for heap objects. Tracks leaks and bursts better than RSS. Peak per bucket.",
                },
              ]
            : []),
        ]}
      />
      {game && (
        <LineChart
          title="Game loop — mean tick (budget 100 ms)"
          {...p}
          format={ms}
          lines={[
            {
              label: "Mean tick",
              values: ratio(col(data, "tick_busy_micros_total"), col(data, "ticks"), 1 / 1000),
              color: C1,
              description:
                "Average time the game thread spent working per 100 ms tick (idle waiting excluded). Near 100 ms the server falls behind.",
            },
          ]}
        />
      )}
      {game && (
        // The mean hides spikes; this is where they show. A count per
        // bucket, not a rate: one overrun is already worth seeing.
        <LineChart
          title="Game loop — ticks over 50 ms"
          {...p}
          format={formatCount}
          lines={[
            {
              label: "Overruns",
              values: col(data, "tick_overruns"),
              color: C2,
              description:
                "Ticks in this bucket that took over 50 ms. Each one also logs a warning naming the slowest steps.",
            },
          ]}
        />
      )}
      {/* `audit_blocked` should be a flat zero: the audit sink never drops,
          so a full queue stalls the caller instead, and this is where that
          cost shows. */}
      <LineChart
        title="Audit records"
        {...p}
        format={perSec(formatCount)}
        lines={[
          {
            label: "Written",
            values: rate("audit_written"),
            color: C1,
            description:
              "Audit records (chat, items, enchants, GM commands, logins) written to disk per second.",
          },
          {
            label: "Stalls",
            values: rate("audit_blocked"),
            color: C2,
            description:
              "Times per second a server thread had to wait because the audit queue was full. Should stay at zero.",
          },
        ]}
      />
    </div>
  );
}
