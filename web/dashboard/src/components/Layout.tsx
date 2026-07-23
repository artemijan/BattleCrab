import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { Link, NavLink, useNavigate } from "react-router-dom";

import markUrl from "../../assets/favicon.svg";
import { api, type Account } from "../lib/api";
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
      {/*
        Hover is a slow, even magnification — no rotation. The long duration and
        a decelerating ease-out (no overshoot) are what make it read as a zoom
        rather than a pop.

        Only `scale` and `opacity` animate, and both are composited: the GPU
        transforms an already-painted layer, so no frame repaints.

        This matters more here than it looks. The mark sits inside the header's
        `backdrop-filter: blur(22px)`, over three 40rem blurred background
        blobs. Animating `box-shadow` — a paint property, never composited —
        forced a repaint per frame, and each repaint made that backdrop blur
        recompute. Measured: frame stalls of 133ms. Removing any one of the
        three made it clean, so the depth cue moved to the overlay below rather
        than the shadow itself.

        Scaling also never reflows the wordmark beside it, and the global
        prefers-reduced-motion rule collapses the transition.
      */}
      <span
        className="relative shrink-0 transform-gpu transition-[scale] duration-500
                   [transition-timing-function:var(--ease-out-soft)]
                   group-hover:scale-[1.18]
                   group-active:scale-[1.06] group-active:duration-150"
      >
        {/* Same asset as the favicon, so the tab icon and the header mark can
            never drift apart. */}
        <img
          src={markUrl}
          alt=""
          width={36}
          height={36}
          className="size-9 rounded-xl shadow-[0_8px_22px_-8px_rgba(0,87,183,0.9)]"
        />
        {/* The deeper shadow is a separate layer faded in by opacity, because a
            box-shadow that *changes* cannot be composited. */}
        <span
          aria-hidden
          className="pointer-events-none absolute inset-0 rounded-xl
                     shadow-[0_14px_30px_-8px_rgba(0,87,183,1)] opacity-0
                     transition-opacity duration-500
                     [transition-timing-function:var(--ease-out-soft)]
                     group-hover:opacity-100"
        />
      </span>
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

export function Header({ account }: { account?: Account | null }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const logout = useMutation({
    mutationFn: () => api.logout(),
    // onSettled, not onSuccess: if the request fails we are still logging out
    // locally, and leaving the UI signed in would strand the user.
    onSettled: () => {
      // Leave the guarded page BEFORE clearing the session, in that order.
      //
      // Both updates flush in one render, and at that point the router has to
      // already be on "/". Clearing first means RequireAuth re-renders while
      // still mounted on /account, sees no session and redirects to /login —
      // so logging out dumps the user on a "please sign in" screen instead of
      // the home page. Masked until now by the stale-cache bug below, which
      // left RequireAuth reading a session that was supposedly gone.
      navigate("/");

      // Write the signed-out session rather than dropping it.
      //
      // This used to be `queryClient.clear()`, which is why the header kept
      // showing the old address after logging out: clear() removes the query
      // objects, but observers that are already mounted stay bound to the
      // removed one and go on rendering the last result it gave them. A
      // component mounting *after* the clear got a fresh observer and the right
      // answer, so the landing page updated while the header did not — the two
      // disagreeing on the same page is the tell.
      //
      // Setting the value notifies the live observers instead of orphaning
      // them, and null is the truth here, so there is nothing to re-fetch.
      queryClient.setQueryData(["me"], null);

      // Everything else is account-scoped — characters, game accounts — and
      // must not survive into the next login, or signing in as someone else
      // flashes the previous account's data. A predicate rather than a list of
      // keys so a query added later is dropped without anyone remembering to
      // come back here.
      queryClient.removeQueries({ predicate: (query) => query.queryKey[0] !== "me" });
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
              {/* Cosmetic only — the server 403s a non-admin regardless. */}
              {account.isAdmin && <NavItem to="/admin">Admin</NavItem>}
              {/* An address is far longer than the login name this replaced, so
                  it is truncated rather than allowed to push the nav around. */}
              <span
                className="mx-2 hidden max-w-[16ch] truncate text-sm text-[var(--text-muted)] md:inline"
                title={account.email ?? undefined}
              >
                {account.email}
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
  account?: Account | null;
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
