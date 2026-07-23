# G23 slice 20 — Antharas's entry flow wired (Heart of Warding → WAITING → spawn)

## Why

Slice 19's closing note: **`try_enter` is complete, tested, and uncalled** —
the same defect it had just fixed for `baium::manage_skills`. Nothing in the
crate talks to the Heart of Warding, so the whole entry ladder, the 20-minute
window and the (also wired-nowhere-sensible) cinematic are unreachable by a
player. This slice is the wiring: the two NPCs, the WAITING window, the
`SPAWN_ANTHARAS` timer, and a status-model fix the wiring exposed.

## The status collision (fix first)

`grand_boss::on_grand_boss_killed` writes `status = DEAD = 1` for **every**
boss — but the four-state bosses (Antharas/Valakas: DORMANT 0, WAITING 1,
IN_FIGHT 2, DEAD 3) read 1 as *WAITING*: `try_enter` would admit raids into a
dead Antharas's lair, and a dead-with-elapsed-window boot check against
`s == 1` mismatches a stored 3. Latent until now because nothing ever set
WAITING. Fix: `dead_status(boss_id)` (3 for 29068/29028, 1 otherwise) used at
the kill and both boot branches.

Also moved: the entry **cinematic no longer plays at spawn** (that was slice
17's stand-in caller) — Java runs it from `SPAWN_ANTHARAS`, twenty minutes
after the first group enters. Valakas's cinematic stays unwired until its own
entry slice (`TODO(G23)` at the site).

## The flow (Java `Antharas.java` `onEvent("enter")`)

- `scripts/antharas_heart.rs`, a `QuestScript` named **Antharas** — the dist
  htmls already point at `Quest Antharas enter` / `Quest Antharas
  teleportOut`, so the name is load-bearing. First-talk on the Heart (13001)
  serves `13001.html`; the Teleportation Cubic (31859) talks through
  `html/default/31859.htm` (already served by the default path).
- `enter`: the ladder's verdicts map to the five refusal htmls
  (dead → 01, fighting → 02, no stone → 03, full/overfill → 04, not the
  leader → 05); Admitted teleports each gathered member to
  `(179700+rnd(700), 113800+rnd(2100), -7709)` and, **only if the status
  isn't already WAITING**, flips it and arms `SPAWN_ANTHARAS` at
  `AntharasWaitTime` minutes (`GrandBoss.ini`, 20 on this dist) — a second
  party entering during the window must not restart the clock.
- `SPAWN_ANTHARAS`: relocate the boss to `(181323, 114850, -7623, h32542)`,
  status → IN_FIGHT, `PlaySound("BS02_A")` to the lair, then the slice-17
  camera chain (whose tail already starts the minion waves).
- `teleportOut`: `(79800+rnd(600), 151200+rnd(1100), -3534)`.

## New machinery

`relocate_npc` — Orfen's precedent mutates `Position` in place, safe only
within a region; Antharas's teleport crosses regions, so the helper also
moves the `npc_regions` index entry, announces `DeleteObject` near the old
region and `NpcInfo` (via `introduce_npc`) near the new one.

## Tests

Entry ladder e2e through the real bypass (`Quest Antharas enter`): admitted
solo player teleports + WAITING + timer armed exactly once (second entrant
keeps the clock); refusal htmls for dead boss and missing stone; the spawn
timer relocates the boss cross-region (region index checked), flips
IN_FIGHT, plays the sound to a lair player and arms camera step 1; the
teleport-out cubic; the dead-status fix pinned from both sides (a killed
Antharas reads 3 → entry refused as BossDead; boot's elapsed-window branch
still respawns it).
