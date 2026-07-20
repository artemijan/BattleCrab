# G19 — Confuse + RandomizeHate

## Why this slice

The ranking left the same five-way tie at 3 learnable as last time, so the
"prefer the tied entry with siblings" heuristic decided it. Grouping the
unported effects by prefix surfaced three clusters at 5 learnable each:

| cluster | learnable | skills |
|---|---|---|
| `Trigger*` (ByAttack/ByMagicType/ByDamage/…) | 5 | 362 |
| `TwoHanded*` (Blunt/Sword bonus) | 5 | 22 |
| **`Confuse` + `RandomizeHate`** | **5** | **7** |

`Confuse`/`RandomizeHate` won because they share **one blocker**, and it is one
this project already documented: the hate-effects slice deferred
`RandomizeHate` because it "needs a general nearby-visible-creatures query
`faction_call`'s NPC-only neighbour scan doesn't provide". Building that query
once unblocks both — the same "a documented deferral's blocker is gone" logic
that made `Fear` a good pick.

**Four of the five skills carry only the unported effect** — Madness 1105,
Curse Discord 1163, Seal of Mirage 1213 (`Confuse`) and Confusion 2
(`RandomizeHate`) — so all four were dropped whole. Switch 12 pairs
`RandomizeHate` with the already-ported `TargetCancel`, so it landed but never
switched anyone's hate.

## What Java does

Both are instant, both gated by `Formulas.calcProbability`, and both start from
`World.forEachVisibleObject(effected, Creature.class, …)`.

- **`Confuse`** — pick a random visible creature and `setTarget` +
  `AI_INTENTION_ATTACK` on it. Hate is only *added*.
- **`RandomizeHate`** — bail unless the effected is an `Attackable` and not the
  effector; exclude the effector and any attackable **of the victim's own
  faction** ("aggro cannot be transfered to a mob of the same faction"), then
  `getHating` → `stopHating` → `addDamageHate`. Hate is *moved*.

`calcProbability` reduces to `Rnd.get(100) < magicLevel + chance - targetLevel`
once the attribute/trait bonuses (1.0) and abnormal resist (0) drop out. It is
unclamped, so a high-level target can push the threshold to zero and the effect
simply never lands.

### `EffectFlag.CONFUSED` is unreachable here

`Confuse.getEffectFlags()` declares it, and Java has two readers
(`AttackableAI`'s "attack the effect's target rather than the most-hated"
branch, and `Creature.onActionRequest`'s player gate). Neither can fire on this
dist: `isInstant()` is true, so the effect never joins a `BuffInfo`'s effect
list, and none of the skills has an `<abnormalTime>` for a buff to live in.

**The trap:** three of them *look* like they do —
`<effect name="Confuse" abnormalTime="20">`. That is an **attribute**, and
Java's `parseNamedParamInfo` reads only `name`, `level`, `from|toLevel` and
`sub*Level` off an effect element, so `abnormalTime` there is silently ignored.
It appears 7 times datapack-wide (on `Fear` and `Confuse`) and means nothing in
either. Folded as an inert flag, matching the `FEAR`/`MP_BLOCK` precedent, with
a test pinning `abnormal_time == 0`.

## What landed

- **`helpers::visible_creatures`** — the deferred query: every living player or
  NPC in an adjacent region cell, excluding the origin. Java's "visible" is
  exactly this region-neighbourhood test (no LOS or radius term in
  `forEachVisibleObject`), so none is added. Returned **sorted**, so a forced
  roll in a test picks a known candidate; a uniform index over a sorted list is
  still uniform.
- **`formulas::calc_probability`**, and `SkillEffect::Confuse`/`RandomizeHate`
  with their parse arms.
- **`effect_flag::CONFUSED`**, folded and documented as inert.
- The two effect arms plus `random_bystander` / `same_npc_faction` /
  `retarget_onto` helpers. `retarget_onto` reuses the `GetAgro` precedent: the
  ported NPC AI derives its target from `AggroList::most_hated` each think tick
  rather than caching one, so "force-attack this creature" becomes "make its
  hate dominant". A confused *player* just gets their target swapped.

## Tests

`game_loop::tests::confuse_tests` (10). Notable:

- `confuse_adds_a_target_without_erasing_the_old_hate` vs
  `randomize_hate_moves_the_casters_hate_to_a_bystander` — the pair that pins
  the difference between the two effects, which look interchangeable at a
  glance: one adds, the other moves.
- `randomize_hate_refuses_to_pass_aggro_to_a_clan_mate` — the faction exclusion
  `Confuse` does *not* have.
- `confuse_skills_have_no_real_abnormal_time` — pins the `abnormalTime`
  attribute trap above.
- `real_dist_skills_parse_with_their_chances` — the real values are 20/20/60
  (Confuse) and 80/80 (RandomizeHate). **None defaults to 100**, so the
  parser's default is exercised by no shipped skill.

Two tests were initially flaky because the random pick is a coin flip between
the caster and the bystander. Fixed the right way: the query now returns a
stable order and the decisive test forces the index roll, rather than weakening
the assertion.

## Noted, not fixed

The Rust skill parser **ignores `fromLevel`/`toLevel` attributes on `<effect>`
elements** — 775 instances each in this datapack. Java uses them to gate an
effect to a skill-level range, so effects that should apply only at some levels
currently apply at all of them. Out of scope here, but it is a real parity gap
worth its own slice.

## Deferred (not this slice)

- **`TargetMe`** — still paired with `GetAgro` on 2 skills, still needs a
  locked-target UI concept this port lacks.
- The rest of the tied cluster: `TriggerSkillByAttack`, `ReflectSkill`,
  `BlockMove`, `TwoHandedBluntBonus`.
