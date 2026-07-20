# G19 — FatalBlowRate skill effect

## Why this slice

A fresh ranking sweep after `PhysicalAttackRange` closed out the previous
batch left `FatalBlowRate` (Assassination, Critical Blow, Focus Death,
Mortal Strike — 4 learnable skills) in the same tied-at-4 cluster. Directly
tied to the already-ported `Blow`/`Lethal`/`FatalBlow` combat mechanics from
earlier slices: `formulas::calc_blow_success`'s own doc comment flagged
`BLOW_RATE`/`BLOW_RATE_DEFENCE` as hardcoded identity, so this closes a real,
already-documented gap rather than adding an isolated new stat.

## What Java does

`Formulas.calcBlowSuccess`:
```java
final double blowRateMod = creature.getStat().getValue(Stat.BLOW_RATE, 1);
final double blowRateDefenseMod = target.getStat().getValue(Stat.BLOW_RATE_DEFENCE, 1);
final double rate = criticalPosition * critHeightBonus * weaponCritical
    * chanceBoostMod * blowRateMod * blowRateDefenseMod;
return Rnd.get(100) < Math.min(rate, Config.BLOW_RATE_CHANCE_LIMIT);
```
`FatalBlowRate` is a plain single-stat `AbstractStatEffect` wrapping
`Stat.BLOW_RATE`, `mode=PER` on every learnable instance: Assassination (432,
`+3`, unconditioned passive), Focus Death (355, `+60`, self-buff), Critical
Blow (409, per-level `10..30`, self-effect), Mortal Strike (410, per-level
`10..30`, self-buff). `FatalBlowRateDefence`/`Stat.BLOW_RATE_DEFENCE` exists
as a registered Java handler but **no shipped skill grants it** — grepped the
whole datapack, matching the project's recurring `MAX_MOMENTUM`/
`INSTANT_KILL_RESIST` "dead in Java too" pattern.

## What landed

- **`Stat::BlowRate`** (`model/stats.rs`) + an **`EFFECT_REGISTRY`** entry
  (`data/skill_data.rs`) — the same generic single-name-to-single-stat wiring
  as `PhysicalAttackRange`, no bespoke match arm.
- **`formulas::calc_blow_success`** gained a `blow_rate_mod: f64` parameter,
  multiplied into `rate` alongside the existing terms, mirroring Java's
  `blowRateMod` term exactly. `Stat.BLOW_RATE_DEFENCE` stays unmodeled — not
  ported, since nothing grants it.
- **The one production call site** (`game_loop/skills/effects.rs`'s
  `SkillEffect::Blow` arm) reads the caster's finalized `StatModifiers.mul`
  entry for `Stat::BlowRate` (default 1.0, matching Java's `getValue(..., 1)`
  default) and threads it through.

## Test

- `model::formulas::tests::blow_success_rate_cap_and_threshold` — extended
  with an Assassination-shaped case: a `blow_rate_mod=1.03` (the real `+3%
  PER`) shifts the previously-computed rate-11 boundary to 11.33, so roll 11
  now lands (it didn't at the old identity 1.0) while roll 12 still doesn't.
- `game_loop::tests::skills_tests::assassination_passive_raises_blow_rate_
  stat` — real dist data (skill 432, unconditioned): no skill means no
  modifier entry at all; Assassination folds in `×1.03` via `StatModifiers`,
  the same value the formula test's boundary shift exercises.

## Deferred (not this slice)

- `Stat.BLOW_RATE_DEFENCE`/`FatalBlowRateDefence` — genuinely dead in Java
  too (no shipped skill grants it).
