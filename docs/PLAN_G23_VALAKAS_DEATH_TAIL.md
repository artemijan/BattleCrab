# PLAN G23 — Valakas death tail (exit cubes + zone clear)

**Status:** in progress (branch `feat/g23-valakas-death-tail`). The symmetric
counterpart to the Antharas death tail — same player-facing gap (no way out of
the lair after the kill), same shape, more cubes and a longer cinematic.

## Java source (`ai/bosses/Valakas/Valakas.java`, `onKill`)

When Valakas (29028) dies:
1. cancel `regen_task` / `skill_task`.
2. `PlaySound(1, "B03_D", …)` + an opening `SpecialCamera(npc, 1200, 20, -10,
   0, 10000, 13000, …)`.
3. schedule `die_1`..`die_8` (300 / 600 / 3800 / 8200 / 8700 / 13300 / 14000 /
   16500 ms from the kill) — each a `SpecialCamera` beat.
4. respawn window + `setStatus(VALAKAS, DEAD)` — **already handled** by the
   shared `grand_boss::on_grand_boss_killed` (`dead_status(VALAKAS) == 3`).
5. `die_8` also `addSpawn(31759, loc, false, 900000)` for the **fifteen**
   `TELEPORT_CUBE_LOCATIONS`, and `startQuestTimer("remove_players", 900000)`.
6. `remove_players` → `BOSS_ZONE.oustAllPlayers()`.

The cube's `teleportOut` talk (→ `LAIR_EXIT`) is **already ported** — `CUBE
31759` is registered by `scripts::valakas_teleporters` and routes to
`valakas::teleport_out`. Nothing spawned a cube for it; this slice does.

## What this slice adds — `game_loop/valakas.rs`

- `on_valakas_killed(world, valakas_oid)`, called from `death::npc_do_die` right
  after the Antharas hook (gated on the dying NPC being Valakas). Plays the
  death sound + opening camera to the lair, then schedules the eight-beat
  `DEATH_CINEMATIC` **up front from the kill** — the same batch model as the
  ported entry cinematic (the beats are unevenly spaced; a relative chain is
  easy to get subtly wrong).
- `handle_death_cinematic_step(world, valakas_oid, step)` — broadcasts each
  beat's `SpecialCamera` to the lair; the **eighth** beat also spawns the
  fifteen exit cubes and arms `ValakasRemovePlayers` at +15 min.
- `handle_remove_players(world)` — teleport every player still in the lair out
  to `LAIR_EXIT` (`oustAllPlayers`).
- `players_in_lair_oids` — the zone-containment scan, reusing the verified
  `BOSS_ZONE_ID = 12010` (`getZoneById(12010)`; unlike Antharas's, this id was
  already correct and fixture-guarded).

New `ScheduledTask::ValakasDeathCinematic { valakas_oid, step }` and
`ValakasRemovePlayers`.

## Deferred (`TODO(G23)`)

- The death sound uses the ported type-0 quest-sound builder; Java plays the
  type-1 music variant `PlaySound(1, "B03_D", …)` — cosmetic.
- Per-cube 15-minute despawn: Java's `addSpawn(…, 900000)` auto-despawns each
  cube; here they persist (harmless — the lair is empty and locked after
  `remove_players`). A despawn task per cube is left out.

## Tests — `game_loop/tests/valakas_tests.rs` (15 → 18)

- Killing Valakas (through `npc_do_die`) arms the death cinematic's first beat.
- Advancing past `die_8` spawns all fifteen exit cubes and arms
  `remove_players` — through the loop dispatch.
- `remove_players` ousts a lingering player to the exit — through the loop
  dispatch.
- All three wires (death hook, cinematic dispatch, remove-players dispatch)
  sabotage-verified.
