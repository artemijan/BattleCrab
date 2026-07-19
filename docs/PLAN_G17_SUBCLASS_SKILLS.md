# G17 slice 3 — per-subclass skill books

Third G17 slice. Closes the gap the subclass slice shipped with: **a manually
learned skill was lost on a class switch.**

## The gap

Slice 2 swapped class/level/exp/sp on a switch but rebuilt the skill book by
re-deriving the class's *auto-granted* tree. Anything learned by hand from a
trainer — which is most of what a played character knows — simply vanished on
the round trip. `character_skills` has a `class_index` column; the port pinned
it to `0` everywhere.

## What landed

- **Load** — `load_skills` now reads every index into a
  `HashMap<class_index, Vec<(skill_id, level)>>` instead of filtering to 0.
- **Active index on login** — Java keeps the *active* class in
  `characters.classid`, so the index is whichever subclass row carries it (0
  when it's the base class). A character who logs out on a subclass now logs
  back in on it, with that slot's book.
- **Switch** — `set_active_class` banks the outgoing book into
  `Player.skills_by_index` and restores the incoming one, mirroring Java's
  `removeSkill`-everything → `restoreSkills()` (DB rows for the new index) →
  `rewardSkills()` (the auto-granted tree on top, which `set_level` still
  supplies).
- **Save** — `PlayerSaveData` carries `skills_by_index` + `class_index`, and
  the flush rewrites every index rather than deleting only `class_index = 0`.

## Tests

3 added to `game_loop/tests/subclass_tests.rs` (15 total there): a hand-learned
skill survives a switch away and back; two slots keep separate books; and the
save carries the active index plus every banked slot.

The first of those is exactly the behaviour that was broken — it fails against
slice 2's code.

**753 lib tests, all 8 targets green.**

## Process note

Three of my regex-driven field additions landed inside `Clan` literals (which
also have a `skills` field), producing 27 bad lines across four test files. I
drove the cleanup off the compiler's own `E0560` locations rather than another
regex. That's the third time this session a broad `re.sub` over struct literals
has hit an unintended type — worth pattern-matching on: **when adding a field,
match on a neighbouring field unique to the target struct, or let the compiler
enumerate the sites.**

## Deliberate narrowings (`TODO(G17)` at the site)

- Hennas and shortcuts still load and save at `class_index = 0`.
- Certification skills; the village-master flow (G22).
- `dual_class` is written as 0.

## Next in G17

- Per-subclass hennas and shortcuts (the same `class_index` treatment).
- Occupation change through the village-master flow.
- Certification skills.
