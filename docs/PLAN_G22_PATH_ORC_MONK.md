# G22 slice 18 — Path of the Orc Monk

`Q00415_PathOfTheOrcMonk`, 652 Java lines — **the widest quest in the Path
family**. Awards the Khavatari Totem (1615), `OrcChange1`'s second proof.

## The weapon gate is the inverse of quests 401/403

Those demand a specific quest weapon in hand. This one demands the opposite:

```java
return ((weapon == null) || (weapon.getItemType() == WeaponType.FIST)
                         || (weapon.getItemType() == WeaponType.DUALFIST));
```

An Orc Monk fights unarmed, so **"no weapon" is the pass case** — exactly
inverted from `quest_common`'s tag, where an empty hand disqualifies you.
Routing this through the shared helper would have silently flipped the entire
quest: every bare-handed kill would have paid nothing and every sword kill
would have paid.

It also needs the weapon's **type**, not its id, so `QuestCtx::is_bare_or_fist_handed`
is added (reading `item_data.weapon_type`). The test covers all three cases —
bare-handed passes, a sword fails, a fist weapon passes.

The 0 → 1 → 2 state machine is otherwise familiar, keyed on
`Q00415_last_attacker` — a **third** distinct variable name after
`lastAttacker` (401/403) and `firstAttacker` (409).

## The pouch stages take five kills, not four

Java hands out a trophy per kill and converts the pouch when the count is
*already* 4:

```java
if (getQuestItemsCount(killer, KASHA_BEAR_CLAW) == 4) { …convert, consume 4… }
else { giveItems(killer, KASHA_BEAR_CLAW, 1); }
```

So the **fifth** kill is the one that fills the pouch. Reading it as "collect
4" leaves the pouch permanently unfillable — the conversion branch would never
be entered. The fourth pouch is the same shape spread over four mobs at three
each, converting once the combined count hits 11 (the twelfth kill). Both are
tested per-kill.

## Half the quest is unreachable — the same two-sided orphaning as 414

`30587-09c` sets `memoState = 2` and opens an entire alternate ending through
NPCs **31979** and **32056**: its own collection stages, a raid mob (Baar Dre
Vanul), and its own reward hand-out at `31979-03`. None of it is reachable:

- **`30587-09a.html` offers only the `09b` button** — nothing posts `09c`.
- **31979 and 32056 are registered nowhere**, here or in any shipped script, so
  their **13 pages** are orphaned.

Checked in both directions, as in 414, and for the same reason: had only the
serving end been missing, `09c` would be a trap — it consumes Rosheek's letter
and hands out no recommendation, stranding the player. Because neither end is
wired, the whole route ports verbatim at zero risk, with `TODO(dead)` markers
on the events, both dead kill handlers, and the `memoState == 2` talk branch.

That's now two of two Orc quests carrying a fully orphaned alternate route.
Worth expecting in 416.

## Tests

5 added, all green on first run: the three-case weapon gate; the five-kill
pouch (including the conversion consuming its four trophies and Rosheek
handing out the next pouch); the twelfth-kill fourth pouch; the dead-branch
assertion (13 orphaned pages ship, and the fork page offers only `09b`); and
page existence.

## Status

27 quests ported. `OrcChange1` needs 416 (Shaman, 525 lines) to complete the
Orc tier; then Dwarf (417 Scavenger 690, 418 Artisan 562) finishes the whole
first-occupation system.
