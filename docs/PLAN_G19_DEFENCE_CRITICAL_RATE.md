# G19 — DefenceCriticalRate

## Why this slice

The direct mirror of the crit-*damage* slice, and the largest remaining
in-scope entry (2 learnable, 50 skills): Light Armor Mastery 233 (`-15% PER`)
and Pa'agrio's Eye 1364 (`-30%`) make their holder harder to crit.

The port computed the autoattack crit chance as a bare `crit_stat / 10.0`, so
the defender's side of the roll did not exist and both were inert.

## What Java does

`Formulas.calcCrit`'s autoattack branch:

```java
final double criticalRateMod = (target.getStat().getValue(Stat.DEFENCE_CRITICAL_RATE, rate)
                              + target.getStat().getValue(Stat.DEFENCE_CRITICAL_RATE_ADD, 0)) / 10;
rate = criticalLocBonus * criticalRateMod * criticalHeightBonus;
rate = constrain(rate, 3, 97);
```

The two-arg `getValue(stat, rate)` is `mul * rate + add`, so the **defender's**
multiplier scales the **attacker's** rate rather than standing on its own.
Reading it the other way round would turn the stat into a flat chance instead of
a reduction.

`DefenceCriticalRate` is an `AbstractStatEffect` over the same mul/add pair as
`CriticalDamage`, so the parse arm mirrors that one exactly.

## What landed

- `Stat::DefenceCriticalRate` / `DefenceCriticalRateAdd` + the parse arm.
- `formulas::calc_auto_attack_crit` gained `defence_mul`/`defence_add`
  parameters, defaulting to Java's identity `1.0`/`0.0` — which reproduces the
  old expression exactly, so the existing combat tests keep meaning what they
  meant.
- `combat::defence_crit_rate` reads them off the **target** at the swing.

## Tests

`game_loop::tests::defence_crit_tests` (7). Notable:

- `the_defenders_multiplier_scales_the_attackers_rate` — the two-arg `getValue`
  semantics, which is the one thing that could be silently inverted.
- `the_add_term_lands_before_the_divide` — the `_ADD` term is worth ten times
  its face value in percentage points because of where the `/10` sits.
- `identity_defences_reproduce_the_old_formula` — the behaviour-preservation
  guarantee.
- `light_armor_mastery_is_armor_conditioned` / `paagrios_eye_folds_unconditionally`
  — the contrasting pair (see below).

Two corrections the tests forced, both mine rather than the code's:

1. **`calc_critical_height_bonus(0, 0)` is 1.1, not 1.0** (Java's `+10` before
   the `/100`), so even the "plainest" case carries a multiplier. All the
   expected values were recomputed and the reason is written into the test.
2. **Light Armor Mastery is armor-conditioned.** I expected it to fold onto a
   naked character; it correctly contributes nothing without light armour. The
   test now asserts the modifier at the parsed-effect level *and* that the gate
   holds, with Pa'agrio's Eye as the unconditioned contrast.

## Deferred (not this slice)

- `DEFENCE_MAGIC_CRITICAL_RATE` — the magic twin; 0 learnable grantors here.
- The level-difference term in Java's autoattack crit (`creature.getLevel() >=
  78`), which no actor on this dist reaches.
