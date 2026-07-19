# G17 slice 4 — per-subclass hennas and shortcuts

Fourth G17 slice, finishing the per-`class_index` work the subclass slices
started. Dyes and shortcut bars now belong to the class you set them on.

## What changed

The same treatment slice 3 gave skills:

- `load_hennas` / `load_shortcuts` read **every** class index into a map rather
  than filtering to `class_index = 0`.
- `set_active_class` banks the outgoing set into
  `Player.hennas_by_index` / `shortcuts_by_index` and takes the incoming one.
- The save flush rewrites every index instead of deleting only index 0.

**Henna dyes fold into `BaseStats`**, so the swap has to re-fold before stats
are recomputed. That is `apply_henna_change`, which also pushes `HennaInfo` —
exactly what Java's `setActiveClass` does (`restoreHenna(); sendPacket(new
HennaInfo(this))`), so reusing it is faithful rather than convenient.

## Tests

3 added (18 in the subclass suite): dyes are per-subclass and return on
switching back; shortcut bars likewise, and a subclass's bar does not leak into
the base one; the save carries both for every slot.

**762 lib tests; all 8 targets green.**

## Process note — the regex lesson, applied

Slice 3's cleanup cost 27 bad lines because I matched on `skills`, which `Clan`
also has. This time I anchored the mechanical field additions on
`skills_by_index`, which exists **only** on `PlayerSaveData` and `CharData`.
Four files patched, zero collateral, no compiler-driven cleanup needed.

## An intermittent `e2e_create` failure, reported as observed

During verification `e2e_create` failed once inside a *parallel* full-suite run
(fast-fail at ~7.9 s, the shape a boot/registration miss takes), then passed
standalone and in **three** subsequent full runs. `main` passed 2/2 under the
same command.

That is 4/5 on this branch versus 2/2 on main — too thin to call either way,
and I could not capture the assertion because it did not recur. The mechanism
doesn't obviously implicate this slice: the widened queries are *per-character
login* loads, not boot-time work. `e2e_create` has a documented history of
load-sensitive flakiness (see `PLAN_G21_MINIONS.md`, where a real boot slowdown
produced the same signature, and the ambient-`SocialAction` fix).

**Not claimed as clean.** If it recurs, the first thing to check is whether
boot-time work grew, since that is what the fast-fail signature means.

## Remaining in G17

- Occupation change through the village-master flow (the *mechanic* can precede
  G22's quests).
- Certification skills.
- `character_skills_save` (buff restore) is still index-0 only.
