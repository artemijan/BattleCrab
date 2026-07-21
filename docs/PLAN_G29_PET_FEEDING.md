# G29 slice 8 — pet feeding

Closes the G29 gate: *"summon a pet, feed it, and it persists."* Summoning
landed in slice 6, persistence in slice 7; this is the food loop.

## Feeding is not a value, it's a skill

The obvious shape — "food item restores N" — is wrong. Java's `PetFood` item
handler runs the item's **`NORMAL` item-skills**, and the restore lives in a
skill effect:

```
item 2515 Wolf Food  →  skill 2048  →  <effect name="Feed"><normal>100</normal>
```

So this slice needed a `SkillEffect::Feed` variant and a parse arm. Without it
the food was consumed and restored nothing — the same silent-drop failure mode
as the heal and DoT families before it. `<ride>`/`<wyvern>` feed a *mounted
player's* bar instead; mounts aren't ported, so only `normal` is carried
(`TODO(G29)`).

`Config.PET_FOOD_RATE` (`Rates.ini` `PetFoodRate`, 1 on this dist) multiplies
the restore, and is now a real config key rather than an assumed 1.

Reach: 7 `Feed` effect instances across the datapack, 9 items on the `PetFood`
handler.

## The food lives in the *pet's* inventory

This is the part that made the slice bigger than "add an effect". `PetFood`
refuses an unmounted **player** outright — so the owner cannot eat pet food on
the pet's behalf. The only route is to transfer food into the pet's own bag.
That meant porting the container and both transfer packets:

- `PetInventory` (Java `ItemLocation.PET`) — reuses `Inventory` exactly as
  `Warehouse`/`Freight` do. Java keys these rows by the **player-owner's**
  object id (`PetInventory.getOwnerId()` returns `_owner.getOwner()...`), so
  they ride along with the character's items and persist for free through the
  existing reconcile. The pet entity is transient; the rows are not.
- `RequestGiveItemToPet` (0x95), `RequestGetItemFromPet` (0x2C),
  `RequestPetUseItem` (0x94), `PetItemList` (0xB3).

**Known Java quirk, deliberately kept:** the rows carry no per-pet
discriminator, so a player with two collars sees the *same* pet inventory on
both pets. That is this dist's behaviour, not a port bug.

The collar itself is refused as a transfer target — it would become unreachable
the moment the pet was unsummoned.

## The feed tick

Java `Pet.FeedTask`, a fixed 10 s period (`ScheduledTask::PetFeedTick`), with
the usual "dead or gone → the chain ends" contract:

1. Burn one interval: `fed > consume ? fed - consume : 0` — floored, never
   negative. The **battle** rate applies while attacking, the normal rate
   otherwise.
2. If hungry (`fed < hungryLimit%` of the level's `maxMeal` — 55% for a wolf)
   **and** food is in the pet's bag: consume one and run its skills.
3. Otherwise nag — "not much time remaining", or "starving and will not obey"
   once the bar hits zero.

`setCurrentFed` clamps at `maxMeal`, so over-feeding is capped rather than
banked. A test measures from a bar with room in it so the clamp is what's under
test, not an already-full bar — the same trap that hid the mana-restore penalty
in G19 ([[l2r-mana-restore]]).

Java's `deleteMe` ("the pet is now leaving") only fires when the species has
**no** food ids at all; a starving pet with a defined food item sulks instead of
vanishing. Ported as written.

## A fixture that can't catch parse bugs

The feeding tests build their own food item and skill, which means they'd pass
even if the `Feed` parse arm were broken. So there is one extra test that loads
the **real** skill 2048 out of the datapack and asserts `normal == 100`. If the
XML shape stops reaching `SkillEffect::Feed`, every pet food in the game
silently restores nothing, and that test is the only thing that would notice.

Worth generalising: whenever a slice's fixtures hand-build the data the parser
was supposed to produce, add one datapack-backed test alongside them.

## Incidental

`ItemTemplate` gained `Default` (and `ItemKind` a `#[default]` of `Etc`). The
crafting tests were spelling out all thirty fields to build a plain etc-item;
new fixtures no longer have to.

## Tests

`servitor_tests` 41 → 51. Drain, zero-floor, auto-eat when hungry, no-eat when
full, the max-meal clamp, transfer both directions, the collar refusal, manual
feeding, the wrong-food refusal, and the datapack parse check.

**Note for worktree runs:** `character_create_inserts_into_real_schema` copies
`interlude_classic.db` from the repo root, which is untracked and therefore
absent in a git worktree. It fails there and passes from the main checkout — not
a code failure.

## Still open for pets

- Pet death (Java restores a `curHp < 1` row as a dead pet — `TODO(G29)` at the
  site).
- Pet paperdoll (`PET_EQUIP`) — battle-pet armour and weapons; `PetInventory`
  serializes flat until then.
- Pet exp gain / level-up / evolution, collar enchant mirroring pet level,
  auto-resummon on reconnect (`CharSummonTable`).
- Servitor: master-buff inheritance, `ServitorSkillUse`, summon points.
- Cubics, agathions.
