# G29 slice 14 — pet death

Closes the `TODO(G29)` slice 7 left at the restore site ("a pet can never be
stored dead, so this branch is untestable"). It can now.

## The penalty

Java `Pet.deathPenalty`, its own "TODO: Need Correct Penalty" included:

```java
percentLost = (-0.07 * level) + 6.5;
lostExp = (getExpForLevel(level+1) - getExpForLevel(level)) * percentLost / 100;
_expBeforeDeath = getExp();
addExp(-lostExp);
```

The loss is a share of the **current level's band**, so it shrinks as the pet
levels (6.43% of a band at level 1, 5.8% at level 10). Skipped entirely for a
duel or arena death.

`_expBeforeDeath` is captured *before* the penalty so a resurrection can hand
back a share of the gap. It is deliberately **not persisted** — Java holds it on
the live instance, so a pet that dies and logs out forfeits the restorable exp.

`restoreExp(percent)` gives back `(expBeforeDeath − exp) × percent / 100` and
then **zeroes the record**, so a second revive restores nothing.

## Two guards worth naming

- **The penalty cannot de-level a pet.** Exp is floored at the current level's
  threshold; Java's `addExp(-lost)` does not de-level either.
- **At the species' top level there is no next-level band**, so the penalty
  computes to zero. Java would throw here (`getExpForLevel(level+1)` has no row
  and it logs an NPE); a max-level pet losing nothing is the safer reading.

## A fixture that hid the thing under test

The first draft of the death tests reported "exp was lost on death (6000 →
6000)". The shared fixture defined only levels 1 and 2, so a level-2 pet was
already at the species cap — every death test was silently exercising the
empty-band case and measuring nothing.

Fixed by giving the fixture a third level, and the max-level case is now pinned
as its own test rather than being the accidental default. **Same shape as the
vacuous-aggro and clamp-masked bugs from G19: a fixture can quietly make the
edge case the only case.**

## A bug found incidentally

`YOUR_SERVITOR_PASSED_AWAY` was written as **1519** in G29 slice 1. The correct
id is **1520**; 1519 is *"The pet has been killed…"*. So a servitor whose
lifetime expired told its owner its pet had died. Found only because this slice
needed 1519 for its real purpose and the two collided.

Worth generalising: **an off-by-one in a message id is invisible until another
slice needs the neighbouring id.** Both constants now carry the other's number
in a comment.

## Tests

`servitor_tests` 66 → 73: exp lost on death, the pre-death total recorded, no
de-level, duel deaths free, full and partial resurrection restore, the record
spent after one revive, the max-level no-op, and the reopened slice-7 branch —
a pet stored with `curHp < 1` comes back a corpse.

The duel test marks the owner with `DuelRef` directly, since `is_in_duel` is
exactly that component's presence. That also puts `is_in_duel` to use for the
first time, clearing a long-standing dead-code warning.

## Still open for pets

- The corpse itself: `DecayTaskManager` (the 24-hour body), and resurrecting a
  pet from its corpse — `pet_restore_exp` is wired and tested but nothing calls
  it yet, because the pet-resurrection *skill* path is not ported.
- Pet regen (`org_hp_regen`/`org_mp_regen` parsed, `NpcTemplate` has no regen
  fields).
- `PET_EQUIP` paperdoll, soulshot/spiritshot counts, evolution, reconnect
  resummon.
