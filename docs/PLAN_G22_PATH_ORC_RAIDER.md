# G22 slice 17 — Path of the Orc Raider

`Q00414_PathOfTheOrcRaider` (326 Java lines), awarding the **Mark of Raider**
(1592). Opens the Orc tier.

**Scoped down mid-slice.** I set out to pair 414 with 416 (525 lines), but 414
turned out to carry two things worth doing carefully — an unusual summon
mechanic and a branch that is dead at both ends — so it goes alone and 416
follows. Rushing the second quest to hit the announced pairing would have been
the wrong trade.

## Green blood is a rising summon meter, not a collection

Killing Goblin Tomb Raider Leaders looks like a collect step and isn't. Java
races the *held count* against the RNG:

```java
if (getQuestItemsCount(killer, GREEN_BLOOD) <= getRandom(20)) {
    giveItems(killer, GREEN_BLOOD, 1);      // gain one
} else {
    takeItems(killer, GREEN_BLOOD, -1);     // lose the whole stack
    attackPlayer(addSpawn(KURUKA_RATMAN_LEADER, ...), killer);
}
```

`getRandom(20)` is 0..=19, so at 0 blood the gain is certain, at 19 it is 5%,
and at 20 the roll can never succeed — the summon is guaranteed. The blood is
never handed in to anyone: it is wiped the moment Kuruka appears, and the tooth
the quest actually wants drops from **Kuruka**, not the goblins.

Porting the blood as an ordinary capped collection would make the quest
**unfinishable** — nothing else drops the tooth. Two tests pin it: the
gain/summon fork at specific forced rolls (including that the summoned Kuruka
is set on the player), and the tooth arriving from Kuruka while resetting the
meter.

Reuses `QuestCtx::spawn_attacker` from slice 13. One fidelity gap recorded at
the site: Java passes `isSummonSpawn = true` (spawn animation) and seeds hate
999 via `addDamageHate`; our helper seeds dominant hate and skips the
animation.

## A branch dead at both ends — checked before assuming either way

Karukia's `30570-07b` route sets `memoState = 2`, `cond = 5` and leads to events
on NPC **31978**, who ships five pages in this quest's directory. Both ends are
unwired:

- **31978 is registered nowhere** — not in this quest's `addTalkId`, and
  `grep -rln 31978 data/scripts/` returns only this quest's own file and two of
  the orphaned pages. Its pages can never be served.
- **`30570-07.htm` offers only the `07a` button.** Nothing in the UI posts
  `07b`.

The order of those two checks mattered. Had only the serving end been missing,
`07b` would be a **trap**: it consumes the map and all ten teeth but hands out
no reports, and the reports are the sole path to the reward — a player taking
it would be permanently stranded. Because the button doesn't exist either,
there is no trap, and the route can be ported verbatim at zero risk.

Ported as-is with a `TODO(dead)` naming the coupling, plus a test asserting
*both* halves (the orphaned pages ship; the fork page offers only `07a`), so
nobody restores one end without the other.

`TIMORA_ORC_HEAD` (8544) is likewise registered as a quest item and never given
or taken — noted in the constant.

## Tests

5 added, all green on first run: the summon meter's gain/wipe fork; the tooth
source and meter reset; the Umbar report ladder (Zakan's spent first, 20% roll,
through to the Mark of Raider); the dead-branch assertion above; and page
existence.

## Status

26 quests ported. `OrcChange1` needs 415 (Monk, 652 lines) and 416 (Shaman,
525) to complete the Orc tier; then Dwarf (417 Scavenger 690, 418 Artisan 562)
finishes the whole first-occupation system.
