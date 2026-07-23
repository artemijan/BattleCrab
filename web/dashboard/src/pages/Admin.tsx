/**
 * Admin pages: the master-account list (`/admin`) and one account's detail
 * (`/admin/accounts/:email`).
 *
 * Everything here is a thin view over `/api/v1/admin/*` — authorization lives
 * entirely server-side. Two properties of that API shape this UI:
 *
 * - Access levels only ever go DOWN from the dashboard (0 restores, negative
 *   bans; the server refuses anything positive), so the only controls offered
 *   are Ban / Unban — a "promote" control could only ever collect a 400.
 * - Characters are read-only (live state is memory-first in the game server),
 *   so they are displayed and nothing more.
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent, type ReactNode } from "react";
import { Link, useParams } from "react-router-dom";

import {
  ApiError,
  api,
  type AdminGameAccount,
  type AdminMasterSummary,
  type Character,
} from "../lib/api";
import { Alert, Button, Field, Panel, Spinner, cx } from "../components/ui";

const PAGE_SIZE = 25;

/** The value written by the Ban button. -100 mirrors the game's own ban level. */
const BAN_LEVEL = -100;

function formatLastActive(millis: number): string {
  if (!millis) return "never";
  return new Date(millis).toLocaleDateString();
}

function errorMessage(error: unknown): string {
  if (!(error instanceof ApiError)) return "Something went wrong.";
  if (error.code === "forbidden")
    return "Not allowed — you can't modify an account at or above your own level.";
  return error.message;
}

/* -------------------------------------------------------------------------- */
/* Shared bits                                                                */
/* -------------------------------------------------------------------------- */

function StatusBadge({ kind, children }: { kind: "ok" | "warn" | "bad"; children: ReactNode }) {
  const styles = {
    ok: "bg-emerald-500/15 text-emerald-600 dark:text-emerald-300",
    warn: "bg-amber-500/15 text-amber-600 dark:text-amber-300",
    bad: "bg-red-500/15 text-red-600 dark:text-red-300",
  } as const;
  return (
    <span
      className={cx(
        "inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium",
        styles[kind],
      )}
    >
      {children}
    </span>
  );
}

function MasterBadges({ master }: { master: AdminMasterSummary }) {
  return (
    <>
      {master.accessLevel < 0 && <StatusBadge kind="bad">Banned</StatusBadge>}
      {master.accessLevel > 0 && <StatusBadge kind="ok">Level {master.accessLevel}</StatusBadge>}
      {!master.isVerified && <StatusBadge kind="warn">Unverified</StatusBadge>}
    </>
  );
}

/* -------------------------------------------------------------------------- */
/* /admin — the account list                                                  */
/* -------------------------------------------------------------------------- */

