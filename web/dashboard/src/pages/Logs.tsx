/**
 * `/admin/logs` — search the servers' log files (docs/MONITORING.md §6).
 *
 * Results come newest first, a page at a time; "Load older" passes the
 * server's cursor back. A page can also end because the scan hit its time or
 * size budget (`truncated`) — that is not "no more results", so it is said
 * out loud and offered as "keep searching", never shown as an end of list.
 *
 * Every request is recorded server-side in the gmaudit log, because audit
 * streams hold player chat and IP addresses; the page says so up front.
 */
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { type SubmitEvent, useEffect, useMemo, useState } from "react";

import { AdminNav } from "../components/AdminNav";
import { Alert, Button, Field, LabeledSelect, Panel, Spinner, cx } from "../components/ui";
import { ApiError, api, type LogHit, type LogSearchParams } from "../lib/api";
import { formatBytes } from "../lib/chart";

const RANGES = [
  { label: "Last hour", ms: 3_600_000 },
  { label: "Last 24 hours", ms: 86_400_000 },
  { label: "Last 7 days", ms: 7 * 86_400_000 },
  { label: "Last 30 days", ms: 30 * 86_400_000 },
] as const;

const LEVELS = ["", "error", "warn", "info", "debug"] as const;

const PAGE = 100;

const SERVICE_LABELS: Record<string, string> = {
  game_server: "Game server",
  login_server: "Login server",
  dashboard_api: "Dashboard",
};

function streamLabel(stream: string): string {
  if (stream === "diagnostic") return "Diagnostic log";
  if (stream === "error") return "Errors (WARN+)";
  return `Audit — ${stream.slice("audit:".length)}`;
}

function errorText(error: unknown): string {
  if (error instanceof ApiError && error.code === "unavailable") {
    return "Log search is disabled on this dashboard (LogSearchRoots is empty).";
  }
  if (error instanceof ApiError && error.code === "rate_limited") {
    return "Too many searches are running right now — try again in a moment.";
  }
  return error instanceof ApiError ? error.message : "Something went wrong.";
}

export function Logs() {
  const streams = useQuery({
    queryKey: ["admin", "logs", "streams"],
    queryFn: api.admin.logs.streams,
  });

  const [service, setService] = useState("");
  const [stream, setStream] = useState("");
  const [rangeMs, setRangeMs] = useState<number>(RANGES[1].ms);
  const [q, setQ] = useState("");
  const [regex, setRegex] = useState(false);
  const [level, setLevel] = useState("");
  /** The search actually run — set on submit, so typing does not re-query. */
  const [submitted, setSubmitted] = useState<LogSearchParams | null>(null);

  const services = useMemo(
    () => [...new Set((streams.data?.streams ?? []).map((s) => s.service))],
    [streams.data],
  );
  const serviceStreams = (streams.data?.streams ?? []).filter((s) => s.service === service);
  const isAudit = stream.startsWith("audit:");

  // Default the pickers to the first available choice once streams load, and
  // keep the stream valid when the service changes.
  useEffect(() => {
    if (!service && services[0]) setService(services[0]);
  }, [service, services]);
  useEffect(() => {
    if (serviceStreams.length && !serviceStreams.some((s) => s.stream === stream)) {
      setStream(serviceStreams[0]!.stream);
    }
  }, [serviceStreams, stream]);

  const results = useInfiniteQuery({
    queryKey: ["admin", "logs", "search", submitted],
    queryFn: ({ pageParam }) =>
      api.admin.logs.search({ ...submitted!, cursor: pageParam ?? undefined }),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.cursor,
    enabled: submitted !== null,
    // A log search is a point-in-time read: never refetch it behind the
    // admin's back (each request is also an audit record).
    staleTime: Number.POSITIVE_INFINITY,
    gcTime: 0,
  });

  const onSubmit = (e: SubmitEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!service || !stream) return;
    const to = Date.now();
    setSubmitted({
      service,
      stream,
      from: to - rangeMs,
      to,
      q,
      regex,
      level: isAudit ? undefined : level || undefined,
      limit: PAGE,
    });
  };

  const pages = results.data?.pages ?? [];
  const hits = pages.flatMap((p) => p.hits);
  const last = pages[pages.length - 1];
  const scanned = pages.reduce((n, p) => n + p.scannedBytes, 0);

  return (
    <div className="space-y-5 pb-6">
      <AdminNav />
      <section className="animate-rise">
        <h1 className="text-3xl font-black tracking-tight">Logs</h1>
        <p className="mt-1.5 text-(--text-muted)">
          Search each server's log files, newest first. Audit logs include player chat and IP
          addresses —{" "}
          <strong className="font-semibold text-(--text)">every search is recorded</strong> in the
          GM audit log under your account.
        </p>
      </section>

      {streams.isError ? (
        <Alert kind="error">{errorText(streams.error)}</Alert>
      ) : (
        <Panel className="p-4">
          <form onSubmit={onSubmit} className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            <LabeledSelect
              label="Server"
              value={service}
              onChange={(e) => setService(e.target.value)}
            >
              {services.map((s) => (
                <option key={s} value={s}>
                  {SERVICE_LABELS[s] ?? s}
                </option>
              ))}
            </LabeledSelect>
            <LabeledSelect label="Log" value={stream} onChange={(e) => setStream(e.target.value)}>
              {serviceStreams.map((s) => (
                <option key={s.stream} value={s.stream}>
                  {streamLabel(s.stream)}
                </option>
              ))}
            </LabeledSelect>
            <LabeledSelect
              label="Time range"
              value={rangeMs}
              onChange={(e) => setRangeMs(Number(e.target.value))}
            >
              {RANGES.map((r) => (
                <option key={r.ms} value={r.ms}>
                  {r.label}
                </option>
              ))}
            </LabeledSelect>
            <LabeledSelect
              label="Minimum level"
              value={isAudit ? "" : level}
              disabled={isAudit}
              title={isAudit ? "Audit records have no level" : undefined}
              onChange={(e) => setLevel(e.target.value)}
            >
              {LEVELS.map((l) => (
                <option key={l} value={l}>
                  {l ? l.toUpperCase() : "Any"}
                </option>
              ))}
            </LabeledSelect>
            <div className="sm:col-span-2 lg:col-span-3">
              <Field
                label={regex ? "Regular expression" : "Text (case-insensitive)"}
                value={q}
                onChange={(e) => setQ(e.target.value)}
                placeholder={
                  regex ? "player_\\d+ (died|logged out)" : "account name, item id, message…"
                }
                maxLength={512}
                autoComplete="off"
                spellCheck={false}
              />
            </div>
            <div className="flex items-end justify-between gap-3">
              <label className="mb-2.5 inline-flex items-center gap-2 text-sm text-(--text-muted)">
                <input
                  type="checkbox"
                  checked={regex}
                  onChange={(e) => setRegex(e.target.checked)}
                />
                Regex
              </label>
              <Button
                type="submit"
                loading={results.isFetching && !results.isFetchingNextPage}
                disabled={!stream}
              >
                Search
              </Button>
            </div>
          </form>
        </Panel>
      )}

      {results.isError && <Alert kind="error">{errorText(results.error)}</Alert>}

      {submitted && results.isSuccess && (
        <>
          <p className="text-sm text-(--text-muted)">
            {hits.length === 0
              ? "No matching lines"
              : `${hits.length} line${hits.length === 1 ? "" : "s"}`}
            {" · "}
            {formatBytes(scanned)} scanned across{" "}
            {new Set(pages.flatMap((p) => p.filesScanned)).size} file(s)
          </p>

          {hits.length > 0 && (
            <Panel className="divide-y divide-(--surface-border)">
              {hits.map((h) => (
                <HitRow key={`${h.file}:${h.offset}`} hit={h} />
              ))}
            </Panel>
          )}

          {last?.truncated && (
            // Not an error: the budget is doing its job. Amber, and phrased
            // so it cannot be read as "that's everything".
            <p
              role="status"
              className="rounded-xl border border-amber-400/40 bg-amber-500/10 px-4 py-3 text-sm
                text-amber-700 dark:text-amber-300"
            >
              The search stopped at its {last.stopped === "deadline" ? "time" : "size"} budget
              before reaching the start of the range — older lines have not been searched yet.
            </p>
          )}
          {last?.cursor && (
            <div className="flex justify-center">
              <Button
                variant="ghost"
                loading={results.isFetchingNextPage}
                onClick={() => results.fetchNextPage()}
              >
                {last.truncated ? "Keep searching" : "Load older"}
              </Button>
            </div>
          )}
        </>
      )}
      {submitted && results.isPending && (
        <Panel className="flex items-center gap-3 p-6 text-sm text-(--text-muted)">
          <Spinner /> Searching…
        </Panel>
      )}
    </div>
  );
}

