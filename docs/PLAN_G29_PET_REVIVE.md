# G29 slice 15 — pet resurrection

Slice 14 left `pet_restore_exp` wired, tested and **called by nothing**: pets
could die and lose exp, but there was no way to bring one back. This closes that
loop.

## The dialog goes to the owner

Java `Resurrection`:

```java
effected.getActingPlayer().reviveRequest(effector.getActingPlayer(),
                                         effected.isPet(), _power, …);
```

`getActingPlayer()` on a pet returns its **owner**, so casting a resurrection on
a dead pet puts the `ConfirmDlg` in front of the owner, who answers for it.
That is why Java keeps one `_reviveRequested` block on the player with a
`_revivePet` flag rather than a separate pet field — and why the port now does
the same.

The proposal was a five-element tuple; a sixth flag made it unreadable, so it
became a named `ReviveRequest` struct. The flag is load-bearing: one field
carries both cases, so a test asserts that reviving the pet does **not** revive
a dead owner.

## `PcBody` was rejecting pets

`targethandlers/PcBody.java` is `if (!selectedTarget.isPlayer() &&
!selectedTarget.isPet())` — the port had only the player half, so a dead pet
could not be targeted at all and the resurrection never reached the effect.
Fixed alongside.

## Restored exp

A player's restorable exp is `lost_exp_on_death`; a pet's is the gap the death
penalty opened (`exp_before_death − exp`), which is what `restoreExp` reads. The
two sources differ, so the request path branches on the flag when computing the
number shown in the dialog.

`restoreExp` runs **before** `doRevive` and consumes the record, so a second
revive restores nothing (pinned in slice 14).

Reviving also restarts the food clock — it stopped when the pet died — and
syncs the pet row, so the revived state is what persists if the owner logs out
immediately.

## Tests

`servitor_tests` 73 → 78: the proposal lands on the owner and is flagged as a
pet revival, accepting revives and restores exp, declining leaves it dead but
consumes the proposal, a living pet is not a target, and — the one that pins the
flag — a pet revival with a dead owner present revives the pet and **not** the
owner.

All 10 existing player-resurrection tests passed unchanged through the struct
conversion.

## Still open for pets

- The **corpse timer**: Java's `DecayTaskManager` gives a dead pet 24 hours
  before the body (and its items) disappear. The port's pet corpse currently
  persists indefinitely.
- Pet regen (`org_hp_regen`/`org_mp_regen` parsed; `NpcTemplate` has no regen
  fields).
- `PET_EQUIP` paperdoll, soulshot/spiritshot counts, evolution, reconnect
  resummon.
