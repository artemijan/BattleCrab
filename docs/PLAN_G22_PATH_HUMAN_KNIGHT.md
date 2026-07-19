# G22 slice 11 — Path of the Human Knight

`Q00402_PathOfTheHumanKnight`, 629 Java lines — the widest quest in the Path
family, taken alone because **it completes the proof set for
`ElfHumanFighterChange1`**. All five of that script's targets (Warrior, Knight,
Rogue, Elven Knight, Elven Scout) are now reachable in normal play. That closes
the gap opened three slices ago, at least on the fighter side.

## Structurally unlike its siblings: six sub-quests, you need three

Sir Klaus Vasper issues a Squire's Mark; six officers then each offer the same
bargain — take my badge, bring back N trophies, receive a Coin of Lords. Three
coins is enough, so most of the 629 lines is the same block six times over.
Ported as a `BRANCHES` table plus a `DROPS` table.

## The completion path forks, and the 6-coin case is the odd one

| Coins | Page | Completes? |
|---|---|---|
| < 3 | `30417-09` | no |
| 3 | `30417-10` | no — confirm button posts `30417-13` |
| 4–5 | `30417-11` | no — confirm button posts `30417-14` |
| 6 | `30417-12` | **yes, immediately inside `onTalk`** |

The "collected everything" case is the *only* one without a confirmation step.
It reads like an oversight, but the dist backs it up: `-12` is a completion
page, not a prompt. Kept and tested in both directions, because a reader
tidying the asymmetry would either add a prompt nobody can answer or drop the
6-coin completion entirely — and a player who did all six sub-quests is exactly
the one who'd notice.

The two confirm handlers also sweep **all** leftover badges and trophies, not
just coins, since a player may have part-finished the other sub-quests. The
6-coin path takes only coins and the mark, which is right there: every badge
was already spent buying a coin.

## Two smaller quirks, both verified rather than assumed

- **The quest never calls `setCond`** — not once in 629 lines. `startQuest`
  sets cond 1 and it stays there, so the client's quest window shows one step
  the whole way through. Confirmed by grep (`setCond` count: 0) rather than
  inferred from the sections I happened to read.
- **Vasper's page extensions alternate**: `-01..-05`, `-07`, `-08` are `.htm`;
  `-06` and `-09..-15` are `.html`. Not a prefix split like the other Path
  quests, so it can't be derived — copied per page, and the test asserts both
  `30417-07.html` and `30417-06.htm` are absent so the alternation can't be
  "regularised".
- Raymond (30289) alone ships six pages: an extra intermediate page shifts all
  his later pages up by one. Encoded per branch instead of derived from the
  offer page; the test asserts no other officer has a `-06`.

**Two of the six trophies have no chance roll at all** (Bugbear Necklace,
Venomous Spider's Leg) — easy to miss in six near-identical blocks, so the
table stores `Option<i32>` and the no-roll case is tested with ten unforced
kills yielding exactly ten necklaces.

## Tests

6 added, all green on first run: the 3-coin confirm path; the 6-coin
talk-completes path; both confirm buttons refusing the wrong coin count; one
officer's sub-quest end to end including the unrolled drop; the badge gate
(right mob, no badge → nothing); and the page sweep covering the alternating
extensions and Raymond's extra page.

## Status

17 quests ported. `ElfHumanFighterChange1` is **fully reachable**.
`ElfHumanWizardChange1` still needs all four of 404, 405, 408, 409 — the
obvious next slice is a pair of those.
