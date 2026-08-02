# PLAN_G34 — Skills, effects & abnormal-state parity (epic)

**Status:** 🚧 **S0 + S1 + S2 landed** (S0/S1 merged to `main`; S2 on
`feat/g34-basic-property`) · **Milestone:**
G34 (follows G33) · **Kind:** epic (9 slices, each a landable branch with its
own gate + sabotage-verified regression test).

> **One-line scope.** Everything that lives *inside* `<skill>` in
> `dist/game/data/stats/skills/*.xml` and is still not honoured by the port:
> effect handlers, skill conditions, abnormal-state flags, buff-lifecycle tags,
> targeting/scope enums and the skill-level tags the parser never reads.

---

## 0. Why this epic exists

G19 grew the effect system from ~10 effects to ~118 names across ~25 slices, and
the crowd-control family (stun/root/sleep/mute/debuff-block) works. But the
coverage was grown **on demand**, slice by slice, and the parser's design is
*fail-open*: an `<effect name="X">` the parser doesn't recognise produces no
`SkillEffect`, and an effect list that ends up empty is dropped by
`apply_skill_effects`' empty-effects guard. The skill still parses, still casts,
still plays its animation, still consumes MP and enters reuse — and does
**nothing**. Same for `<conditions>`: the parser reads exactly one condition
(`OpExistNpc`) and silently ignores the rest, so a skill that Java would *refuse*
fires happily here.

That failure mode is invisible from the Rust side. It only shows up as
"Bluff doesn't do anything", "Aegis is a dead passive", "Scroll of Escape:
Oren does nothing", "I can cast Rapid Shot without a bow", "I can chain-stun a
boss forever". This epic closes it as a system rather than one report at a time.

**Two axioms, unchanged:** the dist data is the spec (never edit it to match the
port), and every intentionally-skipped Java behaviour gets a `TODO(G34): …` at
the exact site naming the Java source.

---

## 1. The census (measured 2026-08-01)

Method — the one from [[l2r-abnormal-flags-cc]]: **rank by learnable-skill
usage, not raw instance count.** "Reachable" = the skill id appears in
`dist/game/data/skillTrees/**` (758 ids), `stats/npcs/*` (2 159),
`stats/items/*` (7 791) or `PetSkillData.xml` (117). Raw counts mislead badly —
`StatUp` tops the raw list at 465 skills but is 9 learnable ones, all Territory
War content outside Interlude's reach.

| Axis | Java | Ported | Gap |
|---|---|---|---|
| Effect handler names used by dist skills | **335** | 121 | **214 unhandled** (1 900 reachable skills, **54 learnable names**) |
| Effects in an unbuilt `<*Effects>` scope | `START`/`END` | — | **5** `block/name` pairs (10 reachable skills) |
| Skill conditions (`<conditions>` etc.) | **121 handlers** | **28 kinds** (S1) | ~~111~~ **69 `block/name` pairs**, ~~215~~ **1 learnable skill** |
| `EffectFlag` states | **38** | 15 | **23 missing** |
| `TargetType` | 23 | 10 + catch-all | 11 unhandled (532 reachable skills; `OTHERS` 3 + `DOOR_TREASURE` 1 learnable) |
| `AffectScope` | 21 | 13 + catch-all | 7 unhandled (3 reachable, **0 learnable** — deferral confirmed) |
| `AffectObject` | 11 | 4 + catch-all | 5 unhandled (**`UNDEAD_REAL_ENEMY` is 4 learnable**) |
| `SkillOperateType` | 18 | 4 + catch-all | 13 unhandled (**A3** 5 learnable, `CA5` 2; A4–A6, DA1–DA5, TG, AU) |
| Skill-level XML tags | ~120 distinct in dist | ~55 parsed | 13 with reachable usage |

**Bottom line for learnable content (758 skills):**

- **77** carry at least one unhandled effect (+1 more loses an effect to an
  unbuilt scope) → the skill fires and part or all of it does nothing.
- ~~**215**~~ **1** carries an unported condition (`OpSweeper`, deliberately —
  see S1) → S1 closed this axis.
- ~~**275**~~ **79 of 758 (10 %)** are wrong in at least one of those ways —
  S0's gate assertion, and now almost entirely the unhandled-effect axis.
- The unhandled-effect tail is *shallow*: 54 names over 77 skills, and only
  `StatUp` (9, out of scope) and `WeightLimit` (3) reach more than two skills.
  That shape dictates the slicing below — batch by *mechanism family*, not by
  effect count.

### Reproducing the census

Every number above comes from the checked-in harness (S0), not from a one-off
script:

```bash
cargo nextest run -p gameserver datapack_skill_coverage_census   # asserts them
cargo test -p gameserver --lib coverage_census -- \
    --ignored --nocapture                                        # prints them
```

