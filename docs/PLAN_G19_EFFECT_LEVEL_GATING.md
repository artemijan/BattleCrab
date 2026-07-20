# G19 — Per-effect level gating (`fromLevel`/`toLevel`/`subLevel`)

## Why this slice

Not from the effect ranking. The `Confuse`/`RandomizeHate` slice, while probing
`<effect>` attributes, found that the Rust skill parser reads **only** the
`name` attribute off an effect element and ignores the rest — including
`fromLevel`, `toLevel`, `fromSubLevel` and `toSubLevel`, which appear **775
times each** in this datapack.

Java uses them to attach an effect only to the skill levels its range covers.
Ignoring them meant every one of those effects was live at *every* level of its
skill. Measured before starting: **329 skills affected, 14 of them learnable.**

That outranks the remaining tied-at-3 entries on two counts: it is more skills,
and it is *already-ported effects behaving wrongly* rather than a missing
feature — a silent correctness bug, not a gap.

Concretely, Frenzy 176 declares two extra `PAtk` and two extra `CriticalRate`
effects at `fromLevel="6" toLevel="9"`. Every level-1 Frenzy was getting all
four.

## What Java does

`SkillData.parseNamedParamInfo` reads `name`, `level`, `fromLevel`, `toLevel`,
`subLevel`, `fromSubLevel`, `toSubLevel` — with `level` supplying the default
for *both* level bounds (so `level="3"` means exactly 3), and `subLevel`
likewise. Everything else on the element is ignored.

`SkillData.forEachNamedParamInfoParam` then gates:

```java
((fromLevel == null && toLevel == null) || (fromLevel <= level && toLevel >= level))
  && ((fromSubLevel == null && toSubLevel == null) || (fromSubLevel <= subLevel && toSubLevel >= subLevel))
```

Both bounds inclusive. **Sub-levels are the skill-enchant routes** (1001+ and
2001+); an unenchanted skill has sub-level 0, so an effect naming a sub-level
range never applies to it.

## What landed

- **`ParsedEffect`** replaces the six-wide tuple the parser had been carrying
  effects in — it was about to become ten-wide, and the struct gives the gate a
  place to be documented.
- **`effect_level_attrs`** reads the four attributes with Java's defaulting.
- **`ParsedEffect::applies_at(level)`** ports the gate verbatim, with
  `sub_level` fixed at 0 (no skill enchanting on this port), so every
  enchant-route effect is correctly excluded.
- Both consumers in `finalize_skill` — the effect list and the `over_hit`
  scan — now filter through it.

## Tests

`game_loop::tests::effect_level_tests` (6):

- `frenzy_gains_its_extra_patk_effects_only_from_level_six` and
  `the_level_range_is_inclusive_at_both_ends` — the headline fix, asserted by
  **counting** modifiers rather than checking presence. My first attempt
  checked presence and failed: Frenzy has three *ungated* `PAtk` effects
  alongside the two gated ones, so "does level 1 have PAtk" was the wrong
  question. Counting is both correct and stricter.
- `enchant_only_effects_never_apply_to_an_unenchanted_skill` and
  `guts_keeps_its_ungated_effect_and_drops_the_enchant_only_one` — the
  sub-level clause, including the case where the *level* clause passes and only
  the sub-level clause rejects.
- `an_ungated_effect_still_applies_at_every_level` — the no-regression case
  covering the vast majority of the datapack.

## A correction this slice forced

The regression sweep caught `mana_restore_tests::the_rest_of_the_family_parses`
failing — a test from the immediately preceding slice.

That slice's plan called Mortal Strike 410 "the one learnable `ManaHeal`" and
counted the cluster at 7 learnable skills. Both were wrong: Mortal Strike's
`ManaHeal` is `fromSubLevel="2001" toSubLevel="2020"`, an enchant-route effect
that never applies here. `ManaHeal` has **zero** reachable learnable skills on
this dist and the cluster's real reach was **6**.

The failing test is the fix working, not a regression. `PLAN_G19_MANA_RESTORE.md`
and the G19 PROGRESS row have both been corrected.

A sweep for the same error elsewhere found it affects only already-ported
effects (`PhysicalDefence` 63→59, `Speed` 55→52, `Heal` 18→16,
`MagicalDefence` 36→34, `PhysicalAttackSpeed` 43→42) — **no slice-selection
decision in this milestone would have changed.**

## Deferred (not this slice)

- **Skill enchanting** itself. When it lands, `applies_at` needs the real
  sub-level threaded through instead of the fixed 0, and the enchant-route
  effects start applying — the gate is already written for it.
