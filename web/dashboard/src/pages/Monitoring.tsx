/**
 * `/admin/monitor` — per-server traffic, connections and pressure
 * (docs/MONITORING.md §5).
 *
 * One tab per server plus the host. Charts are derived client-side from the
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

type Tab = MonitorService | "host";

const TABS: Array<{ id: Tab; label: string }> = [
  { id: "game_server", label: "Game server" },
  { id: "login_server", label: "Login server" },
  { id: "host", label: "Host" },
];

/** Points per chart. Charts are ~600-900 px wide; more would be sub-pixel. */
const MAX_POINTS = 360;

const C1 = "var(--chart-1)";
const C2 = "var(--chart-2)";
const C3 = "var(--chart-3)";

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
  const [tab, setTab] = useState<Tab>("game_server");
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
      return tab === "host"
        ? api.admin.monitor.host(from, to, MAX_POINTS)
        : api.admin.monitor.series(tab, from, to, MAX_POINTS);
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

      {services.isError ? (
        <Alert kind="error">{errorText(services.error)}</Alert>
      ) : (
        <ServiceCards statuses={services.data?.services} />
      )}

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div role="tablist" aria-label="Server" className="flex gap-1">
          {TABS.map((t) => (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={tab === t.id}
              onClick={() => setTab(t.id)}
              className={cx(
                "rounded-xl px-3.5 py-1.5 text-sm font-medium transition-colors",
                tab === t.id
                  ? "bg-brand-500 text-white"
                  : "text-(--text-muted) hover:bg-(--surface-strong) hover:text-(--text)",
              )}
            >
              {t.label}
            </button>
          ))}
        </div>
        <fieldset className="flex gap-1" aria-label="Time range">
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
      </div>

      {series.isPending ? (
        <Panel className="flex items-center gap-3 p-6 text-sm text-(--text-muted)">
          <Spinner /> Loading…
        </Panel>
      ) : series.isError ? (
        <Alert kind="error">{errorText(series.error)}</Alert>
      ) : tab === "host" ? (
        <HostCharts data={series.data} />
      ) : (
        <ServiceCharts data={series.data} game={tab === "game_server"} />
      )}
    </div>
  );
}

function ServiceCards({ statuses }: { statuses: MonitorServiceStatus[] | undefined }) {
  if (!statuses) {
    return (
      <Panel className="flex items-center gap-3 p-4 text-sm text-(--text-muted)">
        <Spinner /> Checking servers…
      </Panel>
    );
  }
  return (
    <div className="grid gap-3 sm:grid-cols-2">
      {statuses.map((s) => (
        <Panel key={s.service} className="p-4" strong>
          <div className="flex items-center justify-between gap-2">
            <p className="font-semibold">
              {s.service === "game_server" ? "Game server" : "Login server"}
            </p>
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
          </div>
          <p className="mt-1 text-sm text-(--text-muted)">
            {s.up && s.uptimeSeconds !== null
              ? `Up ${formatDuration(s.uptimeSeconds)}`
              : s.lastSampleTs
                ? `Last sample ${new Date(s.lastSampleTs).toLocaleString()}`
                : "No samples yet"}
            <span className="text-(--text-faint)"> · {s.address}</span>
          </p>
          {!s.up && s.lastError && (
            <p className="mt-1 truncate text-xs text-red-600 dark:text-red-300" title={s.lastError}>
              {s.lastError}
            </p>
          )}
        </Panel>
      ))}
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
  const interval = data.intervalMs ?? [];
  const rate = (name: string) => perSecond(col(data, name), interval);
  const p = chartProps(data);

  const connections: ChartLine[] = [
    { label: "Open", values: col(data, "connections_open"), color: C1 },
  ];
  if (game) connections.push({ label: "Players", values: col(data, "players_online"), color: C2 });

  return (
    <div className="grid gap-3 lg:grid-cols-2">
      <LineChart
        title="Packets"
        {...p}
        format={perSec(formatCount)}
        lines={[
          { label: "In", values: rate("packets_in"), color: C1 },
          { label: "Out", values: rate("packets_out"), color: C2 },
        ]}
      />
      <LineChart
        title="Bandwidth"
        {...p}
        format={perSec(formatBytes)}
        lines={[
          { label: "In", values: rate("bytes_in"), color: C1 },
          { label: "Out", values: rate("bytes_out"), color: C2 },
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
        lines={[{ label: "Accepted", values: rate("connections_accepted"), color: C3 }]}
      />
      <LineChart
        title="CPU (% of one core)"
        {...p}
        format={percent}
        lines={[{ label: "CPU", values: cpuPercent(col(data, "cpu_micros"), interval), color: C1 }]}
      />
      <LineChart
        title="Memory (peak)"
        {...p}
        format={formatBytes}
        note="Memory is reported on Linux only."
        lines={
          game
            ? [
                { label: "RSS", values: col(data, "rss_bytes"), color: C1 },
                { label: "Heap", values: col(data, "heap_bytes"), color: C2 },
              ]
            : [{ label: "RSS", values: col(data, "rss_bytes"), color: C1 }]
        }
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
          lines={[{ label: "Overruns", values: col(data, "tick_overruns"), color: C2 }]}
        />
      )}
    </div>
  );
}

function HostCharts({ data }: { data: MonitorSeries }) {
  const p = chartProps(data);
  return (
    <div className="grid gap-3 lg:grid-cols-2">
      <LineChart
        title="Load average"
        {...p}
        format={formatCount}
        lines={[
          { label: "1m", values: col(data, "load1"), color: C1 },
          { label: "5m", values: col(data, "load5"), color: C2 },
          { label: "15m", values: col(data, "load15"), color: C3 },
        ]}
      />
      <LineChart
        title="Memory available (low point)"
        {...p}
        format={formatBytes}
        note="Host memory is reported on Linux only."
        lines={[
          { label: "Available", values: col(data, "mem_available_bytes"), color: C1 },
          { label: "Total", values: col(data, "mem_total_bytes"), color: C3 },
        ]}
      />
      <LineChart
        title="Disk free (low point)"
        {...p}
        format={formatBytes}
        lines={[
          { label: "Free", values: col(data, "disk_free_bytes"), color: C1 },
          { label: "Total", values: col(data, "disk_total_bytes"), color: C3 },
        ]}
      />
    </div>
  );
}
