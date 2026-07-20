# G19 — PhysicalAttackRange skill effect

## Why this slice

A fresh ranking sweep after `DispelByCategory` closed out the previous
batch left `PhysicalAttackRange` (Archery, Long Shot, Rapid Fire, Snipe — 4
learnable skills, tied with several others) as the cheapest remaining pick: a
same-shape repeat of the already-solved `ShieldDefenceRate`/`AttackCancel`
pattern (a single-`Stat` `AbstractStatEffect`), with every piece of
infrastructure it needs — the generic `EFFECT_REGISTRY`, the weapon-condition
mask, `model::finalize` — already in place from prior slices. No new code
shape, just a missing table entry and one un-wrapped line.

## What Java does

`PhysicalAttackRange` is a plain single-stat `AbstractStatEffect` wrapping
`Stat.PHYSICAL_ATTACK_RANGE`, consumed by `PRangeFinalizer` — a
`defaultValue(creature, stat, calcWeaponBaseValue(...))` finalizer, the exact
same shape as `ShieldDefenceRate`'s/`AttackCancel`'s finalizers. All four
learnable instances are `<weaponType>BOW</weaponType>`-conditioned: Archery
(431, `DIFF +50`), Snipe (972, `DIFF +200`), Long Shot (113, level-scaled
`DIFF`), and Rapid Fire (413, `PER -50` — a stance that trades range for
reload speed while active).

## What landed

- **`Stat::PhysicalAttackRange`** (`model/stats.rs`) + an
  **`EFFECT_REGISTRY`** entry (`data/skill_data.rs`) — no bespoke match arm
  needed; the generic single-name-to-single-stat table already reads
  `amount`/`mode` and, critically, the `weaponType`/`armorType` condition
  mask (`armor_condition`/`weapon_condition` on `StatModifierEffect`) for
  *every* registry entry, not just specially-cased ones — so the four bow-
  conditioned skills needed nothing extra to gate correctly.
- **`recalculate_stats`' `combat.atk_range` line** (`model/mod.rs`) — was a
  bare `eq.weapon_atk_range.unwrap_or(t.base_atk_range)` with no stat
  modifier applied at all (the equivalent gap `ShieldDefenceRate` had before
  an earlier slice: parsed into `EFFECT_REGISTRY` already existing doesn't
  help if the finalizer line never calls `finalize()`). Now wrapped:
  `finalize(mods, Stat::PhysicalAttackRange, base) as i32`.
- Weapon-conditioned passives fold into `StatModifiers` at `Player::
  from_char` (spawn) and re-fold on every equip/unequip via
  `passive_skills::refresh_conditioned_passives` — both already-ported paths
  — so switching from a bow to a sword correctly drops Archery's bonus
  without any new wiring.

## Test

- `data::skill_data::tests::physical_attack_range_parses_diff_and_per_bow_
  conditioned` — real dist shapes inline: Archery's `DIFF +50` and Rapid
  Fire's `PER -50`, both confirmed bow-conditioned (`weapon_condition != 0`).
- `game_loop::tests::skills_tests::archery_passive_raises_bow_attack_range` —
  real dist data (item 14 "Bow", `pAtkRange` 500; skill 431 Archery, `+50`):
  a bare bow-wielder reports 500, an Archery-knowing one reports 550, and an
  unarmed Archery-knower reports the same base as an unarmed non-Archery
  character — proving the `<weaponType>BOW</weaponType>` gate actually gates.

## Deferred (not this slice)

Nothing — this was a complete, self-contained wire-up with no partial
behavior. `Long Shot`'s level-scaled `<amount>` table and `Snipe`'s plain
`DIFF +200` use the exact same parse/finalize path exercised by the tests
above and needed no separate handling.