export function AdminAccounts() {
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);

  const list = useQuery({
    queryKey: ["admin", "accounts", query, offset],
    queryFn: () => api.admin.accounts(query, offset, PAGE_SIZE),
    // Typing a new search resets paging; keeping the previous page on screen
    // while the next loads stops the list from flashing empty.
    placeholderData: (previous) => previous,
  });

  const total = list.data?.total ?? 0;
  const shownFrom = offset + 1;
  const shownTo = Math.min(offset + PAGE_SIZE, total);

  return (
    <div className="space-y-5 pb-6">
      <section className="animate-rise">
        <h1 className="text-3xl font-black tracking-tight">Accounts</h1>
        <p className="mt-1.5 text-[var(--text-muted)]">
          Every master account on the server. Search by email — or by game account username to find
          its owner.
        </p>
      </section>

      <Field
        label="Search"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setOffset(0);
        }}
        placeholder="email or game account username"
        autoComplete="off"
      />

      {list.isPending ? (
        <Panel className="flex items-center gap-3 p-6 text-sm text-[var(--text-muted)]">
          <Spinner /> Loading accounts…
        </Panel>
      ) : list.isError ? (
        <Alert kind="error">{errorMessage(list.error)}</Alert>
      ) : list.data.accounts.length === 0 ? (
        <Panel className="p-8 text-center text-sm text-[var(--text-muted)]">
          No accounts match{query ? <> “{query}”</> : null}.
        </Panel>
      ) : (
        <>
          <Panel className="overflow-hidden">
            <ul className="divide-y divide-[var(--surface-border)]">
              {list.data.accounts.map((master) => (
                <li key={master.email}>
                  <Link
                    to={`/admin/accounts/${encodeURIComponent(master.email)}`}
                    className="flex flex-wrap items-center gap-x-4 gap-y-1 px-5 py-3.5
                               transition-colors hover:bg-[var(--surface-strong)]"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <p className="truncate font-medium">{master.email}</p>
                        <MasterBadges master={master} />
                      </div>
                      <p className="mt-0.5 text-sm text-[var(--text-muted)]">
                        {master.gameAccounts} game account{master.gameAccounts === 1 ? "" : "s"} ·{" "}
                        {master.characters} character{master.characters === 1 ? "" : "s"} · last
                        active {formatLastActive(master.lastActive)}
                      </p>
                    </div>
                  </Link>
                </li>
              ))}
            </ul>
          </Panel>

          <div className="flex items-center justify-between text-sm text-[var(--text-muted)]">
            <span>
              {shownFrom}–{shownTo} of {total}
            </span>
            <div className="flex gap-2">
              <Button
                variant="ghost"
                className="px-3 py-1.5 text-xs"
                disabled={offset === 0}
                onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
              >
                Previous
              </Button>
              <Button
                variant="ghost"
                className="px-3 py-1.5 text-xs"
                disabled={shownTo >= total}
                onClick={() => setOffset(offset + PAGE_SIZE)}
              >
                Next
              </Button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* /admin/accounts/:email — one account                                       */
/* -------------------------------------------------------------------------- */

export function AdminAccountDetail() {
  const { email = "" } = useParams();

  const detail = useQuery({
    queryKey: ["admin", "account", email],
    queryFn: () => api.admin.account(email),
  });

  return (
    <div className="space-y-5 pb-6">
      <section className="animate-rise">
        <Link to="/admin" className="text-sm text-[var(--text-muted)] hover:text-[var(--text)]">
          ← All accounts
        </Link>
        <h1 className="mt-1 break-all text-3xl font-black tracking-tight">{email}</h1>
      </section>

      {detail.isPending ? (
        <Panel className="flex items-center gap-3 p-6 text-sm text-[var(--text-muted)]">
          <Spinner /> Loading account…
        </Panel>
      ) : detail.isError ? (
        <Alert kind="error">
          {detail.error instanceof ApiError && detail.error.code === "not_found"
            ? "No master account exists at this address."
            : errorMessage(detail.error)}
        </Alert>
      ) : (
        <>
          <MasterCard master={detail.data.master} />

          <section>
            <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-[var(--text-faint)]">
              Game accounts
            </h2>
            {detail.data.gameAccounts.length === 0 ? (
              <Panel className="p-6 text-sm text-[var(--text-muted)]">
                No game accounts under this address.
              </Panel>
            ) : (
              <div className="space-y-3">
                {detail.data.gameAccounts.map((gameAccount) => (
                  <GameAccountCard
                    key={gameAccount.login}
                    gameAccount={gameAccount}
                    characters={detail.data.characters.filter(
                      (c) => c.accountName === gameAccount.login,
                    )}
                  />
                ))}
              </div>
            )}
          </section>
        </>
      )}
    </div>
  );
}

/** Invalidate both the detail view and any cached list pages after a change. */
function useInvalidateAdmin() {
  const queryClient = useQueryClient();
  return () => queryClient.invalidateQueries({ queryKey: ["admin"] });
}

function MasterCard({ master }: { master: AdminMasterSummary }) {
  const invalidate = useInvalidateAdmin();

  const verify = useMutation({
    mutationFn: () => api.admin.verifyMaster(master.email),
    onSuccess: invalidate,
  });
  const setLevel = useMutation({
    mutationFn: (level: number) => api.admin.setMasterAccessLevel(master.email, level),
    onSuccess: invalidate,
  });

  const banned = master.accessLevel < 0;
  const error = verify.error ?? setLevel.error;

  return (
    <Panel className="p-6">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="font-semibold">Master account</h2>
        <MasterBadges master={master} />
      </div>
      <p className="mt-1 text-sm text-[var(--text-muted)]">
        Created {master.createdTime || "unknown"} · last active{" "}
        {formatLastActive(master.lastActive)}
      </p>

      {error != null && (
        <div className="mt-3">
          <Alert kind="error">{errorMessage(error)}</Alert>
        </div>
      )}

      <div className="mt-4 flex flex-wrap gap-2">
        {!master.isVerified && (
          <Button
            variant="secondary"
            loading={verify.isPending}
            onClick={() => verify.mutate()}
            className="px-3 py-2 text-xs"
          >
            Mark verified
          </Button>
        )}
        <Button
          variant="ghost"
          loading={setLevel.isPending}
          onClick={() => setLevel.mutate(banned ? 0 : BAN_LEVEL)}
          className={cx("px-3 py-2 text-xs", !banned && "text-red-500 dark:text-red-400")}
        >
          {banned ? "Lift dashboard ban" : "Ban from dashboard"}
        </Button>
      </div>
      <p className="mt-2 text-xs text-[var(--text-faint)]">
        A dashboard ban blocks signing in here and kills open sessions; the game accounts below keep
        working unless banned individually.
      </p>
    </Panel>
  );
}

function GameAccountCard({
  gameAccount,
  characters,
}: {
  gameAccount: AdminGameAccount;
  characters: Character[];
}) {
  const invalidate = useInvalidateAdmin();
  const banned = gameAccount.accessLevel < 0;

  const setLevel = useMutation({
    mutationFn: (level: number) => api.admin.setGameAccountAccessLevel(gameAccount.login, level),
    onSuccess: invalidate,
  });

  return (
    <Panel className="overflow-hidden">
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-[var(--surface-border)] px-5 py-3.5">
        <div className="flex min-w-0 items-center gap-2.5">
          <span
            className="grid size-8 shrink-0 place-items-center rounded-lg bg-brand-500/12 text-xs
                       font-black text-brand-600 ring-1 ring-brand-500/20 dark:text-brand-200"
            aria-hidden
          >
            {gameAccount.login.charAt(0).toUpperCase()}
          </span>
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <p className="truncate font-semibold">{gameAccount.login}</p>
              {banned && <StatusBadge kind="bad">Banned</StatusBadge>}
              {gameAccount.accessLevel > 0 && (
                <StatusBadge kind="ok">Level {gameAccount.accessLevel}</StatusBadge>
              )}
            </div>
            <p className="text-xs text-[var(--text-faint)]">
              last active {formatLastActive(gameAccount.lastActive)} · last IP{" "}
              {gameAccount.lastIp ?? "—"}
            </p>
          </div>
        </div>
        <Button
          variant="ghost"
          loading={setLevel.isPending}
          onClick={() => setLevel.mutate(banned ? 0 : BAN_LEVEL)}
          className={cx("px-3 py-1.5 text-xs", !banned && "text-red-500 dark:text-red-400")}
        >
          {banned ? "Unban" : "Ban"}
        </Button>
      </div>

      {setLevel.isError && (
        <div className="px-5 pt-3">
          <Alert kind="error">{errorMessage(setLevel.error)}</Alert>
        </div>
      )}

      {characters.length === 0 ? (
        <p className="px-5 py-4 text-sm text-[var(--text-muted)]">No characters.</p>
      ) : (
        <ul className="divide-y divide-[var(--surface-border)]">
          {characters.map((character) => (
            <li key={character.name} className="flex items-center gap-3 px-5 py-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <p className="truncate font-medium">{character.name}</p>
                  {character.online && <StatusBadge kind="ok">Online</StatusBadge>}
                </div>
                <p className="mt-0.5 text-sm text-[var(--text-muted)]">
                  Level {character.level} · {Math.floor(character.onlineTime / 3600)}h played
                </p>
              </div>
            </li>
          ))}
        </ul>
      )}

      <ResetPasswordRow login={gameAccount.login} />
    </Panel>
  );
}

/**
 * Admin password reset for a game account — the support path for "player lost
 * access". Masters have no equivalent on purpose: their recovery goes through
 * the email flow, which proves inbox control instead of trusting an admin.
 */
function ResetPasswordRow({ login }: { login: string }) {
  const [open, setOpen] = useState(false);
  const [password, setPassword] = useState("");

  const reset = useMutation({
    mutationFn: () => api.admin.setGameAccountPassword(login, password),
    onSuccess: () => {
      setPassword("");
      setOpen(false);
    },
  });

  if (!open) {
    return (
      <div className="border-t border-[var(--surface-border)] px-5 py-3">
        {reset.isSuccess ? (
          <span className="text-sm text-emerald-600 dark:text-emerald-300">
            Password reset — it works in the game client immediately.
          </span>
        ) : (
          <button
            type="button"
            onClick={() => {
              reset.reset();
              setOpen(true);
            }}
            className="text-sm text-[var(--text-muted)] transition-colors hover:text-[var(--text)]"
          >
            Reset password…
          </button>
        )}
      </div>
    );
  }

  return (
    <form
      onSubmit={(e: FormEvent) => {
        e.preventDefault();
        reset.mutate();
      }}
      className="flex flex-wrap items-end gap-3 border-t border-[var(--surface-border)] px-5 py-4"
    >
      {reset.isError && (
        <div className="w-full">
          <Alert kind="error">{errorMessage(reset.error)}</Alert>
        </div>
      )}
      <div className="min-w-56 flex-1">
        <Field
          label={`New password for ${login}`}
          type="text"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          hint="At least 8 characters. Tell the player to change it after logging in."
          autoComplete="off"
          required
        />
      </div>
      <div className="flex gap-2 pb-6">
        <Button type="submit" variant="secondary" loading={reset.isPending} className="px-3 py-2 text-xs">
          Set password
        </Button>
        <Button
          type="button"
          variant="ghost"
          onClick={() => setOpen(false)}
          className="px-3 py-2 text-xs"
        >
          Cancel
        </Button>
      </div>
    </form>
  );
}
