/**
 * Where the project is, and what comes next.
 *
 * `nextWhen` deliberately carries **no date**. It named a month until
 * 2026-09-03, on the reasoning that a relative phrase keeps reading as true
 * forever while a named month visibly goes stale and prompts someone to fix
 * it. That still holds — but it assumed the date was knowable. The port is
 * finished and the project is in its testing phase, so what stands between
 * here and open beta is a bug count nobody has yet: promising a month would be
 * inventing one. The panel copy on the landing page says as much rather than
 * leaving "Soon" to stand on its own.
 *
 * Put a month back the moment there is a real one — and when that happens the
 * old reasoning applies again, so name it here rather than saying "soon".
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
  nextWhen: "Soon",
} as const;
