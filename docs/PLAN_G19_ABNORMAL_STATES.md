# G19 — Abnormal-state flags & crowd control

The second **G19** slice, after [PLAN_G19_EFFECTS.md](PLAN_G19_EFFECTS.md)
(affect scopes & toggles). Where that one fixed *who* a skill reaches, this
fixes a category of effect that reached its target and then did nothing.

Java sources: `EffectFlag`, `EffectList.computeEffectFlags`,
`Creature.hasBlockActions`/`isRooted`/`isMovementDisabled`/`isAllSkillsDisabled`/
`isDisabled`, `handlers/effecthandlers/BlockActions.java`, `Root.java`.

---

## 1. Why this next

A survey of `<effect name>` across the datapack put **11 698 of 28 259 effect
instances (41 %) on effects the port doesn't handle**. Picking from the top by
usage, filtered for "meaningful on an Interlude server":

- `DefenceAttribute` (1192) is the single biggest, but elemental attributes are
  Kamael-era and explicitly **out of scope** per ROADMAP's scope gate.
- `StatUp` (887) is a straightforward base-stat pump — valuable, but nothing is
  *broken* without it.
- **`BlockActions` (540) + `Root` (79)** are stun / sleep / paralyze /
  immobilise: core combat mechanics that were **completely inert**. A stun
  landed, showed its icon, and the victim kept attacking, casting and running.

The last one wins on visible impact, and it needs a piece of infrastructure —
the abnormal-state mask — that many later effects (`BlockControl`, `Fear`,
`MUTED`, `DISARMED`, `HP_BLOCK`, …) will hang off.

## 2. The mask (`model::skill::effect_flag`)

Java keeps a cached `EffectFlag` bitmask on `EffectList` and recomputes it on
every effect add/remove. **This port instead stamps each `ActiveBuff` with the
flags its skill contributes (`Skill::effect_flags`) and ORs the live buff list
on read** (`game_loop::abnormal::flags_of`).

Same answer, deliberately different mechanism: buffs are added and removed from
several places here (skill application, the separate NPC-buff path, natural
expiry, dispel, toggle-off), and a cached mask would need invalidating at every
one of them. A fold over a list that is never longer than the buff-slot cap is
cheap and cannot go stale.

Two flags are defined, being the ones with a ported consumer:

| flag | set by | blocks |
|---|---|---|
| `BLOCK_ACTIONS` | `BlockActions` (stun/sleep/paralyze) | attack, cast, move |
| `ROOTED` | `Root` | move only |

Java's `CONDITIONAL_BLOCK_ACTIONS` (a `BlockActions` carrying an `allowedSkills`
whitelist) maps onto the same bit: `hasBlockActions()` ORs the two, so the only
divergence is that the whitelisted skills are blocked too — `TODO(G19)`.

## 3. The gates

Ported from `Creature`'s gate family, at the equivalent Rust entry points:

| Java | site | behaviour |
|---|---|---|
| `isAllSkillsDisabled()` | `use_magic_on` | stunned → `ActionFailed`, no cast |
| `isAttackDisabled()`→`isDisabled()` | `handle_attack_request` | stunned → `ActionFailed` |
| `isMovementDisabled()` | `handle_move_backward_to_location` | stunned or rooted → `StopMove` + `ActionFailed` |
| `AttackableAI.onEvtThink` | `npc_ai::think` | a stunned mob does nothing at all |
| `Creature.moveToLocation` | `npc_ai::move_npc_to` | a rooted mob stays put |

Java's `isMovementDisabled` also ORs `_isOverloaded`, `_isImmobilized`,
`isAlikeDead()` and `_isTeleporting`; overload/immobilise have no ported source,
and death and teleport are already gated separately at each call site, so only
the two effect-driven terms are folded in.

A *rooted* monster still `think`s — it can attack an adjacent target — and the
movement primitive refuses only the chase leg, which is what Java does.

## 4. Interrupting what was already happening

A stun must stop the victim mid-action, not merely prevent the next one, so
`apply_block_actions_interrupt` runs when a `BLOCK_ACTIONS` skill lands (both
the player and NPC buff paths): abort the cast (`abortCast`), then freeze
movement and broadcast `StopMove`.

**Order matters, and a test caught it.** `stop_casting` resumes the movement the
cast interrupted (`start_casting` stashes it), so clearing movement *before*
aborting the cast let the victim keep walking while stunned. Cast first, then
movement.

A root deliberately does none of this: it stops future movement but leaves a
running cast alone.

## 5. Keeping the buffs alive

`apply_skill_effects` drops buffs whose effect list yields nothing (the guard
behind the "icon-only buff" family). Stun and root carry **no stat modifier at
all**, so they needed the same exemption the DoT and icon-only effects already
have — `has_state_flag`. Without it a stun would have been dropped whole and
never landed, exactly the failure mode recorded for the community-board buffs.

## 6. Tests

