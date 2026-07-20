# G19 — Fear skill effect

## Why this slice

`Fear` has been the named G19 hold-out since the CC-breadth slice, which
deferred it explicitly: *"`Fear` is the notable CC hold-out — it needs forced
flee movement, so it belongs with G21's AI breadth."* **G21 is now complete**
(NPC pathfinding, `move_npc_to`, the geodata-aware chase), so the blocker is
gone.

A fresh ranking sweep (learnable-skill usage, per the standing rule — not raw
instance count) puts it top of what's actually in scope:

| learnable | skills | effect | verdict |
|---|---|---|---|
| 31 | 634 | `DefenceAttribute` | out of scope — Kamael-era elemental attributes |
| 24 | 44 | `Summon` | G29 (summons/pets/servitors) |
| 12 | 28 | `SummonCubic` | G29 |
| 9 | 84 | `SummonNpc` | G29 |
| **8** | **68** | **`Fear`** | **this slice** |
| 4 | 51 | `StatByMoveType` | next candidates |

The 8 learnable instances are Horror 65, Banish Undead 405, Banish Seraph 450,
Fear 1092, Curse Fear 1169, Word of Fear 1272, Mass Curse Fear 1381 and Turn
Undead 1400.

**Why this was a quiet gap rather than a loud one:** every one of those skills
*also* carries `BlockControl`, which was already ported. So the buff landed —
icon, duration, `BLOCK_CONTROL` flag and all — and the skill looked like it
worked. It just never moved anyone, which is the entire point of a fear.

## What Java does

`handlers/effecthandlers/Fear.java` is much smaller than its reputation:

```java
public long getEffectFlags() { return EffectFlag.FEAR.getMask(); }
public int getTicks()        { return 5; }

public void onStart(...)      { effected.getAI().notifyEvent(CtrlEvent.EVT_AFRAID);
                                fearAction(effector, effected); }
public boolean onActionTime(...) { fearAction(null, effected); return false; }
public void onExit(...)       { if (!effected.isPlayer())
                                    effected.getAI().notifyEvent(CtrlEvent.EVT_THINK); }
```

Two findings from reading the surrounding tree, both of which shrank the port:

- **`EffectFlag.FEAR` has no reader.** There is no `isAfraid()` on `Creature`
  and nothing anywhere `isAffected(FEAR)`. A feared creature is *not* gated out
  of attacking, casting or walking. This is the recurring "dead in Java too"
  pattern already documented on `MP_BLOCK`, `MAX_MOMENTUM` and
  `INSTANT_KILL_RESIST`.
- **`EVT_AFRAID` has no handler.** `AbstractAI.notifyEvent`'s switch has no
  case for it, so the `onStart` notify is a no-op.

So the whole mechanic is the forced repositioning in `fearAction`:

```java
final double radians = Math.toRadians((effector != null)
    ? Util.calculateAngleFrom(effector, effected)
    : Util.convertHeadingToDegree(effected.getHeading()));
final int posX = (int) (effected.getX() + (FEAR_RANGE * Math.cos(radians)));  // 500
final int posY = (int) (effected.getY() + (FEAR_RANGE * Math.sin(radians)));
final Location destination = GeoEngine.getInstance().getValidLocation(..., posX, posY, posZ, ...);
effected.getAI().setIntention(CtrlIntention.AI_INTENTION_MOVE_TO, destination);
```

`onStart` aims **away from the caster**; every 5-tick repeat passes `null` and
aims along the victim's **own heading**, so they keep running the way the first
shove threw them rather than being re-aimed at a caster who may by then be dead
or long out of range.

`canStart` bails on `isRaid()`, and on the NPC side admits only the
`Attackable` subtree minus `Defender` / `FortCommander` / `SiegeFlag` /
`Race.SIEGE_WEAPON` — a fear must not scatter a castle's stationed defenders.

## What landed

- **`SkillEffect::Fear { ticks }`** + the parse arm. `<effect name="Fear"/>`
  carries no params in this dist and Java's constructor ignores its `StatSet`
  outright, so the cadence is the literal `FEAR_TICKS = 5` (`getTicks()`), not
  a parsed value. Added to `has_periodic` so the buff survives the
  empty-effects guard on its own merits, and to `schedule_dam_over_time`'s
  interval scan so it shares the existing DoT tick chain rather than growing a
  scheduler of its own.
- **`effect_flag::FEAR`**, folded for completeness with an explicit
  no-consumer note — matching Java's own dead code the way `MP_BLOCK` does.
- **`fear_action`** — the shove: bearing (caster-relative on start, heading on
  repeat), 500-unit projection, `geo.get_valid_location` clamp, then the
  player half (`position::intention_move_to`) or the NPC half
  (`npc_ai::move_npc_to`) of what Java keeps as one `Creature.moveToLocation`.
  Java's `toRadians(atan2-in-degrees)` round-trip collapses to the raw `atan2`,
  so the caster-relative case is computed directly in radians.
- **`fear_can_start`** — the raid and siege-defence carve-outs.
- **`NpcIntention::MoveTo`** — the load-bearing bit. `AttackableAI.onEvtThink`
  switches on the intention and has **no `AI_INTENTION_MOVE_TO` case**, so a
  fleeing mob thinks about nothing while it runs. Without this a feared mob
  would be dragged straight back by the next think tick re-issuing its chase,
  and the flee would be invisible. `CreatureAI.onEvtArrived`'s "`MOVE_TO` →
  `ACTIVE`" reset is ported alongside it, off a new `TickOutcome.arrived`.
- **`Fear.onExit`'s non-player `EVT_THINK`** — a mob whose fear runs out
  mid-flight is still parked on `MoveTo`, so `handle_buff_expire`'s NPC branch
  puts it back on `Active`. Gated on the expiring buff actually carrying the
  `FEAR` flag (read before the buff is dropped), so it stays specific to fear
  rather than firing for any expiring NPC buff.

## Tests

`game_loop::tests::fear_tests` (9):

- `fear_shoves_the_victim_directly_away_from_the_caster` — caster at the
  origin, victim due east: destination is x + 500 on the same y.
- `fear_keeps_pushing_the_victim_further_each_beat` — the heading-steered
  repeat, asserted as monotonic progress along the original bearing.
- `fear_stops_pushing_once_the_buff_is_gone` — the in-flight leg finishes, then
  nothing shoves again.
- `fear_never_moves_a_raid_boss`, `fear_never_moves_siege_defenders`,
  `fear_does_not_move_a_non_attackable_npc` — the three `canStart` legs.
- `feared_mob_stops_thinking_until_it_arrives` — an aggroed mob, AI ticking:
  it flees instead of chasing back. This is the test that fails without
  `NpcIntention::MoveTo`.
- `fear_expiry_returns_the_mob_to_active` — the `onExit` re-think.
- `real_dist_fear_skills_parse_a_fear_effect` — all 8 learnable ids parse a
  `Fear { ticks: 5 }` *and* keep their `BlockControl`, pinning the pairing that
  made this gap quiet.

## Deferred (not this slice)

- **`isSummon()`** in `canStart` — servitors are `TODO(G29)`; it folds into the
  player case once they exist.
- **`EffectFlag.FEAR` consumers** — there are none in Java either. If a later
  chronicle's gate is ever ported, the flag is already there to read.
