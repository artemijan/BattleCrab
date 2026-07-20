# G19 — DispelByCategory skill effect (the "Cancel" family)

## Why this slice

A fresh ranking sweep after `hate-manipulation effects` closed out the
previous batch turned up a tied cluster of seven effect names at 4 learnable
skills each. Rather than pick by raw count alone (a genuine tie), a
cost/value pass across all seven favored `DispelByCategory`: it's the
long-awaited "Cancel" mechanic, the one place `Stat::ResistDispelBuff` —
pumped by `ResistDispelByCategory` since the G19 abnormal-resist slice — was
already flagged as "consumer-less until `Cancel` lands." `PhysicalAttackRange`
was the cheapest of the seven (a same-shape repeat of the already-solved
`ShieldDefenceRate` pattern) but offered no new value; `DispelByCategory`
closes a real, previously-documented gap instead.

## What Java does

`DispelByCategory.java` reads `slot` (`DispelSlotType`: `ALL`/`BUFF`/`DEBUFF`,
default `BUFF`), `rate`, `max`, then calls
`Formulas.calcCancelStealEffects(effector, effected, skill, slot, rate, max)`
and force-removes whatever it returns via `stopSkillEffects`.

`calcCancelStealEffects`:
- `BUFF` slot: walks the target's dances (`getDances()`) in **reverse cast
  order** first, then — only if still under `max` — walks buffs
  (`getBuffs()`) reverse. Each candidate needs `canBeStolen()` (not passive/
  toggle/debuff/irreplaceable/hero/GM/static, and `canBeDispelled`) **and**
  (`rate>=100` **or** `calcCancelSuccess` passes).
- `DEBUFF` slot: walks debuffs reverse; each candidate needs
  `canBeDispelled()` **and** a flat `Rnd.get(100) <= rate` roll — no magic-
  level math at all.
- `ALL` isn't handled by any switch case — dead code, no shipped skill uses
  it.

`calcCancelSuccess`: `chance = clamp(rate + (casterMagicLvl -
buffMagicLvl)*2 + (buffAbnormalTime/120)*Stat.RESIST_DISPEL_BUFF, 25, 75)`,
then `Rnd.get(100) < chance`. This is the sole Java consumer of
`Stat.RESIST_DISPEL_BUFF`.

Per-skill params (`dist/game/data/stats/skills/`): Cancellation (1056) and
Touch of Death (342) are `BUFF`/`rate=25`/`max=5`; Cleanse (1409) and
Purification Field (1425) are `DEBUFF`/`rate=100`/`max=10`.

## What landed

- **`DispelSlot` enum** (`Buff`/`Debuff`/`All`) + **`SkillEffect::
  DispelByCategory { slot, rate, max }`** (`model/skill.rs`) + the parse arm
  (`data/skill_data.rs`), following `DispelBySlotProbability`'s established
  `value_at(params, key, level)` shape.
- **The full `BUFF`/`DEBUFF` walk** (`game_loop/skills/effects.rs`): dead
  targets are skipped up front (`Vitals.dead`); candidates are snapshotted
  from `Buffs` in reverse order (`.iter().rev()`), matching Java's newest-
  cast-first traversal. `BUFF` slot filters `ActiveBuff::slot == Dance` then
  `== Buff` — which, by construction (`Skill::buff_slot()`), already
  excludes passive/toggle/debuff, covering most of `canBeStolen()` for free;
  only `can_be_dispelled` needed an explicit check. `DEBUFF` slot filters on
  the buff's source skill's `is_debuff` flag (looked up via `skill_data`,
  the same pattern `DispelBySlot`/`DispelBySlotProbability` already use for
  reading a candidate buff's own skill metadata).
- **`calcCancelSuccess` ported for the `BUFF` path**: `Stat::
  ResistDispelBuff`'s finalized value (via `model::finalize`, base 1.0) folds
  into the same clamp-to-`[25,75]` formula; `rate>=100` (Cancellation/Touch
  of Death never hit this — both are 25 — but the branch is real) skips the
  roll entirely, matching Java's short-circuit.
- **`DEBUFF` path**: a flat `world.roll(100) <= *rate` (note `<=`, not `<` —
  matches Java's operator exactly, unlike the project's other per-item rolls
  which are all `<`).
- **`ALL` slot**: a deliberate no-op, matching Java's own dead branch.

## Test

- `data::skill_data::tests::dispel_by_category_parses_slot_rate_max` — real
  dist shapes: Cancellation (`BUFF`/25/5), Cleanse (`DEBUFF`/100/10).
- `game_loop::tests::skills_tests::dispel_by_category` (2 tests):
  `buff_slot_prefers_dances_and_respects_cant_dispel` — three landed buffs
  (a dance, a regular buff, an undispellable regular buff), `max=1` at
  `rate=100`; only the dance is stolen, proving both the dance-before-buff
  order and the `can_be_dispelled` gate. `debuff_slot_strips_only_debuffs` —
  a debuff and a positive buff both landed; `Cleanse` (`DEBUFF`/100) strips
  only the debuff.

## Deferred (not this slice)

- `isIrreplacableBuff()`/hero-skill/GM-skill/static-skill exclusions from
  `canBeStolen()` — none of these fields exist on the ported `Skill` struct
  yet (same gap `DispelBySlotProbability` already left a TODO for; no
  learnable skill on this dist needs them to behave correctly).
- `ALL` slot — genuinely unreachable in Java too.
