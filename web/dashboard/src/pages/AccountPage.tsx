import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";

import { ApiError, api, type Character } from "../lib/api";
import { Alert, Button, Field, Panel, Spinner } from "../components/ui";

/** Interlude has five playable races; the ids are the datapack's own. */
const RACES = ["Human", "Elf", "Dark Elf", "Orc", "Dwarf"] as const;

function raceName(race: number): string {
  return RACES[race] ?? "Unknown";
}

function playtime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  if (hours < 1) return "< 1h played";
  if (hours < 24) return `${hours}h played`;
  return `${Math.floor(hours / 24)}d ${hours % 24}h played`;
}

export function AccountPage() {
  return (
    <div className="space-y-5 pb-6">
      <section className="animate-rise">
        <h1 className="text-3xl font-black tracking-tight">Your account</h1>
        <p className="mt-1.5 text-[var(--text-muted)]">
          Manage your game accounts, characters and credentials.
        </p>
      </section>

      <UnverifiedEmailBanner />

      <GameAccountsSection />

      <section className="grid gap-4 lg:grid-cols-2">
        <ChangePasswordCard />
        <ChangeEmailCard />
      </section>
    </div>
  );
}

/**
 * Game accounts, each with the characters that live on it.
 *
 * The two lists are fetched separately and joined here rather than server-side:
 * a game account with no characters has to appear too (it is the thing you have
 * just created and want to log in with), so the character list alone cannot
 * drive this view.
 */
function GameAccountsSection() {
  const me = useQuery({ queryKey: ["me"], queryFn: api.me });
  const gameAccounts = useQuery({ queryKey: ["gameAccounts"], queryFn: api.gameAccounts });
  const characters = useQuery({ queryKey: ["characters"], queryFn: api.characters });

  const isPending = gameAccounts.isPending || characters.isPending;
  const isError = gameAccounts.isError || characters.isError;

  const charactersByAccount = new Map<string, Character[]>();
  for (const character of characters.data ?? []) {
    const existing = charactersByAccount.get(character.accountName);
    if (existing) existing.push(character);
    else charactersByAccount.set(character.accountName, [character]);
  }

  return (
    <section>
      <div className="mb-3 flex items-end justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold uppercase tracking-wide text-[var(--text-faint)]">
            Game accounts
          </h2>
          <p className="mt-1 text-sm text-[var(--text-muted)]">
            These are the usernames you log into the game client with.
          </p>
        </div>
      </div>

      {isPending ? (
        <Panel className="flex items-center gap-3 p-6 text-sm text-[var(--text-muted)]">
          <Spinner /> Loading your game accounts…
        </Panel>
      ) : isError ? (
        <Alert kind="error">Couldn't load your game accounts. Try refreshing.</Alert>
      ) : (
        <div className="stagger space-y-3">
          {gameAccounts.data!.length === 0 ? (
            <Panel className="p-8 text-center">
              <p className="font-medium">No game accounts yet</p>
              <p className="mt-1.5 text-sm text-[var(--text-muted)]">
                Create one below, then use it to log into the game client. Your characters will
                appear here.
              </p>
            </Panel>
          ) : (
            gameAccounts.data!.map((login) => (
              <GameAccountPanel
                key={login}
                login={login}
                characters={charactersByAccount.get(login) ?? []}
              />
            ))
          )}

          <CreateGameAccountCard
            isVerified={me.data?.isVerified ?? false}
            count={gameAccounts.data!.length}
          />
        </div>
      )}
    </section>
  );
}

function GameAccountPanel({ login, characters }: { login: string; characters: Character[] }) {
  return (
    <Panel className="overflow-hidden">
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-[var(--surface-border)] px-5 py-3.5">
        <div className="flex items-center gap-2.5">
          <span
            className="grid size-8 shrink-0 place-items-center rounded-lg bg-brand-500/12 text-xs font-black
                       text-brand-600 ring-1 ring-brand-500/20 dark:text-brand-200"
            aria-hidden
          >
            {login.charAt(0).toUpperCase()}
          </span>
          <p className="font-semibold">{login}</p>
        </div>
        <p className="text-xs text-[var(--text-faint)]">
          {characters.length === 0
            ? "No characters"
            : `${characters.length} character${characters.length === 1 ? "" : "s"}`}
        </p>
      </div>

      {characters.length === 0 ? (
        <p className="px-5 py-5 text-sm text-[var(--text-muted)]">
          Log into the game client as <span className="font-medium">{login}</span> to create your
          first character.
        </p>
      ) : (
        <ul className="divide-y divide-[var(--surface-border)]">
          {characters.map((character) => (
            <CharacterRow key={character.name} character={character} />
          ))}
        </ul>
      )}
    </Panel>
  );
}

