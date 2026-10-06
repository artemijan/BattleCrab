/**
 * `/admin/audit` — every connection open right now, on both servers
 * (docs/MONITORING.md §10).
 *
 * The list is asked of the servers on each refresh, not stored: a client that
 * left between two refreshes simply isn't in the next one. Clicking a row
 * opens its details, which keep following the client while it stays
 * connected and say so once it is gone.
 */
import { useQuery } from "@tanstack/react-query";
import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";

import { AdminNav } from "../components/AdminNav";
import { Alert, Field, Panel, Spinner, StatusBadge, cx } from "../components/ui";
import { ApiError, type ConnectedClient, type ConnectedClients, api } from "../lib/api";
import {
  FIRST_DIR,
  SERVICE_LABEL,
  STAGES,
  type SortKey,
  clientKey,
  formatSince,
  matches,
  sharedCounts,
  sortClients,
  stageInfo,
} from "../lib/audit";
import { formatBytes, formatCount } from "../lib/chart";
import { className, raceName } from "../lib/classes";

const REFRESH_MS = 5_000;

const COLUMNS: Array<{ key: SortKey; label: string }> = [
  { key: "service", label: "Server" },
  { key: "stage", label: "Status" },
  { key: "account", label: "Account" },
  { key: "character", label: "Character" },
  { key: "ip", label: "IP" },
  { key: "connected", label: "Connected" },
  { key: "idle", label: "Last packet" },
];

type ServiceFilter = "all" | "game_server" | "login_server";

function errorText(error: unknown): string {
  if (error instanceof ApiError && error.code === "unavailable") {
    return "Server monitoring is disabled on this dashboard (MonitorTargets is empty), so there is no one to ask for the client list.";
  }
  return error instanceof ApiError ? error.message : "Something went wrong.";
}

/** The server's clock, ticking locally between refreshes. Durations are
 *  measured against it so a viewer's skewed clock can't make them negative. */
function useServerNow(data: ConnectedClients | undefined, receivedAt: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  return data ? now + (data.nowMs - receivedAt) : now;
}

