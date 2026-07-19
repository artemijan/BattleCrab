# G21 slice 3 — raid-boss persistence (`DBSpawnManager`)

Third G21 slice. Covers the **"a boss keeps its HP across restart"** clause —
**G21's gate is now met** (spoil/sweep landed in G15, casting in slice 1, guard
aggro in slice 2).

## Data survey

| Fact | Number |
|---|---|
| `dbSave="true"` spawn lines | **225**, all in `RaidbossSpawns.xml` |
| Table | `npc_respawns` (id, x, y, z, heading, respawnTime, currentHp, currentMp) |

Before this, `dbSave` was parsed by nobody and the module docs said plainly
*"dbSave spawns are placed like static ones for now"*. Every restart handed
players a fresh full-HP raid boss and silently wiped any pending respawn timer
— a boss killed one minute before a restart was back immediately, at full HP.

## The ownership split (the part that's easy to get wrong)

Java does **not** spawn a `dbSave` NPC from the static spawn pass.
`NpcSpawnTemplate.spawnNpc` hands it to `DBSpawnManager.addNewSpawn(spawn,
true)` — and only when `!DBSpawnManager.isDefined(id)`. The DB owns these NPCs;
the XML pass defers.

Reproducing that in the port needed care because `spawn_all` runs
**synchronously at boot, before any DB event is drained**. Rather than block
boot on the DB thread, `spawn_all` now collects `db_save` definitions into
`World.pending_boss_spawns` without placing them, and `resolve_boot` settles
them when `DbEvent::NpcRespawnsLoaded` arrives. Boot stays asynchronous and the
"DB wins" rule is preserved. A test pins the invariant directly: the static pass
must place *no* dbSave boss, or the DB restore would double-spawn it.

## Behaviour

`resolve_boot` handles three cases per boss:

| Stored row | Result |
|---|---|
| `respawnTime` in the future | **Not spawned**; a `BossRespawn` task is scheduled for when it's due |
| `respawnTime` elapsed, or alive (`0`) | Spawned, restoring stored HP/MP |
| No row (fresh DB) | Spawned full, and the row is inserted |

Writes: `persist_alive` on spawn (`respawnTime = 0` + current vitals),
`persist_death_at` at corpse decay (absolute due time banked, so a restart
inside the 24 h window resumes the wait), and `save_all_bosses` on shutdown
(`DBSpawnManager.updateDb`) so a restart mid-fight resumes at the HP the boss
was left on.

Two guards worth their tests:
- **A dead boss's row holds `currentHp = 0`.** Restoring that literally would
  spawn a corpse that dies on the next tick, so only a positive stored value is
  applied.
- **A stored value above the template maximum clamps** rather than over-filling
  the bar (a datapack change since the row was written).

Position is read **before** `despawn_npc`, which drops the entity and its
components — the row needs the boss's spawn location.

## Verification against the real database

The dev SQLite DB already carries `npc_respawns`. I checked the actual column
names and types with `PRAGMA table_info` and ran an insert/select/delete
round-trip through the real table, so the SQL is confirmed against the shipped
schema rather than only against test doubles. The dist parse test asserts
**exactly 225** `db_save` spawns, matching an independent grep of the XML.

## Two boot-event skip-lists needed updating

Adding an unprompted boot event broke `character_create_inserts_into_real_schema`
(lib) and all 7 `char_persistence` integration tests — both drain boot events
through an explicit "skip the unprompted loads" match arm, and the new variant
fell through to `other => return other`. Not a bug in either place; the
skip-lists are the intended mechanism and simply needed the new variant. Worth
knowing that **any future unprompted `DbEvent` has two skip-lists to update.**

## Deliberate narrowings (`TODO(G21)` at the site)

- `respawnPattern` (cron-style `SchedulingPattern` respawns) — 0 occurrences in
  this dist, so the branch isn't ported.
- Java's `DBSpawnManager` also exposes `RaidBossStatus` (ALIVE/DEAD/UNDEFINED)
  for the `//raidinfo` GM view; not ported, no consumer yet.
- Minions of a DB-spawned boss (`spawnMinions` in the same branch) still don't
  spawn — minions remain unported generally.

## Tests

11 new in `game_loop/tests/boss_respawn_tests.rs`: the deferral invariant,
fresh-DB spawn + insert, HP/MP restored across a restart, still-on-timer stays
dead, elapsed timer spawns, the two vitals guards, death banks ~24 h, an
ordinary monster writes *nothing* (tens of thousands of them — the table must
not become a firehose), the shutdown flush, and the dist parse count.

**658 lib tests green**, `char_persistence` 7/7, `e2e_create` 1/1. The one build
warning (`is_in_duel` unused) predates this slice.

## G21 status

**Gate met.** Remaining G21 breadth, none of it gate-blocking:
- **Minions** — templates parse them; nothing spawns them.
- **NPC pathfinding** (the G7.85 worker for NPCs) and NPC regen.
- Wire `skillTargetReconsider` (faction data now exists).
- The other ~33 zone types, fences (`FenceData`), `HtmCache`, walker routes,
  `CreatureSeeTaskManager`.
