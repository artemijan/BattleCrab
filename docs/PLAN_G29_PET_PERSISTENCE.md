# G29 slice 7 — pet persistence

Closes the second half of the gate's *"summon a pet, feed it, and it persists"*
(feeding is slice 8). A pet's level, exp, sp, food bar and vitals now survive a
logout, keyed by the object id of the collar that summons it.

## The `pets` table was already there

`dist/db_installer/sql/{sqlite,mariadb,postgresql}/game/pets.sql` ships the
table, so this is query work, not migration work. It is **absent from the
consolidated dump** (`dumps/l2jmobiusclassicinterlude_dump_*.sql`), which is why
a first grep of the dump makes it look missing — check the per-table `sql/`
tree, which is what the installer actually runs.

```
item_obj_id  PK   the collar's object id
name level curHp curMp exp sp fed ownerId restore
```

## Load at login, not at summon

Java re-reads the row inside `Pet.restore` on **every** summon. This port loads
the character's whole pet set with the character (`load_pets`, one extra query
per login) into a `PlayerPets` component, so summoning is a map lookup and no DB
round-trip sits in the cast path. Same memory-first shape as
`PlayerVariables`/`character_variables`.

Writing back is the mirror: `servitor::sync_pet_row` folds the live pet's
`PetOf` + `Vitals` into `PlayerPets`, and the map rides out with the character's
normal flush. It runs before every `store_player_now` and on
`on_owner_leave_world` — **before** the unsummon, since after it there is
nothing left to read the state from.

## Upsert, never a delete sweep

Every other child table in `store_player` is reconciled delete-then-reinsert.
`pets` must not be: a row is keyed by a **collar**, which the character can trade
away, not by the character. Deleting "all rows for this owner" would destroy
pets that now belong to someone else. `INSERT OR REPLACE` on the `item_obj_id`
primary key collapses Java's `_respawned ? UPDATE : INSERT` into one statement.

Rows are deleted in exactly one place, mirroring Java `RequestDestroyItem`:
destroying a collar unsummons the pet bound to it and drops its row. **Object
ids are recycled**, so an orphan row would eventually hand a stale pet to an
unrelated item — this is a correctness fix, not tidiness.

## Java details worth keeping

- **New-pet level**: `template.getDisplayId() == 12564 ? owner.getLevel() :
  template.getLevel()` — the Sin Eater is the one species summoned at its
  *owner's* level. Then `Math.max(level, getPetMinLevel(id))`.
- **The exp floor** ("DS: update experience based by level. Avoiding pet delevels
  due to exp per level values changed"): a stored exp below what the pet's level
  now costs is raised to that level's floor, so retuning the datapack curve
  can't demote a pet the player already levelled. Ported as
  `PetTemplate::exp_for_level`.
- **The food bar does not refill on summon** — it carries over, which is what
  makes feeding matter at all.
- `restore` ("True restores pet on login") is always written `false`:
  auto-resummon on reconnect needs `CharSummonTable`, which this port lacks.
  `TODO(G29)` at the site.

## Two bugs this slice surfaced

1. **`PlayerPets` was declared on `PlayerData` but never added to the component
   insert tuple in `model/mod.rs`.** Everything compiled; `sync_pet_row` and the
   restore lookup would both have silently no-opped in production. Six tests
   failed on the missing component, which is the only reason it was caught —
   *adding a field to `PlayerData` is not enough, it must join the bundle.*
2. **`crates/gameserver/tests/user_info_packet.rs` had been broken on `main`**
   since the `Player` struct gained `lost_exp_on_death` / `revive_request`
   (G19 resurrection) and `pending_pet_collar` (G29 slice 6). Filtered `--lib`
   runs never compile the `tests/` directory, so it went unnoticed across
   several slices. Fixed here. **Run `--test` targets too, not just `--lib`
   filters.**

## Tests

`servitor_tests` 34 → 41; `char_persistence` 8 → 9.

In-memory: fresh-vs-restored, exp floor, fed clamp, sync write-back, a full
unsummon→resummon round trip, and collar destruction dropping the row.
The fixture gains a level-2 row (`add_wolf_level_2`) — with a single level every
"restored at level N" assertion would pass vacuously.

`pets_persist` is the only test that touches the real schema, so column names
and bind order are covered there; it also re-saves to prove the upsert updates
in place rather than growing the table.

## Still open for pets

- **Feeding** — the `PetFood` item handler and the consumption tick.
  `PetOf.fed` is now persisted and displayed but nothing drains or refills it.
- Pet death (Java restores a `curHp < 1` row as a dead pet — `TODO(G29)` at the
  site; untestable until pet death exists).
- Pet inventory (`PetItemList`), exp gain/level-up, evolution, collar enchant
  mirroring pet level, auto-resummon on reconnect.
