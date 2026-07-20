# G19 — hate-manipulation skill effects (GetAgro/AddHate/DeleteHate/DeleteHateOfMe)

## Why this slice

A fresh ranking sweep after `EnlargeSlot` closed out the previous batch
turned up a tied cluster of six related effect names, all thin wrappers over
the same primitive — an NPC's aggro (hate) list: `GetAgro` (4 learnable —
Aggression, Aggression Aura, Judgment, Tribunal), `AddHate` (2 — Charm,
Lure), `DeleteHate` (3 — Eva's Serenade, Peace, Repose), `DeleteHateOfMe` (3
— Bluff, Forget, Trick), `RandomizeHate` (2 — Confusion, Switch), `TargetMe`
(paired on the same 2 skills as `GetAgro`). Rather than take the top single
name and set the rest aside a fifth time (the running pattern for
`AttackTrait`), this slice bundles the four cheap ones — `GetAgro`,
`AddHate`, `DeleteHate`, `DeleteHateOfMe` (12 learnable-skill instances) —
since they share one tiny, already-ported primitive
(`model::npc::AggroList`) and don't each need separate infrastructure work,
matching the precedent set by the earlier `G19_CC_BREADTH` bundle.

## What Java does

- `GetAgro.java` — instant: `effected.getAI().setIntention(AI_INTENTION_ATTACK,
  effector)` (forces the NPC to attack the caster directly), then finds
  clan-mates in `template.getClanHelpRange()` and calls
  `nearby.addDamageHate(effector, 1, 200)` + the same intention set on each.
  No XML params.
- `AddHate.java` — instant: `power > 0` → `addDamageHate(effector, 0, power)`
  + `setRunning()`; `power < 0` → `reduceHate(effector, -power)`.
- `DeleteHate.java` — instant, chance-gated: `target.clearAggroList()` +
  `setWalking()` + `setIntention(AI_INTENTION_ACTIVE)`.
- `DeleteHateOfMe.java` — instant, chance-gated: `target.stopHating(effector)`
  (zeroes just that one attacker's hate, entry stays) + `setWalking()` +
  `setIntention(AI_INTENTION_ACTIVE)` — the AI disengages *wholesale* even
  though other attackers' hate is untouched.
- `TargetMe.java` — a buff (`onStart`/`onExit`), not instant, and not a
  hate-list effect at all: it force-locks the caster's target onto the
  *player's own* target-selection UI (`Playable.setLockedTarget`). No
  equivalent concept exists on this port (nothing gates `RequestTarget`/
  target-changing packets on a lock flag) — left unported.
- `RandomizeHate.java` — instant, chance-gated: transfers one hated
  attacker's hate value to a random other nearby visible creature. Needs a
  general "find nearby visible creatures" primitive this port doesn't have
  yet (the closest analog, `npc_ai::faction_call`'s neighbour scan, only
  enumerates NPCs, not players) — left unported.

## What landed

- **`SkillEffect::GetAgro`/`AddHate { power }`/`DeleteHate { chance }`/
  `DeleteHateOfMe { chance }`** (`model/skill.rs`) + their parse arms
  (`data/skill_data.rs`, the same `value_at(params, "chance"/"power", level)`
  shape as the existing `TargetCancel`).
- **`GetAgro`**: the ported AI (`npc_ai::think_attack`) derives its attack
  target fresh from `AggroList::most_hated` every think tick — there's no
  cached "current target" field on `NpcAi` to force directly the way Java's
  AI object allows. The faithful equivalent of "force intend-attack the
  caster" is making the caster's hate dominant (`current_max + 1`, not an
  arbitrary huge constant that would make the taunt unbreakable), plus
  setting `NpcAi::intention = Attack` the same way `minions::add_hate`
  already does elsewhere. The clan-help pre-seed is **not** ported — this
  port's already-existing `npc_ai::faction_call` recruits clan-mates on its
  own once the taunted NPC is actually landing hits on the caster, at most
  one think-tick later than Java's immediate pre-seed.
- **`AddHate`**: adds/subtracts the caster's hate entry directly (floored at
  0 on the way down), waking the AI on a positive change.
- **`DeleteHate`/`DeleteHateOfMe`**: both reuse a newly `pub(crate)`
  `npc_ai::set_active` (previously private — the exact function
  `think_attack`'s own timeout/leash paths already call to disengage) so the
  effect handlers and the AI's own internal disengage path share one
  implementation.

## Test

- `data::skill_data::tests::hate_effects_parse_getagro_addhate_deletehate` —
  real dist shapes inline (Aggression's `TargetMe`+`GetAgro` pair, with
  `TargetMe` confirmed to drop silently rather than block `GetAgro`;
  `AddHate`'s `power`; `DeleteHate`/`DeleteHateOfMe`'s `chance`).
- `game_loop::tests::skills_tests::hate_effects` (4 tests): `GetAgro` against
  a decoy with pre-existing higher hate (the caster ends up dominant, AI
  attacks); `AddHate` raising then lowering (floored at 0); `DeleteHate`
  wiping a two-entry list entirely; `DeleteHateOfMe` zeroing only the
  caster's entry while leaving the decoy's hate untouched, and disengaging
  the AI regardless.

## Deferred (not this slice)

- `TargetMe` — needs a locked-target UI concept (`RequestTarget`/other
  target-changing paths would all need to check a lock flag); no such
  primitive exists on this port.
- `RandomizeHate` — needs a general "nearby visible creatures" query (`
  faction_call`'s neighbour scan only walks NPCs, not players); `TODO`
  left on `SkillEffect` doc comments would be premature without the query
  shape decided, so this is tracked here instead.
- `GetAgro`'s clan-mate pre-seed broadcast — `faction_call` covers the same
  ground reactively, one think-tick later.
