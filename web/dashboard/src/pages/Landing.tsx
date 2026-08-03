import { Link } from "react-router-dom";

import logoSmall from "../../assets/logo-420.webp";
import logo from "../../assets/logo.webp";
import { Button, Panel, cx } from "../components/ui";
import { useAccount } from "../lib/session";
import { STATUS } from "../lib/status";

/** The launcher installs and updates the game client. */
const LAUNCHER_URL = "https://static.battlecrab.com/launcher.exe";

/**
 * Says plainly what stage the server is at.
 *
 * Deliberately concrete about the downside — an alpha that only advertises
 * upside sets people up to feel misled by the first bug, and they leave for
 * good. Saying "expect rough edges" up front buys the patience to report one
 * instead.
 */
function ProjectStatus() {
  return (
    <section className="animate-rise">
      <Panel className="p-6 sm:p-7">
        {/* The next milestone and its date are not repeated here: each phase
            below already carries its own chip, two lines away. */}
        <h2 className="text-xl font-bold">Where the project is</h2>

        <ol className="mt-5 space-y-4">
          <li className="flex gap-3.5">
            <span
              className="mt-1.5 size-2.5 shrink-0 rounded-full bg-accent-400 ring-4 ring-accent-400/20"
              aria-hidden
            />
            <div className="min-w-0">
              <p className="font-semibold">
                {STATUS.phase}
                <span className="ml-2 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[11px] font-medium text-emerald-600 dark:text-emerald-300">
                  Live now
                </span>
              </p>
              <p className="mt-1 text-sm/relaxed  text-(--text-muted)">
                The server is up and open to everyone — no key or invite. It is genuinely early, so
                expect rough edges, occasional restarts and content that is still landing. Bug
                reports are worth a lot right now.
              </p>
            </div>
          </li>

          <li className="flex gap-3.5">
            <span
              className="mt-1.5 size-2.5 shrink-0 rounded-full border-2 border-(--text-faint) bg-transparent"
              aria-hidden
            />
            <div className="min-w-0">
              <p className="font-semibold text-(--text-muted)">
                {STATUS.next}
                <span className="ml-2 rounded-full bg-(--surface-strong) px-2 py-0.5 text-[11px] font-medium text-(--text-faint)">
                  {STATUS.nextWhen}
                </span>
              </p>
              <p className="mt-1 text-sm/relaxed  text-(--text-muted)">
                The target is a server steady enough to play properly, with the rest of the
                Interlude Classic content in place.
              </p>
            </div>
          </li>
        </ol>
      </Panel>
    </section>
  );
}

const FEATURES = [
  {
    title: "Built on Interlude Classic",
    body: "Interlude Classic is the foundation, not the ceiling — the familiar systems are all here, with our own changes layered on top.",
  },
  {
    title: "Written in Rust",
    body: "A ground-up server rewrite: fewer stalls, faster restarts, and far less rubber-banding under load.",
  },
  {
    title: "Your account, your control",
    body: "Register in seconds, manage your password and email, and see every character from the web.",
  },
];

