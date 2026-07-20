# G19 — ResistDDMagic (MAGIC_SUCCESS_RES)

## Why this slice

Anti Magic 146 and M. Def. 147 — 2 learnable, 38 skills — are mage-defence
passives that make incoming spells more likely to be resisted.

More interestingly, this slice **corrects a wrong claim the port already
carried**. `calc_magic_success_rate`'s doc comment said:

> Java's `resModifier` (`getMul(MAGIC_SUCCESS_RES, 1)`) is fixed at 1.0 here.
> The only two dist items touching `magicSuccRes` (10207/10208, the enhanced
> shirts) declare it in a `<stats>` block, which Java parses into an *additive*
> func — `getMul` never sees it, so the term is 1.0 on this dist for Java too.

That reasoning is right about the **items** and wrong about the conclusion,
because it never considered **skills**. `ResistDDMagic` is an
`AbstractStatPercentEffect`, which merges *multiplicatively* — precisely what
`getMul` reads. The term was never 1.0.

Same failure mode as the `MP_BLOCK` correction two slices back: a
"provably inert" note that was only as good as the search behind it.

## What Java does

```java
// Formulas.calcMagicSuccess, "general magic resist"
final double resModifier = target.getStat().getMul(Stat.MAGIC_SUCCESS_RES, 1);
final int rate = 100 - Math.round((float) (mAccModifier * lvlModifier * targetModifier * resModifier));
```

It scales the **failure** term, so a value above 1 *lowers* the attacker's
success rate. Inverting that would turn a defensive passive into an offensive
one.

## What landed

- `Stat::MagicSuccessRes` + an `EFFECT_REGISTRY` entry (the effect is a plain
  single-stat percent effect, so the generic wiring suffices).
- `MagicSuccess.res_modifier`, read off the **target**, defaulting to 1.0 —
  which reproduces the previous expression exactly.
- The stale doc comment replaced with the correction, so the next reader
  doesn't re-derive the wrong conclusion.

## Tests

`game_loop::tests::magic_resist_tests` (5):

- `an_identity_res_modifier_changes_nothing` — behaviour preservation.
- `a_higher_res_modifier_lowers_the_success_rate` and its mirror — the
  direction, which is the one thing that could be silently inverted.
- `real_dist_carriers_parse` — Anti Magic is `0` at levels 1-2 and `5` from
  level 3, so a level-1 assertion would prove nothing (the same shape as Rage
  and Resurrection).
- `anti_magic_folds_a_raising_multiplier` — the passive path, asserting `> 1.0`.

One correction the tests forced: the step table bands on
`magic_accuracy - magic_evasion` as `> -20 → 2, > -25 → 30, > -30 → 60,
> -35 → 90`. My first fixture used a −31 deficit thinking it sat in the 60
band; it lands in 90. The fixture now uses −26 with the table written out
beside it.

## Deferred (not this slice)

- The `<stats>`-block items (10207/10208) remain additive and so remain
  invisible to `getMul` — correctly, and in Java too.