const LEVEL_STYLE: Record<string, string> = {
  ERROR: "bg-red-500/15 text-red-600 dark:text-red-300",
  WARN: "bg-amber-500/15 text-amber-700 dark:text-amber-300",
  INFO: "bg-brand-500/10 text-brand-600 dark:text-brand-200",
};

/** Fields the row already shows in its own slots. */
const SHOWN = new Set(["timestamp", "ts", "level", "message", "target", "event"]);

function HitRow({ hit }: { hit: LogHit }) {
  const [open, setOpen] = useState(false);
  const time = hit.ts !== null ? new Date(hit.ts).toLocaleString() : null;

  if (typeof hit.line === "string") {
    return (
      <div className="px-4 py-2.5">
        <pre className="font-mono text-xs break-all whitespace-pre-wrap">{hit.line}</pre>
      </div>
    );
  }

  const line = hit.line;
  const level = typeof line.level === "string" ? line.level : null;
  const headline = String(line.message ?? line.event ?? "");
  const extra = Object.entries(line).filter(([k]) => !SHOWN.has(k));

  return (
    <div className="px-4 py-2.5 text-sm">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        aria-expanded={open}
        className="flex w-full flex-wrap items-baseline gap-x-2 gap-y-1 text-left"
      >
        {time && <span className="text-xs text-(--text-faint) tabular-nums">{time}</span>}
        {level && (
          <span
            className={cx(
              "rounded px-1.5 py-px text-[10px] font-semibold",
              LEVEL_STYLE[level] ?? "bg-(--surface-strong) text-(--text-muted)",
            )}
          >
            {level}
          </span>
        )}
        {typeof line.target === "string" && (
          <span className="font-mono text-xs text-(--text-muted)">{line.target}</span>
        )}
        <span className="min-w-0 basis-full wrap-break-word sm:basis-auto">{headline}</span>
      </button>
      {!open && extra.length > 0 && (
        <p className="mt-1 truncate font-mono text-[11px] text-(--text-faint)">
          {extra
            .map(([k, v]) => `${k}=${typeof v === "string" ? v : JSON.stringify(v)}`)
            .join("  ")}
        </p>
      )}
      {open && (
        <pre className="mt-2 overflow-x-auto rounded-lg bg-(--surface-strong) p-3 font-mono text-xs">
          {JSON.stringify(line, null, 2)}
        </pre>
      )}
    </div>
  );
}
