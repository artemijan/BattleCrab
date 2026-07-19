# G22 slice 20 — Path of the Artisan

`Q00418_PathOfTheArtisan` (562 Java lines), awarding the **Final Pass
Certificate** (1635). Opens the Dwarf tier — the last race whose `*Change1`
script is still proof-starved.

## The leader-tooth roll has a hole in it, and it is Java's

```java
if (getRandom(10) < 5) {
    if (getQuestItemsCount(killer, BOOGLE_RATMAN_LEADERS_TOOTH) == 1) { …give, MIDDLE… }
    // …and nothing at all when the count is 0
} else {
    giveItems(killer, BOOGLE_RATMAN_LEADERS_TOOTH, 1);  // always
}
```

Below 5 the kill pays **only** if you already hold exactly one tooth; at zero
that half of the roll does nothing. So the first tooth drops at 50% and the
second at 100%. Reading it as a flat "50% per tooth" is wrong in both
directions. Three forced-roll cases pin it: roll 0 at zero teeth pays nothing,
roll 5 at zero teeth pays, roll 0 at one tooth pays.

A consequence deliberately not "fixed": the `else` branch hands over the second
tooth **without** the `cond 2` check the `< 5` branch performs, so finishing
the leader teeth that way never advances the cond. The quest still completes —
every downstream branch tests item counts, not the cond — making this a
cosmetic Java bug (a stale quest window). Ported verbatim.

## Two routes to Kluto's letter, differing only in the sound

`30317-04.html` uses `setCond(4, true)`; `30317-07.html` uses the
single-argument `setCond(4)`. Same item, same cond, one chimes and one doesn't.

## Dead at both ends — fourth quest running

`30527-08c` sets `memoState = 10` and, with NPCs **31956 / 31963 / 32052**,
opens alternate routes including their own certificate hand-outs and
Lockirin's `memoState == 101` branch. Only `30527-08b` is offered by any page,
and none of those three NPCs is registered. Omitted rather than stubbed, as in
quest 416.

**The dead-branch test caught my own error, not the port's.** My first version
scanned every file in the quest directory for `30527-08c` — including the
`.java` source, which of course names it as a case label. That is precisely the
handler being proven unreachable, so the assertion fired on the evidence rather
than a defect. Now restricted to `.htm`/`.html` pages.

## Tests

4 added: the lopsided leader-tooth roll (three cases); the 70% ratman roll
(roll 7 misses, 6 pays); the full chain through to the Final Pass Certificate;
and the two-sided dead-branch assertion.

## Status

29 quests ported. **`Q00417_PathOfTheScavenger` (690 lines) is the last Path
quest** — with it, all four races' first-occupation scripts become
self-sufficient.
