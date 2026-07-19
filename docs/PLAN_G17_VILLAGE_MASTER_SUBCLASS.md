# G17 slice 5 — the village-master subclass flow

Fifth G17 slice. The subclass mechanic (slices 2–4) was reachable only through
GM commands; now a player adds and switches subclasses at a village master, the
way the game intends.

## Survey — and one thing found dead

| Item | Status |
|---|---|
| `VillageMaster` NPCs | **46** on this dist — live |
| Certification skill trees | **absent** — no `subClassSkillTree`/certification file ships here |

Certification skills are a later-chronicle feature. G17's roadmap line names
them, but there is nothing on this dist to port, so they're struck rather than
stubbed.

## What landed

`VillageMaster.onBypassFeedback`'s `Subclass <cmd> [arg]` verb, wired to the
mechanic from the earlier slices:

- **0** menu, **1** add list, **2** change list, **4** add action, **5** change
  action.
- **Level 75** (`SUBCLASS_MIN_LEVEL`) and the free-slot check are enforced on
  the *action*, not just the list — a stale link must not slip past. Java does
  the same, and there's a test for each.

`available_subclasses` ports `getAvailableSubClasses` under Interlude's rules:
every `THIRD_CLASS_GROUP` entry, minus the player's own base **lineage** (not
just the exact class — that's Java's "similar class" rule), minus anything
already held or a child of it, minus Overlord/Warsmith (`neverSubclassed`), and
minus the other half of the Elf ↔ Dark Elf pair. Kamael rules are omitted:
the race doesn't exist here.

**Class race needed a lineage walk.** `PlayerTemplate::race()` only answers for
*creatable* (1st-occupation) classes, so an advanced class returns `None`;
`class_race` resolves it by walking the lineage to its root. Getting this wrong
would have silently disabled the Elf/Dark-Elf rule rather than erroring.

## Tests

3 added (21 in the subclass suite): the level-75 gate on both sides; a full
slot list blocks adding even at 75; and — **against the real datapack** — every
offered class is a third-class-group entry, Overlord and Warsmith never appear,
and taking a class removes it from the next offering.

Using the shipped `GameData` there matters: the class hierarchy and category
groups are the real ones, not a fixture's guess at them.

**769 lib tests; all 9 targets green.**

## Deliberate narrowings (`TODO(G17)` at the site)

- Java's cases **3/6/7** (cancel or replace an existing subclass) need the
  slot-wipe Java performs and have no caller until the UI offers them.
- The HTML is built inline rather than from `data/html/villagemaster/*.htm`,
  because those files carry `%list%` placeholders the port's html cache doesn't
  template yet. Link targets and bypasses match Java's.
- Java's flood protector on subclass changes.
- Java also blocks the change while a skill is in use, transformed, with a
  summon out, or over the inventory/weight limit.

## Remaining in G17

- Occupation change proper (1st/2nd/3rd class advancement) — distinct from
  subclasses, and the part that ties into G22's occupation quests.
- `character_skills_save` (buff restore) is still index-0 only.
- Certification skills: **struck** — absent from this dist.
