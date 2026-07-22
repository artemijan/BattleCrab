# G19 — SummonNpc symbols (EffectPoint totems)

## Why this slice

The half of `targetType GROUND` the channeling slice deferred: **3 learnable
skills** — Symbol of Noise 455 (Bladedancer), Day of Doom 1422 (Spellhowler),
Anti-summoning Field 1424 (Phantom Summoner) — beating the 2-learnable
effect-registry tail. Each drops a **seal** at the aimed point that pulses an
aura for 15 s. Today the `SummonNpc` effect is unparsed, so all three cast,
consume their reagent, and do nothing.

## Java sources

`effecthandlers/SummonNpc.java` (the `EffectPoint` branch),
`model/actor/instance/EffectPoint.java`,
`skillconditionhandlers/OpExistNpcSkillCondition.java`, the totem templates
(`npcs/13000-13099.xml`) and aura skills 5124 / 5145 / 5134.

## The moving parts

1. **`SummonNpc` effect** (`npcId`, `npcCount`, `despawnDelay`; our three
   declare only id+count): instant, effected must be a live player; spawn
   position = the stored ground point for GROUND skills. Only the
   `EffectPoint` template type is ported — `Decoy` and the default plain-spawn
   branch are `TODO(G19)` (their carriers are decoys/event NPCs, none
   learnable).
2. **`OpExistNpc` skill condition** — the first entry of a real condition
   layer: `<npcIds>` + `<range>` + `<isAround>`. Java sweeps NPCs within
   `range` **of the caster** (not the aimed point — you can stack two symbols
   900 apart, you just can't cast standing next to one); a listed id found
   returns `isAround`, none returns `!isAround`. Refusal is a bare
   ActionFailed. Dist quirk ported as data: the id list (13018–13024) covers
   Symbol of Noise's totem 13019 but **not** Day of Doom's 13028 or
   Anti-summoning's 13030 — those two don't block re-casting. Dist is spec.
3. **The totem runtime** (`EffectPoint.java`): spawn the NPC (type
   `EffectPoint` — not a monster, so no AI, not attackable), title = owner's
   name, owner link, then a fixed-rate cast task — first fire `cast_time`
   (default **0.1 s**), period `skill_delay` (2 s) — that `doCast`s the
   template's `union_skill` parameter; despawn after the template's
   `despawn_time` (15 s; falling back to the effect's `despawnDelay`).
   Template `<parameters>` (`<param name value/>`, `<skill name id level/>`)
   were parsed only for minions until now — this adds the generic
   `ai_params`/`ai_skill_params` maps.
4. **The owner is a friend of their own seal.** The auras are `SELF` +
   `POINT_BLANK` + `NOT_FRIEND`; Java's friend test runs through
   `getActingPlayer()`, which `EffectPoint` overrides to return the owner —
   so the seal debuffs everyone *except* the owner (and their party/clan).
   The port's `is_friend` said "NPCs are never friends", which would make
   Day of Doom curse its own caster. New `SummonerRef` component + an
   acting-player hop in `is_friend`. (Same shape G29's servitors will need —
   see the `l2r-acting-player` lesson.)

## What the auras actually do (ported-effect audit)

5145 Day of Doom: `PAtk`/`Speed`/`PhysicalDefence`/`HpRegen` percent debuffs
land (ported stat mods); `BuffBlock`, `DefenceAttribute` drop at parse
(unported), `MagicMpCost` lands. 5124 Anti-music: `DispelBySlotProbability`
(ported — the Bane slice). 5134 Anti-summoning: `Unsummon` drops (G29),
`DispelBySlotProbability` + attack-speed debuffs land. So every seal does
something real today, and the dropped effects are the usual
registry-breadth tail.

## Deliberate narrowings (TODO(G19) at the sites)

- `Decoy` and default-spawn branches of `SummonNpc`.
- Per-tick PvP flagging of the owner (Java flags the acting player when the
  aura debuffs a player; the port's NPC cast path doesn't flag).
- `singleInstance`/`randomOffset`/`isSummonSpawn` params (unused by the
  learnable three).

## Tests

1. Dist parse: 1422 carries `SummonNpc{13028, 1}` + `OpExistNpc{13018–13024,
   200, false}`; totem 13028's template exposes `skill_delay` 2 /
   `despawn_time` 15 / `union_skill` 5145.
2. Cast → totem: ground-cast the skill, advance past `hitTime` — the totem
   stands at the aimed point with the owner's name as title.
3. The aura pulses: a bystander player inside 200 gets the debuff; the owner
   does not (SummonerRef friendship); re-entry mid-lifetime still gets hit
   (fixed-rate re-cast).
4. Lifetime: the totem despawns after `despawn_time`, and the pulses stop.
5. `OpExistNpc`: casting with a listed totem within 200 of the caster is
   refused; with it 250 away, allowed.
