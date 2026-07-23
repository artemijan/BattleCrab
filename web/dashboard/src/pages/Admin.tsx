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
import { Link, useNavigate, useParams } from "react-router-dom";

import {
  ApiError,
  api,
  type AdminGameAccount,
  type AdminMasterSummary,
  type AdminSortDir,
  type AdminSortKey,
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

/** Column definitions: label + which direction a fresh click starts with.
 *  Text sorts open ascending, count/date sorts open with the biggest first. */
const COLUMNS: Array<{ key: AdminSortKey; label: string; firstDir: AdminSortDir }> = [
  { key: "email", label: "Email", firstDir: "asc" },
  { key: "accessLevel", label: "Level", firstDir: "desc" },
  { key: "verified", label: "Verified", firstDir: "desc" },
  { key: "gameAccounts", label: "Game accts", firstDir: "desc" },
  { key: "characters", label: "Characters", firstDir: "desc" },
  { key: "lastActive", label: "Last active", firstDir: "desc" },
  { key: "created", label: "Created", firstDir: "desc" },
];

export function AdminAccounts() {
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [sort, setSort] = useState<AdminSortKey>("created");
  const [dir, setDir] = useState<AdminSortDir>("desc");

  const list = useQuery({
    queryKey: ["admin", "accounts", query, offset, sort, dir],
    queryFn: () => api.admin.accounts(query, offset, PAGE_SIZE, sort, dir),
    // Typing a new search resets paging; keeping the previous page on screen
    // while the next loads stops the list from flashing empty.
    placeholderData: (previous) => previous,
  });

  const total = list.data?.total ?? 0;
  const shownFrom = offset + 1;
  const shownTo = Math.min(offset + PAGE_SIZE, total);

  const onSort = (column: (typeof COLUMNS)[number]) => {
    if (sort === column.key) {
      setDir(dir === "asc" ? "desc" : "asc");
    } else {
      setSort(column.key);
      setDir(column.firstDir);
    }
    setOffset(0);
  };

  return (
    <div className="space-y-5 pb-6">
      <section className="animate-rise">
        <h1 className="text-3xl font-black tracking-tight">Accounts</h1>
        <p className="mt-1.5 text-[var(--text-muted)]">
          Every master account on the server. Search by email — or by game account username to find
          its owner.
        </p>
      </section>

      <CreateGameAccountPanel />

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
          {/* The table scrolls inside the panel on narrow screens rather than
              stretching the page. */}
          <Panel className="overflow-x-auto">
            <table className="w-full min-w-[44rem] text-sm">
              <thead>
                <tr className="border-b border-[var(--surface-border)] text-left">
                  {COLUMNS.map((column) => (
                    <th key={column.key} className="px-4 py-2.5 font-medium">
                      <button
                        type="button"
                        onClick={() => onSort(column)}
                        className={cx(
                          "inline-flex items-center gap-1 transition-colors hover:text-[var(--text)]",
                          sort === column.key ? "text-[var(--text)]" : "text-[var(--text-muted)]",
                        )}
                      >
                        {column.label}
                        <span aria-hidden className="text-xs">
                          {sort === column.key ? (dir === "asc" ? "▲" : "▼") : ""}
                        </span>
                      </button>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-[var(--surface-border)]">
                {list.data.accounts.map((master) => (
                  <tr
                    key={master.email}
                    onClick={() => navigate(`/admin/accounts/${encodeURIComponent(master.email)}`)}
                    className="cursor-pointer transition-colors hover:bg-[var(--surface-strong)]"
                  >
                    <td className="max-w-64 px-4 py-3">
                      {/* A real link so middle-click / copy address work; the
                          row onClick covers the rest of the row. */}
                      <Link
                        to={`/admin/accounts/${encodeURIComponent(master.email)}`}
                        onClick={(e) => e.stopPropagation()}
                        className="block truncate font-medium hover:underline"
                      >
                        {master.email}
                      </Link>
                    </td>
                    <td className="px-4 py-3">
                      {master.accessLevel < 0 ? (
                        <StatusBadge kind="bad">Banned</StatusBadge>
                      ) : master.accessLevel > 0 ? (
                        <StatusBadge kind="ok">{master.accessLevel}</StatusBadge>
                      ) : (
                        <span className="text-[var(--text-muted)]">0</span>
                      )}
                    </td>
                    <td className="px-4 py-3">
                      {master.isVerified ? (
                        <span className="text-[var(--text-muted)]">Yes</span>
                      ) : (
                        <StatusBadge kind="warn">No</StatusBadge>
                      )}
                    </td>
                    <td className="px-4 py-3 tabular-nums">{master.gameAccounts}</td>
                    <td className="px-4 py-3 tabular-nums">{master.characters}</td>
                    <td className="px-4 py-3 whitespace-nowrap text-[var(--text-muted)]">
                      {formatLastActive(master.lastActive)}
                    </td>
                    <td className="px-4 py-3 whitespace-nowrap text-[var(--text-muted)]">
                      {master.createdTime || "—"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
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

/**
 * Creates a game account under the admin's own master address. The server
 * copies the admin's accessLevel onto it, so what comes out is a GM game
 * account — hence the explicit warning in the form.
 */
function CreateGameAccountPanel() {
  const invalidate = useInvalidateAdmin();
  const [open, setOpen] = useState(false);
  const [login, setLogin] = useState("");
  const [password, setPassword] = useState("");

  const create = useMutation({
    mutationFn: () => api.admin.createGameAccount(login, password),
    onSuccess: () => {
      setLogin("");
      setPassword("");
      setOpen(false);
      invalidate();
    },
  });

  if (!open) {
    return (
      <div className="flex items-center gap-3">
        <Button
          variant="secondary"
          className="px-3 py-2 text-xs"
          onClick={() => {
            create.reset();
            setOpen(true);
          }}
        >
          Create GM game account…
        </Button>
        {create.isSuccess && (
          <span className="text-sm text-emerald-600 dark:text-emerald-300">
            Created — it appears under your own master account.
          </span>
        )}
      </div>
    );
  }

  return (
    <Panel className="p-5">
      <form
        onSubmit={(e: FormEvent) => {
          e.preventDefault();
          create.mutate();
        }}
        className="flex flex-wrap items-end gap-3"
      >
        {create.isError && (
          <div className="w-full">
            <Alert kind="error">
              {create.error instanceof ApiError && create.error.code === "login_taken"
                ? "That username is already taken."
                : errorMessage(create.error)}
            </Alert>
          </div>
        )}
        <div className="min-w-48 flex-1">
          <Field
            label="Game account username"
            value={login}
            onChange={(e) => setLogin(e.target.value)}
            autoComplete="off"
            required
          />
        </div>
        <div className="min-w-48 flex-1">
          <Field
            label="Password"
            type="text"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            hint="At least 8 characters."
            autoComplete="off"
            required
          />
        </div>
        <div className="flex gap-2 pb-6">
          <Button type="submit" loading={create.isPending} className="px-3 py-2 text-xs">
            Create
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
        <p className="w-full text-xs text-[var(--text-faint)]">
          The new account is created under your master account and copies your access level — it
          logs into the game as a GM.
        </p>
      </form>
    </Panel>
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
        <Button
          type="submit"
          variant="secondary"
          loading={reset.isPending}
          className="px-3 py-2 text-xs"
        >
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
