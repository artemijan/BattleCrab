# G23 slice 8 — the boss-id audit

A bug I introduced in slice 4, found by checking reachability before picking the
next boss rather than by anything failing.

## Antharas is 29068, not 29019

Slice 4's `window_for` mapped Antharas to **29019**. The script uses
`ANTHARAS = 29068` — the "strong" variant — and `grandboss_data` has a row for
29068 and none for 29019.

So Antharas's respawn window never resolved: `on_grand_boss_killed` would return
early, and **Antharas would have died and never come back**. Silent, because
29019 is a perfectly valid NPC template — the id looks right in isolation and is
only wrong against the boss table.

## What the boss table actually tracks

`grandboss_data` ships eight rows on this dist:

| id | boss |
|---|---|
| 29001 | Queen Ant |
| 29006 | Core |
| 29014 | Orfen |
| 29020 | Baium |
| 29022 | Zaken |
| 29028 | Valakas |
| 29068 | **Antharas** (strong variant) |
| 25512 | Gigantic Chaos Golem (DrChaos's second form) |

**Sailren (29065) has no row**, and neither does DrChaos's first form — so
neither is a tracked grand boss here, which is worth knowing before either is
scheduled as a slice.

## The test is a cross-check, not an assertion

`every_configured_boss_id_is_one_the_boss_table_tracks` compares the two lists
in both directions and pins the lookalike explicitly (`29019` must **not**
resolve). The failure mode is precisely the two lists disagreeing, so a test
that only checked one side would not have caught this.

## How it was found

By running the reachability check before picking the next boss — the same habit
that struck agathions, pet evolution and chaos target swaps, and that found
G20.5 already complete. Here it turned up a defect in work I had already merged
and called done.

**Reachability checks are worth running even when you already know what you're
building next.**

## Still open in G23

Baium (787 lines), Valakas (581), Antharas (1056), plus boss barks. Sailren and
DrChaos are untracked on this dist.