The report prints in the same `("name", count)` shape the test's tables use, so
re-baselining after a slice is a copy-paste — but read *which* names moved
before you do it.

---

## 2. The gaps, in detail

### 2A — Effect handlers (216 unhandled names)

The 54 with a **learnable** source, with the skills they back. This *is* the
work list for S4, and it is `SkillGaps::effects` verbatim — the harness prints
it, so it cannot drift from the code:

| Effect | Learnable skills |
|---|---|
| `PhysicalShieldAngleAll` | Aegis (316), Aegis Stance (318) |
| `CounterPhysicalSkill` | Shield of Revenge (439), Counterattack (447) |
| `SkillEvasion` | Ultimate Evasion (111), Evasion (446) |
| `SkillTurning` | Spell Turning (1412) |
| `TriggerSkillByDamage` | Mirage (445) |
| `TargetMeProbability` | Vengeance (368) |
| `TransferDamageToSummon` | Transfer Pain (1262) |
| `AreaDamage` | Iron Body (295), Dance of Protection (311) |
| `TargetMe` | Aggression (28), Aggression Aura (18) |
| `Bluff` | Bluff (358), Blinding Blow (321) |
| `Betray` | Betray (1380) |
| `Unsummon` | Erase (1395) |
| `DeathLink` | Curse Death Link (1159) |
| `HateAttack` | Sword/Blunt Weapon Mastery (217) |
| `BuffBlock` | Dance of Medusa (367) |
| `TriggerSkillByMagicType` | Dance of Shadows (366) |
| `ResurrectionSpecial` | Salvation (1410), Soul of the Phoenix (438) |
| `LimitHp` / `LimitCp` | Noblesse Harmony (1326), Noblesse Symphony (1327) |
| `ReduceDropPenalty` | Noblesse Fortune (1325), Residence Death Fortune (610) |
| `EnlargeAbnormalSlot` | Divine Inspiration (1405) |
| `RebalanceHP` | Balance Life (1335) |
| `CallPc` | Summon Friend (1403) |
| `CallParty` | Chant of Gate (1429) |
| `DispelBySlotMyself` | Flames of Invincibility (1427) |
| `BlockEscape` / `BlockResurrection` | Clan Escape Lock (19113), Clan Resurrection Lock (19114) |
| `SkillMastery` | Skill Mastery (330, 331) |
| `SkillMasteryRate` + `PhysicalSkillPower` | Focus Skill Mastery (334) |
| `PhysicalSkillCriticalDamage` | Heroic Berserker (396) |
| `CriticalRatePositionBonus` | Focus Chance (356) |
| `MpVampiricAttack` | Weapon Mastery (250) |
| `NightStatModify` | Shadow Sense (294) |
| `CubicMastery` | Cubic Mastery (143) |
| `SafeFallHeight` | Acrobatics (173) |
| `Lucky` | Lucky (194) |
| `Passive` | Veil (106), Requiem (1049) |
| `PolearmSingleTarget` | Focus Attack (317) |
| `PhysicalAttackHpLink` | Fatal Counter (314) |
| `Pvp{Physical,Magical}*DamageBonus` | Duelist Spirit (297), Aura Flare (1231) |
| `OpenDoor` / `OpenChest` | Unlock (27) |
| `WeightLimit` / `WeightPenalty` | Weight Limit (150), Quiver of Holding (418), Decrease Weight (1257) |
| `Breath` | Boost Breath (195), Eva's Kiss (1073) |
| `HpByLevel` | Life Scavenge (46), Corpse Life Drain (1151) |
| `CpHealPercent` | Victories of Pa'agrio (1414), Pa'agrio's Fist (1416) |
| `ManaHealOverTime` | Force Meditation (441), Invocation (1430) |
| `ChameleonRest` | Chameleon Rest (296) |
| `ImmobilePetBuff` | Servitor Empowerment (1299) |
| `StatUp` | Territory Benefactions (848–855) — **out of scope**, Territory War |

One learnable skill loses its effect to the *scope* rather than the name:
**Anchor (1170)** declares `CallSkill` in `<endEffects>`, an `EffectScope` this
port never builds. Porting the `CallSkill` handler alone would not fix it — the
`END` lifecycle hook has to exist first. That distinction is why the harness
keeps `effect-scope` as its own category.

Non-learnable but player-visible (S6):

- **`Teleport` — 107 reachable items.** Every destination Scroll of Escape
  (Oren / Goddard / Heine / Cruma / Alligator Island / …) is inert today.
  Only `Escape escapeType=TOWN` is ported; `CASTLE` (5 items incl. King's
  Call/Summon), `CLANHALL` (2) and `FORTRESS` (2) are not.
