/**
 * Where the project is, and what comes next.
 *
 * `nextWhen` is an absolute month rather than "in a month" on purpose. A
 * relative phrase keeps reading as true forever and quietly turns into a lie
 * the day nobody updates it; a named month visibly goes stale, which is what
 * prompts someone to fix it. Set 2026-07-21, targeting one month out.
 *
 * # Updating the phase
 *
 * This is the source of truth, but two things outside the bundle repeat it and
 * cannot import it:
 *
 *   * `index.html` — the meta/og description crawlers read.
 *   * `scripts/og-image.ts` — the badge drawn on the share card.
 *
 * `tests/link-preview.test.ts` fails if the first drifts from this file, so the
 * mismatch surfaces at test time rather than in someone's Discord unfurl. The
 * card is checked by eye — regenerate it with `bun run og-image`.
 */
export const STATUS = {
  phase: "Early alpha",
  /** Open to anyone — no invite, no key, no application. */
  isOpen: true,
  next: "Open beta",
  nextWhen: "August 2026",
} as const;
