/**
 * The Audit page's IP ban list (docs/MONITORING.md §10), and the form the
 * client details use to ban an address.
 *
 * The login server checks this list on every new connection, so a ban stops
 * new logins at once; connections already open stay open unless the ban is
 * placed with "disconnect", which closes everything it covers on both
 * servers.
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type ReactNode, type SubmitEvent, useState } from "react";

import {
  ApiError,
  type ConnectedClient,
  type IpBan,
  type IpBanCreated,
  type IpBanInput,
  api,
} from "../lib/api";
import { BAN_DURATIONS, banCovers, banScope, formatExpiry } from "../lib/audit";
import { Alert, Button, Field, LabeledSelect, Panel, Spinner, StatusBadge, cx } from "./ui";

/** The default length offered for a new ban: long enough to matter, short
 *  enough that a shared or dynamic address does not stay banned forever. */
const DEFAULT_DURATION = "7 days";

export const IP_BANS_KEY = ["admin", "ip-bans"] as const;

/** `bannedBy` of the bans the servers place themselves. */
const AUTOMATIC: Record<string, string> = {
  game_server: "Game server (automatic)",
  login_server: "Login server (automatic)",
};

export function errorMessage(error: unknown): string {
  return error instanceof ApiError ? error.message : "Something went wrong.";
}

/** Refetch the bans and the connection list after a change: a ban with
 *  disconnect empties rows from the latter. */
export function useInvalidateAudit() {
  const queryClient = useQueryClient();
  return () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: IP_BANS_KEY }),
      queryClient.invalidateQueries({ queryKey: ["admin", "monitor", "clients"] }),
    ]);
}

/** "Disconnected 2 connections." and anything that could not be asked. */
export function createdSummary(created: IpBanCreated, disconnect: boolean): ReactNode {
  const n = created.disconnected;
  return (
    <>
      Banned {banScope(created.ban.ip)}
      {disconnect && `; disconnected ${n} connection${n === 1 ? "" : "s"}`}.
      {created.disconnectErrors.map((e) => (
        <span key={e} className="mt-1 block text-amber-700 dark:text-amber-300">
          Could not disconnect everywhere — {e}
        </span>
      ))}
    </>
  );
}

/* ---------------------------------- Form ---------------------------------- */

const KEEP = "keep";

/**
 * Address, length and reason of a ban. Used to add one, to edit one, and from
 * a client's details. `keepExpiry` (editing) offers leaving the expiry as it
 * is. `disconnect` says what happens to open connections the ban covers:
 * `ask` adds a checkbox, `always` says they will be closed.
 */
export function BanForm({
  initial,
  keepExpiry,
  disconnect: disconnectMode = "never",
  submitLabel,
  pending,
  error,
  now,
  clients,
  onSubmit,
  onCancel,
}: {
  initial: { ip: string; expiresAt?: number | null; reason?: string };
  keepExpiry?: number | null;
  disconnect?: "ask" | "always" | "never";
  submitLabel: string;
  pending: boolean;
  error: unknown;
  now: number;
  /** Live connections, to say who the ban covers right now. */
  clients: ConnectedClient[];
  onSubmit: (ban: IpBanInput, disconnect: boolean) => void;
  onCancel: () => void;
}) {
  const [ip, setIp] = useState(initial.ip);
  const [duration, setDuration] = useState(keepExpiry === undefined ? DEFAULT_DURATION : KEEP);
  const [reason, setReason] = useState(initial.reason ?? "");
  const [disconnect, setDisconnect] = useState(true);

  const trimmed = ip.trim();
  const covered = trimmed ? clients.filter((c) => banCovers(trimmed, c.ip)) : [];

  const submit = (e: SubmitEvent) => {
    e.preventDefault();
    const ms = BAN_DURATIONS.find((d) => d.label === duration)?.ms;
    const expiresAt =
      duration === KEEP ? (keepExpiry ?? null) : ms === null || ms === undefined ? null : now + ms;
    const kick = disconnectMode === "always" || (disconnectMode === "ask" && disconnect);
    onSubmit({ ip: trimmed, expiresAt, reason: reason.trim() }, kick);
  };

  return (
    <form onSubmit={submit} className="space-y-3">
      {error != null && <Alert kind="error">{errorMessage(error)}</Alert>}
      <div className="grid gap-3 sm:grid-cols-2">
        <Field
          label="IP address"
          value={ip}
          onChange={(e) => setIp(e.target.value)}
          hint={
            trimmed && banScope(trimmed) !== trimmed
              ? `Covers ${banScope(trimmed)}`
              : "End in .0 to ban a range: 10.1.2.0 is 10.1.2.*"
          }
          autoComplete="off"
          spellCheck={false}
          required
          className="font-mono"
        />
        <LabeledSelect
          label="Length"
          value={duration}
          onChange={(e) => setDuration(e.target.value)}
        >
          {keepExpiry !== undefined && (
            <option value={KEEP}>Keep current ({formatExpiry(keepExpiry, now)})</option>
          )}
          {BAN_DURATIONS.map((d) => (
            <option key={d.label} value={d.label}>
              {d.label}
            </option>
          ))}
        </LabeledSelect>
      </div>
      <Field
        label="Reason"
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        placeholder="Optional — kept with the ban and in the GM audit log"
        maxLength={255}
        autoComplete="off"
      />
      {disconnectMode === "ask" && (
        <label className="flex items-center gap-2 text-sm text-(--text-muted)">
          <input
            type="checkbox"
            checked={disconnect}
            onChange={(e) => setDisconnect(e.target.checked)}
          />
          Disconnect everyone it covers now
          {covered.length > 0 && ` (${covered.length} connected)`}
        </label>
      )}
      {disconnectMode === "always" && covered.length > 1 && (
        <p className="text-xs text-(--text-muted)">
          {covered.length} connections are on {banScope(trimmed)} right now; all of them will be
          disconnected.
        </p>
      )}
      <div className="flex flex-wrap gap-2">
        <Button
          type="submit"
          variant="ghost"
          loading={pending}
          className={cx(
            "px-3 py-2 text-xs",
            keepExpiry === undefined && "text-red-500 dark:text-red-400",
          )}
        >
          {submitLabel}
        </Button>
        <Button type="button" variant="ghost" onClick={onCancel} className="px-3 py-2 text-xs">
          Cancel
        </Button>
      </div>
    </form>
  );
}

