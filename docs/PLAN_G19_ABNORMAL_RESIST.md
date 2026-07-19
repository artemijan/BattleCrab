# G19 — Abnormal resistance, blocking & probabilistic dispel

The third **G19** slice, after affect scopes & toggles
([PLAN_G19_EFFECTS.md](PLAN_G19_EFFECTS.md)) and the CC state flags
([PLAN_G19_ABNORMAL_STATES.md](PLAN_G19_ABNORMAL_STATES.md)). Where the last
one made stun and root *work*, this one makes them **resistable, blockable and
strippable** — the other half of the same system.

Java sources: `Formulas.calcEffectSuccess` (the `buffDebuffMod` term),
`handlers/effecthandlers/ResistAbnormalByCategory.java`,
`ResistDispelByCategory.java`, `BlockAbnormalSlot.java`,
`DispelBySlotProbability.java`, `EffectList.addBlockedAbnormalTypes`.

---

## 1. Picking the slice — and a false lead worth recording

The previous slice's survey ranked unported effects by **raw instance count**,
which put `StatUp` (887) near the top. That ranking is misleading: re-running it
restricted to skills that actually appear in this dist's **skill trees** (758
learnable ids) drops `StatUp` to **9 skills** — its 465-skill footprint is
almost entirely talismans, Freya and agathion content in the 8000–27000 id
ranges, none of it reachable on an Interlude server.

**Rank unported effects by learnable-skill usage, not raw instance count.** The
corrected top of the list (excluding the out-of-scope `DefenceAttribute` and the
summon/transform families that belong to other milestones):

| skills | effect |
|---|---|
| 27 | `ResistAbnormalByCategory` |
| 23 | `BlockAbnormalSlot` |
| 21 | `ResistDispelByCategory` |
| 18 | `DispelBySlotProbability` |

Those four are one mechanic, they back real Interlude skills (Guts, Ultimate
Defense, the Prophecies, the Bane family, Touch of Life/Death), and they sit
directly on top of machinery the previous slices built.

## 2. Debuff resistance

`ResistAbnormalByCategory` pumps `Stat::ResistAbnormalDebuff`, a **multiplier on
incoming debuff landing chance**: Guts 139 (`amount=-50`) → ×0.5, Touch of Death
342 (`amount=+30`) → ×1.3. `ResistDispelByCategory` pumps `ResistDispelBuff` the
same way for being dispelled (Ultimate Defense 110 → ×0.2).

Java is `mergeMul(stat, 1 + amount/100)`, which is *exactly* what this port's
`Per` modifier mode already does — so both map onto plain `StatModifier`s and
need no new machinery. **The mode has to be forced in the parser**, though: the
XML carries no `<mode>`, so the default `DIFF` read would turn Guts' `-50` into
"−50 percentage points" instead of "×0.5". Java's handlers also switch on
`<slot>` and implement only `DEBUFF` (resp. `BUFF`) — "only this one is in use
it seems" — so other slots pump nothing here either.

The stat reaches the roll in `calc_effect_land_rate`, which gained the
`buffDebuffMod` term. Order matters and is Java's: **multiply first, clamp
after** (`constrain(baseMod * … * buffDebuffMod, min, max)`), so a heavy
resistance can pull an otherwise-capped debuff below the 90 ceiling but never
under the 10 floor. `activate_rate == -1` (always-lands) still short-circuits
ahead of the whole formula, so resistance cannot block those.

> **`ResistDispelBuff` has no consumer yet.** Java reads it *only* in
> `Formulas.calcCancelSuccess` — the `Cancel` skill family, which is not
> ported. The stat is pumped and stored correctly; the consumer arrives with
> `Cancel`. It is deliberately **not** wired into `DispelBySlotProbability`,
> which Java does not resist-modify — inventing that would have been a silent
> parity bug.

## 3. `BlockAbnormalSlot`

While the buff is up, the listed abnormal types cannot land on the target at
all. This is what keeps two Prophecies off one character (Prophecy of Water 1355
blocks all five `BUFF_SPECIAL_*` slots) and backs Heroic Miracle 395
(`INVINCIBILITY`).

Implemented with the same **stamp-and-fold** pattern the CC flags introduced:
the blocked set rides on the `ActiveBuff` and is folded over the live buff list
when a new buff tries to land, rather than being cached on the creature and
invalidated. `apply_skill_effects` refuses any buff whose `abnormalType` is in
that set; `"NONE"` is the no-abnormal sentinel and is never blockable.

## 4. `DispelBySlotProbability`

The Bane family (Warrior Bane 1350 at 80 %, Mass Warrior Bane 1344 at 40 %):
like the existing `DispelBySlot`, but the roll is evaluated **per buff** inside
Java's predicate, so a 40 % mass Bane strips roughly two of five matching buffs
rather than all-or-nothing. The spec carries no per-type level (unlike
`DispelBySlot`'s `type,level` pairs), so every level of a listed type is a
candidate. The roll is only spent on buffs that actually match, keeping the RNG
stream tied to the buffs at risk as in Java.

Java also skips `isIrreplacableBuff()` effects; nothing on this dist sets that
flag, so it is not modelled (`TODO(G19)`).

## 5. Tests

Parse assertions against real datapack skills (`skill_data`): Guts 139
(`-50`, **PER** mode asserted explicitly), Touch of Death 342 (`+30`), Ultimate
Defense 110 (`ResistDispelBuff -80`), Prophecy of Water 1355 (all five
`BUFF_SPECIAL_*` slots), Might 1068 (blocks nothing), Warrior Bane 1350 (80 %)
and Mass Warrior Bane 1344 (40 %).

Behaviour (`game_loop/tests/resist_tests.rs`, 5 cases): the resist buff pumping
a ×0.5 multiplier; the landing formula at four points (unresisted, halved,
clamped-after-multiply at the ceiling, floored) plus the always-lands
short-circuit; blocked abnormal types being refused while unrelated ones land,
and blocking ending with the buff; a 100 % Bane stripping every matching buff
and nothing else; and a 0 % Bane stripping nothing, which is what proves the
roll is consulted at all.

> A test-authoring trap worth remembering: the first draft gave both dispellable
> buffs the abnormal type `SPEED_UP`, so the second **replaced** the first
> through the ordinary abnormal-stacking rules and the dispel assertions passed
> for the wrong reason. Distinct abnormal types are required to observe a
> multi-buff dispel.

## 6. What is still missing

`Cancel` (the skill family that would consume `ResistDispelBuff`),
`ResistAbnormalByCategory`'s non-DEBUFF slots (unused by Java too), and
`isIrreplacableBuff`. The wider G19 backlog is unchanged: `EFFECT_REGISTRY`
growth (now correctly ranked by learnable usage — `Transformation` 32,
`MpConsumePerLevel` 11, `ManaDamOverTime` 11, `TargetCancel` 10, `HealOverTime`
10, `Lethal` 9, `EnergyAttack` 9 lead the portable remainder), the CC effects
adjacent to the ported pair (`Fear`, `BlockControl`, `DebuffBlock`,
`DamageBlock`, `KnockBack`), the geometric affect scopes, `calcMagicSuccess`,
the AVE runtime, and skill enchanting.