`game_loop/tests/abnormal_tests.rs` (8 cases): flags set correctly per effect
(and a stun landing at all despite carrying no modifier), the mask clearing on
expiry, movement refused for both stun and root with the `StopMove` answer,
casting refused under stun but allowed under root, the mid-action interrupt of
both cast and movement, a stunned monster's AI going quiet and recovering, and —
tying the two G19 slices together — **an AoE stun that block-actions every mob
in its sweep and none outside it**.

Parsing is asserted against real datapack skills in `skill_data`: Shield Stun 92
(`STUN`, `BLOCK_ACTIONS`, no stat modifiers), Arrest 402 (`ROOT_PHYSICALLY`,
`ROOTED` and explicitly *not* `BLOCK_ACTIONS`), Might 1068 (no flags), and
Thunder Storm 48 — which is both a `POINT_BLANK` sweep and a stun, so it
exercises both slices at once.

## 6b. `<removedOnDamage>` — waking a slept target (2026-08-01 fix)

`BlockActions` gave sleep its lock but nothing ever took the lock back off
early, so a mob could sleep a player and then beat on them for the buff's full
duration without waking them. In Java the wake is not part of the effect at all:
it is a **skill-level tag**, `<removedOnDamage>`, read by
`EffectList.stopEffectsOnDamage()` off `CreatureStatus`/`PlayerStatus
.reduceHp`. The tag was simply unparsed here — `Skill` had no field for it and
no damage path called anything like it.

36 skills carry the tag on this dist. Most are `SLEEP` (the player skills 981 /
1069 / 1072 / 1097 / 1394 and the mob casts 4046 / 4185 / 4201 / 4640 /
4660-4662 / 5735 / 6853), the rest `HIDE` (922, 6093, …) and
`FORCE_MEDITATION` (441, 1430) — so the same fix also makes a hit break stealth
and meditation.

Ported as:

- `Skill::removed_on_damage`, parsed in `skill_data` with the same loose
  `true`/`True` compare `stay_after_death` uses.
- `skills::effects::stop_effects_on_damage` — expires every live buff whose
  skill declares the tag. It resolves `(skill_id, skill_level)` back through the
  skill table per buff rather than stamping a bool on `ActiveBuff`, which is
  what Java does too (`info.getSkill().isRemovedOnDamage()`); nothing to keep in
  sync, and DB-restored buffs behave like freshly-cast ones.
- Two call sites in `game_loop::combat`, mirroring Java's two `reduceHp`
  overrides. **They differ on `isDOT` and that is deliberate**: `CreatureStatus`
  wraps the whole wake block in `if (!isDOT && !isHPConsumption)`, while
  `PlayerStatus` puts `stopEffectsOnDamage()` *above* its `if (!isDOT)` guard —
  so a poison tick wakes a sleeping **player** but not a sleeping **mob**. The
  NPC call therefore sits in `apply_physical_damage`'s `is_npc_oid` branch under
  `!is_dot`; the player call sits ungated at the top of
  `player_receive_damage_ex`, above the sit/store stand-up, where Java has it.

Java's `awake` argument (`(skill == null) || !skill.isToggle()`) is not threaded
through: no ported damage source is a toggle. Toggles that cost HP drain
`Vitals` directly as Java's `isHPConsumption`, which never reaches this path.

Not ported alongside it: `Formulas.calcStunBreak` (a 14 % chance for a hit to
break a *stun*) is gated on `Config.ALT_GAME_STUN_BREAK` ← `BreakStun`, which
neither `dist/game/config` nor Java's default sets — so it is dead on this dist
and a stun correctly survives being hit. `calcRealTargetBreak`'s
`REAL_TARGET` abnormal has no Interlude source either.

Tests: `abnormal_tests::a_hit_wakes_a_slept_player_but_leaves_a_stun_alone`
(with the stun as the control, so the removal is proven to key off the tag
rather than clearing crowd control wholesale),
`a_hit_wakes_a_slept_monster`, and
`a_dot_tick_wakes_a_slept_player_but_not_a_slept_mob` for the asymmetry above.
All three were confirmed to fail with the two call sites disabled.

## 7. What is still missing

Immediately adjacent, all still inert: `BlockControl` (81 — Java's confusion /
mob-control), `Fear` (68 — needs forced flee movement in the AI), `DebuffBlock`
(115), `DamageBlock` (162), `TargetCancel` (101), `KnockBack` (91), and the
mute family (`MUTED`/`PSYCHICAL_MUTED`/`DISARMED`) whose flags are defined in
Java but have no ported effect producing them.

The wider G19 backlog is unchanged: `EFFECT_REGISTRY` growth toward the 369
Java effect classes (`StatUp` 887 is the biggest portable one left, then the
Pvp*DamageBonus family ~1500 combined), the geometric affect scopes,
`calcMagicSuccess`, the AVE runtime, and skill enchanting.