/* --------------------------------- Section -------------------------------- */

export function IpBansSection({ clients }: { clients: ConnectedClient[] }) {
  const list = useQuery({ queryKey: IP_BANS_KEY, queryFn: api.admin.ipBans.list });
  const invalidate = useInvalidateAudit();
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);
  const [notice, setNotice] = useState<ReactNode>(null);

  const create = useMutation({
    mutationFn: (v: { ban: IpBanInput; disconnect: boolean }) =>
      api.admin.ipBans.create(v.ban, v.disconnect),
    onSuccess: (created, v) => {
      setAdding(false);
      setNotice(createdSummary(created, v.disconnect));
      return invalidate();
    },
  });

  const now = list.data?.nowMs ?? Date.now();
  const bans = list.data?.bans ?? [];

  return (
    <section className="space-y-3 pt-4">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 className="text-xl font-bold tracking-tight">Banned IPs</h2>
          <p className="mt-1 text-sm text-(--text-muted)">
            The login server refuses these addresses on every new connection. A ban does not close
            connections already open unless it is placed with "disconnect".
          </p>
        </div>
        {!adding && (
          <Button
            variant="secondary"
            onClick={() => {
              create.reset();
              setNotice(null);
              setEditing(null);
              setAdding(true);
            }}
            className="px-3 py-2 text-xs"
          >
            Ban an IP…
          </Button>
        )}
      </div>

      {notice && !adding && <Alert kind="success">{notice}</Alert>}

      {adding && (
        <Panel className="p-5">
          <BanForm
            initial={{ ip: "" }}
            disconnect="ask"
            submitLabel="Ban"
            pending={create.isPending}
            error={create.error}
            now={now}
            clients={clients}
            onSubmit={(ban, disconnect) => create.mutate({ ban, disconnect })}
            onCancel={() => setAdding(false)}
          />
        </Panel>
      )}

      {editing !== null && (
        <EditBanPanel
          key={editing}
          ban={bans.find((b) => b.ip === editing)}
          now={now}
          clients={clients}
          onDone={() => setEditing(null)}
        />
      )}

      {list.isError ? (
        <Alert kind="error">{errorMessage(list.error)}</Alert>
      ) : list.isPending ? (
        <Panel className="flex items-center gap-3 p-6 text-sm text-(--text-muted)">
          <Spinner /> Loading bans…
        </Panel>
      ) : bans.length === 0 ? (
        <Panel className="p-8 text-center text-sm text-(--text-muted)">No IP is banned.</Panel>
      ) : (
        <Panel className="overflow-x-auto">
          <table className="w-full min-w-200 text-sm">
            <thead>
              <tr className="border-b border-(--surface-border) text-left text-(--text-muted)">
                <th className="px-4 py-2.5 font-medium">IP</th>
                <th className="px-4 py-2.5 font-medium">Expires</th>
                <th className="px-4 py-2.5 font-medium">Reason</th>
                <th className="px-4 py-2.5 font-medium">Banned by</th>
                <th className="px-4 py-2.5 font-medium">
                  <span className="sr-only">Actions</span>
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-(--surface-border)">
              {bans.map((ban) => (
                <BanRow
                  key={ban.ip}
                  ban={ban}
                  now={now}
                  clients={clients}
                  onEdit={() => {
                    setAdding(false);
                    setNotice(null);
                    setEditing(ban.ip);
                  }}
                />
              ))}
            </tbody>
          </table>
        </Panel>
      )}
    </section>
  );
}