- NPC/boss control: `KnockBack` (14 NPCs), `PullBack` (13), `FlyAway` (9),
  `Grow` (7), `Disarm` (4), `BlockSkill` (4), `GetDamageLimit` (4),
  `TriggerSkillByKill` (4), `Blink` (2), `AirBind`.
- Consumables: `AdditionalPotion{Hp,Mp,Cp}`, `Hp`, `CpHeal`, `HpCpHeal`,
  `InstantKillResist`, `DamageShieldResist`, `AttackAttributeAdd`, `RealDamage`.

Explicitly **out of chronicle** (verify each, then record the decision rather
than leaving it implicit): `SummonAgathion`, `SetSkill`, `TalismanSlot`,
`JewelSlot`, `ChangeHairStyle/Color`, `ChangeFace`, `ResetInstanceEntry`,
`CrystalGradeModify`, `EnableCloak`, `AddTeleportBookmarkSlot`,
`RefuelAirship`, `AddPcCafePoints`, the `PveRaid*`/`Pve*DefenceBonus` families,
`VitalityPointsRate`/`VitalityPointUp` (G16 owns vitality), `WorldChatPoints`,
`GiveFame`, `ChangeFishingMastery`.

### 2B — Skill conditions (the largest single hole)

Java evaluates two scopes at cast (`Skill.checkCondition` →
`checkConditions(GENERAL, …)` then `checkConditions(TARGET, …)`), plus
`PASSIVE` for passive skills. On failure it sends
`S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS` — **except** when the caster is the
target and the skill `isBad()`, which sends nothing. None of this exists here.

Conditions ranked by learnable skills carrying them:

