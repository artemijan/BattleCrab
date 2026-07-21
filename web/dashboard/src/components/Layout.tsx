import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { Link, NavLink, useNavigate } from "react-router-dom";

import markUrl from "../../assets/favicon.svg";
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

/**
 * The wordmark is dropped only at widths where it genuinely doesn't fit, so a
 * clipped "Battl…" never appears — an icon-only mark is deliberate, truncation
 * reads as breakage.
 *
 * Measured at 360px: the nav is 180px of a 302px inner width, leaving the
 * wordmark 10px short. The signed-in nav is wider still (Account + Log out), so
 * it needs a higher cutoff than the signed-out one. Both thresholds are set
 * just above where each actually overflows, which keeps the wordmark on 390px
 * phones — the common case — rather than hiding it on every phone.
 */
function Brand({ compact = false }: { compact?: boolean }) {
  return (
    // min-w-0 lets this shrink; without it the flex row's intrinsic width wins
    // and the nav is pushed out of the header on narrow screens.
    <Link
      to="/"
      className="group flex min-w-0 items-center gap-2 sm:gap-2.5"
      aria-label="BattleCrab home"
    >
      {/* Same asset as the favicon, so the tab icon and the header mark can
          never drift apart. */}
      <img
        src={markUrl}
        alt=""
        width={36}
        height={36}
        className="size-9 shrink-0 rounded-xl shadow-[0_8px_22px_-8px_rgba(0,87,183,0.9)]
                   transition-transform duration-300 group-hover:rotate-6 group-hover:scale-105"
      />
      <span
        className={cx(
          "truncate text-base font-bold tracking-tight sm:text-lg",
          compact ? "max-[479px]:hidden" : "max-[389px]:hidden",
        )}
      >
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
      <div
        className="glass glass-sheen mx-auto flex max-w-5xl items-center gap-2 rounded-2xl
                   px-3 py-2.5 sm:gap-3 sm:px-4 sm:py-3"
      >
        <Brand compact={!!account} />

        {/* shrink-0 keeps the controls at their intrinsic width, so any overflow
            is absorbed by the brand's truncate rather than pushing the theme
            toggle outside the header. */}
        <nav className="ml-auto flex shrink-0 items-center gap-0.5 sm:gap-1">
          {account ? (
            <>
              <NavItem to="/account">Account</NavItem>
              <span className="mx-2 hidden text-sm text-[var(--text-muted)] md:inline">
                {account.login}
              </span>
              <Button
                variant="ghost"
                onClick={() => logout.mutate()}
                loading={logout.isPending}
                className="px-2.5 py-2 text-xs sm:px-3"
              >
                Log out
              </Button>
            </>
          ) : (
            <>
              <NavItem to="/login">Log in</NavItem>
              <Link to="/register">
                {/* Shorter label on phones — "Create account" alone is wider
                    than the space left beside the brand at 360px. */}
                <Button className="px-3 py-2 text-xs sm:px-4">
                  <span className="sm:hidden">Sign up</span>
                  <span className="hidden sm:inline">Create account</span>
                </Button>
              </Link>
            </>
          )}
          <ThemeToggle />
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
          "rounded-lg px-2 py-2 text-sm font-medium transition-colors duration-200 sm:px-3",
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
    <footer className="mx-auto mt-10 w-full max-w-5xl px-4 pb-6 sm:mt-12 sm:pb-8">
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
        <span className="ml-auto text-[var(--text-faint)]">
          Custom Lineage II · Interlude Classic
        </span>
      </div>
    </footer>
  );
}

/**
 * The page frame: header, content and footer in one `min-h-dvh` column.
 *
 * The header must live *inside* this column. Rendered as a sibling above it, a
 * sticky header still occupies flow space, so the document came out
 * `100dvh + header` tall and every route scrolled by exactly the header's
 * height (82px) even when the content fit with room to spare.
 */
export function Page({
  account,
  children,
}: {
  account?: { login: string } | null;
  children: ReactNode;
}) {
  return (
    <div className="flex min-h-dvh flex-col">
      <Background />
      <Header account={account} />
      <main className="mx-auto w-full max-w-5xl flex-1 px-4 pt-6 sm:pt-8">{children}</main>
      <Footer />
    </div>
  );
}
