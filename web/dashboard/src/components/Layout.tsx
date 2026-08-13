import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState, type ReactNode } from "react";
import { Link, NavLink, useLocation, useNavigate } from "react-router-dom";

import markUrl from "../../assets/favicon.svg";
import { api, type Account } from "../lib/api";
import { GITHUB_ISSUES_URL, GITHUB_URL } from "../lib/links";
import { Button, cx } from "./ui";
import { ThemeToggle } from "./ThemeToggle";

/**
 * Link to the source, as the GitHub mark.
 *
 * The mark is one filled path rather than the stroked icons used elsewhere —
 * it is a trademark with a defined shape, so it is reproduced rather than
 * redrawn in the local style. `currentColor` still lets it take the footer's
 * muted tone and brighten on hover like its neighbours.
 *
 * The label is hidden below `sm` and the icon carries an accessible name
 * either way, so the footer's single row survives the narrowest screens the
 * header is built for.
 */
function GitHubLink() {
  return (
    <a
      href={GITHUB_URL}
      target="_blank"
      // `noreferrer` implies `noopener`, but both are spelled out: the pairing
      // is what stops the opened tab reaching back through `window.opener`.
      rel="noreferrer noopener"
      title="Source on GitHub"
      className="group flex items-center gap-1.5 text-(--text-faint)
                 transition-colors hover:text-(--text)"
    >
      <svg
        aria-hidden
        viewBox="0 0 16 16"
        fill="currentColor"
        className="size-4 transition-transform duration-200 group-hover:-translate-y-0.5"
      >
        <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z" />
      </svg>
      <span className="hidden sm:inline">Source</span>
    </a>
  );
}

/**
 * Link to the issue tracker, on every page.
 *
 * The landing page makes the full case for reporting things; this is the copy
 * that has to be reachable from wherever someone actually hits the bug — the
 * account page, a password reset, anywhere. Unlike its neighbour the label
 * stays visible at every width: a lone icon reads as decoration, and an
 * invitation nobody recognises is not an invitation.
 */
function IssuesLink() {
  return (
    <a
      href={GITHUB_ISSUES_URL}
      target="_blank"
      rel="noreferrer noopener"
      title="Report a bug or suggest an improvement on GitHub"
      className="group flex items-center gap-1.5 text-(--text-faint)
                 transition-colors hover:text-(--text)"
    >
      <svg
        aria-hidden
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        className="size-4 transition-transform duration-200 group-hover:-translate-y-0.5"
      >
        <circle cx="12" cy="12" r="9" />
        <path d="M12 8v5M12 16.5v.01" />
      </svg>
      Report a bug
    </a>
  );
}

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
 * Below `sm` the nav collapses into the hamburger, so the brand shares the row
 * with just two icon buttons and the wordmark fits at every supported width —
 * only the truly tiny screens (<340px) drop it. `truncate` stays as the
 * guarantee that whatever happens, a clipped "Battl…" never renders: the
 * wordmark is whole or absent.
 */
function Brand() {
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
                   ease-out-soft
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
                     ease-out-soft
                     group-hover:opacity-100"
        />
      </span>
      <span className="truncate text-base font-bold tracking-tight max-[339px]:hidden sm:text-lg">
        Battle<span className="text-brand-500 dark:text-brand-300">Crab</span>
      </span>
    </Link>
  );
}