function CharacterRow({ character }: { character: Character }) {
  return (
    <li className="flex items-center gap-4 px-5 py-3.5">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <p className="truncate font-medium">{character.name}</p>
          {character.online && (
            <span
              className="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5
                         text-[11px] font-medium text-emerald-600 dark:text-emerald-300"
            >
              <span className="size-1.5 rounded-full bg-emerald-400" aria-hidden />
              Online
            </span>
          )}
        </div>
        {/* The owning account is the panel heading now, so it is not repeated. */}
        <p className="mt-0.5 truncate text-sm text-[var(--text-muted)]">
          Level {character.level} · {raceName(character.race)} · {playtime(character.onlineTime)}
        </p>
      </div>
    </li>
  );
}

/**
 * Creating a game account requires a confirmed address: ownership of everything
 * created here is recorded only by the shared address, so the server refuses
 * until it is proven. Saying so up front beats letting the user fill the form
 * and collect a 403.
 */
function CreateGameAccountCard({ isVerified, count }: { isVerified: boolean; count: number }) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [login, setLogin] = useState("");
  const [password, setPassword] = useState("");

  const submit = useMutation({
    mutationFn: () => api.createGameAccount(login, password),
    onSuccess: () => {
      setLogin("");
      setPassword("");
      setOpen(false);
      queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
      queryClient.invalidateQueries({ queryKey: ["characters"] });
    },
  });

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    submit.mutate();
  };

  if (!isVerified) {
    return (
      <Panel className="p-6">
        <h3 className="font-semibold">Create a game account</h3>
        <p className="mt-1.5 text-sm text-[var(--text-muted)]">
          Confirm your email address first — it's the only record of which game accounts are yours,
          and the only way to recover them.
        </p>
      </Panel>
    );
  }

  if (!open) {
    return (
      <Panel className="flex flex-wrap items-center justify-between gap-3 p-5">
        <div>
          <h3 className="font-semibold">Create a game account</h3>
          <p className="mt-0.5 text-sm text-[var(--text-muted)]">
            {count === 0
              ? "You'll need one to log into the game."
              : "Add another username for the game client."}
          </p>
        </div>
        {submit.isSuccess && <Alert kind="success">Game account created.</Alert>}
        <Button type="button" onClick={() => setOpen(true)}>
          New game account
        </Button>
      </Panel>
    );
  }

  return (
    <Panel className="animate-rise p-6">
      <h3 className="font-semibold">Create a game account</h3>
      <p className="mt-1 text-sm text-[var(--text-muted)]">
        This username and password are what you type into the game client — they're separate from
        the ones you sign in here with.
      </p>

      <form onSubmit={onSubmit} className="mt-4 flex flex-col gap-3.5">
        {submit.isError && <Alert kind="error">{createErrorMessage(submit.error)}</Alert>}

        <Field
          label="Username"
          value={login}
          onChange={(e) => setLogin(e.target.value)}
          hint="4–45 letters and digits. It can't be changed later."
          autoComplete="off"
          required
        />
        <Field
          label="Password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          hint="At least 8 characters."
          autoComplete="new-password"
          required
        />

        <div className="flex gap-2">
          <Button type="submit" loading={submit.isPending}>
            Create account
          </Button>
          <Button type="button" variant="ghost" onClick={() => setOpen(false)}>
            Cancel
          </Button>
        </div>
      </form>
    </Panel>
  );
}

