# G29 slice 5 — PetData (the pet-template foundation)

## What this is, and what it isn't

This is a **foundation slice**: the `dist/game/data/stats/pets/*.xml` loader
(56 templates) that pet summoning needs before it can exist. It does not summon
a pet. I want that stated plainly rather than implied by a hopeful title.

## Why it stops here

Pets are the second half of G29's gate — *"summon a pet, feed it, and it
persists"* — and three things have to exist first:

1. **This table.** A pet's stats, food item, hunger limit and food capacity come
   from `PetData`, not from its NPC template, and the summon is keyed by the
   **collar item** (`itemId` → `npcId`). Without the loader there is nothing to
   summon *from*.
2. **The collar→cast binding.** Java's `SummonPet` effect does not receive the
   item; the `SummonItems` handler stashes a `PetItemHolder` script on the
   player and the effect pulls it back out (`player.removeScript(...)`). The
   port's `use_item_skills` has no equivalent "this cast came from item X"
   channel, so that plumbing is genuinely new work and belongs with the summon
   slice, not bolted onto a loader.
3. **Persistence.** The gate says *persists*, and a pet's identity is the
   collar's **object id** (`pets.item_obj_id`) — which is how two collars of the
   same kind stay two different pets. The `pets` table already ships in the
   dist schema this port uses, so it is query work rather than migration work,
   but it is still its own slice.

Splitting there keeps each piece testable against real data instead of
producing one large half-verified change.

## What landed

`data::pet_data` — `PetTemplate` (npc id, collar item, food item, hunger limit,
load, per-level rows) and `PetData` with Java's two lookups: `get(npc_id)` and
`getPetDataByItemId`.

Two parsing details worth naming:

- **Species-wide and per-level `<set>` elements share a tag name**, separated
  only by being inside `<stats>`. The parser tracks that, and a test asserts
  the two don't bleed into each other — reading `food` into a level row (or
  `max_meal` into the species) would be silent and wrong.
- `max_meal(level)` **clamps to the table**, falling back to the highest row at
  or below the level asked for, matching Java's bounds behaviour.

Per-level combat stats (`org_pattack`, `org_hp`, …) are parsed into the level
rows but not yet consumed — the NPC template's own stats stand in until pet
levelling lands. Recorded so the fields aren't mistaken for dead code.

## Tests

Three, all against the real dist: the whole directory loads (56 templates), the
Wolf reads back its declared collar/food/hunger/capacity, the species-vs-level
separation holds, and `max_meal` clamps past the table.

## Next

Pet summoning (the collar→cast binding + spawn, reusing the servitor entity
machinery), then feeding and persistence.
