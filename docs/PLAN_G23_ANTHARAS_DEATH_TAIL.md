# PLAN G23 — Antharas death tail (exit cube + zone cleanup)

**Status:** in progress (branch `feat/g23-antharas-death-tail`). Closes a
player-facing gap in the Antharas fight: after the kill there was no way out of
the lair (the Java `onKill` tail — teleport cube, minion despawn, zone clear —
was never ported), so players were stranded until they died or relogged.

## Java source (`ai/bosses/Antharas/Antharas.java`, `onKill`)

When Antharas (29068) dies with the killer in the lair:
1. `notifyEvent("DESPAWN_MINIONS")` — deletes every Behemoth (29069) / Terasque
   (29190) in the zone.
2. death cinematic: `SpecialCamera(npc, 1200, 20, -10, 0, 10000, 13000, …)` +
   `PlaySound("BS01_D")`.
3. `addSpawn(CUBE 31859, 177615, 114941, -7709, 0, false, 900000)` — the
   Teleportation Cubic, for 15 minutes.
4. respawn window + `setStatus(DEAD)` — **already handled** by the shared
   `grand_boss::on_grand_boss_killed` (window roll + persist + timer) and
   `dead_status(ANTHARAS) == 3`.
5. `startQuestTimer("CLEAR_ZONE", 900000)` — 15 minutes later, everything in the
   lair leaves: NPCs `deleteMe`, players `teleToLocation(EXIT_POINT)`.

The cube's `teleportOut` talk (→ `EXIT_POINT` = `79800+rnd(600),
151200+rnd(1100), -3534`) is **already ported** — the `AntharasHeart`
`QuestScript` lists CUBE 31859 in its talk npcs and handles the `teleportOut`
event. Nothing ever spawned a cube for it to act on; this slice does.

## What this slice adds — `game_loop/antharas.rs`

- `on_antharas_killed(world, npc_oid)`, called from `death::npc_do_die` right
  after `grand_boss::on_grand_boss_killed` (which has already flipped the status
  to DEAD and armed the respawn). Gated on the dying NPC being Antharas.
  - `despawn_lair_minions` — `death::despawn_npc` for every Behemoth/Terasque
    standing in the lair zone.
  - death cinematic to the lair (`SpecialCamera` + `PlaySound("BS01_D")`).
  - spawn CUBE 31859 at `(177615, 114941, -7709)`.
  - schedule `ScheduledTask::AntharasClearZone` at +15 min.
- `handle_clear_zone(world)` (the `CLEAR_ZONE` timer): teleport every player in
  the lair to `EXIT_POINT`, then `despawn_npc` every NPC in the lair (the cube
  and any straggler minions — the boss is already gone).
- Helpers `players_in_lair_oids` / `npcs_in_lair` (zone-containment scans, the
  same `zone.contains` the entry occupancy check already uses).

New `ScheduledTask::AntharasClearZone`. The `DEATH_CUBE` location and the
already-present `EXIT_POINT` / `LAIR_ZONE_ID` / `BEHEMOTH` / `TERASQUE` /
`CUBE` constants back it.

## Bug found: wrong lair zone id

`LAIR_ZONE_ID` was `12016` — but in the dist that id is a Talking Island
`ScriptZone`, not the Antharas Nest. Java uses `getZoneById(70050,
NoRestartZone.class)` (`antaras_no_restart` in `no_restart.xml`, the polygon
around `173386..188132, 110284..119391`, z `-8380..-4880`). The wrong id was
latent because its only reader — the entry occupancy check `players_in_lair` —
**fails open**: a zone that reads as empty just means "nobody inside", so the
`MAX_PEOPLE` gate never tripped and no test noticed. Fixed to `70050`, which
both this slice's zone scans and the occupancy check now depend on.

## Deferred (`TODO(G23)`)

Valakas's symmetric death tail (15 teleport cubes 31759 + `oustAllPlayers` after
the 8-beat death cinematic) is the natural follow-up — same shape, different
cube count and the cinematic beats. Left for the next slice.

## Tests — `game_loop/tests/antharas_tests.rs`

- Killing Antharas spawns the exit cube (31859 present in the lair) and despawns
  its minions.
- The cube's `teleportOut` talk moves a player to the exit (through the real
  `Quest Antharas teleportOut` bypass — the router-level entry).
- `CLEAR_ZONE` firing ousts a lingering player to the exit and removes the cube.
- Sabotage: drop the death hook / the clear-zone dispatch and the matching test
  fails.
