# G21 slice 4 — minions (`MinionList`)

Fourth G21 slice, and the first past the gate. Ports `util/MinionList`: leaders
now spawn the escort they declare, and the pack fights, dies and rebuilds
together.

## Data survey

| Fact | Number |
|---|---|
| NPCs declaring an escort | **460** |
| `<minions><npc>` entries | **962** |
| Minions actually placed in a full world spawn | **3289** |
| `RaidMinionRespawnTime` | 300000 ms |
| `CustomMinionsRespawnTime` overrides | 23 npc ids |
| `ForceDeleteMinions` | False |

The parser previously *skipped* minion refs deliberately (they'd otherwise be
mistaken for template starts), so every leader — raid bosses included — stood
alone.

**A survey correction worth recording:** a first `grep -A4 '<minions>'` showed
blocks that looked empty and nearly had me write minions off as dead data. They
aren't: all 467 blocks carry children. Separately, the XML has 467 `<minions>`
*blocks* across only **460** NPCs, because a handful declare many groups each
(25100 has 28, 25200 has 29, 22602 has 15). The dist test asserts the **962
entry count** rather than the block count, since that's the number that proves
nothing was dropped.

## What landed

- `NpcTemplate.minions: Vec<MinionHolder>` parsed from
  `<parameters><minions><npc id count/>`, with its own `in_minions` scope flag
  (`<parameters>` carries other `<npc>`-shaped rows).
- `NpcConfig`: `RaidMinionRespawnTime`, `CustomMinionsRespawnTime` (an
  `id,seconds;…` map), `ForceDeleteMinions`.
- `game_loop/minions.rs` — spawn/top-up, respawn, master death, pack aggro.
- `MinionOf` on each minion, `Minions` roster on the leader (Java's
  `_spawnedMinions`).

**Rules that are easy to get backwards, each with a test:**
- A **non-raid** leader's minions **never respawn** (`respawnTime < 0 ? isRaid ?
  cfg : 0`), and a **`CustomMinionsRespawnTime` entry of 0 beats the raid
  default** — 4 npc ids on this dist use exactly that to mean "gone for good".
- Only a **raid** leader's death clears its escort (or any leader under
  `ForceDeleteMinions`, off here). An ordinary leader's minions outlive it —
  that's why killing the big mob in a camp doesn't evaporate the camp.
- Pack aggro is asymmetric: hitting the **leader** aggros the escort at 10,
  hitting a **minion** at 1, ×10 again for a raid.
- Top-up spawns `count - alive`, so the respawn path can't stack extras.

## A real performance bug, caught by the e2e test

My first version counted a leader's live minions with a **full world scan**, run
once per minion spawned. At boot that's ~3289 spawns × ~39k NPCs. The lib tests
passed; `e2e_create` failed instantly with a **login failure**, because the game
server was still spawning and never registered with the login server in time.

Replaced with the per-master `Minions` roster (which is what Java's
`_spawnedMinions` is for). Boot is back to normal and the e2e passes.

Worth remembering: **a hot path over `world.objects` can be invisible to unit
tests and only show up as a boot-time failure somewhere unrelated.**

## Two test-only hazards found

1. **Object-id collision.** `add_test_npc` hand-places at `NPC_OID`, which *is*
   `FIRST_NPC_OBJECT_ID` — the next id the runtime allocator hands out. Minions
   spawn through that allocator, so the first minion **overwrote the leader's
   own entity**, and the raid tests failed with the leader reporting the
   *minion's* template. Fixtures that mix hand-placed and runtime-spawned NPCs
   must advance `world.next_npc_object_id`.
2. **Ambient `SocialAction` (0x27).** `e2e_create` asserts a positional packet
   stream and skips a fixed set of ambient opcodes. NPC *idle animations* fire
   on a random timer for any NPC near the player and weren't in that set. Adding
   escorts near the spawn point made this land almost every run — but the hazard
   predates this slice, and it is **the most likely cause of the intermittent
   `e2e_create` failures noted in earlier slices**. Added `0x27` to the skip
   list; the e2e then passed 4/4 consecutive runs.

## Deliberate narrowings (`TODO(G21)` at the site)

- `onMasterTeleported` (minions follow a teleporting leader) — no NPC teleport
  plumbing yet.
- The `respawnTime`/`weightPoint` attributes on each `<npc>` row are parsed past
  but unused: the standard death path takes its delay from
  `CustomMinionsRespawnTime`/`RaidMinionRespawnTime`, exactly as Java does.
  `MinionHolder.getRespawnTime()` only feeds script-driven spawns.
- Minions of a minion: `spawn_npc_at` deliberately doesn't recurse.

## Tests

14 new in `game_loop/tests/minion_tests.rs` (escort spawns, placement ring,
top-up doesn't overshoot, dead leader spawns nothing; the four respawn rules;
the three master-death rules; the three aggro rules), plus a dist-backed parse
test asserting 460 leaders / 962 entries.

**673 lib tests green**, `char_persistence` 7/7, `e2e_create` 1/1 (×4). The two
build warnings predate this slice.

## Next in G21

- **Zones** — the biggest remaining block. Only 5 of ~30 kinds are ported;
  unported by weight: `ConditionZone` **1080**, `EffectZone` 218, `ScriptZone`
  133, `TaxZone` 122, `LandingZone` 69, `HqZone` 59, `ClanHallZone` 48,
  `DamageZone` 35, `SwampZone` 20. (2779 `<zone>` elements total.)
- NPC pathfinding (the G7.85 worker for NPCs) and NPC regen.
- Wire `skillTargetReconsider` — faction data now exists.
- Fences (`FenceData`), `HtmCache`, walker routes, `CreatureSeeTaskManager`.
