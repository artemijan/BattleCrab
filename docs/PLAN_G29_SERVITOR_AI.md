# G29 slice 2 — Servitor follow & attack

## Why this next

Slice 1 put a servitor in the world; it stood there. This is the half that
meets G29's first gate clause — *"summon a servitor that follows and attacks."*

## What Java does

`SummonAI` is a `PlayableAI`, **not** an `AttackableAI`. The difference is the
whole design:

- Idle → `AI_INTENTION_FOLLOW` on the owner (`_startFollow`, defaulting to
  `getFollowStatus()` = true).
- It never scans for prey. It fights what its owner points it at, via the
  action bar: `ServitorAttack` (22), `ServitorStop` (23), `ServitorHold` (21),
  declared in `dist/game/data/ActionData.xml` and delivered by
  `RequestActionUse` (0x56).
- `ServitorAttack` bails to `AI_INTENTION_FOLLOW` when the target is more than
  **3000 units** from the owner, so a stray click cannot send the summon across
  the map.

## What landed

- **`ServitorOf.following`** — Java's `_startFollow`/`getFollowStatus()`.
- **`servitor_follow_tick`** — an idle, following servitor closes to
  `FOLLOW_RANGE` (150) of its owner. Reuses `npc_ai::move_npc_to`, so it
  inherits the geodata/pathfinding work from G21.
- **The NPC think dispatch** now branches on `ServitorOf` *before* the
  `AttackableAI` state machine: a servitor follows, and only runs the ordinary
  attack think once it has been ordered. That is what stops it hunting on its
  own — pinned by `a_servitor_does_not_pick_its_own_fights`.
- **`RequestActionUse`** (0x56) + the three servitor commands. An ordered
  attack seeds hate and flips the intention — the same primitive `GetAgro` and
  `Confuse` use, because this port's NPC AI derives its target from the aggro
  list each think rather than caching one. Ordering also **clears** the follow
  flag, or the servitor would drift home between swings.
- `servitor_stop` clears hate, halts movement and resumes following;
  `servitor_toggle_follow` is the hold/follow switch.

## Tests

`servitor_tests` grew to 16. The ones that matter:

- `a_servitor_does_not_pick_its_own_fights` — the `PlayableAI`-not-`AttackableAI`
  distinction, asserted by standing a monster next to it and running 200 ticks.
- `a_far_target_is_refused_and_the_servitor_keeps_following` — the 3000-unit
  bail.
- `an_ordered_attack_targets_the_owners_target` — including that following is
  switched off.
- `hold_toggles_following` and `stop_cancels_the_attack_and_resumes_following`.

**A test trap worth recording:** three tests failed at first because the
sparring dummy was placed at `NPC_OID`. A servitor is spawned through the
*runtime* allocator, which starts at `FIRST_NPC_OBJECT_ID` — exactly `NPC_OID` —
so the fixture NPC silently replaced the servitor. The dummy now has its own
`FOE` constant with the reason attached. This is the same collision the
`add_test_npc` doc comment already warns about, in a new guise: it bites
whenever a test spawns something at runtime *before* placing a fixture.

## Still open in G29

- **`SummonInfo` (0x8B)** — other players still cannot see a servitor.
- Lifetime expiry, item consumption, unsummon on logout/death, persistence.
- Master-buff inheritance, servitor skills (the `ServitorSkillUse` actions),
  exp/level, summon points.
- Pets (the second half of the gate), cubics, agathions.