export function Audit() {
  const list = useQuery({
    queryKey: ["admin", "monitor", "clients"],
    queryFn: api.admin.monitor.clients,
    refetchInterval: REFRESH_MS,
    placeholderData: (previous) => previous,
  });
  const now = useServerNow(list.data, list.dataUpdatedAt);

  const [query, setQuery] = useState("");
  const [service, setService] = useState<ServiceFilter>("all");
  const [stage, setStage] = useState<string | null>(null);
  const [sort, setSort] = useState<SortKey>("connected");
  const [dir, setDir] = useState<"asc" | "desc">("desc");
  const [selected, setSelected] = useState<string | null>(null);

  const clients = list.data?.clients ?? [];
  const shared = useMemo(() => sharedCounts(clients), [clients]);
  const byService = clients.filter((c) => service === "all" || c.service === service);
  const stageCounts = new Map<string, number>();
  for (const c of byService) stageCounts.set(c.stage, (stageCounts.get(c.stage) ?? 0) + 1);
  const rows = sortClients(
    byService.filter((c) => (!stage || c.stage === stage) && matches(c, query)),
    sort,
    dir,
  );

  const onSort = (key: SortKey) => {
    if (sort === key) setDir(dir === "asc" ? "desc" : "asc");
    else {
      setSort(key);
      setDir(FIRST_DIR[key]);
    }
  };

  return (
    <div className="space-y-5 pb-6">
      <AdminNav />
      <section className="animate-rise">
        <h1 className="text-3xl font-black tracking-tight">Audit</h1>
        <p className="mt-1.5 text-(--text-muted)">
          Everyone connected right now, on both servers. Refreshes every {REFRESH_MS / 1000}{" "}
          seconds; click a client for details.
        </p>
      </section>

      {list.isError && !list.data ? (
        <Alert kind="error">{errorText(list.error)}</Alert>
      ) : list.isPending ? (
        <Panel className="flex items-center gap-3 p-6 text-sm text-(--text-muted)">
          <Spinner /> Asking the servers…
        </Panel>
      ) : (
        <>
          <SourceNotes data={list.data} stale={list.isError} />

          <div className="grid gap-3 sm:grid-cols-[1fr_auto] sm:items-end">
            <Field
              label="Search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="IP, account, character or HWID"
              autoComplete="off"
              spellCheck={false}
            />
            <fieldset className="flex gap-1" aria-label="Server">
              {(["all", "game_server", "login_server"] as const).map((s) => (
                <button
                  key={s}
                  type="button"
                  aria-pressed={service === s}
                  onClick={() => {
                    setService(s);
                    setStage(null);
                  }}
                  className={cx(
                    "rounded-xl px-3.5 py-2 text-sm font-medium transition-colors",
                    service === s
                      ? "bg-brand-500 text-white"
                      : "text-(--text-muted) hover:bg-(--surface-strong) hover:text-(--text)",
                  )}
                >
                  {s === "all" ? "All" : SERVICE_LABEL[s]}
                </button>
              ))}
            </fieldset>
          </div>

          <fieldset className="flex flex-wrap gap-1.5" aria-label="Status">
            <StageChip
              active={stage === null}
              onClick={() => setStage(null)}
              label="Any status"
              count={byService.length}
            />
            {Object.entries(STAGES)
              .filter(([key]) => stageCounts.has(key))
              .map(([key, info]) => (
                <StageChip
                  key={key}
                  active={stage === key}
                  onClick={() => setStage(stage === key ? null : key)}
                  label={info.label}
                  title={info.hint}
                  count={stageCounts.get(key) ?? 0}
                />
              ))}
          </fieldset>

          {rows.length === 0 ? (
            <Panel className="p-8 text-center text-sm text-(--text-muted)">
              {clients.length === 0
                ? "Nobody is connected."
                : "No connected client matches these filters."}
            </Panel>
          ) : (
            <Panel className="overflow-x-auto">
              <table className="w-full min-w-200 text-sm">
                <thead>
                  <tr className="border-b border-(--surface-border) text-left">
                    {COLUMNS.map((column) => (
                      <th key={column.key} className="px-4 py-2.5 font-medium">
                        <button
                          type="button"
                          onClick={() => onSort(column.key)}
                          className={cx(
                            "inline-flex items-center gap-1 transition-colors hover:text-(--text)",
                            sort === column.key ? "text-(--text)" : "text-(--text-muted)",
                          )}
                        >
                          {column.label}
                          <span aria-hidden className="text-xs">
                            {sort === column.key ? (dir === "asc" ? "▲" : "▼") : ""}
                          </span>
                        </button>
                      </th>
                    ))}
                    <th className="px-4 py-2.5 font-medium text-(--text-muted)">HWID / MAC</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-(--surface-border)">
                  {rows.map((c) => (
                    <ClientRow
                      key={clientKey(c)}
                      client={c}
                      now={now}
                      sharedIp={c.service === "game_server" ? (shared.ip.get(c.ip) ?? 0) : 0}
                      sharedHwid={c.hwid ? (shared.hwid.get(c.hwid) ?? 0) : 0}
                      onOpen={() => setSelected(clientKey(c))}
                      onFilter={setQuery}
                    />
                  ))}
                </tbody>
              </table>
            </Panel>
          )}
          <p className="text-xs text-(--text-faint)">
            {rows.length === clients.length
              ? `${clients.length} connection${clients.length === 1 ? "" : "s"}`
              : `${rows.length} of ${clients.length} connections`}
            . Addresses and HWIDs marked ×N are shared by N game-server connections.
          </p>
        </>
      )}

      {selected && (
        <ClientDialog
          clientKey={selected}
          clients={clients}
          now={now}
          onSelect={setSelected}
          onClose={() => setSelected(null)}
        />
      )}
    </div>
  );
}

