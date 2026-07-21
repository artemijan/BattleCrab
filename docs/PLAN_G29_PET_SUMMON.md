# G29 slice 6 — Pet summoning

## What landed

A pet collar now summons its pet, which follows its owner and is visible to
everyone. This is the piece the previous slice deliberately stopped short of.

## The collar→cast channel

Java's `SummonPet` effect **never receives the item**. `SummonItems` attaches a
`PetItemHolder` script to the player before casting and the effect pulls it back
out with `removeScript`. The item-use path and the effect are separated by the
whole cast pipeline, so the item identity has to be parked somewhere in between.

Ported as `Player.pending_pet_collar`, set in `use_item_skills` when the item
is a collar and **taken** (not copied) by the effect — so an unused one cannot
linger into an unrelated cast. Two tests pin that: the collar is consumed by the
summon, and reaching the effect without one summons nothing.

## A pet is a servitor plus a collar

The owner link, follow state and AI all come from **`ServitorOf`**, which a pet
also carries: "owned summon" is the same relationship whether it came from a
skill or a collar, so pets inherit follow, attack, stop/hold and the leash for
free. `PetOf` holds only what a servitor has no equivalent of — the collar's
object id and the food bar.

A pet sets `life_time`/`consume_item` to "none": it does not expire and pays no
upkeep, it is *fed* instead. The lifecycle tick therefore leaves it alone.

**The collar's object id is the pet's identity** (Java's `pets.item_obj_id`),
not the item type — that is how two Wolf Collars stay two different wolves. The
summon test asserts the binding is to the object id specifically.

## `summonType` is load-bearing

`PetInfo`'s second byte is 1 for a pet and 2 for a servitor, and the client uses
it to decide whether to offer the pet inventory and food bar. Both values are
pinned by one test that summons each and reads the byte, so the distinction
can't silently collapse.

The same field pair carries different things for the two: a pet's food bar, a
servitor's remaining lifetime. That is Java's own reuse, now commented at the
site.

## Tests

`servitor_tests` is now 34. The seven new ones cover the collar binding, follow
inheritance, the holder being consumed, summoning without a holder, the
"you already have a pet" refusal, a collar no longer in the inventory (which is
what stops a traded or dropped collar working), and the `summonType` byte.

## Still open for pets

- **Persistence** — the `pets` table (already in the dist schema) keyed by the
  collar's object id; the gate's "and it persists".
- **Feeding** — the `PetFood` item handler and the food-consumption tick;
  `PetOf.fed` is tracked and displayed but nothing yet drains or refills it.
- Pet inventory (`PetItemList`), exp/level, evolution, the collar's enchant
  level mirroring the pet's level.
