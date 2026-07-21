# G19 — GROUND casts + skill channeling (Volcano family)

## Why this slice

The last structural G19 item with real learnable reach. `targetType GROUND`
covers 22 skills, **7 learnable**; they split into two families:

- **Channeled ground AoEs** (this slice, 4 learnable): Volcano 1419, Cyclone
  1420, Raging Waves 1421, Gehenna 1423 — `operateType CA1`, damage delivered
  by `<channelingEffects>` ticks, `mpPerChanneling` drain, POINT_BLANK sweep
  centred on the **ground point**. Today the cast doesn't even start (GROUND
  resolves no target; the ex-packet is undispatched).
- **Symbol skills** (deferred, 3 learnable): Symbol of Noise 455, Day of Doom
  1422, Anti-summoning Field 1424 — `SummonNpc` effect + `OpExistNpc`
  condition; a totem-NPC subsystem of its own. `TODO(G19)`.

This beats the 2-learnable effect-registry tail, and the channeling runtime it
builds is the missing hook the effect-scopes slice named for
`channelingEffects` (24 skills / 4 learnable — all four are this family).

## Java sources

`RequestExMagicSkillUseGround` (ex **0x41**), `targethandlers/Ground.java`,
`affectscope/PointBlank.java` (the GROUND branch), `SkillChannelizer.java`,
`SkillCaster.java` (channeling start/stop, cast time, reagent consume),
`Skill.java` (channeling field parsing), `ZoneRegion.checkEffectRangeInsidePeaceZone`.

## The flow, end to end

1. **Packet** (ex 0x41: `x, y, z, skillId, ctrl(int), shift(byte)`): store the
   position (Java `Player._currentSkillWorldPosition` — **never cleared**,
   only overwritten by the next ground cast), face the point
   (`calculateHeadingFrom` + broadcast `ValidateLocation` — the client doesn't
   turn on its own for these), then enter the normal `useMagic` path.
2. **Target resolution** (`Ground.java` + the `useMagic` gate): GROUND with no
   stored position → ActionFailed. With one: `shift` (dontMove) checks 2D
   range vs `castRange + collisionRadius`; LOS caster→point
   (`CANNOT_SEE_TARGET`); for **bad** skills the effect circle must not clip a
   peace zone — Java samples **five points** (centre + N/S/E/W at
   `effectRange`), message `YOU_CANNOT_USE_SKILLS_THAT_MAY_HARM_...`. Returns
   **the caster** as sentinel target.
3. **Sweep** (`PointBlank.java` GROUND branch): candidates within
   `affectRange` of the **world position** (3D `isInsideRadius3D`), rest of
   the point-blank filter unchanged. Non-player casters get nothing (Java's
   `isPlayable` gate — the 15 NPC GROUND skills are inert there too, because
   `Ground.getTarget` already returns null for NPCs).
4. **Channeling** (`SkillChannelizer`): started by `startCasting` when
   `operateType.isChanneling()` (CA1), stopped by `finishSkill` — **both on
   completion and on abort**. Fixed-rate task: first fire at
   `channelingStart` s, period `channelingTickInterval` s (both
   seconds→ms ×1000). Each tick, in order:
   - `mpPerChanneling` gate: not enough MP → SM **140** ("Your skill was
     deactivated due to lack of MP") + `abortCast()`. Else drain.
     **Default is `mpConsume`**, not 0 (`set.getInt("mpPerChanneling",
     _mpConsume)`) — a channeling skill without the tag still drains.
   - Re-resolve the target and **re-sweep the scope** (a mob that walked into
     the volcano mid-channel burns; one that left stops burning). The affect
     limit re-rolls per tick (Java calls `getAffectLimit()` per handler run).
   - Per target: `effectRange` 3D check + LOS caster→target, then
     `applyChannelingEffects` — the CHANNELING effect scope through the
     normal apply pipeline (Volcano's `MagicalAttack power=500` gets the full
     resist/crit machinery).
   - The `channelingSkillId > 0` branch (stacking "channelized" buffs, per-
     tick PvP flag, `MagicSkillLaunched` echo) is **not** this family —
     carriers are hero stances 426/427 and Ertheia-era boss content.
     `TODO(G19)` at the branch point.
5. **Cast time is static for channeling**: `_hitTime = max(hitTime −
   cancelTime, 0)`, `_cancelTime = 2866` — **no** `calcSkillTimeFactor`
   scaling, so Volcano channels its full ~15 s regardless of casting speed.
6. **Reagent consume** (`SkillCaster`, general — not channeling-specific):
   `checkUseConditions` refuses without `itemConsumeCount × itemConsumeId` in
   inventory (SM **2156**); `startCasting` consumes them **at cast start**
   when `skill.isBad() || defaultAction == NONE` (Volcano: bad + Magic Symbol
   8876 → start). The finish-side/handler-side consume paths stay as they are
   (scrolls already consume through the item handler — no double consume).

## Rust mapping

- `Skill`: `mp_per_channeling` (default `mp_consume`), `channeling_tick_ms`,
  `channeling_start_ms`, `channeling_effects: Vec<SkillEffect>`,
  `OperateType::Channeling` for `CA1` (+ `DA*`? — only CA1 exists on this
  dist's reachable channelers; parse `CA1` alone, others stay `Other`).
- `EffectScope::Channeling` in the parser (was `Other`/dropped — the census
  test that pins the drop gets updated).
- `GroundSkillTarget { x, y, z }` component on the player (the Java field).
- Dispatch ex 0x41 → new handler in `cast.rs` (store + heading +
  `ValidateLocation` + `use_magic`).
- `resolve_cast_target`: `TargetType::Ground` leg per step 2.
- `sweep_radius` `Centre::Caster` + `target_type == Ground` → centre on the
  stored position (step 3).
- `ScheduledTask::ChannelingTick { caster, cast_seq }`: staleness = the
  `Casting` component's `seq` (removed by `stop_casting`, which every finish/
  abort path already funnels through — Java's `stopChanneling` for free).
  Re-schedules itself each fire while the cast lives.
- Per-tick shots: Java uncharges + recharges each tick; the port's shot model
  is recharge-only (`recharge_shots`), so the tick calls that like cast start
  does. Damage-side spiritshot bonus flows through the existing pipeline.

## Tests

1. Dist parse: Volcano → `Channeling`, tick 2 s, start 1 s, `mp_per_channeling`
   80, `channeling_effects` non-empty (closes the effect-scopes census gap),
   `item_consume` 8876×1.
2. Ground cast happy path: ex-packet → cast starts, ticks damage a mob
   standing at the ground point (not at the caster), MP drains per tick.
3. Re-sweep: a mob moved onto the point mid-channel takes later ticks; the
   ticks stop when the cast ends (scheduler drains clean).
4. Abort: move/abort mid-channel → no further tick damage. MP starvation →
   abort + SM 140.
5. Ground validation: no stored position → ActionFailed; shift beyond
   castRange refused; bad skill with effect circle clipping a peace zone
   refused.
6. Static cast time: channeling hit time ignores the caster's casting speed.
7. Reagent: cast refused without Magic Symbol; consumed at cast start.

## Deferred

Symbol skills (`SummonNpc` + `OpExistNpc`, 3 learnable) — `TODO(G19)`; the
`channelingSkillId` detention branch — `TODO(G19)`; NPC ground casts (inert in
Java too).
