# G19 — EnlargeSlot skill effect

## Why this slice

Next on a fresh ranking sweep after `DamageBlock` closed out the previous
batch, re-run with the `EFFECT_REGISTRY` generic stat-modifier table (`PAtk`,
`PhysicalDefence`, `MaxHp`, `ShieldDefence`, …) now correctly excluded from
the survey — that table quietly absorbs dozens of otherwise-unported-looking
effect names, which inflated earlier raw counts. With it excluded,
`EnlargeSlot` tops the list: 5 learnable skills (Expand Inventory/Warehouse/
Trade/Common Craft/Dwarven Craft), 84 skill levels, 162 raw `<effect>`
instances (Expand Trade carries two per level).

## What Java does

`EnlargeSlot.java` (`dist/game/data/scripts/handlers/effecthandlers/`) is a
passive-recalc effect: no `onStart`/`onExit`, just `pump()`, which maps its
`type` param (one of 6 `Stat` values, default `INVENTORY_NORMAL`) and calls
`getStat().mergeAdd(stat, amount)`. All 5 skills are `operateType="P"`,
`toLevel="8"`, `mode=DIFF` (each level replaces the bonus, not additive across
casts — there's no "cast" for a passive anyway). Consumed by six `Player`
getters (`getInventoryLimit`/`getWareHouseLimit`/`getPrivateSellStoreLimit`/
`getPrivateBuyStoreLimit`/`getDwarfRecipeLimit`/`getCommonRecipeLimit`), each
`baseConfigValue + (int) getStat().getValue(Stat.X, 0)`. Fully reachable —
`CreatureStat.recalculateStats` re-invokes `pump()` on every passive
effect on every stat recompute (login, skill learn, level up). No DB
persistence: 100% derived from currently-known passive skills.

Learned via a village master (fishing guild NPC), not a scroll — same
skill-tree path as any other class passive.

## What landed

- **6 new `Stat` variants** (`InventoryNormal`, `StoragePrivate`, `TradeSell`,
  `TradeBuy`, `RecipeDwarven`, `RecipeCommon`) + the `"EnlargeSlot"` parse arm
  (`data/skill_data.rs`): reads `type` (a plain string, not `param()`'s f64,
  same shape as `DamageBlock`'s `type`), maps it to a `Stat`, and emits a
  `StatModifier` via the existing `stat_mod` helper — the identical
  `EFFECT_REGISTRY`-style single-stat passive as `ShieldDefence`, just with a
  type-selected stat instead of a 1-name-1-stat table entry (`CriticalDamage`'s
  pattern, not `DamageBlock`'s bespoke variant).
- **`PlayerView` gained a `mods: &StatModifiers` field** (`model/mod.rs`,
  populated at both construction sites — `PlayerView::of` and
  `PlayerData::view`) so packet builders that need a finalized
  storage-capacity number can call `model::finalize` without threading `World`
  through every call site.
- **`UserInfo`'s INVENTORY_LIMIT block** (`network/user_info.rs`) now folds
  `Stat::InventoryNormal` in via `finalize` instead of reporting the bare
  race-based config value.
- **`ExStorageMaxCount`** (`network/enter_world.rs`) now takes a `mods`
  parameter and folds `InventoryNormal`/`StoragePrivate`/`TradeSell`/
  `TradeBuy`/`RecipeDwarven`/`RecipeCommon` into the six corresponding fields
  (previously all six were Java's static config-default placeholders,
  including a literal comment "`Stat.INVENTORY_NORMAL` not wired"). Freight and
  clan-warehouse slots stay placeholders — no such systems on this port yet.
- **`crafting::learn_recipe`**'s recipe-book cap check (the one place that
  *already enforced* a config-based slot limit) now finalizes through
  `Stat::RecipeDwarven`/`RecipeCommon` instead of reading the bare config
  value — this is the one consumer with real enforcement behind it today.
- **A newly learned passive now applies live**: `handle_request_acquire_skill`
  (`game_loop/skills/mod.rs`) calls `passive_skills::recompute_conditioned_
  passives` right after inserting the skill into `SkillBook`. That function
  already diffs "book vs currently-applied passive buffs" generically (despite
  the module's armor-conditioned-passives framing — the condition check is a
  no-op when an effect carries no armor/weapon condition, which `EnlargeSlot`
  never does), so this was a one-line reuse, not new logic. Before this, *any*
  stat-modifier passive (including previously-landed ones like `ShieldDefence`)
  only took effect on the next login — a real, pre-existing gap this slice's
  own test would otherwise have had to route around.

## Test

- `data::skill_data::tests::enlarge_slot_picks_stat_by_type_param` — real dist
  shapes inline: Expand Inventory's absent `<type>` defaults to
  `InventoryNormal`; Expand Dwarven Craft's `RECIPE_DWARVEN` is picked; Expand
  Trade's two effect blocks land as `TradeBuy` + `TradeSell` both.
- `game_loop::tests::skills_tests::enlarge_slot_expand_inventory_raises_
  reported_cap` — real dist data (skill 1372 level 3, `+18`), passives folded
  via `Player::from_char`, checked through `model::finalize` exactly as the
  packet builders now do.
- `game_loop::tests::crafting_tests::enlarge_slot_expand_dwarven_craft_
  raises_recipe_limit` — end to end through `items::handle_use_item` →
  `learn_recipe`: a book filled to the config base limit refuses a new recipe
  (`UP_TO_S1_RECIPES_CAN_BE_REGISTERED`), then learning Expand Dwarven Craft
  (1368, real dist data, `+6`) via the same code path
  `RequestAcquireSkill` now uses lets the same registration through.

## Deferred (not this slice)

- Warehouse deposit and private-store listing aren't capacity-checked
  *anywhere* in this port yet — only the number `ExStorageMaxCount` reports
  changed here, not an enforcement gate (`TODO(G29+)`:
  `Warehouse.java`'s over-limit reject on deposit, `PrivateStore`'s
  slot-count reject on `handle_set_list`).
- Freight and clan-warehouse slots — no such systems exist on this port;
  `ExStorageMaxCount` still carries Java's static defaults for those two
  fields.
