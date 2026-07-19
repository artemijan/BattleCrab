# G19 — Skills & effects breadth: affect scopes & toggles

The first slice of **G19**, the milestone [ROADMAP.md](ROADMAP.md) calls "the
long pole for combat and content" (G20/G21/G22/G28/G29 all pull from it). G19 is
explicitly a *grow-continuously* milestone rather than one with a finish line —
this slice takes the two structural gaps that block whole categories of skill
from working at all, and leaves the per-effect breadth to accrete.

Java sources: `Skill.getTargetsAffected`/`forEachTargetAffected`,
`handlers/targethandlers/affectscope/*`, `handlers/targethandlers/affectobject/*`,
`handlers/targethandlers/None.java`, `SkillCaster.run`'s instant-cast branch,
`Player.useMagic`'s toggle branch.

---

## 1. Why these two

Before this slice **only `SINGLE` target scope resolved**. Every area skill in
the datapack — 820 `RANGE`, 785 `POINT_BLANK`, 272 `PARTY`, 44 `PLEDGE` — cast
successfully and then landed on exactly one creature. An AoE nuke was a
single-target nuke with a wider animation. Separately, all 104 `operateType=T`
toggles were silently unreachable (`use_magic` dropped anything that wasn't
`Active`).

## 2. Affect scopes (`game_loop/skills/affect.rs`)

`targets_affected(world, caster, target, skill)` ports
`Skill.getTargetsAffected`: it expands the primary target into the full affected
list, **primary target first** so callers that treat the first entry specially
(the resist message, the "main target") keep working.

Ported scopes:

| Scope | Centre | Notes |
|---|---|---|
| `SINGLE` | — | the target alone; also the fallback for unported scopes |
| `RANGE` | target | `affect_range` sweep around the **target** |
| `POINT_BLANK` | caster | sweep around the **caster** — which is why these skills carry `targetType SELF` |
| `PARTY` | target | the target's party; an unpartied target is a party of one |
| `PLEDGE` | target | the target's clan mates in range |

Java's per-candidate filters are ported with them: the `affectLimit` cap, the
dead-creature skip (with the `NPC_BODY` corpse-skill exemption), "range skills
don't affect you unless you are the main target", the `affectObject` filter, and
the LOS check — which Java measures **from the target** in both radius handlers,
even for `POINT_BLANK`.

`affectLimit` deserves a note: `Skill.getAffectLimit()` is `min + Rnd.get(max)`,
not a roll between min and max. The dist's ubiquitous `5-12` therefore yields
5..=16 targets, and `10-10` yields 10..=19. That looks more like a datapack
authoring assumption than an intent, but it is what the live server does, so
`Skill::affect_limit` reproduces it exactly (asserted at both roll extremes).

### Affect objects

`ALL`, `NOT_FRIEND` (1637 skills), `FRIEND` (463), `CLAN`. "Friend" is the
caster themselves, a party mate, or a clan mate; NPCs are never friends, matching
Java's `getActingPlayer()`-null fallthrough. `NOT_FRIEND` also carries the
peace-zone leg, which is the one that visibly matters — an AoE must not clip a
player standing in town.

### Deviations & gaps

- **Candidate set.** Java sweeps `World.forEachVisibleObjectInRange`, bounded by
  the region grid rather than purely by radius. This port sweeps the same 3×3
  region block and then applies the radius — the same set for every
  `affect_range` the dist uses (max 2000, comfortably inside a block).
- **Unported scopes fall back to single-target** (`TODO(G19)`), where Java would
  refuse the cast outright with "affect scope is not currently handled". Falling
  back is much less disruptive than refusing. Still open: the geometric
  `FAN`/`FAN_PB` (179 skills) and `SQUARE`/`SQUARE_PB` (52) — both need the
  caster-heading arc/rect math; `RING_RANGE` (18, an annulus); `RANGE_SORT_BY_HP`
  (4); `SUMMON_EXCEPT_MASTER` (22) and `WYVERN_SCOPE`/`BALAKAS_SCOPE`, blocked on
  summons (G29) and boss scripting (G23); the `DEAD_*` mass-resurrect family;
  `PARTY_PLEDGE` (5); `STATIC_OBJECT_SCOPE`.
- **`GROUND`-targeted casts** (22 skills) are not ported, so the ground-position
  branch both radius handlers carry is absent.

## 3. Fanning out the cast pipeline

`handle_skill_finish` previously applied effects, PvP flagging and monster hate
to one `cast.target_object_id`. It now resolves the affected list and loops,
with the per-target half of `callSkill` extracted into
`apply_cast_consequences` so every creature an AoE touches gets the flagging and
hate the single target used to get. Each target is re-checked inside the loop —
an AoE's effect on an early target can kill or despawn a later one. Attack
stance stays caster-scoped and fires once per cast.

## 4. Toggles

`Player.useMagic`'s toggle branch, ahead of every other check:

- Recasting a live toggle **switches it off** — the effect is stripped and
  nothing is cast (`ActionFailed`).
- Otherwise, a toggle with a `toggleGroupId` first stops its group siblings
  (`EffectList.stopAllTogglesOfGroup`), then switches on.
- Toggles are **instant casts** (`SkillCaster.run`'s `instantCast` short
  circuit): no cast bar, no launch/finish phases, no `Casting` component — they
  go straight to `triggerCast`. This is what the first attempt got wrong; a
  toggle routed through the phased pipeline never appeared to switch on.
- `targetType NONE` is new (`targethandlers/None.java` returns the caster —
  `SELF` minus the peace-zone gate); it is what every toggle on this dist uses.
- Toggles carry no `abnormalTime`, and the existing `abnormal_time <= 0 →
  permanent` rule already gives them "on until switched off".

Java's `isNecessaryToggle()` exemption is not ported — no skill on this dist
sets it.

## 5. Incidental cleanup

`Skill.single_target` (a bool that only gated the debuff-chance chat line) is
gone, folded into `affect_scope == Single`. Keeping both would have meant two
representations of one fact.

## 6. Gate

The milestone gate is *"a debuff lands on a mob, an AoE nuke hits a cluster, a
toggle skill switches on."* Debuffs on mobs already worked; the other two are
covered by `game_loop/tests/affect_tests.rs` (12 cases): the scope sweeps
(single / range / point-blank / zero-range / party), the `affectLimit` cap, the
dead skip, the affect-object filters (caster excluded from their own AoE, party
mates spared by `NOT_FRIEND` and reached by `FRIEND`), toggle on/off, toggle
group exclusion, and — the gate proper — **an end-to-end RANGE nuke cast through
the real pipeline that damages every mob in the sweep and none outside it**.

Parsing is asserted against real datapack skills: Tempest 1176 (RANGE /
NOT_FRIEND / range 200 / limit 5-12, including the roll extremes) and Thunder
Storm 48 (POINT_BLANK from a `SELF` target type).

## 7. What G19 still owes

This slice deliberately does not touch the milestone's open-ended half: growing
`EFFECT_REGISTRY` toward Java's 369 effect classes and the 230-entry `Stat`
enum. Known consumers waiting on it include the ~11 community-board buffs that
still land icon-only, the `VITALITY_CONSUME_RATE`/`BONUS_EXP`/`BONUS_SP` stats
G16 left as identities, and `calcMagicSuccess` (`ALT_GAME_MAGICFAILURES`).
Also still open from the milestone's scope list: the abnormal-visual-effect
runtime + per-creature team/targetable state (and the AdminEffects AVE handlers
they unblock), `ExAbnormalStatusUpdateFromTarget`, the remaining
`AcquireSkillType`s, and skill enchanting (`EnchantSkillGroupsData`).
