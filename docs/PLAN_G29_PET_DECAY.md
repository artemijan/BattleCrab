# G29 slice 16 — pet corpse decay

## The bug I nearly introduced

Slice 15 shipped pet resurrection, and I closed it noting the corpse "persists
indefinitely" and needed Java's 24-hour `DecayTaskManager` timer. Both halves of
that were wrong, and checking the datapack is what caught it:

- The port does **not** persist the corpse — `npc_do_die` already schedules
  decay from the NPC template's `corpseTime`.
- There is **no 24-hour timer**. `DecayTaskManager.add` has no pet branch, no
  pet NPC template overrides `corpseTime`, and `DefaultCorpseTime = 7`. **Java
  decays a pet corpse after 7 seconds**, exactly like the port already did.

The "24 hours" in `THE_PET_HAS_BEEN_KILLED…` is flavour text that does not match
the mechanic. Had I trusted the message, I would have replaced faithful
behaviour with a divergence — the `dist/` data is the specification, not the
client strings.

Seven seconds also makes the resurrection from slice 15 genuinely tense, which
is retail-correct.

## The real gap: decay destroys the pet

What the port was actually missing is what happens *at* decay. `Summon.onDecay`
is `unSummon(owner)` then `deleteMe(owner)`, and `Pet.deleteMe` is:

```java
_inventory.transferItemsToOwner();
super.deleteMe(owner);
destroyControlItem(owner, false); // "this should also delete the pet from the db"
```

So **letting a dead pet rot destroys it permanently** — the collar is consumed
and the saved row deleted. Before this slice a decayed pet corpse just despawned
like any NPC, leaving the collar and row intact: death cost the player an exp
penalty and nothing else, and they could re-summon the same pet immediately.

Order matters: the pet's inventory is handed back **before** the collar goes, so
its cargo is not destroyed with it.

## Tests

`servitor_tests` 78 → 82:

- Decay consumes the collar, drops the saved row, and leaves the owner petless.
- Decay hands the pet's inventory back first (food in the pet's bag ends up in
  the owner's).
- **Resurrecting before decay saves the pet** — the decay task still fires and
  must find a living pet and no-op. This is the interaction between slices 15
  and 16, and it is the one a regression would most likely break.
- A *servitor* corpse does not take the pet path (the branch is keyed on
  `PetOf`, and a servitor has no collar to destroy).

## Still open for pets

- Pet regen (`org_hp_regen`/`org_mp_regen` parsed; `NpcTemplate` has no regen
  fields at all).
- `PET_EQUIP` paperdoll, soulshot/spiritshot counts, evolution, reconnect
  resummon (`CharSummonTable`).
