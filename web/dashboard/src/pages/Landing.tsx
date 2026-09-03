import { Link } from "react-router-dom";

import logoSmall from "../../assets/logo-420.webp";
import logo from "../../assets/logo.webp";
import { Button, Panel, cx } from "../components/ui";
import { GITHUB_ISSUES_URL, GITHUB_NEW_ISSUE_URL } from "../lib/links";
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
        {/* The next milestone and its chip are not repeated here: each phase
            below already carries its own, two lines away. */}
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
                expect rough edges, occasional restarts and content that is still landing.
                {/* What to do about those rough edges is the panel directly
                    below, so it is not spelled out a second time here. */}
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
                Interlude Classic content in place. There is no date on it: the content is all
                ported and what is left is testing, so the beta arrives when the bugs stop turning
                up rather than on a day we picked in advance.
              </p>
            </div>
          </li>
        </ol>
      </Panel>
    </section>
  );
}

/**
 * Where to put a bug, and why it is worth the trouble.
 *
 * This follows the status panel deliberately. That panel has just admitted the
 * server is rough; the obvious next question is what someone is supposed to do
 * when they hit one of those rough edges, and leaving it unanswered turns a
 * fixable bug into a player who quietly stops logging in.
 *
 * Both links are plain anchors to GitHub — the tracker is the real one, so the
 * site does not pretend to own a form in front of it. "Browse open issues"
 * comes first in reading order for a reason: a duplicate report costs the
 * reporter their effort and us the triage.
 */
function ReportIssues() {
  return (
    <section className="mt-4">
      <Panel className="animate-rise flex flex-wrap items-center gap-5 p-6 sm:p-7">
        <div className="min-w-56 flex-1">
          <h2 className="text-xl font-bold">Found something broken?</h2>
          <p className="mt-1.5 text-sm/relaxed text-(--text-muted)">
            {STATUS.phase} means bugs, and the ones nobody reports are the ones nobody fixes. If a
            skill misbehaves, a quest dead-ends or the client drops you — or you just think
            something could work better — open an issue on GitHub. Ideas and balance suggestions are
            as welcome as crashes.
          </p>
          <p className="mt-2 text-xs/relaxed text-(--text-faint)">
            What helps most: your character name, roughly when it happened, and what you were doing
            just before. A screenshot beats a paragraph. Reporting needs a free GitHub account.
          </p>
        </div>
        <div className="flex gap-3">
          {/* Leaves the SPA for github.com, so plain anchors rather than router
              Links — and a new tab, because losing the page you were reading is
              a poor trade for filing a report. */}
          <a href={GITHUB_ISSUES_URL} target="_blank" rel="noreferrer noopener">
            <Button variant="ghost">Browse open issues</Button>
          </a>
          <a href={GITHUB_NEW_ISSUE_URL} target="_blank" rel="noreferrer noopener">
            <Button variant="secondary">Report an issue</Button>
          </a>
        </div>
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

      <ReportIssues />

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
            {/* Said here rather than in a requirements list: this is the moment
                someone on a Mac decides the download is pointless and leaves.
                Naming Parallels specifically is the point — "use a VM" reads as
                a guess, whereas a named product that is known to work reads as
                an answer. */}
            <p className="mt-2 text-xs/relaxed text-(--text-faint)">
              The client is Windows software. On a Mac it runs under{" "}
              <span className="text-(--text-muted)">Parallels Desktop</span> — install the launcher
              inside Windows exactly as you would on a PC.
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
