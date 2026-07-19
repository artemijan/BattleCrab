# G22 slice 21 — Path of the Scavenger (the last Path quest)

`Q00417_PathOfTheScavenger`, 690 Java lines — the largest in the family and the
last of it. Awards the **Ring of Raven** (1642).

**All eighteen `Path of the *` quests (401–418) are ported.** Every race's
first-occupation script — `ElfHumanFighterChange1`, `ElfHumanWizardChange1`,
`DarkElfChange1`, `OrcChange1`, `DwarfBlacksmithChange1`,
`DwarfWarehouseChange1` — is now proof-complete and reachable in normal play.

## `dropChance` is documented as 0..1, and this quest passes 50

```java
giveItemRandomly(killer, npc, HONEY_JAR, 1, 5, 50, true)
```

`AbstractScript.giveItemRandomly`'s javadoc is explicit — *"the drop chance as
a decimal digit from 0 to 1"*. So 50 is not 50%; it is fifty times certainty,
and **every qualifying kill drops**. `q00303` passes `0.4` for a real 40%, so
the convention is not in doubt.

This is a datapack bug with a live effect, and the repo's rule is that the dist
is authoritative — so the port passes `50.0` and drops on every kill, exactly
as the shipped server does. Writing the "obviously intended" `0.5` would halve
the rate against retail: a silent divergence in the direction that looks like a
fix. The test kills six tarantulas with no forced rolls and asserts six beads;
at a real 0.5 it would fail about 98% of the time.

## Spoil-gated payouts — the Scavenger's own mechanic

Honey jars and beads pay only off a corpse that `isSpoiled()`, and `onAttack`
separately disqualifies a mob whose spoiler *is* the attacker
(`getSpoilerObjectId() == attacker` → script value 2). Needed
`QuestCtx::npc_is_spoiled` / `npc_spoiler_object_id` over the existing
`Npc.spoiler_object_id`.

The npc variable is `FIRST_ATTACKER` — a **fourth** spelling after
`lastAttacker` (401/403), `firstAttacker` (409) and `Q00415_last_attacker`.

## Two counters packed into one integer

`memoStateEx(1)` is radix-packed: **+10 per delivery** (tens) and **+1 per Mion
dialogue step** (units), read back with `% 10` for the units and `< 20` / `< 50`
thresholds for the tens. Treating it as one counter breaks both halves. Needed
`QuestCtx::memo_state_ex` / `set_memo_state_ex` (Java's `MEMO_EX_VAR + slot`),
a second memo axis independent of `memoState`.

Separately, `FLAG` is a **third** summon-meter shape: each ordinary Hunter Bear
kill raises it and the Honey Bear spawns at `20 * flag` percent, resetting on
success — after 414's green blood (roll against the held count) and 416's Durka
parasites (fixed thresholds per count).

`npc.deleteMe()` on Torai needed `QuestCtx::delete_npc` over the existing
`death::despawn_npc`.

## Dead at both ends — fifth quest running

NPC **31958** ships pages and is registered nowhere; the `BEAD_PARCEL2` /
`memoState 2` route that reaches it (`30556-06b`) is offered by no page.
Omitted, as in 416 and 418.

## Tests

5 added, all green on first run: the spoil gate (unspoiled pays nothing); the
`50`-means-always drop rate; the Honey Bear summon meter (flag 0 never summons,
flag 1 does inside `20 * flag`); the radix-packed delivery counter and its
cond-3 promotion; and Torai deleting himself before Raut pays the ring.

## Status

**30 quests ported. The Path family is complete (401–418).** G22 continues with
~161 remaining quests, ~81 `ai/` scripts, daily quests, the tutorial and
`//reload`.