function createErrorMessage(error: unknown): string {
  if (!(error instanceof ApiError)) return "Something went wrong.";
  switch (error.code) {
    case "login_taken":
      return "That username is already taken. Try another.";
    case "email_not_verified":
      return "Confirm your email address before creating game accounts.";
    // too_many_game_accounts and bad_request both carry a message naming the
    // actual limit or rule, which beats anything restated here.
    default:
      return error.message;
  }
}

/**
 * An unverified address can still sign in, so nothing blocks the user — but it
 * is the account's only identity and its only password-recovery route, which
 * makes leaving it unconfirmed worth being loud about.
 */
function UnverifiedEmailBanner() {
  const me = useQuery({ queryKey: ["me"], queryFn: api.me });
  const resend = useMutation({ mutationFn: api.resendVerification });

  if (!me.data || me.data.isVerified) return null;

  return (
    <Alert kind="error">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span>
          Confirm {me.data.email} to secure your account — without it you can't reset your password.
        </span>
        {resend.isSuccess ? (
          <span className="text-sm font-medium">Sent — check your inbox.</span>
        ) : (
          <Button
            type="button"
            variant="secondary"
            loading={resend.isPending}
            onClick={() => resend.mutate()}
          >
            Resend link
          </Button>
        )}
      </div>
    </Alert>
  );
}

function ChangePasswordCard() {
  const queryClient = useQueryClient();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");

  const submit = useMutation({
    mutationFn: () => api.changePassword(current, next),
    onSuccess: () => {
      setCurrent("");
      setNext("");
      // The server re-issued this browser's cookie, but any other session is
      // now dead — refetch so the UI reflects reality if that included us.
      queryClient.invalidateQueries({ queryKey: ["me"] });
    },
  });

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    submit.mutate();
  };

  return (
    <Panel className="animate-rise p-6">
      <h2 className="font-semibold">Change password</h2>
      <p className="mt-1 text-sm text-[var(--text-muted)]">
        This changes your game password too — and signs out every other device.
      </p>

      <form onSubmit={onSubmit} className="mt-4 flex flex-col gap-3.5">
        {submit.isError && (
          <Alert kind="error">
            {submit.error instanceof ApiError && submit.error.code === "invalid_credentials"
              ? "Your current password isn't right."
              : submit.error instanceof ApiError
                ? submit.error.message
                : "Something went wrong."}
          </Alert>
        )}
        {submit.isSuccess && <Alert kind="success">Password updated.</Alert>}

        <Field
          label="Current password"
          type="password"
          value={current}
          onChange={(e) => setCurrent(e.target.value)}
          autoComplete="current-password"
          required
        />
        <Field
          label="New password"
          type="password"
          value={next}
          onChange={(e) => setNext(e.target.value)}
          hint="At least 8 characters."
          autoComplete="new-password"
          required
        />
        <Button type="submit" variant="secondary" loading={submit.isPending} className="self-start">
          Update password
        </Button>
      </form>
    </Panel>
  );
}

function ChangeEmailCard() {
  const me = useQuery({ queryKey: ["me"], queryFn: api.me });
  const [email, setEmail] = useState("");

  const submit = useMutation({
    mutationFn: () => api.changeEmail(email),
    onSuccess: () => setEmail(""),
  });

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    submit.mutate();
  };

  return (
    <Panel className="animate-rise p-6">
      <h2 className="font-semibold">Email address</h2>
      <p className="mt-1 text-sm text-[var(--text-muted)]">
        {me.data?.email
          ? `Currently ${me.data.email}. This is how you sign in.`
          : "Add an email so you can reset your password if you forget it."}
      </p>

      <form onSubmit={onSubmit} className="mt-4 flex flex-col gap-3.5">
        {submit.isError && (
          <Alert kind="error">
            {submit.error instanceof ApiError ? submit.error.message : "Something went wrong."}
          </Alert>
        )}
        {/* The move happens only once the link is clicked: the address is the
            login, so switching before it is proven would lock the user out. */}
        {submit.isSuccess && (
          <Alert kind="success">
            Check your inbox — you'll sign in with the new address once you click the link.
          </Alert>
        )}

        <Field
          label="New email"
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          autoComplete="email"
          required
        />
        <Button type="submit" variant="secondary" loading={submit.isPending} className="self-start">
          Send verification
        </Button>
      </form>
    </Panel>
  );
}
