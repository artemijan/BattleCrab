# G17 slice 2 — subclasses

Second G17 slice, and the milestone's **gate headline**: *"a subclass can be
added and switched."*

## What was missing

Nothing at all. `class_index = 0` was hard-coded in six places in `db.rs`, each
with a comment saying "no subclasses on this dist". `character_subclasses`
existed in the shipped schema and was never read or written.

Config on this dist: `MaxSubclass = 5`, `BaseSubclassLevel = 40`.

## What landed

- `Player.class_index` + `Player.subclasses: Vec<SubClass>`, loaded at login
  from `character_subclasses`.
- `game_loop/subclass.rs`: `add_subclass` (`Player.addSubClass`) and
  `set_active_class` (`Player.setActiveClass`).
- `DbCommand::StoreSubClass` — upsert one slot, keyed `(charId, class_id)` like
  Java's primary key.
- `//setsubclass <classId>` (no arg lists the slots) and
  `//changesubclass <index>`.

**Slot allocation** — Java takes an explicit `classIndex` from the
village-master flow and refuses index 0. Picking the lowest free slot here is
the same outcome for every caller that exists and keeps ids dense; noted at the
site.

## The banking problem, which is the whole mechanic

Everything the character *is* — class, level, exp, sp — belongs to the active
slot. Switching therefore has to **write the current slot's progress back
before loading the target's**, which is why Java calls `store()` *before*
touching `_classIndex` ("to avoid skill effects rollover").

The base class needs the same treatment and has nowhere obvious to go: its row
in `characters` holds whatever class is *active*, so a level-7 base character
who switches to a level-40 subclass would come back as level 40. `Player` now
stashes `base_level`/`base_exp`/`base_sp` for exactly that round trip, and a
test pins it.

## Deliberate narrowings (`TODO(G17)` at the site)

- **Per-subclass skills aren't persisted.** A switch re-derives the class's
  auto-granted tree via the same `set_level` path `//setclass` uses, so a
  *manually learned* skill is lost on the round trip. `character_skills` needs
  a real `class_index` key — that's the next slice.
- Hennas and shortcuts still load with `class_index = 0`.
- Certification skills, the village-master UI flow (G22's occupation quests),
  and Java's `_subclassLock` held across the swap.
- `dual_class` is written as 0 — a later-chronicle feature.

## Tests

12 in `game_loop/tests/subclass_tests.rs`: fresh character state; add takes
slot 1 at level 40 and does *not* switch; the base class and a duplicate are
both refused; `MaxSubclass` caps; unknown class refused; switching swaps
class/level; **switching back restores the base level**; a subclass's own
progress is banked across a switch away and back; a missing slot and a no-op
switch both fail cleanly; and the new slot reaches the DB.

**767 tests green across all 8 targets.**

## Next in G17

- `character_skills` keyed by `class_index`, so learned skills are per-subclass.
- Occupation change through the village-master flow.
- Certification skills.
