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
  const characters = useQuery({ queryKey: ["characters"], queryFn: api.characters });

  return (
    <div className="space-y-5 pb-6">
      <section className="animate-rise">
        <h1 className="text-3xl font-black tracking-tight">Your account</h1>
        <p className="mt-1.5 text-[var(--text-muted)]">
          Manage your credentials and review your characters.
        </p>
      </section>

      <section>
        <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-[var(--text-faint)]">
          Characters
        </h2>

        {characters.isPending ? (
          <Panel className="flex items-center gap-3 p-6 text-sm text-[var(--text-muted)]">
            <Spinner /> Loading characters…
          </Panel>
        ) : characters.isError ? (
          <Alert kind="error">Couldn't load your characters. Try refreshing.</Alert>
        ) : characters.data.length === 0 ? (
          <Panel className="p-8 text-center">
            <p className="font-medium">No characters yet</p>
            <p className="mt-1.5 text-sm text-[var(--text-muted)]">
              Log into the game client with this account to create your first one.
            </p>
          </Panel>
        ) : (
          <div className="stagger grid gap-3 sm:grid-cols-2">
            {characters.data.map((character) => (
              <CharacterCard key={character.name} character={character} />
            ))}
          </div>
        )}
      </section>

      <section className="grid gap-4 lg:grid-cols-2">
        <ChangePasswordCard />
        <ChangeEmailCard />
      </section>
    </div>
  );
}

function CharacterCard({ character }: { character: Character }) {
  return (
    <Panel className="group flex items-center gap-4 p-5 transition-transform duration-300 hover:-translate-y-1">
      <div
        className="grid size-12 shrink-0 place-items-center rounded-xl bg-brand-500/12 text-lg font-black
                   text-brand-600 ring-1 ring-brand-500/20 transition-transform duration-300
                   group-hover:scale-105 dark:text-brand-200"
        aria-hidden
      >
        {character.name.charAt(0).toUpperCase()}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <p className="truncate font-semibold">{character.name}</p>
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
        <p className="mt-0.5 truncate text-sm text-[var(--text-muted)]">
          Level {character.level} · {raceName(character.race)}
        </p>
        <p className="mt-0.5 text-xs text-[var(--text-faint)]">{playtime(character.onlineTime)}</p>
      </div>
    </Panel>
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
          ? `Currently ${me.data.email}. Used only for password resets.`
          : "Add an email so you can reset your password if you forget it."}
      </p>

      <form onSubmit={onSubmit} className="mt-4 flex flex-col gap-3.5">
        {submit.isError && (
          <Alert kind="error">
            {submit.error instanceof ApiError ? submit.error.message : "Something went wrong."}
          </Alert>
        )}
        {/* The address is stored only once the link is clicked — that is what
            makes a stored address mean "verified" (PLAN_DASHBOARD.md §5.4). */}
        {submit.isSuccess && (
          <Alert kind="success">
            Check your inbox — the address is saved once you click the link.
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