export function Landing() {
  const account = useAccount();
  const signedIn = !!account.data;

  // Rendered but not painted while the session resolves. Hiding the row
  // outright would shift the page when it appears; rendering the signed-out
  // buttons would flash "Create your account" at someone who already has one,
  // which is the whole complaint.
  const ctaVisibility = cx(account.isPending && "invisible");

  return (
    <div className="pb-8">
      <section className="animate-rise py-10 text-center sm:py-16">
        {/* The full artwork gets the hero, where it has room to read; the header
            and favicon use the flat L2R mark instead (see assets/favicon.svg).
            eager + fetchpriority because this is the LCP element. */}
        <img
          src={logo}
          srcSet={`${logoSmall} 420w, ${logo} 761w`}
          sizes="(max-width: 640px) 260px, 340px"
          width={761}
          height={711}
          alt="BattleCrab — Lineage 2 Rust server"
          loading="eager"
          fetchPriority="high"
          className="mx-auto mb-6 h-auto w-65 drop-shadow-[0_18px_40px_rgba(0,44,92,0.35)] sm:w-85"
        />

        <p
          className="mb-4 inline-flex items-center gap-2 rounded-full border border-(--surface-border)
                     bg-(--surface) px-3.5 py-1.5 text-xs font-medium text-(--text-muted) backdrop-blur-md"
        >
          {/* The pulse reads as "running right now", which is the part people
              miss: an alpha is usually something you wait for. */}
          <span className="relative flex size-1.5" aria-hidden>
            <span className="absolute inline-flex size-full animate-ping rounded-full bg-accent-400 opacity-70" />
            <span className="relative inline-flex size-1.5 rounded-full bg-accent-400" />
          </span>
          {STATUS.phase} — open to everyone
        </p>

        <h1 className="mx-auto max-w-3xl text-balance text-5xl font-black leading-[1.05] tracking-tight sm:text-6xl">
          The world of{" "}
          <span className="bg-linear-to-br from-brand-500 to-brand-300 bg-clip-text text-transparent dark:from-brand-200 dark:to-brand-400">
            Lineage II
          </span>
          , made our own.
        </h1>

        <p className="mx-auto mt-5 max-w-xl text-pretty text-lg text-(--text-muted)">
          {/* The phase is in the badge directly above and spelled out in the
              status panel below — a third mention in between is just noise. */}
          BattleCrab is a custom server built on Lineage II Interlude Classic, written from scratch
          in Rust. Create an account and play in under a minute.
        </p>

        <div className={cx("mt-9 flex flex-wrap items-center justify-center gap-3", ctaVisibility)}>
          {signedIn ? (
            <>
              <Link to="/account">
                <Button className="px-7 py-3 text-base">Go to your account</Button>
              </Link>
              {/* Still the useful next step once you have an account, so it
                  takes the slot the sign-in button vacated. */}
              <a href={LAUNCHER_URL} download>
                <Button variant="ghost" className="px-7 py-3 text-base">
                  Download launcher
                </Button>
              </a>
            </>
          ) : (
            <>
              <Link to="/register">
                <Button className="px-7 py-3 text-base">Create your account</Button>
              </Link>
              <Link to="/login">
                <Button variant="ghost" className="px-7 py-3 text-base">
                  I already have one
                </Button>
              </Link>
            </>
          )}
        </div>
      </section>

      <ProjectStatus />

      <section className="stagger mt-4 grid gap-4 sm:grid-cols-3">
        {FEATURES.map((feature) => (
          <Panel
            key={feature.title}
            className="p-6 transition-[translate] duration-300 hover:-translate-y-1"
          >
            <h2 className="text-base font-semibold">{feature.title}</h2>
            <p className="mt-2 text-sm/relaxed  text-(--text-muted)">{feature.body}</p>
          </Panel>
        ))}
      </section>

      <section className="mt-4">
        <Panel strong className="animate-rise flex flex-wrap items-center gap-5 p-7">
          <div className="min-w-56 flex-1">
            <h2 className="text-xl font-bold">Ready to play?</h2>
            <p className="mt-1.5 text-sm text-(--text-muted)">
              {signedIn
                ? "Grab the launcher — it installs and updates the game client for you — then log in with one of your game accounts."
                : "Grab the launcher — it installs and updates the game client for you — then create your account. Either order works."}
            </p>
          </div>
          <div className={cx("flex gap-3", ctaVisibility)}>
            {signedIn ? (
              <Link to="/account">
                <Button>Your account</Button>
              </Link>
            ) : (
              <Link to="/register">
                <Button>Create account</Button>
              </Link>
            )}
            {/* Direct .exe download, so it leaves the SPA — a plain anchor, not
                a router Link. */}
            <a href={LAUNCHER_URL} download>
              <Button variant="secondary">Download launcher</Button>
            </a>
          </div>
        </Panel>
      </section>
    </div>
  );
}
