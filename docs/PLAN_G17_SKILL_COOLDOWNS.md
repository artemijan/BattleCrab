# G17 slice 7 — skill cooldowns per class index

Seventh and last G17 slice. Finishes the per-`class_index` sweep.

## Simpler than the others, because Java says so

I expected to bank cooldowns per slot the way slices 3–4 do for skills, hennas
and shortcuts. Reading `setActiveClass` first showed that's wrong: it calls
**`resetTimeStamps()`**, which is `_reuseTimeStampsSkills.clear()`.

So a class switch **wipes** cooldowns rather than preserving them per slot —
which also closes an obvious exploit: park a long reuse on one class, switch
away, switch back, and sit it out for free.

Reading the Java before designing the port saved building the wrong thing.

## What landed

- `set_active_class` clears the `Reuses` component (`resetTimeStamps`).
- `load_skill_reuses` reads the **active** class index rather than hard-coded
  `0`, and the save writes under `s.class_index`. Before this, a character on a
  subclass saved its cooldowns onto the base slot and loaded the base slot's on
  login.

`restore_type = 0` (actual buff restore across logout) has since landed — see
`PLAN_BUFF_PERSISTENCE.md`.

## Tests

2 added (28 in the subclass suite): a switch wipes cooldowns; and the rows are
saved under the active slot, not index 0.

**786 lib tests; all 9 targets green.**

## G17 status

**Complete**, with one item struck rather than implemented:

| Item | Status |
|---|---|
| Nobless + noble skill tree | ✅ slice 1 |
| Subclass add / switch / persist | ✅ slice 2 |
| Per-subclass skill books | ✅ slice 3 |
| Per-subclass hennas + shortcuts | ✅ slice 4 |
| Village-master subclass flow | ✅ slice 5 |
| Occupation change (`setClassId`) | ✅ slice 6 |
| Skill cooldowns per index | ✅ slice 7 |
| Certification skills | **struck** — no data ships on this dist |

G17's gate — *"a character changes class and gets the new skill tree; a
subclass can be added and switched"* — is met, and the class-change mechanic is
in place for G22's occupation quests to call.

Buff restore across logout (`restore_type = 0`) is the one piece of the
`character_skills_save` table still unported; it was never G17's.
