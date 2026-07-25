# PLAN — Frintezza (Last Imperial Tomb) instanced encounter

Port of `dist/game/data/scripts/ai/bosses/Frintezza/LastImperialTomb.java` (+
`ScarletVanHalisha.java`), the one Interlude instanced boss. Template 136 already
loads (G27 slice 3); the G27 instance engine (partition, visibility, lifecycle,
scoped NPC broadcasts) is the foundation. This is a mini-milestone under G27
content — sliced because the Java script is ~1060 lines spanning a dungeon crawl,
a 20-step cinematic, a two-morph boss fight with adds, and a finish sequence.

Home: a native `game_loop/frintezza.rs` module (state machine, like
`antharas.rs`) driven by a thin `scripts/last_imperial_tomb.rs` QuestScript
(talk/kill/timer/attack/spawn hooks → the module), mirroring Java's
`LastImperialTomb extends AbstractInstance`.

## Enabling primitives (built as slices need them)

- **Instance `status` + scratch vars** (Java `Instance.getStatus/setStatus`,
  `setParameter`): `Instance.status: i32` + `vars: HashMap<String,i64>` with
  `InstanceManager` accessors. Object-ref parameters (frintezza/scarlet/demons)
  stored as object ids in `vars` / a small ref map. — **slice 1** (status + int
  vars); ref storage in the boss slice.
- **`instances::spawn_group(world, id, name)`** (Java `Instance.spawnGroup`):
  spawn a named non-default group into a live instance. — **slice 1**
- **Per-instance doors** (Java `Instance.openCloseDoor`): instanced door objects
  + per-instance open state + instance-scoped DoorInfo. — **slice 2**
- **`SpecialCamera` cinematic driver** (packet exists): a `ScheduledTask`-chained
  step machine (like `AntharasCinematic`) + `disablePlayers`/`enablePlayers`. —
  **slice 3**
- **ScarletVanHalisha AI** + Frintezza song casting, demon/portrait spawns,
  Dewdrop-of-Destruction suicide. — **slice 4**

## Slices

1. **Entry + room-crawl progression** (this slice). GUIDE (32011) talk with the
   Magic Force Field Removal Scroll (8073) → build instance 136 + enter (spawns
   the default HALL_ALARM group). Kill machine (Java `onKill` status 0→4): kill
   HALL_ALARM → status 1, `spawnGroup("room1")`, set `monstersCount`; clear a
   room → advance (room2_part1 → room2_part2 → status 4). CUBE (29061) talk →
   teleport out. Doors + the aggro-nudge + Dewdrop drop are TODO'd for later
   slices. Testable end-to-end with a minimal seeded template 136.
2. **Per-instance doors**: open/close the four door groups as the crawl advances
   and on finish; instanced door objects.
3. **Frintezza intro cinematic**: `FRINTEZZA_INTRO_START` (10 min after status 4)
   → the 20-step SpecialCamera chain, dummy/Frintezza/Scarlet/portrait spawns,
   player disable/enable, ending with the fight enabled.
4. **Boss fight**: Scarlet (29046) morphs at 80%/20%, second morph → doDie →
   respawn as Scarlet2 (29047); Frintezza random songs (5007/5008); demon spawns
   (29050/29051, every 20 s to 24) from portraits (29048/29049); Dewdrop skill
   (2276) suicides portraits; ScarletVanHalisha AI.
5. **Finish + rewards**: Scarlet2 death → FINISH_CAMERA chain → Frintezza dies,
   doors open, teleport CUBE spawns; loot.

## Interlude scope note

Verify each NPC id / item / skill exists in the Interlude datapack before
porting. Chamber-of-Delusion and later reuse-time mechanics are out of scope.
