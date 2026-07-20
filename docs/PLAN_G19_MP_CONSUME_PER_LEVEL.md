# G19 — MpConsumePerLevel skill effect

## Why this slice

Continuing the learnable-skill ranking after `Transformation`: `DefenceAttribute`
(33, out of scope) and `Transformation` (32, landed) were ahead of it;
`Summon`/`SummonCubic`/`SummonNpc` (24/12/9) are G29 territory; `StatUp` (9,
887 raw instances) was already written off in an earlier slice as mostly
talisman/Freya/agathion content; `Fear` (9) is explicitly deferred to G21 (it
needs forced-flee AI movement, not a flag). `MpConsumePerLevel` (11 learnable
skills, 19 instances) is next, and unlike those, every instance in this
datapack is real, reachable, in-scope content: the MP-upkeep half of the core
fighter-class toggles — Accuracy (256), Guard Stance (288), Vicious Stance
(312), Shield Fortress (322), Focus Skill Mastery (334), Parry/Riposte Stance
(339/340), Polearm Accuracy (422), War Frenzy (424), Super Haste (7029), and
a few more only reachable via item skills (Embody Mana Armor, True Berserker,
Transfer Pain family, Strike Back, Hard March, Focus Shield, Lakcis Disc).

Every one of these already carries a real `StatModifier` effect that lands
correctly today (e.g. Accuracy's own `+3 ACCUSTOM_COMBAT`, parsed via
`EFFECT_REGISTRY`) — `MpConsumePerLevel` was the *other* effect on the same
skill, silently dropped because its XML name wasn't recognized. The practical
effect: every one of these toggles has been a **free** buff on this port, with
no MP upkeep cost at all — a real combat-balance divergence from Java, not
just a missing cosmetic.

## What Java does

`handlers/effecthandlers/MpConsumePerLevel.java`'s `onActionTime`:

```
base = power * getTicksMultiplier()   // ticks * EFFECT_TICK_RATIO / 1000
consume = skill.getAbnormalTime() > 0
    ? ((effected.getLevel() - 1) / 7.5) * base * skill.getAbnormalTime()
    : base
```

A survey of every `MpConsumePerLevel` instance in this datapack (all 19, not
just the 11 learnable ones) found **none** set an `<abnormalTime>` — they're
all toggles (`operateType=T`) or one `AU` item skill. So the level-scaled
branch is dead code for this build: the formula every instance in this
datapack actually exercises reduces to `power * getTicksMultiplier()` —
**identical** to the already-ported `ManaDamOverTime`'s per-tick drain.

## What landed

- **`SkillEffect::MpConsumePerLevel { power, ticks }`** (`model/skill.rs`) +
  the `"MpConsumePerLevel"` parse arm (`data/skill_data.rs`).
- **Shares `ManaDamOverTime`'s tick-chain arm** in
  `handle_dam_over_time_tick` (`| SkillEffect::MpConsumePerLevel { power,
  ticks }`), since the formula is identical for every skill that actually
  carries it: same periodic drain, same "toggle switches itself off when a
  tick's drain would exceed current MP" behavior (SM 140
  `YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP`). Also joined
  `schedule_dam_over_time`'s interval lookup and `apply_continuous_effects`'
  `has_periodic` empty-effects-guard exemption (needed for any instance that
  isn't paired with a stat modifier, e.g. Embody Mana Armor).
- TODO(G19) left at the merged match arm: split `MpConsumePerLevel` out with
  its own level-scaled formula if a future datapack skill ever sets
  `abnormalTime` on it — none does today.

## Test

`skills_tests::mp_consume_per_level_toggle_drains_mp_and_self_deactivates` —
real dist data (skill 256 "Accuracy"): toggling on lands both the `+3
Accuracy` stat and the MP-drain buff in one `ActiveBuff`; the first tick
drains the exact `power * ticksMultiplier` amount; draining the MP pool to
exhaustion self-deactivates the toggle (stat reverts, buff count → 0, SM 140
sent) rather than continuing for free.

## Collateral fix

`admin_tests::admin_superhaste_applies_and_persists` broke: Java's
`AdminSuperHaste` casts Super Haste (7029, also `MpConsumePerLevel`) through
the real `applyEffects` path, so `//superhaste` is subject to the same MP
drain as a real cast — correctly so, now that the effect is ported. The test
used `admin_world()` + a bare `skill_data` override, whose `for_test()`
`player_templates` is empty, so its level-1 GM computed **0 max MP** and the
very first tick immediately self-deactivated the "permanent" buff the test
was asserting. Fixed by loading the full datapack (`GameData::load_from`) so
the GM has a real MP pool the negligible `power 0.0001` drain won't exhaust
within the test's 100-tick window — matching how every other real-data test
in this suite is set up, and the same fix the `Transformation` slice's own
test needed for the same underlying reason (an HP/MP precheck against an
empty template).
