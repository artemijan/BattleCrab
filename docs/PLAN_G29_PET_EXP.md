# G29 slice 12 — pet experience and levelling

Slice 7 made a pet's `level`/`exp`/`sp` round-trip through the `pets` table, but
nothing ever awarded them: every pet stayed at its summon level forever. This
closes that loop.

## The pet's exp comes *out of* the owner's

The important semantic, and the one that would be easy to get backwards.
Java `PlayerStat.addExpAndSp`:

```java
ratioTakenByPlayer = pet.getPetLevelData().getOwnerExpTaken() / 100f;
if (!pet.isDead()) pet.addExpAndSp(addToExp * (1 - ratioTakenByPlayer), …);
addToExp *= ratioTakenByPlayer;   // the owner's award is then reduced
```

So hunting with a pet **costs the player exp** — the pet is not minting extra.
`get_exp_type` (73 on most species) is the share the **owner keeps**; the pet
takes the remainder. The XML attribute name reads like a type enum and is
actually a percentage, which is exactly the kind of thing worth a comment at
the parse site.

Two details preserved deliberately:

- The split happens **after** the vitality/premium bonuses, so the pet shares
  them.
- A **dead** pet earns nothing, but Java reduces the owner's ratio *outside*
  that guard — so with a dead pet nearby the exp is simply lost. Faithful, and
  noted at the site so it doesn't look like a bug later. (The port reads it as
  "no eligible pet → ratio 1.0", which is the same outcome.)

## A starving pet stops growing

`PetStat.addExp` is guarded by `isUncontrollable()` — a pet whose food bar hits
zero earns no experience. That is a real link between the feeding loop (slice 8)
and progression, not an incidental check, and it has its own test.

## Levelling

- Advances through **every** level the new total has earned, not just one, so a
  large award can't leave the pet under-levelled.
- Capped at the highest level the species table defines — past that, every
  per-level lookup would fall off the end.
- `max_meal` is per level, so the food capacity moves with the level (and the
  current bar is clamped down to it).
- Java sends **no system message** for a pet level — just
  `SocialAction(LEVEL_UP)`. Easy to over-implement; checked rather than assumed.
- **`getControlItem().setEnchantLevel(getLevel())`** — the collar's enchant
  level *is* the pet's level. That was on the remaining-work list as its own
  item; it turned out to be three lines here, so it landed with the level-up it
  belongs to. It is how a collar advertises its pet without being summoned.

## Tests

`servitor_tests` 54 → 62.

The load-bearing one is `the_reward_path_actually_splits_with_the_pet`: the
split helper being correct proves nothing if `add_exp_and_sp` never calls it, so
that test runs the **real reward path** twice — pet in range and pet far away —
and asserts the owner's award differs (1000 vs 730) and the pet's matches (0 vs
270). The unit tests around it cover the ratio, range gate, no-pet case,
starvation, multi-level gain, the species cap, and the collar stamp.

## Still open for pets

- Pet death (Java restores a `curHp < 1` row as a dead pet; also `restoreExp`
  on resurrection, and the exp *lost* on death).
- `PET_EQUIP` paperdoll — battle-pet armour and weapons.
- Evolution, auto-resummon on reconnect (`CharSummonTable`).
- Pet stats still come from the NPC template rather than the per-level pet
  table, so a levelled pet does not yet get stronger in combat — the level
  number moves but `org_pattack`/`org_hp` are still unread. **That is the
  natural next slice**, and it is a "parsed but unconsumed" case the sweep
  would find.
