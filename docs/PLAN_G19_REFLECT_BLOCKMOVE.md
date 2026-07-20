# G19 — ReflectSkill + BlockMove

## Why this slice

Re-running the ranking after `TriggerSkillByAttack` left three clusters tied.
The previous slice's "5 learnable" for `TwoHanded*` turned out to be an
artifact: skills 94 and 176 carry *both* `TwoHandedBluntBonus` and
`TwoHandedSwordBonus`, so counting per-effect double-counted them. Counting
**distinct learnable skills** instead:

| cluster | distinct skills |
|---|---|
| `TwoHanded*` | 3 (Rage 94, Frenzy 176, Two-handed Weapon Mastery 293) |
| `Reflect*` | 3 (Riposte Stance 340, Physical Mirror 350, Magical Mirror 351) |
| `Block*` | 3 (Ultimate Defense 110, Snipe 313, Vengeance 368) |
| `Trigger*` | 2 |

A genuine three-way tie, so this slice takes two of them — both defensive-stance
effects, both small, and both closing something already documented:

- **Physical Mirror 350 and Magical Mirror 351 carry nothing but
  `ReflectSkill`**, so both were dropped whole.
- **`BlockMove` is the `_isImmobilized` source** that `game_loop::abnormal`'s
  own module docs listed as having "no ported source".

## What Java does

**`BlockMove`** is `setImmobilized(true)` on start, `false` on exit — a pure
state flag. `isMovementDisabled()` ORs it beside `ROOTED`, so the creature is
pinned but can still attack and cast, which is the point of these stances.

**`ReflectSkill`** is a `pump` — `mergeAdd(REFLECT_SKILL_PHYSIC|MAGIC, amount)`.
Despite the name it is **not** damage reflection: its only consumer is

```java
// Formulas.calcBuffDebuffReflection
if (!skill.isDebuff() || (skill.getActivateRate() == -1)) return false;
return target.getStat().getValue(skill.isMagic() ? REFLECT_SKILL_MAGIC : REFLECT_SKILL_PHYSIC, 0) > Rnd.get(100);
```

called from `Skill.applyEffects`, where a successful roll **swaps the roles** —
`applyEffects(target, caster, …)` — so the debuff lands on its own caster.

Two gates before the roll: the skill must be a debuff, and it must declare an
`activateRate` (one with the default `-1`, i.e. always-lands, is never
reflected). Which stat is read depends on the *incoming skill's* `isMagic`, not
on the defender.

## What landed

- `Stat::ReflectSkillPhysic` / `ReflectSkillMagic`, `effect_flag::IMMOBILIZED`,
  and the two `SkillEffect` variants with their parse arms.
- `ReflectSkill` is expressed as an ordinary `StatModifierEffect` in
  `stat_modifier_effects()`, so it rides the existing buff/passive pipeline
  rather than needing its own plumbing — which is exactly what Java's `pump` is.
- `abnormal::is_movement_disabled` now ORs `IMMOBILIZED`, and its module docs
  are corrected: `_isImmobilized` is no longer sourceless.
- `calc_buff_debuff_reflection` at the per-target apply loop in
  `skills::cast`, with the role swap. The hate/PvP consequences stay
  unconditional — the caster still *cast* a bad skill at that target, reflected
  or not.

## Three things the data corrected

1. **`type` is `MAGIC`, not `MAGICAL`.** I guessed the latter; `BasicProperty`
   is `NONE`/`PHYSICAL`/`MAGIC`. This was a **real bug**, not just a wrong test
   — it would have routed every magic reflect into the physical stat. Caught by
   a failing assertion, and now pinned by one.
2. **Both Mirrors carry two `ReflectSkill` effects each**, physical and magic,
   weighted 30/10 and 10/30. They differ by emphasis, not by kind.
3. **Their `<armorTYpe>SHIELD</armorTYpe>` gate is a datapack typo** (10
   occurrences against 220 correct `<armorType>`). Java matches element names
   exactly too, so the shield condition is inert on both sides — faithfully
   reproduced by not special-casing it.

## Noted, not fixed

**The parser reads only the default `<effects>` block.** Vengeance 368 puts its
`BlockMove` in `<selfEffects>`, so the skill's other effects load and the
immobilise silently does not. Datapack-wide:

| scope | skills | learnable |
|---|---|---|
| `selfEffects` | 91 | 7 |
| `endEffects` | 58 | 1 |
| `pvpEffects` | 38 | 1 |
| `pveEffects` | 33 | 1 |
| `channelingEffects` | 24 | 4 |
| `startEffects` | 3 | 0 |

~14 learnable skills affected — comparable in reach to the `fromLevel`/`toLevel`
gap the level-gating slice fixed, and a strong candidate for its own slice.
`vengeance_block_move_is_in_an_unread_effect_scope` documents it and will start
failing when it lands, which is the point.

## Tests

`game_loop::tests::reflect_tests` (8). Notable:
`an_immobilised_creature_can_still_act` (immobilise is not a stun);
`the_mirrors_carry_only_reflect_effects` (the two-of-each shape);
`reflect_skill_folds_into_an_additive_stat` (asserts Physical Mirror carries a
*magic* share too — the assertion that would fail under the `MAGICAL` bug);
`vengeance_block_move_is_in_an_unread_effect_scope` (the gap above).

## Deferred (not this slice)

- Java's extra `applyEffectScope(GENERAL, …)` block after a reflection, which
  re-applies instant effects to the original target — it would double-apply
  them here, and the port has no effect-scope split to hang it on.
- `ReflectMagic` (0 learnable), `TwoHanded*`, and the remaining `Trigger*`
  siblings.
