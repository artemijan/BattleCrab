# G29 slice 25 — pet equipment

Closes the `TODO(G29)` slice 8 left in `PetInventory::to_rows`: *"Pet paperdoll
(`PET_EQUIP`) is not modelled yet, so everything serializes flat."*

## Reachable content

96 equippable pet-armour items ship on this dist (Wolf's Hide Armor 3891 and
friends: `type="Armor"`, `default_action="EQUIP"`, `bodypart="chest"`). Pet
**evolution**, by contrast, has no item handler at all here — struck rather than
scheduled.

## The paperdoll was already there

`PetInventory` wraps the ordinary `Inventory`, which owns a paperdoll and every
slot-displacement rule. So pet armour reuses the player's equip logic wholesale
instead of growing a second copy — which is what Java does too
(`PetInventory extends Inventory`).

Clicking a worn item takes it off, matching Java's `useEquippableItem` toggle.

## Two halves that had to be added

1. **Stats.** The NPC stat pipeline has no inventory step, so worn armour
   changed nothing. `recalculate_pet_stats` now sums the pet's *own* paperdoll
   through the existing `item_stats` bonuses. Only defensive stats are folded:
   the 96 items are armour and a pet has no weapon slot worth modelling.
2. **Persistence.** `to_rows` now emits `PET_EQUIP` for worn rows and `PET` for
   carried ones. `Inventory::to_rows` already writes the slot into `loc_data`
   for `PAPERDOLL` rows, so renaming the location preserves it — and
   `from_rows` renames back, letting the shared loader restore the pet's worn
   slots exactly as it restores a player's. A pet's armour comes back **on**,
   not loose in its bag.

## Tests

`servitor_tests` 111 → 114; inventory/items/char_persistence re-run clean
(17/37/9).

- Armour goes on the pet's own paperdoll and its defence counts.
- Clicking a worn piece takes it off, and the defence goes with it.
- The round trip: worn → `PET_EQUIP` with a non-zero slot, carried → `PET`, and
  `from_rows` restores it worn. This is the assertion that actually closes the
  slice-8 TODO.

## Still open in G29

Pet spiritshots (parse done; needs pets to cast), reconnect resummon
(`CharSummonTable`), servitor master-buff inheritance, `ServitorSkillUse`. Pet
**evolution** is struck — no handler on this dist.
