# G23 slice 6 — Core

The second grand-boss script, and the shared lifecycle from slice 4 meant the
whole slice was Core's own mechanics: script-spawned minions with a respawn
loop.

## The 19 that are 3

Java builds its spawn table as:

```java
private static final Map<Integer, Location> MINNION_SPAWNS = new HashMap<>();
static {
    MINNION_SPAWNS.put(DEATH_KNIGHT, new Location(17191, 109298, -6488));
    MINNION_SPAWNS.put(DEATH_KNIGHT, new Location(17564, 109548, -6488));
    …19 puts in total: 10 Death Knights, 5 Doom Wraiths, 4 Susceptors
}
```

The map is keyed by **npc id**, so every `put` overwrites the previous one for
that type. **Three entries survive**, one per minion type, at the *last*
location listed for each.

So Core spawns **3 minions, not 19**. That is plainly not what the author
intended — the 19 distinct coordinates are laid out around the lair — but it is
what the server does. Porting the list faithfully would have handed Core **six
times the adds** and a materially harder fight.

Ported as it behaves, with the reasoning at the constant and a test named for
it. This is the same principle as the `dist/` data being authoritative, applied
to script code: **port what it does, not what it looks like it means.**

## The rest

- Minions respawn 60 s after dying, **only while Core is alive** — Java guards
  on `getStatus(CORE) == ALIVE`, so a cleared lair stays cleared instead of
  repopulating around a corpse. Both directions tested.
- Core's death clears its minions after **20 s**, not immediately — the adds
  linger briefly rather than vanishing mid-animation. Tested as *still standing
  right after the kill*, which is the part a naive immediate-despawn would get
  wrong.

Barks (`REMOVING_INTRUDERS`, `A_FATAL_ERROR_HAS_OCCURRED`, …) are left for a
follow-up: the port's `npc_say` lives on the quest context and isn't reachable
from a boss script yet.

## Tests

New `core_boss_tests` (6), including the one that matters —
`core_spawns_three_minions_not_nineteen`.

## Still open in G23

Eight scripts (Antharas 1056 lines, Baium 787, Valakas 581, Orfen 384,
Sailren 326, DrChaos 321, Zaken 109), plus boss barks.
