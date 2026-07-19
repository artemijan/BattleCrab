# G22 slice 13 — Path of the Elven Oracle

`Q00409_PathOfTheElvenOracle` (408 Java lines), awarding the **Leaf of Oracle**
(1235) — `ElfHumanWizardChange1`'s third proof of four.

**Taken alone rather than paired with 408 as planned.** I checked the framework
needs of both before starting: Q00408 uses none of `addSpawn`,
`addAttackPlayerDesire` or `setMemoState`; Q00409 uses all three (4 + 4 + 15
call sites). Carrying three new primitives *and* a second 446-line quest in one
slice is how the sloppiness these slices keep catching gets in, so 408 — which
is a plain quest — is the natural next one and finishes the wizard script.

## The first quest in the port that spawns its own monsters

Allana's re-enactment and Perrin's Tamil are ambushes conjured beside the NPC
you are talking to and set on you, not spawns you go and find. New framework:

- **`QuestCtx::memo_state` / `set_memo_state`** — Java stores this as the quest
  variable `memoState` (`QuestState.MEMO_VAR`), confirmed in the source rather
  than guessed.
- **`QuestCtx::spawn_attacker`** — `addSpawn(..., randomOffset, ...)` +
  `addAttackPlayerDesire`, reproducing Java's `Rnd.get(50, 100)` per axis with
  an independent sign so a group doesn't stack on one point.
- `npc_ai::seed_attack` promoted to `pub(crate)`; it already existed and is
  exactly `addAttackPlayerDesire`.

## `memoState` is a second progress axis and is not `cond`

Java drives this quest on both: `cond` for the client's quest window,
`memoState` for the script's own bookkeeping (never displayed). They move
independently and sometimes in opposite directions — talking to Manuel
empty-handed while `memoState == 2` **rewinds** it to 1 while pushing `cond` to
8. Collapsing them into one counter would break the re-enactment's restart
path, which is the whole reason `memoState` exists here. Tested directly.

That `setCond(8)` is also Java's single-argument form — no middle sound, unlike
every other cond change in the quest. Kept.

## The ambush tag is *not* `quest_common`'s

Quests 401/403 gate on "right weapon, one attacker". This one gates on **one
attacker only**, no weapon check, and keys `firstAttacker` rather than
`lastAttacker`. Same 0 → 1 → 2 shape, different predicate — so it is written
out rather than routed through `quest_common`, where sharing would have
silently imposed a weapon requirement the quest doesn't have. The test kills an
ambusher **bare-handed** to pin that.

## The bug this slice actually cost time on — in the test fixture

The memo-state test failed with the reply "you are either not on a quest that
involves this NPC". Instrumenting `on_talk` showed the final talk arriving at
**npc 27032 — a lizardman** — instead of Priest Manuel.

Cause: `NPC_OID` (the fixture's NPC object id) and `world.next_npc_object_id`
(the runtime spawn allocator) **both start at `FIRST_NPC_OBJECT_ID`**. The
first runtime spawn therefore lands on the same object id as the fixture's NPC
and silently replaces it. No test had ever spawned an NPC at runtime before, so
nothing had tripped it.

Fixed in the shared `add_test_npc` helper — it now reserves each id it
registers against the allocator — rather than by moving my own test's ids out
of the way. Every future quest that spawns would have hit this otherwise. All
seven major test modules re-run green afterwards (quests 76, combat 33, npc 71,
guard_aggro 13, admin 89, items 37, clans 16), since the helper is shared by
all of them.

## Tests

4 added, one of which found the fixture bug above: the three ambushers spawn
**and** aggro (asserting both halves of `spawn_attacker`, since a spawn that
doesn't aggro would leave the quest unfinishable); the first-attacker tag with
a bare-handed kill; `memoState` rewinding while `cond` advances; and page
existence.

## Status

20 quests ported. `ElfHumanWizardChange1` has 3 of 4 proofs — **Q00408
(Elven Wizard, 446 lines) alone finishes it**, and with it the entire Elf/Human
first-occupation tier becomes self-sufficient.
