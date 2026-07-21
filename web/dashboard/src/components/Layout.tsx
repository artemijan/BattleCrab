import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { Link, NavLink, useNavigate } from "react-router-dom";

import { api } from "../lib/api";
import { Button, cx } from "./ui";
import { ThemeToggle } from "./ThemeToggle";

/** The drifting colour fields behind every page. */
export function Background() {
  return (
    <div className="bg-field" aria-hidden>
      <span />
      <span />
      <span />
    </div>
  );
}

function Brand() {
  return (
    <Link to="/" className="group flex items-center gap-2.5">
      <span
        className="grid size-9 place-items-center rounded-xl bg-brand-500 text-base font-black text-accent-400
                   shadow-[0_8px_22px_-8px_rgba(0,87,183,0.9)] transition-transform duration-300
                   group-hover:rotate-6 group-hover:scale-105"
        aria-hidden
      >
        B
      </span>
      <span className="text-lg font-bold tracking-tight">
        Battle<span className="text-brand-500 dark:text-brand-300">Crab</span>
      </span>
    </Link>
  );
}

export function Header({ account }: { account?: { login: string } | null }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const logout = useMutation({
    mutationFn: () => api.logout(),
    // Clear cached account/character data on the way out so a subsequent login
    // as a different account can't flash the previous one's characters.
    onSettled: () => {
      queryClient.clear();
      navigate("/");
    },
  });

  return (
    <header className="sticky top-0 z-40 px-4 pt-4">
      <div className="glass glass-sheen mx-auto flex max-w-5xl items-center gap-3 rounded-2xl px-4 py-3">
        <Brand />

        <nav className="ml-auto flex items-center gap-1">
          {account ? (
            <>
              <NavItem to="/account">Account</NavItem>
              <span className="mx-2 hidden text-sm text-[var(--text-muted)] sm:inline">
                {account.login}
              </span>
              <Button
                variant="ghost"
                onClick={() => logout.mutate()}
                loading={logout.isPending}
                className="px-3 py-2 text-xs"
              >
                Log out
              </Button>
            </>
          ) : (
            <>
              <NavItem to="/login">Log in</NavItem>
              <Link to="/register">
                <Button className="px-4 py-2 text-xs">Create account</Button>
              </Link>
            </>
          )}
          <div className="ml-1">
            <ThemeToggle />
          </div>
        </nav>
      </div>
    </header>
  );
}

function NavItem({ to, children }: { to: string; children: ReactNode }) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        cx(
          "rounded-lg px-3 py-2 text-sm font-medium transition-colors duration-200",
          isActive
            ? "text-brand-600 dark:text-brand-200"
            : "text-[var(--text-muted)] hover:text-[var(--text)]",
        )
      }
    >
      {children}
    </NavLink>
  );
}

export function Footer() {
  const status = useQuery({
    queryKey: ["status"],
    queryFn: api.status,
    // The count comes from persisted `online` flags, so it is approximate;
    // refreshing every half-minute is plenty.
    refetchInterval: 30_000,
    retry: false,
  });

  return (
    <footer className="mx-auto mt-16 w-full max-w-5xl px-4 pb-10">
      <div className="glass glass-sheen flex flex-wrap items-center gap-x-6 gap-y-2 rounded-2xl px-5 py-4 text-sm">
        <span className="flex items-center gap-2">
          <span
            className={cx(
              "size-2 rounded-full",
              status.data?.online ? "bg-emerald-400" : "bg-[var(--text-faint)]",
            )}
            aria-hidden
          />
          <span className="text-[var(--text-muted)]">
            {status.isPending
              ? "Checking server…"
              : status.data?.online
                ? `Server online — ${status.data.playersOnline} playing`
                : "Server status unavailable"}
          </span>
        </span>
        <span className="ml-auto text-[var(--text-faint)]">Lineage II Interlude</span>
      </div>
    </footer>
  );
}

export function Page({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-dvh flex-col">
      <Background />
      <main className="mx-auto w-full max-w-5xl flex-1 px-4 pt-10">{children}</main>
      <Footer />
    </div>
  );
}
