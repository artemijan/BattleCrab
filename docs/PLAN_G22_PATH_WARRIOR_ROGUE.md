# G22 slice 10 — Path of the Warrior / Path of the Rogue

`Q00401_PathOfTheWarrior` (332 Java lines) and `Q00403_PathOfTheRogue` (374),
awarding the Medallion of Warrior (1145) and Beziques' Recommendation (1190).
Continues closing the proof-source gap: `ElfHumanFighterChange1` needs five
proofs and now has four — only `Q00402_PathOfTheHumanKnight` (629 lines, its
own slice) is left before that transfer is fully reachable.

## The finding: the same holder type, two different denominators

Both quests build drop tables out of `ItemChanceHolder`, exactly as quest 406
does. But the roll is written per call site, and it is **not the same**:

| Quest | Roll | A "chance" of 2 means |
|---|---|---|
| Q00406 | `getRandom(100) < chance` | 2% |
| Q00403 | `getRandom(REQUIRED_ITEM_COUNT) < chance` — i.e. **`getRandom(10)`** | 20% |
| Q00401 | `getRandom(10) < 4` | 40% |

Reading Q00403's table as percentages — the obvious assumption, since it is the
same type used that way one quest earlier — would have made every Spartoi bone
**10× too rare**, turning a ~13-kill stage into a ~125-kill one. The
denominator is a property of the call, not of the table.

This is the same shape as last slice's `giveItemRandomly`-vs-hand-roll finding:
**the drop-rate details of a quest are not inferable from the types it uses.**
Read the roll.

Q00401's spider stage has **no chance roll at all** — every qualifying kill
pays. It is the weapon gate below, not a rate, that makes it slow.

## The weapon/solo tag, now shared

Quests 401 and 403 have byte-identical `onAttack` state machines, so it is
factored into `scripts/quest_common.rs`:

- **0** (untouched): record the attacker, go to **1** if they hold the quest
  weapon, else **2**.
- **1**: drop to **2** if the weapon changes *or* a second player joins.
- **2**: terminal — nothing re-qualifies the mob.

`onKill` pays only on `isScriptValue(1)`. Both hooks are load-bearing in
opposite directions, the same trap as quest 407. Tested: an unarmed kill of a
venomous spider pays nothing, the same kill holding Auron's sword pays.

More of the family (402, 415, …) uses this, which is why it is shared rather
than copied a third time.

## Two new framework pieces

- **`Npc.vars`** — Java's `npc.getVariables()`, needed for `lastAttacker`. I
  checked the breadth before choosing the shape: 11 Interlude quests use it
  under 6 distinct keys, so a generic `HashMap<String, i32>` beats six named
  fields in the `spoiler_object_id` style. An empty `HashMap` does not
  allocate, so idle NPCs pay only the struct size.
- **`QuestCtx::npc_say_to_player`** — the Cat's Eye Bandit taunts its attacker
  with `sendPacket` (that player only) but broadcasts its death line. The
  existing `npc_say` broadcasts; using it for the taunt would have leaked the
  line to bystanders.

Also `QuestCtx::equipped_weapon_id`.

## Tests

5 added, all passing on first run:

- the spider gate (unarmed → nothing, sword → a leg);
- Q00401's `getRandom(10) < 4` pinned by forcing the roll to **4** — no drop;
  read as `getRandom(100) < 40` it would drop;
- Q00403's `/10` denominator. **This one is deliberately statistical**: a
  forced roll returns its value regardless of the bound, so no forced-roll test
  can distinguish `/10` from `/100`. It instead asserts the *rate* — Ruin
  Spartoi at chance 8 is 80%, and 40 kills reliably cap the 10-bone collection,
  where 8% essentially never would. Run 10× to confirm it isn't flaky
  (P(false failure) ≈ 1e-7);
- the Cat's Eye taunt fires once and to the player, the death line broadcasts,
  and a stolen good drops;
- page existence for both quests.

## Status

16 quests ported. `ElfHumanFighterChange1` is one quest (402) from fully
reachable; the wizard side needs 404, 405, 408, 409.
