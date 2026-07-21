# G23 slice 10 — Valakas (attack rules)

## The first boss with a four-state ladder

Valakas doesn't use the ALIVE/DEAD pair the simpler bosses share:

| status | meaning |
|---|---|
| 0 `DORMANT` | spawned, nobody entered; entry unlocked |
| 1 `WAITING` | someone entered, 30-min window for others; entry unlocked |
| 2 `FIGHTING` | engaged; entry **locked** |
| 3 `DEAD` | killed; entry locked |

This slice ports the `onAttack` half only. The lair entry flow and the
30-minute window are their own slice, and are stated as such rather than left
implied.

## Three rules, and the order is the mechanic

```java
if (!BOSS_ZONE.isInsideZone(attacker)) { attacker.doDie(attacker); return; }
if (getStatus(VALAKAS) != FIGHTING)    { attacker.teleToLocation(ATTACKER_REMOVE); return; }
if (mounted on strider && !affected)   { doCast(4258); }
```

- **Attacking from outside the lair kills you.** A hard anti-exploit against
  plinking at Valakas from safety. Self-inflicted in Java (`doDie(attacker)`),
  so it carries no PvP or karma consequence for anyone.
- **The zone check comes first**, so an out-of-zone attacker dies *whatever* the
  boss's status — including while Valakas is dead, when the status branch would
  otherwise merely have teleported them. Its own test, because that is the half
  a reordering silently loses.
- The strider debuff is cast **once** (`!isAffectedBySkill(4258)`), not every
  swing.

Zone 12010 is a `ScriptZone`, which slice 3 taught the loader to read — this is
the first script to actually consume that.

## A fixture guard, added before it was needed

`the_fixtures_lair_point_is_actually_inside_the_zone` asserts the coordinate the
other tests use really is inside zone 12010. Without it, a wrong coordinate
would make every "inside the lair" test silently exercise the *outside* path and
still pass — the fifth-instance failure mode from Queen Ant, pre-empted rather
than rediscovered.

## Tests

New `valakas_tests` (5): the fixture guard, death for attacking from outside,
the zone check preceding the status check, removal-not-death before the fight,
and a legitimate hit allowed through.

## Still open in G23

Valakas's entry flow and 30-minute window; Baium (787 lines); Antharas (1056).