| Condition | Learnable | Effect if unported |
|---|---|---|
| `conditions/EquipWeapon` | 88 (+1 in `passiveConditions`) | bow/dagger/polearm skills cast bare-handed or with the wrong weapon |
| `CanTransform` | 32 | (partly covered ad-hoc in `cast.rs`'s transform gate — fold in) |
| `CanSummon` | 24 | summon limits/state unchecked at cast time |
| `conditions/TargetMyParty` | 11 (+1 in `passiveConditions`) | party-only skills land on non-party targets |
| `CanSummonCubic` | 12 | cubic count/limits unchecked |
| `EnergySaved` | 10 | force-charge consumers fire with no charges |
| `OpExistNpc` | 9 | **ported** |
| `TargetRace` | 7 | race-gated skills (undead/beast) land on anything |
| `EquipShield` | 6 | shield skills usable with no shield |
| `ConsumeBody` | 5 | corpse skills without a corpse |
| `RemainHpPer` | 5 | HP-threshold skills fire at full HP |
| `OpEncumbered` | 5 (1 415 items) | overweight gate |
| `CanSummonSiegeGolem` | 3 | |
| `OpCanEscape`, `OpResurrection`, `CanUseInBattlefield`, `RemainCpPer`, `OpSocialClass`, `OpEnergyMax`, `Op2hWeapon`, `OpSkillAcquire`, `OpSiegeHammer`, `OpWyvern`, `OpStrider`, `OpSweeper`, `BuildCamp`, `NotInUnderwater`, `RemainMpPer`, `OpTargetPc` | 1–2 each | long tail |

Plus the `SkillCaster.checkUseConditions` legs not ported at all:
`weapon.useWeaponSkillsOnly()`, `skill.isBlockedInOlympiad()` (59 learnable
skills), the itemConsume "not enough items" branch and its
summon-specific message, `famePointConsume`, `clanRepConsume`,
`isMounted() && isBad()` refusal, `isFlyType() && isMovementDisabled()`,
observer mode.

### 2C — Abnormal states (`EffectFlag`) — the user-facing headline

15 of Java's 38 flags exist. Missing, with whether this dist has a reachable
source (verify each before porting — several are the recurring
"declared, unreachable here" shape):

| Flag | Reachable source here? |
|---|---|
| `BETRAYED` | ✅ Betray (1380) |
| `PSYCHICAL_ATTACK_MUTED` | ✅ `PhysicalAttackMute` (pet skill) |
| `BUFF_BLOCK` | ✅ Dance of Medusa (367) + 7 NPC skills |
| `CANNOT_ESCAPE` | ✅ Clan Escape Lock (19113) |
| `BLOCK_RESURRECTION` | flag exists, source (`BlockResurrection`) unported |
| `RESURRECTION_SPECIAL` | ✅ Salvation (1410), Soul of the Phoenix (438) |
| `RELAXING` | ✅ Chameleon Rest (296) / `Relax` |
| `DISARMED` | ✅ `Disarm` (4 NPC skills) |
| `PHYSICAL_SHIELD_ANGLE_ALL` | ✅ Aegis (316), Aegis Stance (318) |
| `ABNORMAL_SHIELD`, `UNTARGETABLE`, `PROTECTION_BLESSING`, `PROTECT_DEATH_PENALTY`, `TARGETING_DISABLED`, `IGNORE_DEATH`, `HPCPHEAL_CRITICAL`, `CHEAPSHOT`, `ATTACK_BEHIND`, `FACEOFF`, `DOUBLE_CAST`, `DUELIST_FURY`, `CHAT_BLOCK`, `PASSIVE` | item-only or unreachable — **audit, port the gate, TODO the source** |

**And the one that is not a flag at all — `BasicPropertyResist`.** This is a
live, retail-faithful mechanic that the port not only misses but has a comment
actively justifying its absence:

> `formulas.rs`: *"`getAbnormalResist(basicProperty, target)` stays 0:
> `BasicPropertyResist` is granted by no skill on this dist … so it can never
> leave its identity."*

That conflates two different things and is **wrong** (a textbook
[[l2r-deviation-comments-self-justify]]):

1. `Formulas.getAbnormalResist(basicProperty, target)` reads
   `Stat.ABNORMAL_RESIST_{PHYSICAL,MAGICAL}` — granted by the
   `PhysicalAbnormalResist`/`MagicalAbnormalResist` effects (3 items each, no
   learnable source). *That* half is fairly written off.
2. `Formulas.getBasicPropertyResistBonus(basicProperty, target)` is a
   **separate** multiplier on the same line, and it is not granted by anything
   — it is *accrued by being debuffed*. `Skill.applyEffects` calls
   `effected.getBasicPropertyResist(_basicProperty).increaseResistLevel()`
   after every landed debuff with a non-`NONE` `<basicProperty>`. The bonus is
   `1.0 / 0.6 / 0.3 / 0` for resist level `0 / 1 / 2 / 3+`, decaying 15 s after
   the last mesmerizing debuff (`BasicPropertyResist.RESIST_DURATION`).

`Creature.hasBasicPropertyResist()` returns `true` unconditionally;
`Player` overrides it to `isInCategory(SIXTH_CLASS_GROUP)`, which is empty in
Interlude. So on this dist the mechanic is **live for every NPC, monster, pet
and servitor, and off for players** — i.e. chain-stunning a mob gets
progressively harder and then stops working, which is exactly the retail
stun-lock behaviour the port currently lacks. `<basicProperty>` is on **390
learnable skills** and is not parsed at all.

Buff-lifecycle tags in the same family, all unparsed:

- `removedOnDamage` — **done**, merged to `main` 2026-08-01 (`e58c7f64`,
  `fix/sleep-wake-on-damage`). Read it before touching the rest of the family:
  the two `reduceHp` overrides disagree on `isDOT` *and both are right* (a
  poison tick wakes a sleeping player, not a sleeping mob), and `calcStunBreak`
  is dead on this dist, so a stun is the right control when testing any
  wake-on-damage change.
- `removedOnAnyActionExceptMove` (4 learnable), `abnormalInstant`,
  `irreplacableBuff` (30 learnable — currently only referenced in a comment),
  `subordinationAbnormalType` (10), `abnormalResists`, `blockActionUseSkill`,
  `specialLevel` (44).

### 2D — Targeting, scopes, operate types

- **`AffectObject::UNDEAD_REAL_ENEMY` (4 learnable)** — falls through to "no
  filtering", so an undead-only AoE currently hits everything. A correctness
  bug, not just a hole.
- `FRIEND_PC` (16 learnable) / `NOT_FRIEND_PC` collapse onto
  `FRIEND`/`NOT_FRIEND` — close, but they exclude summons/pets.
- `TargetType::ITEM` — **452 reachable items** (every item-targeted skill:
  enchant scrolls, crystallisation, extraction).
  Also `OTHERS` (22), `OWNER_PET` (19), `DOOR_TREASURE` (34), `MY_PARTY`.
- `AffectScope`: `RANGE_SORT_BY_HP`, `SUMMON_EXCEPT_MASTER` (22),
  `PARTY_PLEDGE`, `STATIC_OBJECT_SCOPE`, `DEAD_PARTY`.
- `SkillOperateType`: **A3** (5 learnable — "instant for target + continuous +
  continuous for self") falls to `Other`; `DA1`/`DA2` need
  `SkillCaster.handleSkillFly` (charge/rush); `A5` aura, `A6` synergy, `TG`,
  `AU`, `CA2`/`CA5`.

### 2E — Skill-level tags never parsed

`basicProperty` (390 learnable — §2C), `magicCriticalRate` (756 learnable —
confirm whether the port uses a global constant instead of the per-skill
value), `blockedInOlympiad` (59), `specialLevel` (44), `nextAction` (39),
`irreplacableBuff` (30), `subordinationAbnormalType` (10),
`removedOnAnyActionExceptMove` (4), `abnormalInstant`, `blockActionUseSkill`,
`abnormalResists`, `isTriggeredSkill`, `soulMaxConsumeCount`,
`famePointConsume`, `clanRepConsume`, `targetConditions`, `passiveConditions`.

---

## 3. Slices

Ordering rationale: **S0 first** (nothing else is measurable without it), then
the **condition engine (S1)** — the largest single hole, the only slice needing
a new subsystem, and the one whose absence means skills *fire when Java would
refuse them*, which is worse than an effect that quietly does nothing. The
abnormal-state half (S2–S3) follows, then breadth.

*(Reordered 2026-08-01 at the user's request: the conditions engine was
originally S3, behind the two abnormal-state slices. Nothing depends on the old
order — S1/S2/S3 are mutually independent; only S4 has a real dependency, on
S3's flags.)*

Each slice: one branch, one gate, at least one regression test **verified by
sabotage** (disable the fix, confirm the test fails — [[l2r-verify-test-detects-bug]];
remember `mv .bak` back keeps the old mtime and cargo skips the rebuild).

---

### S0 — Coverage harness & fail-loud parsing ✅ **DONE** (`feat/g34-skill-census`)

The parser now **records what it drops** instead of dropping it silently
([`SkillGaps`] in `data/skill_data.rs`), in seven categories: unrecognised
`<effect name>`; effects in an `<*Effects>` scope this port never builds
(`startEffects`/`endEffects`); `<condition name>` in any of Java's three
`SkillConditionScope` blocks; and `<targetType>`/`<affectScope>`/
`<affectObject>`/`<operateType>` values that fell to the `Other` catch-all.
Recording sits **at the fallback arms themselves**, so it cannot drift from the
match: a name stops being reported the moment a handler claims it.

1. ✅ `coverage_census::datapack_skill_coverage_census` — the checked-in gate.
   Intersects the gap record with the datapack's own reachability (a text scan
   of `skillTrees/**`, `stats/npcs`, `stats/items`, `PetSkillData.xml` — *not*
   the ported loaders, which would measure the port against itself), then
   asserts the exact learnable-source name list per category, the per-category
   totals, and the headline `275 / 758`. Both denominators are asserted too, so
   a datapack change explains itself instead of moving every other number
   silently. Sabotage-verified twice: faking a `"Bluff"` handler arm and faking
   an enforced `EquipWeapon` each fail the right assertion with a diff naming
   the moved entry.
2. ✅ `log_gaps` — one `warn!` per category at boot, worst-first with counts,
   so the log names what is inert. Counts there are **raw skill ids, not
   Interlude-reachable** ones: reachability needs the skill trees, NPC and item
   data, none of which are loaded at that point. The test does the intersection.
3. ✅ `coverage_census::print_coverage_report` — an `#[ignore]`d reporting aid
   that prints the tables in the literal shape the test consumes, so
   re-baselining after a slice is a copy-paste.
4. ⏳ Deferred: the `//skillinfo <id>` admin dump (per-skill "which effects
   resolved / which conditions were ignored"). The boot log and the census
   cover the sweep case; the per-skill view is only worth building once a slice
   is actually being debugged.

**Gate met:** boot logs list the inert names with counts; the census fails on
any change to either list, in either direction.

**Two things the harness deliberately does *not* claim.** Absence from the gap
record means "recognised", **not** "correctly ported" — an effect can resolve
to a `SkillEffect` variant that nothing downstream consumes, which this cannot
see ([[l2r-skill-rate-stats]]). And the reachability scan counts a skill id
referenced *anywhere* in the datapack, including content outside Interlude's
reach; the learnable column is the one to rank by.

---

### S1 — The skill-condition engine ✅ **DONE** (`feat/g34-skill-conditions`)

`<conditions>` / `<targetConditions>` / `<passiveConditions>` now parse into a
`Vec<SkillCondition>` per Java `SkillConditionScope`, and
`skills::conditions::check_cast` evaluates GENERAL then TARGET from
`use_magic_on` — **after target resolution**, exactly where
`Player.useMagic` calls `skill.checkCondition(this, target)`.

**Result: unported conditions on learnable skills went from 215 skills / 111
`block/name` pairs to 1 skill / 69 pairs**, and the epic's headline number from
**275 → 79**. The residue is now almost entirely unhandled *effects* (S4).

Landed:

- **Parsing.** Conditions carry per-level `<value level="N">` tables *and*
  ranged `fromLevel`/`fromSubLevel` rows, exactly like effect params
  (`OpEnergyMax`'s `amount` is a 7-level table; `RemainHpPer`'s uses both), so
  they reuse the effect machinery rather than being read as flat scalars.
  `targetConditions`/`passiveConditions` were not even *entered* by the old
  parser.
- **28 condition kinds**, covering every one with a learnable source:
  `EquipWeapon` (88 skills), `CanTransform` (32), `CanSummon` (24),
  `CanSummonCubic` (12), `TargetMyParty` (11), `EnergySaved` (10),
  `TargetRace` (7), `EquipShield` (6), `ConsumeBody`/`OpEncumbered`/
  `RemainHpPer` (5 each), `CanSummonSiegeGolem` (3), and the 1–2-skill tail
  (`OpCanEscape`, `OpResurrection`, `OpUnlock`, `OpTargetPc`, `OpCallPc`,
  `OpSocialClass`, `OpEnergyMax`, `RemainCpPer`/`RemainMpPer`, `Op2hWeapon`,
  `OpSkillAcquire`, `OpStrider`, `OpWyvern`, `NotInUnderwater`, `BuildCamp`,
  `CanUseInBattlefield`/`OpSiegeHammer`, `CheckLevel`, `CheckSex`).
- **Refusal semantics.** Java sends the failing handler's own message **and**
  then `S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS`, suppressing the generic one
  only when the caster aimed a *bad* skill at themselves. The GM bypass
  (`PlayerCondOverride.SKILL_CONDITIONS`, `GM_SKILL_RESTRICTION` off on this
  dist) and the mounted-and-bad refusal are ported with it.
- **Both ad-hoc gates folded in.** `OpExistNpc` lost its dedicated `Skill`
  field and inline check; `CanTransform` lost its inline block. One
  representation each, no drift.
- **The census is now driven by the builder**, not the parse: a condition is
  recorded as a gap only when `build_condition` returns `None`. Porting a
  condition shrinks the census automatically — there is no second "ported
  names" list to keep in step.

Deliberately **not** ported, and recorded rather than hidden:

- **`OpSweeper`** (1 learnable skill) — Java re-runs the skill's whole affect
  scope and asks each corpse about spoil ownership, corpse age and the
  sweeper's free inventory. `effects::sweep` already does all of it at *apply*
  time with the right per-corpse messages; gating the cast too would double
  every message.
- **`<passiveConditions>` is wired but inert on this dist.** Both learnable
  users are covered elsewhere — Sword/Blunt Weapon Mastery (205) has the same
  `<weaponType>` on its own `PAtk` effect, and Inner Rhythm (428) declares
  `TargetMyParty` in a passive block, which Java answers `false` to (no
  target), disabling the passive outright; not reproduced, since that reads as
  datapack noise and matching it would nerf a learnable skill on a guess.
- **One deliberate deviation kept:** Java has *two* transform gates —
  `ConditionPlayerCanTransform` (the item-condition system) ends with a
  registered-on-an-event leg, `CanTransformSkillCondition` (what a skill
  actually resolves to) does not. The port keeps the stricter leg on the skill
  path, documented in one place instead of implied by a merged block.

**Four fixtures were wrong and are now right** — Sonic Focus, Sonic Blaster and
both Lethal Blow tests cast bare-handed (`EquipWeapon`), and the Revival test
cast at 20 % HP when the skill's own `RemainHpPer` is `LESS 10`. They passed
only because nothing was enforced.

**Gate met:** the named skills are refused with the right messages and consume
nothing. Regressions in `skill_condition_tests`, sabotage-verified (disabling
`check_cast` fails three of the four plus the symbol-gate test).

---

### S2 — `basicProperty` + `BasicPropertyResist` ✅ **DONE** (`feat/g34-basic-property`)

Retail's PvE stun-lock resistance, which the port did not have — and which a
comment in `formulas.rs` had written off on a false premise.

`<basicProperty>` (390 learnable skills) now parses onto `Skill`, and
`game_loop::basic_property` implements **both** terms
`Formulas.calcEffectSuccess` reads off it, which the old comment had conflated
into one:

| Java | what it is | where it enters |
|---|---|---|
| `getAbnormalResist` | the `ABNORMAL_RESIST_PHYSICAL/_MAGICAL` **stat** | **subtracted inside `baseMod`** — still subject to the 10–90 clamp |
| `getBasicPropertyResistBonus` | the **accrual chain**, earned by being debuffed | **multiplied after the clamp** — so it can reach 0 |

That second row is the whole mechanic: every landed debuff with a non-`NONE`
property bumps a per-target counter, worth **1.0 / 0.6 / 0.3 / 0** at level
0/1/2/3+, decaying **15 s after the last one landed**. Level 3 is a hard
immunity precisely because Java multiplies it in *after* `constrain(rate, min,
max)` — port it before the clamp and a chain-stunned mob keeps taking a 10 %
stun forever.

Details that matter:

- **Accrual is on the landed path only.** Java's call sits inside
  `applyEffects`' `if (addContinuousEffects)` branch, past `calcEffectSuccess`,
  so a debuff you keep *failing* to land never builds the resistance that would
  lock you out of it. An expired chain restarts at 1, not where it left off.
- **Mobs accrue; players do not.** `Creature.hasBasicPropertyResist()` is
  unconditionally `true`, but `Player` overrides it to
  `isInCategory(SIXTH_CLASS_GROUP)` — awakened classes only, so empty in
  Interlude. PvE gets stun-lock resistance, PvP chain-CC is untouched. Getting
  this backwards silently rewrites PvP, so it is asserted directly.
- Expiry is checked **on read**, never swept — Java's own `isExpired()` inside
  `getResistLevel`. No scheduler entry, no cleanup pass.
- `PhysicalAbnormalResist`/`MagicalAbnormalResist` (plain
  `AbstractStatAddEffect`s) joined `EFFECT_REGISTRY` now that the stat has a
  consumer: **effect names 216 → 214**. Neither has a learnable source, so the
  learnable count is unchanged — the shape to expect from the item-only tail.

**Gate met:** stun a mob repeatedly — the 2nd lands at 0.6×, the 3rd at 0.3×,
the 4th cannot land at all; 16 s later the chain is gone. A player takes all
four at full rate. Regressions in `basic_property_tests` (ladder, decay window,
the mob/player asymmetry, both formula insertion points, and an end-to-end real-
dist Stun Attack), **sabotage-verified twice**: disabling the accrual fails the
end-to-end test, and moving the chain term to *before* the clamp — the subtle
wrong port — fails the formula test.

---

### S3 — Abnormal-state breadth: the remaining `EffectFlag`s + buff lifecycle

1. Audit all 23 missing flags against the dist for a reachable source (§2C
   table). For each with a source: add the constant, stamp it in
   `Skill::effect_flags()` (that is the *only* place needed — stamp-and-fold),
   and port its one Java gate. For each without: port the gate anyway if it is
   one line, and leave a `TODO(G34)` naming the unreachable source, matching the
   existing `MP_BLOCK`/`FEAR`/`CONFUSED` precedent.
2. Buff-lifecycle tags (`removedOnDamage` already landed —
   `e58c7f64`): `removedOnAnyActionExceptMove`, `abnormalInstant`,
   `irreplacableBuff`, `subordinationAbnormalType`, `abnormalResists`,
   `blockActionUseSkill`.

**Trap:** any new modifier-less effect must join one of the empty-effects
guard's three categories (*periodic* / *icon-only* / *state flag*) or the buff
is dropped whole and nothing happens — this has bitten four slices running.

**Gate:** Betray turns a servitor on its owner; Dance of Medusa blocks incoming
buffs; Clan Escape Lock refuses an SoE; Aegis's shield angle covers all sides;
a `PhysicalAttackMute` pet cannot auto-attack.

---

### S4 — The learnable-skill effect sweep (77 skills, 5 families)

- **S4a Defensive stances & counters** — `PhysicalShieldAngleAll`,
  `CounterPhysicalSkill`, `SkillEvasion`, `SkillTurning`,
  `TriggerSkillByDamage`, `TargetMeProbability`, `TransferDamageToSummon`,
  `AreaDamage`.
- **S4b Aggro & control** — `TargetMe`, `Bluff`, `Betray` (with S3's flag),
  `Unsummon`, `CallSkill`, `DeathLink`, `HateAttack`, `BuffBlock`,
  `TriggerSkillByMagicType`.
- **S4c Noblesse / high-level utility** — `ResurrectionSpecial`, `LimitHp`,
  `LimitCp`, `ReduceDropPenalty`, `EnlargeAbnormalSlot`, `RebalanceHP`,
  `CallPc`, `CallParty`, `DispelBySlotMyself`, `BlockEscape`,
  `BlockResurrection`.
- **S4d Passives & masteries** — `SkillMastery`, `SkillMasteryRate`,
  `PhysicalSkillPower`, `PhysicalSkillCriticalDamage`,
  `CriticalRatePositionBonus`, `MpVampiricAttack`, `NightStatModify`,
  `CubicMastery`, `SafeFallHeight`, `Lucky`, `Passive`, `PolearmSingleTarget`,
  `PhysicalAttackHpLink`, the `Pvp*Bonus` pair.
- **S4e Utility & sustain** — `OpenDoor`/`OpenChest`, `WeightLimit`/
  `WeightPenalty`, `Breath`, `HpByLevel`, `CpHealPercent`, `ManaHealOverTime`,
  `ChameleonRest`, `ImmobilePetBuff`.

**Method note:** several of these are "parsed but unconsumed stat" shapes — the
stat exists, nothing reads it. Grep the consumer *before* declaring the effect
done ([[l2r-skill-rate-stats]], [[l2r-regen-stat-pipeline]]).

---

### S5 — Targeting, scope & operate-type breadth

`AffectObject::UNDEAD_REAL_ENEMY` first (it is a live correctness bug), then
`FRIEND_PC`/`NOT_FRIEND_PC`, `TargetType::ITEM` (452 items),
`OTHERS`/`OWNER_PET`/`DOOR_TREASURE`/`MY_PARTY`, the four missing affect
scopes, and `operateType` A3 (+ `isSelfContinuous`). `DA1`/`DA2` charge/rush
(`handleSkillFly`) is a judgement call — gate it on whether any reachable skill
uses it after the S0 census reports.

---

### S6 — Item- & NPC-reachable effects

1. **Destination Scrolls of Escape first** — the `Teleport` effect (107 items)
   and `Escape` `CASTLE`/`CLANHALL`/`FORTRESS` (9 items). Most visible gap in
   this whole epic for a normal player.
2. NPC/boss control: `KnockBack`, `PullBack`, `FlyAway`, `Grow`, `Disarm`,
   `BlockSkill`, `GetDamageLimit`, `TriggerSkillByKill`, `Blink`, `AirBind`.
3. Consumables: the `AdditionalPotion*` family, `Hp`, `CpHeal`, `HpCpHeal`,
   `InstantKillResist`, `DamageShieldResist`, `AttackAttributeAdd`,
   `RealDamage`.

---

### S7 — Skill-tag & formula tail

`magicCriticalRate` per-skill (756 learnable skills — first *confirm* the port
isn't already using a global; if it is, that is the finding), `specialLevel`,
`nextAction` (39), `isTriggeredSkill`, `soulMaxConsumeCount`, `hitCancelTime`
verification, `abnormalResists`.

---

### S8 — Epic gate & close-out

Re-run S0's census; every remaining entry must be either **ported** or
**explicitly recorded as out-of-chronicle with the reason**, with no silent
third category. Update `docs/PROGRESS.md`, `docs/ROADMAP.md`, and write the
memory entry.

**Epic gate:** of the 758 learnable skills, 0 carry an unhandled effect or an
unported condition that is not on the recorded out-of-scope list.

---

## 4. Cross-cutting traps (read before starting any slice)

1. **The parser fails open.** An unknown effect name yields no `SkillEffect`;
   an empty effect list is then dropped by `apply_skill_effects`' guard. The
   skill still casts and does nothing. S0 exists to make this loud.
2. **Modifier-less effects need a guard category** — *periodic*, *icon-only* or
   *state flag*. Four slices have been bitten by this.
3. **New state flags need only `Skill::effect_flags()`** — the mask is folded on
   read, not cached ([[l2r-abnormal-flags-cc]]).
4. **Rank by learnable reach, never raw instance count** — `StatUp` is the
   canonical trap.
5. **Deviation comments self-justify.** The `formulas.rs` `basicProperty`
   comment in §2C is a live example: it was written from a grep of one of two
   mechanisms and then read as settled for months. When a comment explains why
   Java's behaviour doesn't apply here, re-derive the claim before trusting it.
6. **Grep the helper, not the feature** ([[l2r-two-java-call-sites]]) — and
   remember the effect handlers live under
   `dist/game/data/scripts/handlers/effecthandlers/`, **not** under `java/`. A
   `java/`-only grep is how `MP_BLOCK` got written off as dead when five
   handlers read it.
7. **The two `reduceHp` overrides disagree on `isDOT` and both are right.**
8. **A config-disabled feature is still ported** ([[l2r-config-disabled-still-port]]).
9. **Never edit the datapack to match the port.**
10. **Sabotage-verify every regression test**, and remember `mv .bak` restores
    the old mtime so cargo skips the rebuild ([[l2r-sabotage-restore-mtime]]).
11. Use `cargo nextest run` (plain `cargo test` hangs here), and check
    `$pipestatus` when piping cargo output.

---

## 5. Out of scope

Kamael/Goddess-era content (elemental `DefenceAttribute` ladders, agathions,
talismans, awakening classes, mentoring, airships, Sayune), anything gated on
`SIXTH_CLASS_GROUP`, and the effect families listed as out-of-chronicle in §2A —
each to be **recorded** with its reason in S8 rather than left as an unexplained
gap.

---

## 6. Sizing

| Slice | Rough size | Depends on |
|---|---|---|
| S0 harness ✅ | S | — |
| S1 condition engine ✅ | XL | S0 |
| S2 basicProperty / BasicPropertyResist ✅ | M | S0 |
| S3 EffectFlag breadth + lifecycle tags | L | S0 |
| S4 learnable effect sweep (a–e) | XL | S0, S3 (for flag-backed effects) |
| S5 targeting/scope breadth | M | S0 |
| S6 item/NPC effects | M | S0 |
| S7 tag & formula tail | S | S0 |
| S8 close-out | S | all |
