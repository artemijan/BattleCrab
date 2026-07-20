# G19 — MagicalAttackMp (MP drain)

## Why this slice

The unconsumed-stat sweep came back clean again, so the name ranking decides.
With `SilentMove` taken last slice, `MagicalAttackMp` is the top in-scope entry
at 4 learnable — everything above it is out of scope (`DefenceAttribute`,
Kamael elemental) or G29 (`Summon`/`SummonCubic`/`SummonNpc`).

**Mana Burn 1398 and Mana Storm 1399 carry only this effect**, so both parsed
to an empty effect list and were dropped whole: the nukes cast, played their
animation and drained nothing. Aura Sink 1102 and Seal of Gloom 1210 pair it
with an already-ported `ManaDamOverTime`, so they landed but did none of the
up-front damage.

## What Java does

`MagicalAttackMp` is an instant effect with its own success gate and its own
damage formula — it shares neither with `MagicalAttack`.

`calcSuccess`: `effected.isMpBlocked()` refuses outright; then
`Formulas.calcMagicAffected`, which on failure messages both sides
(`YOUR_ATTACK_HAS_FAILED` / `C1_RESISTED_C2_S_DRAIN`) and bails.

```java
// calcMagicAffected
double defence = (skill.isActive() && skill.isBad()) ? target.getMDef() : 0;
double attack  = 2 * actor.getMAtk() * traitBonus;
double d = (attack - defence) / (attack + defence) + 0.5 * Rnd.nextGaussian();
return d > 0;
```

A *noisy* mAtk-vs-mDef comparison — a large edge pushes toward certainty
without ever reaching it.

```java
// calcManaDam
mAtk *= bss ? 4 * shotsBonus : sps ? 2 * shotsBonus : 1;
double damage = (Math.sqrt(mAtk) * power * (mp / 97)) / mDef;   // mp = target MAX MP
... if (magic failure) damage /= 2;
if (mcrit) { damage *= 3; damage = Math.min(damage, critLimit); }
```

Three things differ from the HP formula and are easy to get wrong:

- the target's **max MP is a direct multiplier**, so the same nuke drains far
  more from a mage than a fighter;
- spiritshots scale `mAtk` **before** the square root, so the gain is
  `sqrt(bonus)`, not `bonus`;
- a crit triples and then **clamps to a per-skill `criticalLimit`** (1600 on
  the two debuffs, 7000 on the two nukes) — a cap with no HP-side equivalent.

Also note there is **no `damage = 1` floor** on a full magic resist here, only
the halving, so `Resisted` and `Half` do the same thing. Ported as written.

## A wrong turn worth recording

I first read `<magicType>` out of the datapack, found it absent on all four
skills, concluded `isMagic()` was false, and built the crit on `calcCrit`'s
**physical** branch — adding a `Skill.magic_critical_rate` field to feed it.

A failing test caught it. The field is `<isMagic>`; this dist's schema has no
`<magicType>` tag at all, and all four skills are `<isMagic>1</isMagic>`. The
magic branch of `calcCrit` then **discards the rate it was passed** and reads
the caster's `MAGIC_CRITICAL_RATE` stat instead — so `<magicCriticalRate>` is
dead input, and the correct roll is exactly the per-cast `mcrit` the port
already computes. The speculative field was backed out entirely (it had
rippled into 15 test files for no benefit).

## A correction to an earlier slice

`MagicalAttackMp.calcSuccess` calls `effected.isMpBlocked()`. The `MP_BLOCK`
flag was documented in the `DamageBlock` slice as having **no callers anywhere
in the Java tree** — but that grep covered `java/` only, and every effect
handler lives under `dist/game/data/scripts/handlers/effecthandlers/`. Five of
them read it: `MagicalAttackMp`, `Mp`, `ManaHeal`, `ManaHealByLevel`,
`ManaHealPercent`. The flag is live, not dead code.

The doc comment is corrected and `abnormal::is_mp_blocked` now exists and is
wired here; a `TODO(G19)` marks the four MP-restore handlers to read it as they
land. **Lesson: grep both trees — `java/` *and* `dist/game/data/scripts/`.**

## What landed

- **`SkillEffect::MagicalAttackMp { power, critical, critical_limit }`** + parse
  arm.
- **`formulas::calc_mana_dam`** and **`formulas::calc_magic_affected`** (pure;
  the gaussian is passed in).
- **`World::roll_gaussian`** — Box–Muller over two `roll_f64` draws rather than
  a distribution crate, so tests can still force it through the same
  `forced_rolls` queue. Only the distribution matches Java, not the stream.
- **`abnormal::is_mp_blocked`** + the corrected `MP_BLOCK` doc.
- The effect arm: MP-block gate, `calcMagicAffected` with both failure
  messages, `calcShldUse` (perfect block → 1), the crit, the
  `min(currentMp, damage)` clamp, and all three success messages
  (`M_CRITICAL`, the victim's and the caster's).

## Tests

`game_loop::tests::mana_drain_tests` (12). The formula ones pin each term
separately — `a_bigger_mp_pool_is_drained_harder`,
`spiritshots_scale_matk_before_the_square_root`,
`a_crit_triples_the_drain_and_then_clamps_to_the_limit`,
`a_resisted_drain_is_halved_not_floored` — because none of them behave like the
HP formula. `magic_affected_compares_attack_against_defence` and
`the_gaussian_can_flip_the_verdict_either_way` cover the landing roll with the
gaussian pinned, then swung. `the_drain_skills_are_magic_skills` pins the
`<isMagic>` reading that the wrong turn above got backwards.

## Deferred (not this slice)

- **`stopEffectsOnDamage`** on the victim — a general buff-cancel-on-damage
  path, not specific to this effect.
- **`DUELIST_FURY`** in `calcSuccess` — unmodeled flag.
- **The `ManaHeal*` family** reading `isMpBlocked()` (`TODO(G19)`), pending
  those effects.