/** A server that could not answer contributes no rows; say so, rather than
 *  let its players look disconnected. */
function SourceNotes({ data, stale }: { data: ConnectedClients | undefined; stale: boolean }) {
  const down = data?.sources.filter((s) => !s.up) ?? [];
  if (!stale && down.length === 0) return null;
  return (
    <div
      role="status"
      className="space-y-1 rounded-xl border border-amber-400/40 bg-amber-500/10 px-4 py-3 text-sm
                 text-amber-700 dark:text-amber-300"
    >
      {stale && <p>The last refresh failed; showing the list from before it.</p>}
      {down.map((s) => (
        <p key={s.service}>
          <span className="font-semibold">{SERVICE_LABEL[s.service] ?? s.service} server</span> did
          not answer, so its clients are missing below{s.error ? `: ${s.error}` : "."}
        </p>
      ))}
    </div>
  );
}

function StageChip({
  active,
  onClick,
  label,
  count,
  title,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
  count: number;
  title?: string;
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      title={title}
      onClick={onClick}
      className={cx(
        "rounded-lg px-2.5 py-1 text-xs font-semibold transition-colors",
        active ? "bg-(--surface-strong) text-(--text)" : "text-(--text-muted) hover:text-(--text)",
      )}
    >
      {label} <span className="tabular-nums opacity-70">{count}</span>
    </button>
  );
}

function StageBadge({ stage }: { stage: string }) {
  const info = stageInfo(stage);
  return (
    <span title={info.hint}>
      <StatusBadge kind={info.tone}>{info.label}</StatusBadge>
    </span>
  );
}

/** A "×N" marker that filters the list down to what it counts. */
function SharedMarker({
  count,
  value,
  onFilter,
}: {
  count: number;
  value: string;
  onFilter: (q: string) => void;
}) {
  if (count < 2) return null;
  return (
    <button
      type="button"
      title={`${count} game-server connections share this — show them`}
      onClick={(e) => {
        e.stopPropagation();
        onFilter(value);
      }}
      className="ml-1.5 rounded-full bg-amber-500/15 px-1.5 py-0.5 text-[11px] font-semibold
                 text-amber-700 hover:bg-amber-500/25 dark:text-amber-300"
    >
      ×{count}
    </button>
  );
}

const muted = <span className="text-(--text-faint)">—</span>;

function ClientRow({
  client: c,
  now,
  sharedIp,
  sharedHwid,
  onOpen,
  onFilter,
}: {
  client: ConnectedClient;
  now: number;
  sharedIp: number;
  sharedHwid: number;
  onOpen: () => void;
  onFilter: (q: string) => void;
}) {
  const last = c.traffic.lastPacketMs;
  return (
    <tr onClick={onOpen} className="cursor-pointer transition-colors hover:bg-(--surface-strong)">
      <td className="px-4 py-3 whitespace-nowrap text-(--text-muted)">
        {SERVICE_LABEL[c.service] ?? c.service}
      </td>
      <td className="px-4 py-3">
        <StageBadge stage={c.stage} />
      </td>
      <td className="max-w-40 truncate px-4 py-3">
        {/* The row's own control for keyboard users; the row click covers the mouse. */}
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onOpen();
          }}
          className="max-w-full truncate text-left font-medium hover:underline"
        >
          {c.account ?? <span className="font-normal text-(--text-faint)">not logged in</span>}
        </button>
      </td>
      <td className="max-w-40 truncate px-4 py-3">{c.character ?? muted}</td>
      <td className="px-4 py-3 font-mono text-xs whitespace-nowrap">
        {c.ip}
        <SharedMarker count={sharedIp} value={c.ip} onFilter={onFilter} />
      </td>
      <td
        className="px-4 py-3 whitespace-nowrap text-(--text-muted)"
        title={new Date(c.connectedMs).toLocaleString()}
      >
        {formatSince(now - c.connectedMs)} ago
      </td>
      <td className="px-4 py-3 whitespace-nowrap text-(--text-muted)">
        {last === null ? "never" : `${formatSince(now - last)} ago`}
      </td>
      <td className="max-w-48 truncate px-4 py-3 font-mono text-xs">
        {c.hwid ?? muted}
        {c.hwid && <SharedMarker count={sharedHwid} value={c.hwid} onFilter={onFilter} />}
      </td>
    </tr>
  );
}

