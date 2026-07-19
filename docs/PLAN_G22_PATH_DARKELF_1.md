# G22 slice 15 — Path of the Palus Knight / Path of the Assassin

`Q00410_PathOfThePalusKnight` (314 Java lines) and `Q00411_PathOfTheAssassin`
(329), awarding the Gaze of Abyss (1244) and the Iron Heart (1252).

Opens the **Dark Elf** tier. `DarkElfChange1` is ported and needs four proofs;
it now has two. Same shape as the Elf/Human situation before quests 401–409:
the transfer script exists but nothing in the world produces what it consumes.

## Every drop in both quests is unrolled

No `getRandom` anywhere in either `onKill`: 13 lycanthrope kills is 13 skulls,
10 moonstone beasts is 10 molars, Calpico always yields the tears. Worth
stating because the sibling quests in this tier (412/413) *do* roll, and
porting them by analogy would either add a chance that isn't here or drop one
that is. The tests use no forced rolls at all — the exact kill counts are the
whole requirement, which is only a valid assertion because the drops are
unrolled.

## Q00411 is one token walking a chain

Java writes every talk branch as "hold *this* item and **none** of the others"
— `!hasAtLeastOneQuestItem(a, b, c, d, e) && hasQuestItems(f)` — seventeen
times across three NPCs. That verbosity encodes one fact: **exactly one token
is in the bag at a time**, because every hand-over takes the old before giving
the new.

```
Shilen's Call → Arkenia's Letter → Leikan's Note → (10 molars)
  → Shilen's Tears → Arkenia's Recommendation → Iron Heart
```

So the port asks *which* token is held and matches on it — the same predicate,
written once. The invariant is the quest's own design (verified transition by
transition), not an assumption layered on top. The molars are the deliberate
exception: they coexist with Leikan's note, which is why his branches test them
separately, and there's a test pinning that his page tracks the molar count
while the token stays put.

## Two redundant Java terms, collapsed with the reasoning recorded

- Q00410's silk branch reaches `== 5` and then re-tests `silk >= 4` — trivially
  true.
- Q00410's Kalinta chain has a **dead branch**: `!has(SILK) && has(CARAPACE)`
  sits below `!hasQuestItems(SILK, CARAPACE)` (i.e. *not both*), which already
  catches carapace-only. The port collapses it and documents the reachable
  state→page table in `talk_kalinta` so the equivalence is checkable.

## The page test earned its keep

It failed on first run: I had asserted the `.htm`/`.html` split identically for
both quests, but **410's accept page `30329-06` is `.htm` while 411's
`30416-06` is `.html`** (411's accept page is `-05`). The split point differs
per quest even inside one race tier. Now asserted separately, plus an explicit
assertion that `30416-06.htm` does *not* exist, so the uniform assumption can't
come back.

## Tests

5 added: both full chains end to end; Q00410's talisman drop-gate; Q00411's
Leikan page tracking the molar count under a fixed token; and the page sweep
that caught the split difference above.

## Status

23 quests ported. `DarkElfChange1` has 2 of 4 proofs — 412 (Dark Wizard, 384
lines) and 413 (Shillien Oracle, 328) finish the Dark Elf tier.
