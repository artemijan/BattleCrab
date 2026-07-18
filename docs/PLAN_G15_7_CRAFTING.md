# G15.7 — Crafting & recipes

Port of the Java crafting subsystem (`CraftingEnabled = True` on this dist).
Ground truth: `interlude_classic/java/.../instancemanager/RecipeManager.java`,
`data/xml/RecipeData.java`, `model/RecipeList.java`, the
`handlers/itemhandlers/Recipes.java` item handler, and the
`RequestRecipeShop*` / `RequestRecipeItem*` / `RequestRecipeBook*` packet
family. Data: `dist/game/data/Recipes.xml` (631 recipes).

## The decisive config: `AltGameCreation = False`

On this dist `AltGameCreation = False` and `StoreRecipeShopList = False`, which
collapses the whole staged multi-pass craft into a **synchronous one-shot**:

- No `_activeMakers` scheduling, no `MagicSkillUse` craft animation loop, no
  `SetupGauge`, no crafting XP/SP (`calculateAltStatChange` / the `rewardPlayer`
  XP block), no HP/MP rest-waiting (`isWait` branch), no `grabSomeItems` /
  `_creationPasses` — all of that is gated behind `Config.ALT_GAME_CREATION`.
- `RecipeManager.requestMakeItem` / `requestManufactureItem` call `maker.run()`
  inline, which for `!ALT_GAME_CREATION` runs `finishCrafting()` immediately.
- Manufacture stores are **not persisted** (`StoreRecipeShopList = False`), so
  the store is a transient component like `PrivateStore`.

So the port is: validate → consume MP/HP once → pay adena (manufacture) →
consume materials → roll `Rnd.get(100) < successRate` → reward (with masterwork
rare roll) or send the failure SM.

## Pieces

### Data — `data/recipe_data.rs`
`RecipeList { id, level (craftLevel), recipe_item_id (recipeId = the item that
teaches it), success_rate, item_id, count, rare: Option<{item_id, count,
rarity}>, is_dwarven, ingredients: Vec<(item_id, count)>, mp_use, hp_use }`.
Parse `Recipes.xml`; `statUse` only HP/MP matter (XP/SP/GIM are AltGameCreation-
only); `altStatChange` ignored. Keep the Java production-block max-equipable-
grade filter (`MAX_EQUIPABLE_ITEM_GRADE`, EVENT escape). Lookups: `get(id)`,
`by_recipe_item_id(item_id)`. Register in `GameData`.

### Config — `config/character.rs`
`crafting_enabled` (CraftingEnabled), `dwarf_recipe_limit`,
`common_recipe_limit`, `craft_masterwork`, `craft_masterwork_chance`.

### Model — `model/components.rs`
- `RecipeBook { dwarven: Vec<i32>, common: Vec<i32> }` (recipe **list** ids) —
  persisted, part of `PlayerData`.
- `ManufactureStore { items: Vec<(i32 recipe_list_id, i64 cost)>, title }` —
  transient component (store_type byte MANUFACTURE = 5).

### DB — `db.rs`
`load_recipe_book` (mirror `load_skills`) → `Vec<i32>` list ids into `CharData`;
`from_char` splits into dwarven/common via `data.recipe_data`. Persist in the
store transaction (delete + insert; `type` = 1 dwarven / 0 common derived from
RecipeData, `classIndex` 0). `PlayerSaveData.recipe_book: Vec<(i32, bool)>`.

### Packets
Server (`server_packets/recipe.rs`, opcodes 0xDC–0xE1): `recipe_book_item_list`
(already present — feed real ids), `recipe_item_make_info`,
`recipe_shop_manage_list`, `recipe_shop_sell_list`, `recipe_shop_item_info`,
`recipe_shop_msg`. Client opcodes 0xB5–0xC0 + read helpers.

### Handlers — `game_loop/crafting.rs` (RecipeManager port)
- Recipe learning (`learn_recipe`, from `use_etc_item` `ItemHandler::Recipes`):
  craft-skill gate (dwarven = skill 172, common = 1320), level gate, recipe
  limit, already-registered check → register + consume + `S1_HAS_BEEN_ADDED`.
- Book: `request_book_open` (real book), `handle_book_destroy`.
- Self-craft: `handle_make_info` (→ RecipeItemMakeInfo), `handle_make_self` →
  `do_craft(crafter, customer=crafter, price=0)`.
- Manufacture store: `open_manage`, `handle_message_set`, `handle_list_set`
  (sit + broadcast RecipeShopMsg), `handle_manage_quit`, `open_sell_list`
  (click), `handle_shop_make_info`, `handle_shop_make_item` →
  `do_craft(manufacturer, customer, price)`.
- Core `do_craft`: the synchronous `RecipeItemMaker` (materials on the
  *customer's* inventory, MP/HP on the *crafter*, adena customer→crafter,
  masterwork rare roll).

## Gate
Learn a recipe from a recipe item; craft an item from materials (self); buy a
craft from another player's manufacture store.

## Deferred (TODO markers at the sites)
- AltGameCreation staged crafting + crafting XP/SP (config False here).
- Manufacture-store persistence (StoreRecipeShopList False).
- `Stat.CRAFT_RATE` / `Stat.CRAFTING_CRITICAL` bonuses (no item/skill in the
  ported set grants them → identity, `+0`).
- Recipe-limit `Stat.RECIPE_DWARVEN/COMMON` modifiers (no source → base config).
