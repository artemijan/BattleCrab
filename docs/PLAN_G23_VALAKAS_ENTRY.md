# G23 slice 21 — Valakas's entry flow wired (Klein → Heart of Volcano → cinematic)

## Why

Slice 15 landed Valakas's 10-beat entry cinematic (`begin_cinematic`), but —
like Antharas's ladder before slice 20, and Baium/Antharas's `manage_skills`
before that — it is **complete, tested, and uncalled**. Nothing triggers the
`"beginning"` event, so no player can ever reach Valakas. This slice is the
`ValakasTeleporters` script (`ai/others/ValakasTeleporters`) that drives it.

## Java flow (`ValakasTeleporters.java`)

Six NPCs, all reached through the bare `Quest ValakasTeleporters` bypass in
their `html/default/<id>.htm` (→ `on_talk`), plus one sub-event:

- **Watcher Klein (31540)** — the gate to the antechamber. `on_talk` shows a
  crowding html by lifetime entry count (`31540-01`<50, `-02`<100, `-03`<150,
  `-04`<200, `-05` full). Its `-01` button carries `Quest ValakasTeleporters
  31540` → `on_event("31540")`: requires the **Vacualite Floating Stone
  (7267)**, teleports to the Hall of Flames `(183813, -115157, -3303)`, and
  sets the player's `allowEnter` flag; without the stone, `31540-06.htm`.
- **Heart of Volcano (31385)** — the lair door. `on_talk`: only while Valakas
  is DORMANT/WAITING (`31385-01` when dead/regen, `-02` when fighting), under
  the 200 cap (`-03`), and only with `allowEnter` set by Klein (`-04`
  otherwise). On success: consume `allowEnter`, teleport into the lair
  `(204328+rnd600, -111874+rnd600, 70)`, bump the count, and **on the first
  entry (DORMANT)** arm the `"beginning"` timer at `ValakasWaitTime` (30 min)
  and flip WAITING.
- **Teleport Cubic (31759)** — `on_talk` teleports out to `(150037+rnd500,
  -57720+rnd500, -2976)`.
- **Gatekeepers (31384 / 31686 / 31687)** — `on_talk` opens door 24210004 /
  24210005 / 24210006 (the path Klein → Heart).

## The count quirk — ported faithfully

Java's `playerCount` is a `static int` on the script that **only ever
increments** — never reset on spawn, death, or window close. After 200
lifetime entries the lair locks forever until a server restart. Clearly not
intended, but it is what the server does (the Core-minions precedent: *port
what the script does*). Stored as `World.valakas_entry_count`, documented, and
a test pins that a second cycle's entrants keep counting up rather than
resetting.

## The `"beginning"` wiring

New `ScheduledTask::ValakasBeginning` → `valakas::begin_cinematic(find_valakas)`
— the same shape as slice 20's `AntharasSpawn`. A GM-killed boss mid-window is
guarded (status must still be WAITING). `find_valakas` mirrors `find_antharas`.

## New machinery

- `World.valakas_entry_count: u32` (default 0).
- `GrandBossConfig.valakas_wait_minutes` (30).
- `doors::open_door_by_id` — scan `door_regions` for the Door with a given
  `door_id`, then `open_door` its object id (door oids are dynamically
  allocated, so there is no arithmetic mapping).

## Tests (`valakas_tests`, currently 10)

1. Klein: no stone → `31540-06`; with stone → teleported to the Hall of Flames
   + `allowEnter` set. The crowding htmls by count.
2. Heart of Volcano: refusals (fighting → `31385-02`, no `allowEnter` →
   `31385-04`, cap → `31385-03`); success teleports into the lair, consumes
   `allowEnter`, and the FIRST entry arms `"beginning"` + WAITING while a
   second entrant does not re-arm.
3. `"beginning"` fires after 30 min → `begin_cinematic` runs (boss on the lair
   coords, cinematic beats pending), and the final beat still flips FIGHTING.
4. The count never resets across a kill+respawn cycle.
5. The router e2e (the slice-20 lesson): `Quest ValakasTeleporters` on a real
   Heart-of-Volcano click reaches the entry through the bypass router, not a
   direct call.
6. Cubic teleports out; a gatekeeper opens its door.