/** Edits one ban above the table, where a phone has room for the form. */
function EditBanPanel({
  ban,
  now,
  clients,
  onDone,
}: {
  ban: IpBan | undefined;
  now: number;
  clients: ConnectedClient[];
  onDone: () => void;
}) {
  const invalidate = useInvalidateAudit();
  const update = useMutation({
    mutationFn: (input: IpBanInput) => api.admin.ipBans.update(ban?.ip ?? "", input),
    onSuccess: async () => {
      await invalidate();
      onDone();
    },
  });
  // Lifted elsewhere meanwhile: nothing left to edit.
  if (!ban) return null;
  return (
    <Panel className="p-5">
      <p className="mb-3 text-sm font-semibold">
        Editing the ban on <span className="font-mono">{ban.ip}</span>
      </p>
      <BanForm
        initial={ban}
        keepExpiry={ban.expiresAt}
        submitLabel="Save"
        pending={update.isPending}
        error={update.error}
        now={now}
        clients={clients}
        onSubmit={(input) => update.mutate(input)}
        onCancel={onDone}
      />
    </Panel>
  );
}

function BanRow({
  ban,
  now,
  clients,
  onEdit,
}: {
  ban: IpBan;
  now: number;
  clients: ConnectedClient[];
  onEdit: () => void;
}) {
  const invalidate = useInvalidateAudit();
  const [confirmRemove, setConfirmRemove] = useState(false);

  const remove = useMutation({
    mutationFn: () => api.admin.ipBans.remove(ban.ip),
    onSuccess: invalidate,
  });

  const expired = ban.expiresAt !== null && ban.expiresAt <= now;
  const connected = clients.filter((c) => banCovers(ban.ip, c.ip)).length;

  return (
    <tr className={cx(expired && "opacity-60")}>
      <td className="px-4 py-3 whitespace-nowrap">
        <span className="font-mono text-xs">{ban.ip}</span>
        {banScope(ban.ip) !== ban.ip && (
          <span className="ml-1.5 text-xs text-(--text-muted)">({banScope(ban.ip)})</span>
        )}
        {connected > 0 && (
          <span
            className="ml-1.5"
            title="Connections open from this address right now; the ban only refuses new ones"
          >
            <StatusBadge kind="warn">{connected} connected</StatusBadge>
          </span>
        )}
      </td>
      <td
        className="px-4 py-3 whitespace-nowrap"
        title={ban.expiresAt === null ? undefined : new Date(ban.expiresAt).toLocaleString()}
      >
        {expired ? (
          <StatusBadge kind="neutral">{formatExpiry(ban.expiresAt, now)}</StatusBadge>
        ) : ban.expiresAt === null ? (
          <StatusBadge kind="bad">Permanent</StatusBadge>
        ) : (
          formatExpiry(ban.expiresAt, now)
        )}
      </td>
      <td className="max-w-64 truncate px-4 py-3" title={ban.reason || undefined}>
        {ban.reason || <span className="text-(--text-faint)">—</span>}
      </td>
      <td
        className="px-4 py-3 whitespace-nowrap text-(--text-muted)"
        title={`Since ${new Date(ban.createdAt).toLocaleString()}`}
      >
        {AUTOMATIC[ban.bannedBy] ?? (ban.bannedBy || "—")}
      </td>
      <td className="px-4 py-3 text-right whitespace-nowrap">
        {remove.isError && (
          <span role="alert" className="mr-2 text-xs text-red-500 dark:text-red-400">
            {errorMessage(remove.error)}
          </span>
        )}
        {confirmRemove ? (
          <span className="inline-flex items-center gap-1.5">
            <span className="text-xs text-(--text-muted)">Lift this ban?</span>
            <Button
              variant="ghost"
              loading={remove.isPending}
              onClick={() => remove.mutate()}
              className="px-2.5 py-1.5 text-xs"
            >
              Lift
            </Button>
            <Button
              variant="ghost"
              onClick={() => setConfirmRemove(false)}
              className="px-2.5 py-1.5 text-xs"
            >
              Keep
            </Button>
          </span>
        ) : (
          <span className="inline-flex gap-1.5">
            <Button variant="ghost" onClick={onEdit} className="px-2.5 py-1.5 text-xs">
              Edit
            </Button>
            <Button
              variant="ghost"
              onClick={() => {
                remove.reset();
                setConfirmRemove(true);
              }}
              className="px-2.5 py-1.5 text-xs text-red-500 dark:text-red-400"
            >
              Remove
            </Button>
          </span>
        )}
      </td>
    </tr>
  );
}