export function Header({ account }: { account?: Account | null }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const location = useLocation();
  const [menuOpen, setMenuOpen] = useState(false);

  // Navigating (from a menu link or anywhere else) closes the menu; without
  // this the panel would sit open over the page the user just moved to.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the path is not read in the body — it IS the trigger; any navigation closes the menu
  useEffect(() => setMenuOpen(false), [location.pathname]);

  // Escape closes, matching what everyone expects of an overlay.
  useEffect(() => {
    if (!menuOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMenuOpen(false);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [menuOpen]);

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
        <Brand />

        {/* shrink-0 keeps the controls at their intrinsic width, so any overflow
            is absorbed by the brand's truncate rather than pushing the theme
            toggle outside the header. */}
        <div className="ml-auto flex shrink-0 items-center gap-0.5 sm:gap-1">
          {/* Full nav from `sm` up; below that it lives in the hamburger. */}
          <nav className="hidden items-center gap-0.5 sm:flex sm:gap-1">
            {account ? (
              <>
                <NavItem to="/account">Account</NavItem>
                {/* Cosmetic only — the server 403s a non-admin regardless. */}
                {account.isAdmin && <NavItem to="/admin">Admin</NavItem>}
                {/* An address is far longer than the login name this replaced, so
                    it is truncated rather than allowed to push the nav around. */}
                <span
                  className="mx-2 hidden max-w-[16ch] truncate text-sm text-(--text-muted) md:inline"
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
                  <Button className="px-3 py-2 text-xs sm:px-4">Create account</Button>
                </Link>
              </>
            )}
          </nav>

          <ThemeToggle />

          {/* The hamburger. aria-expanded ties the button to the panel state
              for assistive tech; the icon swap is the visual equivalent. */}
          <button
            type="button"
            onClick={() => setMenuOpen((open) => !open)}
            aria-expanded={menuOpen}
            aria-label={menuOpen ? "Close menu" : "Open menu"}
            className="grid size-9 place-items-center rounded-lg text-(--text-muted)
                       transition-colors hover:text-(--text) sm:hidden"
          >
            <svg
              viewBox="0 0 24 24"
              className="size-5"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
            >
              {menuOpen ? <path d="M6 6l12 12M18 6L6 18" /> : <path d="M4 7h16M4 12h16M4 17h16" />}
            </svg>
          </button>
        </div>
      </div>

      {menuOpen && (
        <>
          {/* Click-away layer. Sits under the dropdown inside the header's
              stacking context, dimming the page it covers. */}
          {/* `touch-manipulation`: the base-layer instant-tap rule only reaches
              form controls, and this div closes on tap too — without it iOS
              Safari holds the tap ~350ms for double-tap-to-zoom. */}
          <div
            aria-hidden
            onClick={() => setMenuOpen(false)}
            className="fixed inset-0 touch-manipulation bg-black/25 sm:hidden"
          />
          {/* Absolutely positioned so opening the menu overlays the content
              instead of shifting it — a sticky header that grows would push
              the whole page down. */}
          <div className="absolute inset-x-4 top-full z-50 mt-2 sm:hidden">
            <nav
              aria-label="Mobile navigation"
              className="glass mx-auto flex max-w-5xl flex-col gap-0.5 rounded-2xl p-2"
            >
              {account ? (
                <>
                  <span
                    className="truncate px-3 pb-1.5 pt-2 text-xs text-(--text-faint)"
                    title={account.email ?? undefined}
                  >
                    {account.email}
                  </span>
                  <MenuItem to="/account">Account</MenuItem>
                  {account.isAdmin && <MenuItem to="/admin">Admin</MenuItem>}
                  <button
                    type="button"
                    onClick={() => logout.mutate()}
                    disabled={logout.isPending}
                    className="rounded-lg px-3 py-2.5 text-left text-sm font-medium
                               text-(--text-muted) transition-colors
                               hover:bg-(--surface-strong) hover:text-(--text)"
                  >
                    Log out
                  </button>
                </>
              ) : (
                <>
                  <MenuItem to="/login">Log in</MenuItem>
                  <MenuItem to="/register">Create account</MenuItem>
                </>
              )}
            </nav>
          </div>
        </>
      )}
    </header>
  );
}

/** A nav link inside the mobile dropdown: full-width row, same active colours
 *  as the desktop `NavItem`. */
function MenuItem({ to, children }: { to: string; children: ReactNode }) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        cx(
          "rounded-lg px-3 py-2.5 text-sm font-medium transition-colors",
          isActive
            ? "bg-(--surface-strong) text-brand-600 dark:text-brand-200"
            : "text-(--text-muted) hover:bg-(--surface-strong) hover:text-(--text)",
        )
      }
    >
      {children}
    </NavLink>
  );
}

function NavItem({ to, children }: { to: string; children: ReactNode }) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        cx(
          "rounded-lg p-2  text-sm font-medium transition-colors duration-200 sm:px-3",
          isActive
            ? "text-brand-600 dark:text-brand-200"
            : "text-(--text-muted) hover:text-(--text)",
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
              status.data?.online ? "bg-emerald-400" : "bg-(--text-faint)",
            )}
            aria-hidden
          />
          <span className="text-(--text-muted)">
            {status.isPending
              ? "Checking server…"
              : status.data?.online
                ? `Server online — ${status.data.playersOnline} playing`
                : "Server status unavailable"}
          </span>
        </span>
        {/* `ml-auto` moves to the group so the tagline and the two links travel
            together to the right edge. They wrap among themselves rather than
            as one unit, because three items no longer fit on one line on a
            phone — and the tagline is the piece that should drop to its own
            row, not the links. */}
        <span className="ml-auto flex flex-wrap items-center gap-x-4 gap-y-1.5">
          <span className="text-(--text-faint)">Custom Lineage II · Interlude Classic</span>
          <IssuesLink />
          <GitHubLink />
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
export function Page({ account, children }: { account?: Account | null; children: ReactNode }) {
  return (
    <div className="flex min-h-dvh flex-col">
      <Background />
      <Header account={account} />
      <main className="mx-auto w-full max-w-5xl flex-1 px-4 pt-6 sm:pt-8">{children}</main>
      <Footer />
    </div>
  );
}
