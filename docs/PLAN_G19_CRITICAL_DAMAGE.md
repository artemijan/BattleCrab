# G19 — Critical damage stats

## Why this slice

The name-based ranking left only a two-way tie at 4 learnable
(`MagicalAttackMp`, `SilentMove`). But the previous slice's post-mortem
flagged that **the ranking is structurally blind to "parsed but unconsumed"
stats** — it counts unported effect *names*, so anything already in
`EFFECT_REGISTRY` or a match arm scores as done even if nothing reads the stat
it pumps.

So this time I ran that check first: for every `Stat` variant, count references
outside `stats.rs`/`skill_data.rs`. Exactly two came back with **zero
consumers** — `CriticalDamage` and `CriticalDamageAdd`.

All three damage formulas hard-coded the crit multiplier:

```rust
let attack = (p_atk * random_mul + prox_bonus) * ss_bonus * if crit { 2.0 } else { 1.0 } * 77.0;
```

So **18 learnable skills** were completely inert, including some of the most
used buffs in the game: Death Whisper 1242, Focus Attack 317, Vicious Stance
312, Frenzy 176, Dance of Fire 274, Zealot 420, Dead Eye 414, Chant of Victory
1363, Prophecy of Fire 1356. Pulling the thread gathered the rest of the
family:

| effect | learnable | role |
|---|---|---|
| `CriticalDamage` | 18 | the multiplier / flat add |
| `CriticalDamagePosition` | 3 | position-qualified multiplier (also on the ranking) |
| `MagicCriticalDamage` | 2 | the magic-crit branch |
| `DefenceCriticalDamage` | 1 | target-side vulnerability |

**24 learnable skills**, and a coherent mechanic: "crit damage stats become
real".

## What Java does

`Formulas.calcCritDamage` returns the multiplier and `calcCritDamageAdd` the
flat bonus, each with three branches (magic skill / physical skill / autoattack):

```java
// autoattack branch
criticalDamage = getValue(CRITICAL_DAMAGE, 1) * getPositionTypeValue(CRITICAL_DAMAGE, position);
defenceCriticalDamage = target.getValue(DEFENCE_CRITICAL_DAMAGE, 1);
return 2 * criticalDamage * defenceCriticalDamage * balanceMod;   // balanceMod 1 here
```

`calcAutoAttackDamage` then applies them with load-bearing bracketing:

```java
attack = (((attack * cAtk * ssBonus) + cAtkAdd) * critMod) * 77
       + (attack * (1 - critMod) * ssBonus * 77);
```

`critMod` is 1 on a melee crit and 0 otherwise, so a crit takes the first term
and a non-crit the second. Note `cAtkAdd` lands **after** the soulshot multiply
and **inside** the ×77 — it is not scaled by shots, but it *is* amplified by
the weapon mod and divided by pDef, which makes a flat +32 worth far more than
it looks.

`CriticalDamagePosition` merges `(amount/100)+1` **multiplicatively** into
`_positionTypeStats` — a different map, merge and identity from the move-type
one added last slice.

## What landed

- **Four new `Stat` variants**: `DefenceCriticalDamage(Add)`,
  `MagicCriticalDamage`, `DefenceMagicCriticalDamage`, with parse arms for
  `DefenceCriticalDamage`/`MagicCriticalDamage`/`DefenceMagicCriticalDamage`.
- **`StatQualifier`** — last slice's `StatModifierEffect.move_type` field
  generalised to an enum, rather than growing a second parallel `Option` that
  could rot:

  | variant | Java map | merge | identity |
  |---|---|---|---|
  | `MoveType` | `_moveTypeStats` | add | `0.0` |
  | `Position` | `_positionTypeStats` | multiply | `1.0` |

  The two maps stay separate on `StatModifiers` precisely because the merges
  and identities differ, mirroring Java.
- **`formulas::CritDamage { mul, add }`** with `Default` = Java's stat-free
  `2.0`/`0.0`, so the refactor is provably behaviour-preserving for an actor
  with no crit buffs (pinned by a test).
- **`calc_auto_attack_damage`** rewritten to Java's two-section expression;
  **`calc_physical_skill_damage`** and **`calc_magic_dam`** take their crit
  multiplier instead of hard-coding 2.
- **`combat::crit_damage_auto`** reads the stats off both actors plus the
  position term; **`combat::crit_damage_skill`** covers the two skill branches.

## Tests

`game_loop::tests::crit_damage_tests` (10). The ones worth naming:

- `crit_stats_do_not_touch_a_normal_hit` — a non-crit must ignore cAtk/cAtkAdd,
  or every crit-damage buff silently becomes a flat damage buff.
- `default_crit_damage_reproduces_the_old_hard_coded_double` — the
  behaviour-preservation guarantee the pre-existing damage tests rest on.
- `crit_multiplier_and_flat_add_follow_javas_bracketing` — pins that `cAtkAdd`
  is inside the ×77 but outside the soulshot multiply.
- `focus_death_penalises_frontal_crits_and_rewards_backstabs` — Focus Death 355
  carries **two** position entries with opposite signs (front −30% → ×0.7, back
  +90% → ×1.9). The asymmetry only survives because the position map
  multiplies; treating −30 as additive would be nonsense.
- `position_qualified_stats_multiply_from_one` — absent reads as 1.0, not 0.0.

Two of my initial assumptions were wrong and the data corrected them: Focus
Death's front entry is *negative*, and skill 193 "Critical Damage" is `mode=DIFF`
(a flat +32 cAtkAdd), not a percentage. Both are now pinned.

## Deferred (not this slice)

- **`PHYSICAL_SKILL_CRITICAL_DAMAGE`** — no learnable skill on this dist grants
  it (40 non-learnable ones do), so that branch stays the stat-free 2.0, per the
  `BLOW_RATE_DEFENCE`/`MP_BLOCK` precedent of not inventing plumbing for a stat
  nothing reachable sets.
- **`MAGIC_CRITICAL_DAMAGE_ADD`** — Java computes it in `calcCritDamageAdd` but
  `calcMagicDam` never applies it (there is a TODO on that line in Java too).
- **`calcBlowDamage`'s** own `cdMult`/`cdPatk` shape — the blow path has a
  different formula and its own identity simplifications; a separate slice.
- **`MagicalAttackMp` and `SilentMove`**, the name-ranking's remaining tie.
