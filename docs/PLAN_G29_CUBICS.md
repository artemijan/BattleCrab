# G29 slice 9 — cubics

## Why cubics, not agathions

The learnable-skill ranking settled it, and the raw counts would have pointed
the wrong way:

| effect | skills | **learnable** |
|---|---|---|
| `SummonCubic` | 28 | **12** |
| `SummonAgathion` | 166 | **0** |

All 166 agathion skills are off every skill tree on this dist — event/item
content that never ships here. Ranking by raw instance count would have put six
times the work into something no player can reach. Same lesson as
`l2r-abnormal-resist-dispel`: **rank by reachable content, not by count.**

## A cubic is not a world object

Unlike a servitor, a cubic has no template, no position, no AI and no object id.
It lives on the player as a `Cubics` component, and other players see it only as
an id in the owner's `CharInfo`. A test asserts no NPC entity is spawned —
otherwise a cubic would be targetable and attackable.

## `MAX_CUBIC` is always 1 here

Java reads `Stat.MAX_CUBIC` (`cubicCount`), defaulting to 1. **Nothing in this
entire datapack sets `cubicCount`** — Cubic Mastery does not exist on Interlude
Classic — so a player can only ever have one cubic, and a second always
displaces the first. The code keeps Java's "drop a random existing cubic" shape
rather than hard-coding the simplification, with a `TODO(G29)` to read the stat
if a `cubicCount` skill ever appears.

## `CharInfo` was hard-coding zero cubics

`w.write_i16(0); // cubic count` — so a summoned cubic would have been invisible
to every other player. Exactly the shape of the abnormal-visual-effect bug G19
slice 6 found (`l2r-abnormal-visuals`). Now `char_info` takes the id list, and
`visibility::refresh_char_info` re-sends the whole record when a cubic is gained
or lost — this chronicle has no incremental cubic packet.

**Worth a standing check:** hard-coded `0` counts in packet builders are latent
"feature invisible" bugs. Two found so far in the same file.

## Behaviour ported

- **Action loop** (`Cubic.readyToUseSkill`) as `ScheduledTask::CubicAction`.
  Java's `scheduleAtFixedRate(..., 0, delay)` fires **immediately** on summon,
  not after one delay — ported as a 0-delay first schedule.
- **Skill choice** (`chooseSkill`): cumulative `triggerRate` weights against one
  roll, so the weights are shares of 100, not independent chances. A lone skill
  with no `triggerRate` defaults to 100 so it always wins the roll.
- **`successRate`** is rolled *after* the skill is chosen — it gates the cast,
  not the choice.
- **`maxCount` counts actions, not attempts.** A cubic that fails its roll, has
  no target, or is out of range has not spent a charge. Two tests pin this,
  including the dead-target case.
- **Owner `<hp>` condition** — a badly wounded player's attack cubic holds fire.
- **`<range>`** and the target-side **`<healthPercent>`** band.
- **Target types**: `TARGET` (owner's current target), `HEAL` (most wounded of
  owner + party within party range, skipping the dead — "Life Cubic should not
  try to heal dead targets"), `MASTER`, and `BY_SKILL` deferring to the nested
  skill's own type.
- Cubics do not survive the owner leaving the world; nothing persists them.

## Tests

`cubic_data` 2 (both datapack-backed — the real table parses, and multi-skill
cubics carry real weights), `cubic_tests` 13.

One test failure was informative rather than a bug: the dummy died to the first
cast, so the second action correctly found no live target and spent no charge.
Fixed the fixture and **pinned the behaviour it revealed** as its own test.

## Still open

- `power` is parsed but unconsumed — the cubic skill's own power is used
  instead. Java folds the template `power` into the cast; worth checking whether
  any cubic skill relies on it (`TODO(G29)` at the field).
- Cubic skills that need the *owner's* stats (m.atk scaling) currently run
  through `apply_skill_effects` with the owner as caster, which is right for
  damage but means a cubic can crit off the owner's stats.
- Agathions — deliberately deferred as unreachable content, with the numbers
  above as the justification.

---

# Addendum — slice 11: `power` was not unconsumed after all

The cubics plan closed with "`power` is parsed but unconsumed … worth checking
whether any cubic relies on it." Checking said **yes, and the port had it
wrong.**

`CubicTemplate` in Java:

```java
_power = set.getDouble("power") / 10;
@Override public int getBasePAtk() { return (int) _power; }
@Override public int getBaseMAtk() { return (int) _power; }
```

`Cubic extends Creature`, and the cast is `skill.activateSkill(this, target)` —
**the cubic is the caster, not its owner**. The port passed the owner, so cubic
damage scaled off the *player's* m.atk. Storm Cubic level 1 is `power=282` →
m.atk **28.2**; a levelled mage's m.atk is many times that, so cubics hit far
harder than retail.

Nothing would have caught this: damage was non-zero, the cubic "worked", and
every existing test passed. It took reading the Java for a field the port had
already parsed. **"Parsed but unconsumed" cuts both ways — the port can also be
consuming the wrong thing.**

## The fix

A cubic now gets a real caster entity carrying `CombatStats` (p.atk = m.atk =
`power / 10`), `Vitals` and `Position`, but **no `Npc`, `Player`, `RegionCell`
or `Movement`** — every store sweep in the server is anchored on one of those
five, so it stays invisible to visibility, targeting, movement and AI while
being a valid caster for the damage formulas. That check was done by
enumerating `for_each_mut::<…>` call sites, not assumed.

The entity is despawned with the cubic; a test asserts it, since otherwise every
summon leaks one.

## Two bugs found while fixing it

1. **`Cubic.getLevel()` returns `_owner.getLevel()`.** A cubic borrows its
   owner's *level* while using its own *power*. Without that link the caster
   resolved to level 1, the level gap made every cast resist, and the cubic did
   **zero** damage — worse than the bug being fixed. Ported as a `CubicOf`
   component that `creature_level` checks first.
2. **`add_components` silently no-ops on an id the store has never seen.** The
   caster entity was allocated but never `spawn`ed, so it had no stats at all.
   Worth knowing generally: `spawn` first, then `add_components`.

## Tests

`cubic_tests` 13 → 16. The load-bearing one runs the same cast twice with the
owner's m.atk at 10 and at 5000 and asserts the damage is **identical** — a
500× swing in the owner's stats must not move cubic damage. Plus the owner-level
delegation and caster-entity cleanup.
