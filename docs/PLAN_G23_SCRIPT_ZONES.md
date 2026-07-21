# G23 slice 3 — `ScriptZone` support

Groundwork for the `ai/bosses` family. Every grand-boss script opens with
`ZoneManager.getZoneById(…)` and uses the result for `isInsideZone` and
`movePlayersTo`; none of that existed.

## What a `ScriptZone` is

**Nothing, behaviourally.** It has no `ZoneId` in Java, so it sets no membership
bit and no flag — it exists purely to be *addressed by id* from a script. Queen
Ant's lair is `getZoneById(12012)`, a zone named "Queen Ant Boss" in
`custom_script.xml`.

That is why `ZoneKind::Script.bit()` is `0`, with a test asserting it: giving it
a bit would put every player standing in one into a zone state nothing intends.

## Three pieces

- `ZoneKind::Script` + `type="ScriptZone"` mapping (133 zones across two files).
- `Zone.id`, kept from `<zone id="…">` — the spatial lookups never needed it, so
  it was being discarded.
- `ZoneData::by_id` and `Zone::contains`, the two operations the scripts use.

## Adding a file for one kind must not change another

`custom_script.xml` is loaded for its 109 script zones — and it also ships a
stray `SiegeZone`, **"GainakSiege"**, a later-chronicle area with no `castleId`.

Letting it through would set the Siege membership bit on anyone standing there,
and `death.rs` reads that bit as a **free-death zone**: dying in Gainak would
silently skip the exp penalty. The two script files are therefore filtered to
script zones only.

This was caught by the existing zone-census test, which is exactly what a
census test is for — it failed on the count, and following *why* the count moved
found the siege zone rather than just bumping the number. **A census assertion
that gets "corrected" without reading the diff has been thrown away.**

## Tests

New `boss_zone_tests` (4), and the zone census updated to 1031 with the reason
recorded in the assertion.

All four read the **real** dist data: zone 12012 is addressable and is a
`Script` kind; containment matches the polygon (Queen Ant's own spawn point is
inside, a distant point and one above `maxZ` are outside); a script zone claims
no membership bit; an unknown id finds nothing rather than returning the first
zone.

## Next

QueenAnt itself — `GrandBossManager` status/statset already load, and the zone
it needs now resolves. The remaining work is the script: spawn/respawn window,
the nurse/guard/royal minions, the heal behaviour, and `movePlayersTo` on
spawn.
