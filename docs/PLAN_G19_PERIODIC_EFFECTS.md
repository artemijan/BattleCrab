# G19 — Periodic HP/MP effects, healing modifiers & CP

The fourth **G19** slice, after affect scopes & toggles
([PLAN_G19_EFFECTS.md](PLAN_G19_EFFECTS.md)), the CC state flags
([PLAN_G19_ABNORMAL_STATES.md](PLAN_G19_ABNORMAL_STATES.md)) and abnormal
resistance ([PLAN_G19_ABNORMAL_RESIST.md](PLAN_G19_ABNORMAL_RESIST.md)).

Java sources: `handlers/effecthandlers/HealOverTime.java`,
`ManaDamOverTime.java`, `HealEffect.java`, `Cp.java`, `Heal.java` (the
`HEAL_EFFECT` application), `BuffInfo.onTick`.

---

## 1. Why this next

Ranking unported effects by **learnable-skill usage** (the correction from the
previous slice) and setting aside the out-of-scope `DefenceAttribute` and the
summon/transform families that belong to other milestones, the top of the list
is a single coherent family:

| learnable skills | effect |
|---|---|
| 11 | `ManaDamOverTime` |
| 10 | `HealOverTime` |
| 9 | `HealEffect` |
| 7 | `Cp` |

They also close a loop: `Fury Fists` 222, `Silent Move` 221 and `Arcane Wisdom`
336 are **toggles** — ported in the first G19 slice, but until now free. Their
whole cost is a periodic HP/MP drain, so toggles had upside with no upkeep.

## 2. Periodic HP/MP — riding the existing tick chain

Rather than adding schedulers, both effects join the `DamOverTimeTick` chain the
poison/bleed port already built (`ticks * 666 ms` interval,
`power * ticksMultiplier` per tick, self-terminating when the buff is gone or
the target dies).

**`HealOverTime` is not a heal-only effect.** Its `power` is routinely
*negative* on this dist (Fury Fists `-12`, Arcane Wisdom `-50`): those are
toggles paying an HP upkeep. The two branches are Java's:

- `power > 0`: skip the tick at full HP, otherwise add and cap at max HP.
- `power <= 0`: subtract and **floor at 1** — an upkeep never kills its owner.

Java's negative-power early bail (`hp - _power <= 0`) can never fire for a
negative power (it reads as `hp + |power| <= 0`); it is ported as written rather
than "corrected", since the datapack is the spec and the floor already does the
protecting.

**`ManaDamOverTime`** drains MP, and when a tick's drain exceeds current MP on a
**toggle** it switches the toggle off and sends "Your skill was deactivated due
to lack of MP". That is Java's `onActionTime` returning `false`, which
`BuffInfo.onTick` honours *only for toggles* — a non-toggle drain simply floors
at 0 and keeps ticking. Both behaviours are tested.

## 3. `HealEffect` and `Cp`

`HealEffect` is a two-stat `AbstractStatEffect` (`HEAL_EFFECT` multiplicative /
`HEAL_EFFECT_ADD` additive, selected by `mode`), applied in `Heal` as
`amount = amount * HEAL_EFFECT + HEAL_EFFECT_ADD` — read off the **recipient**,
not the healer. Touch of Life 341 (`+30 PER`) raises received healing; Touch of
Death 342 (`-30 PER`) cuts it.

`Cp` is an instant CP change with Java's `DIFF`/`PER` modes, the gain capped at
the recoverable headroom. Braveheart 440 grants a flat `+1000`; Wrath 320 and
Touch of Death 342 take CP away.

## 4. The guard, for the third time

`apply_skill_effects` drops buffs whose effect list produces no stat modifier.
Stun/root needed an exemption last slice; `BlockAbnormalSlot` needed one the
slice before; and both periodic effects needed one here — **and the failure mode
is silent**: the buff never lands, so the tick chain never starts, so the effect
simply does nothing.

Rather than adding a third special case, the check is now a single
`has_periodic` covering every effect whose work happens on the tick chain. The
guard now reads as three honest categories: *periodic*, *icon-only*, *state
flag*. **Any future effect that carries no stat modifier must join one of
them.**

## 5. Tests

Parse assertions against real skills (`skill_data`): Fury Fists 222
(`HealOverTime` power `-12`/2 ticks, and asserted to be a toggle), Silent Move
221 (`ManaDamOverTime` 9/5), Braveheart 440 (`Cp +1000 DIFF`), Touch of Death
342 (`Cp -90 PER`), Touch of Life 341 (`HealEffect +30` on the multiplicative
stat).

Behaviour (`game_loop/tests/periodic_tests.rs`, 7 cases): a positive HoT healing
and capping at full; a **negative** HoT draining and flooring at 1 without
killing; an MP upkeep draining; a toggle switching itself off when MP runs out
*with* the system message; a non-toggle drain flooring at 0 and continuing;
`HealEffect` reducing received healing (via the target's stat, with the ×0.5
multiplier asserted); and `Cp` restoring, draining, and never exceeding the
pool.

## 6. What is still missing

`HealPercent` and the potion-specific `ADDITIONAL_POTION_HP`/`_CP` bonuses;
`Heal`'s magic-crit ×3 branch; `getMaxRecoverableHp` (this port caps at plain
max HP, which differs only once HP-block/vitality-cap effects exist).

The wider G19 backlog, by learnable-skill usage: `Transformation` (32, partly
G13), `MpConsumePerLevel` (11), `TargetCancel` (10), `EnergyAttack` (9),
`Lethal` (9), `BlockControl` (9), `Fear` (9, needs flee AI), `StatUp` (9),
`ShieldDefence` (8), `AttackTrait` (7), `Mute` (6), `DebuffBlock` (6) — the last
two map onto `effect_flag` bits and would be cheap follow-ons to the CC slice.
Also still open: the geometric affect scopes, `calcMagicSuccess`, the AVE
runtime, and skill enchanting.
