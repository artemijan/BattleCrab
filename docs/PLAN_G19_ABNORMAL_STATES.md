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
