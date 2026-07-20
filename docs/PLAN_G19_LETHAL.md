# G19 — Lethal skill effect

## Why this slice

`AttackTrait` (7 learnable) stayed set aside a third time — it needs a whole
`TraitType` attacker-bonus/weakness system this port doesn't model anywhere
(the attacker-side trait map, plus wiring `calcGeneralTraitBonus`/
`calcWeaknessBonus`/`calcAttributeBonus` into every physical damage formula:
auto-attack, `PhysicalAttack`, `Blow`, `EnergyAttack`, …), a cross-cutting
project of its own rather than a slice. `Lethal` (9 learnable, 38 instances)
is next on the ranking and is the effect the codebase had already been
pointing at: `SkillEffect::Blow`'s own doc comment already carried "the
accompanying `Lethal` instant-kill effect is still dropped" as a TODO.

Every learnable instance pairs `Lethal` with an *already-ported* damage
effect on the same skill — Backstab (30, `Backstab`+`Lethal`), Lethal Blow
(344), Deadly Blow (263), Critical Blow (409, `FatalBlow`+`Lethal`), Lethal
Shot (343, `PhysicalAttack`+`Lethal`), Turn/Banish Undead/Seraph (1400/405/
450, `BlockControl`+`Fear`+`Lethal`) — so before this slice those skills'
damage landed but the bonus instant-kill/half-kill chance the skill is
actually *named* for never rolled.

## What Java does

`Lethal.instant()`: gated on the skill's level vs. the target's (`skill.
getMagicLevel() < target.getLevel() - 6` refuses silently), `isLethalable()`
(false only for `GrandBoss`/`RaidBoss`/`Door`), `isHpBlocked()`
(`DamageBlock`'s gate), and a Duelist-Fury asymmetry (a later-chronicle/event
mechanic, not in this datapack). Then three rolls in sequence: resist
(`INSTANT_KILL_RESIST` stat), full-lethal (`fullLethal · chanceMultiplier`),
half-lethal (`halfLethal · chanceMultiplier`) — `chanceMultiplier` is an
attribute/general-trait bonus. A landed full-lethal sets a player's CP *and*
HP to 1 (a monster's HP only); half-kill sets a player's CP to 1 (a monster's
HP to 50%). Always finishes with `calcCounterAttack` (a reflect-on-lethal
mechanic).

## What landed

- **`SkillEffect::Lethal { full_lethal, half_lethal }`** (`model/skill.rs`) +
  the `"Lethal"` parse arm (`data/skill_data.rs`) — both params are already
  0-100 percentages (unlike `AttackTrait`'s own effect, Java's `Lethal`
  constructor doesn't `/100` them).
- **Level gate** ported as-is (`skill.magic_level < target_level - 6`).
- **Raid-boss immunity** ported, reusing the exact same `is_raid()` check
  `apply_mute_interrupt` already has — `GrandBoss`/`Door` immunity isn't
  (neither concept exists on this port yet).
- **Full-lethal / half-lethal rolls and outcomes**, with `chanceMultiplier`
  at `1.0` (no trait/attribute math anywhere on this port, same simplification
  every other physical effect on this build already makes) — SM 1667/1668/
  2336/2337 (`LETHAL_STRIKE`/`HIT_WITH_LETHAL_STRIKE`/`HALF_KILL`/`YOUR_CP_
  WAS_DRAINED...`).
- **Not ported**: the resist roll (`INSTANT_KILL_RESIST` is never set by
  anything in this datapack — like `MAX_MOMENTUM` before it — so Java's own
  roll against it always loses and is skipped outright rather than rolled for
  show); `isHpBlocked()` (this port's `DamageBlock` gap, already noted on
  `HealPercent`); `calcCounterAttack`'s reflect (no counter mechanic modeled).

## Test

Two tests, both against the real dist datapack (skill 344 "Lethal Blow" —
`fullLethal` 0, `halfLethal` 15, paired with `FatalBlow`):

- `skills_tests::lethal_half_kill_sets_player_cp_to_1` — force-targets a
  second player (`ctrl`) so the CP-drained-to-1 assertion is decoupled from
  `FatalBlow`'s own HP damage, which lands first in the same effect list;
  every roll is flooded with `0` rather than pinning just the half-kill roll,
  since `FatalBlow`'s own land/crit rolls (and a spawned NPC's periodic AI
  think tick, in the sibling test) draw from the same queue ahead of it —
  the same lesson the `Force/charges` slice's test landed on.
- `skills_tests::lethal_spares_a_raid_boss` — a real dist raid boss (3404
  "Tracker Captain Sharuk", level 23, well under Lethal Blow's `magicLevel`
  76 so the separate level gate doesn't interfere): `FatalBlow`'s damage
  still lands, but a landed Lethal never gets to halve *what's left after
  that* on top of it.

## Deferred (not this slice)

- `AttackTrait` — still needs the `TraitType` system.
- `DamageBlock`'s `BLOCK_HP` gate, `calcCounterAttack`'s reflect,
  `GrandBoss`/`Door` lethal-immunity.
