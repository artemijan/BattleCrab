# G19 — TwoHandedBluntBonus / TwoHandedSwordBonus

## Why this slice

The top remaining in-scope entry at 3 distinct learnable skills: Rage 94,
Frenzy 176 and Two-handed Weapon Mastery 293. Everything above it is out of
scope (`DefenceAttribute`, Kamael elemental) or G29 (the `Summon*` family).

Rage and Frenzy carry *both* the Blunt and the Sword variant — which is exactly
why the naive per-effect cluster count read 5 while only 3 distinct skills are
involved (corrected in the `ReflectSkill` slice).

## What Java does

Both handlers are the same class with a different weapon type, and both are
gated by **two static conditions**:

```java
new ConditionUsingItemType(WeaponType.BLUNT.mask());     // or SWORD
new ConditionUsingSlotType(ItemTemplate.SLOT_LR_HAND);   // two-handed
```

The handler declares **eleven** stat/mode pairs (pAtk, mAtk, both attack
speeds, both accuracies, both crit rates, both crit damages, speed). Checking
what the reachable content sets narrowed that sharply: the only pairs any of
the three skills use are **`pAtk` and `pAccuracy`**. So those two are read and
the rest keep their zero default — the same
scope-to-what-the-dist-reaches call the `TriggerSkillByAttack` slice made.

## What landed

- **`StatModifierEffect.two_handed`** — a *separate* condition axis from
  `weapon_condition`, because "a blunt" and "a two-handed weapon" are
  independent tests that both have to pass. Folded into the same place the
  armor/weapon conditions are evaluated.
- **`model::two_handed_weapon_equipped`** — reads the weapon template's
  `bodypart == SLOT_LR_HAND` rather than inferring two-handedness from an empty
  off-hand, which would wrongly match an unarmed or shield-less one-hander.
- **`impl Default for StatModifierEffect`**, so condition axes can keep being
  added without breaking every literal — the same investment `Skill` got last
  slice, and this time the conversion went cleanly in one pass using the
  single-line `qualifier:` anchor.

## Tests

`game_loop::tests::two_handed_tests` (7). Notable:

- `the_weapon_and_slot_conditions_are_separate_axes` — both are recorded, not
  conflated into one mask.
- `rage_and_frenzy_cover_both_weapon_families` — pins the both-variants shape
  that caused the earlier miscount.
- `rage_grants_nothing_at_level_one` — Rage declares `pAtkAmount = 0` at level
  1 and only starts granting at level 2. My first test asserted on level 1 and
  failed; a zero-amount modifier is dropped rather than stored, which is
  behaviourally identical to Java's `mergeAdd(stat, 0)`.
- `the_slot_condition_reads_the_weapon_bodypart` — checks the datapack premise
  the condition rests on.

## Deferred (not this slice)

- The nine unused stat pairs (mAtk, attack speeds, crit rates/damages, speed) —
  no skill on this dist sets them.
- The remaining `Trigger*` siblings (2 learnable) and the 2-learnable tail
  (`DefenceCriticalRate`, `Escape`, `Resurrection`, `ResistDDMagic`).