/* --------------------------------- Details -------------------------------- */

function ClientDialog({
  clientKey: key,
  clients,
  now,
  onSelect,
  onClose,
}: {
  clientKey: string;
  clients: ConnectedClient[];
  now: number;
  onSelect: (key: string) => void;
  onClose: () => void;
}) {
  // Follows the client across refreshes; once it is gone, keeps showing the
  // last thing seen of it.
  const live = clients.find((c) => clientKey(c) === key);
  const [lastSeen, setLastSeen] = useState<ConnectedClient | undefined>(live);
  useEffect(() => {
    if (live) setLastSeen(live);
  }, [live]);
  const c = live ?? lastSeen;

  // Native modal dialog, as in CharacterItemsDialog: Escape, focus trap and
  // backdrop come from the platform; the page behind is held still.
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const { scrollX, scrollY } = window;
    ref.current?.showModal();
    window.scrollTo(scrollX, scrollY);
    const root = document.documentElement;
    const previous = root.style.overflow;
    root.style.overflow = "hidden";
    return () => {
      root.style.overflow = previous;
    };
  }, []);

  if (!c) return null;
  const title = c.character ?? c.account ?? c.ip;
  const siblings = clients.filter((o) => o.ip === c.ip && clientKey(o) !== clientKey(c));

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: Escape already closes a native modal dialog.
    <dialog
      ref={ref}
      onClose={onClose}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      aria-label={`Connection details: ${title}`}
      className="glass-strong glass-sheen animate-rise inset-0 m-auto max-h-[85vh] w-[calc(100%-2rem)]
                 max-w-xl touch-manipulation overflow-hidden rounded-2xl bg-transparent p-0
                 text-(--text) backdrop:bg-black/60 backdrop:backdrop-blur-sm"
    >
      <div className="flex max-h-[85vh] flex-col">
        <div className="flex items-start justify-between gap-3 border-b border-(--surface-border) px-5 py-3.5">
          <div className="min-w-0">
            <p className="truncate font-semibold">{title}</p>
            <div className="mt-1 flex flex-wrap items-center gap-1.5 text-xs text-(--text-muted)">
              <span>{SERVICE_LABEL[c.service] ?? c.service} server</span>
              <StageBadge stage={c.stage} />
              {!live && <StatusBadge kind="bad">Disconnected</StatusBadge>}
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="grid size-8 shrink-0 place-items-center rounded-lg text-(--text-faint)
                       transition-colors hover:bg-(--surface-strong) hover:text-(--text)"
          >
            <svg viewBox="0 0 20 20" aria-hidden className="size-4">
              <path
                d="m5.5 5.5 9 9m0-9-9 9"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
              />
            </svg>
          </button>
        </div>

        <div className="space-y-5 overflow-y-auto px-5 py-4 text-sm">
          {!live && (
            <p className="rounded-lg bg-red-500/10 px-3 py-2 text-red-700 dark:text-red-300">
              This client has disconnected. Below is the last thing seen of it.
            </p>
          )}
          <ConnectionSection c={c} now={now} />
          <AccountSection c={c} />
          <CharacterSection c={c} />
          <LobbySection c={c} />
          <HardwareSection c={c} />
          {siblings.length > 0 && (
            <Section title={`Same IP (${siblings.length} other)`}>
              <ul className="space-y-1">
                {siblings.map((o) => (
                  <li key={clientKey(o)}>
                    <button
                      type="button"
                      onClick={() => onSelect(clientKey(o))}
                      className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left
                                 hover:bg-(--surface-strong)"
                    >
                      <span className="w-12 shrink-0 text-xs text-(--text-muted)">
                        {SERVICE_LABEL[o.service] ?? o.service}
                      </span>
                      <StageBadge stage={o.stage} />
                      <span className="truncate">
                        {o.character ?? o.account ?? "not logged in"}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </Section>
          )}
        </div>
      </div>
    </dialog>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="mb-2 text-xs font-semibold tracking-wide text-(--text-faint) uppercase">
        {title}
      </h2>
      {children}
    </section>
  );
}

/** A two-column list of label/value pairs; null values are left out. */
function Facts({ rows }: { rows: Array<[string, ReactNode | null | undefined]> }) {
  return (
    <dl className="grid grid-cols-[minmax(7rem,auto)_1fr] gap-x-4 gap-y-1.5">
      {rows
        .filter(([, v]) => v !== null && v !== undefined)
        .map(([label, value]) => (
          <div key={label} className="contents">
            <dt className="text-(--text-muted)">{label}</dt>
            <dd className="min-w-0 wrap-break-word">{value}</dd>
          </div>
        ))}
    </dl>
  );
}

const mono = (v: string) => <span className="font-mono text-xs">{v}</span>;

function ConnectionSection({ c, now }: { c: ConnectedClient; now: number }) {
  const t = c.traffic;
  const seconds = Math.max(1, (now - c.connectedMs) / 1000);
  const rate = (bytes: number) => `${formatBytes(bytes / seconds)}/s avg`;
  return (
    <Section title="Connection">
      <Facts
        rows={[
          ["Address", mono(`${c.ip}:${c.port}`)],
          [
            "Connected",
            <>
              {new Date(c.connectedMs).toLocaleString()}{" "}
              <span className="text-(--text-muted)">({formatSince(now - c.connectedMs)})</span>
            </>,
          ],
          [
            "Last packet",
            t.lastPacketMs === null
              ? "never — it has not sent anything yet"
              : `${formatSince(now - t.lastPacketMs)} ago`,
          ],
          [
            "Received",
            `${formatCount(t.packetsIn)} packets · ${formatBytes(t.bytesIn)} · ${rate(t.bytesIn)}`,
          ],
          [
            "Sent",
            `${formatCount(t.packetsOut)} packets · ${formatBytes(t.bytesOut)} · ${rate(t.bytesOut)}`,
          ],
          ["Protocol", c.details.protocolVersion?.toString()],
          ["Connection id", mono(`${c.service}#${c.id}`)],
        ]}
      />
    </Section>
  );
}

/** A function rather than a component: its results go into `Facts` rows. */
function accessLevel(level: number): ReactNode {
  if (level < 0) return <StatusBadge kind="bad">Banned ({level})</StatusBadge>;
  if (level > 0) return <StatusBadge kind="ok">GM ({level})</StatusBadge>;
  return <span className="text-(--text-muted)">0 (player)</span>;
}

function AccountSection({ c }: { c: ConnectedClient }) {
  const d = c.details;
  if (!c.account) {
    return (
      <Section title="Account">
        <p className="text-(--text-muted)">Not authenticated yet.</p>
      </Section>
    );
  }
  return (
    <Section title="Account">
      <Facts
        rows={[
          [
            "Game account",
            <>
              <span className="font-medium">{c.account}</span>{" "}
              <Link
                to={`/admin?q=${encodeURIComponent(c.account)}`}
                className="text-xs text-brand-600 hover:underline dark:text-brand-200"
              >
                find owner
              </Link>
            </>,
          ],
          ["Access level", d.accessLevel === undefined ? null : accessLevel(d.accessLevel)],
          ["Last server", d.lastServer?.toString()],
          ["Going to server", d.joiningServer?.toString()],
        ]}
      />
    </Section>
  );
}

function Bar({
  label,
  gauge,
  color,
}: {
  label: string;
  gauge: { cur: number; max: number } | null;
  color: string;
}) {
  if (!gauge) return null;
  const pct = gauge.max > 0 ? Math.min(100, (gauge.cur / gauge.max) * 100) : 0;
  return (
    <div className="flex items-center gap-2">
      <span className="w-6 text-xs text-(--text-muted)">{label}</span>
      <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-(--surface-strong)">
        <div className={cx("h-full rounded-full", color)} style={{ width: `${pct}%` }} />
      </div>
      <span className="w-28 text-right text-xs tabular-nums text-(--text-muted)">
        {Math.round(gauge.cur).toLocaleString("en-US")} / {gauge.max.toLocaleString("en-US")}
      </span>
    </div>
  );
}

function CharacterSection({ c }: { c: ConnectedClient }) {
  const ch = c.details.character;
  if (!ch) {
    return c.stage === "entering" && c.character ? (
      <Section title="Character">
        <p>
          <span className="font-medium">{c.character}</span>{" "}
          <span className="text-(--text-muted)">— loading into the world.</span>
        </p>
      </Section>
    ) : null;
  }
  const subclass = ch.classId !== ch.baseClassId;
  return (
    <Section title="Character">
      <Facts
        rows={[
          [
            "Name",
            <>
              <span className="font-medium">{ch.name}</span>
              {ch.title && <span className="text-(--text-muted)"> “{ch.title}”</span>}
            </>,
          ],
          [
            "Class",
            `Lv ${ch.level} ${className(ch.classId)}${subclass ? ` (base: ${className(ch.baseClassId)})` : ""} · ${raceName(ch.race)}`,
          ],
          ["Clan", ch.clan],
          ["Access level", ch.accessLevel !== 0 ? accessLevel(ch.accessLevel) : null],
          [
            "Status",
            [ch.dead && "dead", ch.hero && "hero", ch.noble && "noblesse"]
              .filter(Boolean)
              .join(", ") || null,
          ],
          ["PvP / PK", `${ch.pvpKills} / ${ch.pkKills}`],
          ["Reputation", ch.reputation !== 0 ? ch.reputation.toString() : null],
          [
            "Position",
            ch.position ? mono(`${ch.position.x}, ${ch.position.y}, ${ch.position.z}`) : null,
          ],
          ["Object id", mono(ch.objectId.toString())],
        ]}
      />
      <div className="mt-3 space-y-1.5">
        <Bar label="CP" gauge={ch.cp} color="bg-amber-500" />
        <Bar label="HP" gauge={ch.hp} color="bg-red-500" />
        <Bar label="MP" gauge={ch.mp} color="bg-sky-500" />
      </div>
    </Section>
  );
}

function LobbySection({ c }: { c: ConnectedClient }) {
  const chars = c.details.lobbyCharacters;
  if (!chars) return null;
  return (
    <Section title={`Characters on the account (${chars.length})`}>
      {chars.length === 0 ? (
        <p className="text-(--text-muted)">None yet.</p>
      ) : (
        <ul className="space-y-1">
          {chars.map((ch) => (
            <li key={ch.name} className="flex justify-between gap-3">
              <span className="font-medium">{ch.name}</span>
              <span className="text-(--text-muted)">
                Lv {ch.level} {className(ch.classId)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </Section>
  );
}

function HardwareSection({ c }: { c: ConnectedClient }) {
  const hw = c.details.hardware;
  if (c.service !== "game_server") return null;
  return (
    <Section title="Hardware">
      {hw || c.hwid ? (
        <Facts
          rows={[
            ["HWID (MAC)", c.hwid ? mono(c.hwid) : null],
            ["CPU", hw ? `${hw.cpu} · ${hw.cpuCores} cores · ${hw.cpuSpeedMhz} MHz` : null],
            ["GPU", hw ? `${hw.gpu} (driver ${hw.gpuDriver})` : null],
            ["Windows", hw?.windows],
          ]}
        />
      ) : (
        <p className="text-(--text-muted)">
          Not reported. Hardware info and the HWID come from the client's RequestHardWareInfo
          packet, which the stock Interlude client does not send.
        </p>
      )}
    </Section>
  );
}
