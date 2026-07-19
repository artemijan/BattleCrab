# G22 slice 19 — Path of the Orc Shaman

`Q00416_PathOfTheOrcShaman` (525 Java lines), awarding the **Mask of Medium**
(1631). **The Orc first-occupation tier is complete** — `OrcChange1` has all
three proofs. Three of four races done.

Ported off the groundwork from the previous session's aborted attempt: I'd
stopped short rather than rush a 525-line quest needing framework I hadn't
checked. That analysis made this pass straightforward, and two of the three
things it flagged turned out not to be needed at all.

## `ItemChanceHolder.count` is a *cond selector*, not a quantity

The drop table looks ordinary and isn't:

```java
final ItemChanceHolder item = MOBS.get(npc.getId());
if (item.getCount() == qs.getCond())
```

The **count** field carries the cond in which that mob is live (1, 6 or 9);
**chance** is a 0..1 probability fed to `giveItemRandomly`. Read `count` as a
quantity — its normal meaning, and what it means in quests 403/406 — and
grizzly bears would drop **six** bloods per kill while the cond gate silently
disappeared. Tested from both sides: a grizzly at cond 1 drops nothing, and at
cond 6 drops exactly one.

Fourth distinct reading of this one type across the family, after `/100`,
`/10` and `== 0` equality.

## Two summon meters that differ in the one way that matters

The Durka parasites escalate exactly like quest 414's green blood — 5 gives
1-in-10, 6 and 7 give 2-in-10, 8 is certain, and success wipes the stack and
conjures a Durka Spirit. But **Java does not set this one on the player**:
there is no `attackPlayer` call after `addSpawn`, where 414 has one.

So this needed `QuestCtx::spawn_near_npc` (added, with `spawn_attacker`
refactored to build on it). Reusing `spawn_attacker` would have been the
natural move and would have invented aggro the datapack doesn't ask for. The
test asserts the conjured spirit is *not* in the player's aggro list, which is
the only thing distinguishing the two mechanics.

## What the groundwork predicted, and what it got wrong

Right: the `memoState` 100–110 branch (Black Leopard, NPCs 31979 / 32057 /
32090) is dead at both ends — its sole entry `30585-14.html` is offered by no
page, and none of those NPCs is registered. Third Orc quest in a row.

Wrong, usefully: I'd flagged two framework gaps as blockers.

- **`NpcSay` with a string parameter** — not needed. Both of Java's
  player-name lines live *inside* the dead branch, so the live path never
  reaches them. The packet stays unextended.
- **`getRandomPartyMemberState`** — reduces to the killer, exactly as
  `q00303_collect_arrowheads` already documents. Recorded as a
  `TODO(G13+)` deviation rather than new machinery.

Unlike 414/415 the dead branch here is **omitted rather than stubbed**: it is
large, and half-porting it would mean carrying dead `memoState` handling and a
packet feature we don't have. Documented at the module head instead.

Also: the accept event is **`START`**, not the `ACCEPT` every other Path quest
uses; and `cond 10` is never assigned — the chain jumps 9 → 11.

## Tests

6 added, all green on first run: the cond-gate/quantity distinction from both
sides; the three-trophy first stage through Tataru's swap; the parasite meter
including the no-aggro assertion; the finish to the Mask of Medium (pinning the
9 → 11 jump); the dead-branch assertion; and page existence. Adds a
`set_quest_cond` test helper for jumping into mid-quest stages.

## Status

28 quests ported. **Three of four first-occupation tiers complete**
(Elf/Human, Dark Elf, Orc). Only Dwarf remains: 417 (Scavenger, 690 lines) and
418 (Artisan, 562) — after which every race's `*Change1` script is
self-sufficient.
