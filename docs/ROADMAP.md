# Remaining-work Roadmap (G14 → G33)

Master plan for the rest of the Java→Rust port: every Java game-server subsystem
not yet built, broken into dependency-ordered milestones. Supersedes the old
single "G14 Long tail & parity sweep" catch-all in
[PLAN_GAME_SERVER.md](PLAN_GAME_SERVER.md) §6. Live status lives in
[PROGRESS.md](PROGRESS.md).

**Where we are:** G0–G13 done (login complete; game through enter-world, items,
stats/skills, combat/AI, social, quests/clan-creation, static world, and the
full admin framework + all *portable* admin handlers incl. mounts, transforms,
mob-group AI, and the geo-editor). What's left is **subsystem breadth** — the
whole-feature systems the vertical slices deferred.

**The admin lens.** The 202 still-unimplemented `//` commands are almost all
*gated-but-bodiless* because their backing subsystem doesn't exist yet. Each
milestone below lists the admin handler(s) it unblocks, so "finish the admin
system" and "reach Java parity" are the same backlog.

**Definition of done (per milestone):** faithful port of the Java behavior,
verifiable against the live Java server on the same DB/client, with the milestone
gate met. Same vertical-slice-then-breadth discipline used through G13.

**2026-07 Java-surface audit.** A sweep of the Java tree (`instancemanager/`,
`handler/`, `taskmanager/`, `network/clientpackets/`, dist config) against this
roadmap added six milestones the original breakdown missed — G15.5 (teleporters
& user commands), G15.7 (crafting), G20.5 (recommendations), G24.5 (boats),
G26.5 (lottery & monster race), G30.5 (item auction) — plus per-milestone
**Audit additions** notes and the Classic/custom **scope gate** after the
out-of-scope list.

---

## Milestone map

| # | Milestone | Track | Unblocks (admin) | Depends on |
|---|-----------|-------|------------------|------------|
| G14 | Item stats & equipment combat accuracy ✅ | Foundations | `//setparam` ✅ | — |
| G15 | Economy & item actions | Foundations | — | G14 |
| G15.5 | Teleporters & user commands 🚧 | Foundations | — | — |
| G15.7 | Crafting & recipes ✅ | Foundations | — | G15 |
| G16 | Character variables, premium & vitality ✅ | Foundations | `//premium*` `//pccafepoints` `//primepoints` `//set_vitality_level` | — |
| G17 | Sub-classes, class change & nobless | Progression | `//setnoble` `//setsubclass` (editchar) | G22¹ |
| G18 | Clans — full ✅ | Progression | `//clan_*` `//pledge` `//add_clan_skill` | G15 |
| G19 | Skills & effects breadth 🚧 | Combat | `//ave_abnormal` `//setteam` `//settargetable` `//para` `//playmovie` … (AdminEffects) | — |
| G20 | Combat breadth ✅ | Combat | — | G14, G19 |
| G20.5 | Recommendations | Support | — | G16 |
| G21 | NPC AI & world-content breadth | Combat | `//scan` extras, guard/faction | G20 |
| G22 | Quest & script breadth | Content | `//quest_*` `//charquestmenu` `//setcharquest` `//reload` (scripts) | G17, G19 |
| G23 | Grand bosses & raid bosses | End-game | `//grandboss` (AdminGrandBoss) | G21 |
| G24 | Castles, sieges, clan halls & territory war | End-game | `//siege`/AdminFortSiege, `//castle`, `//clanhall`, territory war | G18, G21 |
| G24.5 | Boats | End-game | — | — |
| G25 | Olympiad & hero | End-game | AdminOlympiad, `//saveolymp` `//endolympiad` `//sethero` `//givehero` `//settruehero` | G17 |
| G26 | Seven Signs, Manor & Mammon | End-game | `//manor`, `//mammon_*` | G24, G15 |
| G26.5 | Lottery & Monster Race | End-game | — | G15 |
| G27 | Instances | End-game | AdminInstance, AdminInstanceZone | G21 |
| G28 | Events engine & cursed weapons | End-game | AdminEvents, `//tvt_*`, AdminCursedWeapons | G20 |
| G29 | Summons, pets, servitors, cubics, agathions | Support | AdminEditChar summon/pet subcommands | G19, G20 |
| G30 | Mail, community board & party matching | Support | AdminBBS | G18 |
| G30.5 | Item auction | Support | — | G15, G30 |
| G31 | Moderation, accounts, petitions & HWID | Support | AdminPunishment, AdminLogin, AdminHwid, AdminPetition, AdminFakePlayers, editchar find_ip/dualbox/tracert | IP plumbing |
| G32 | Fishing | Support | — | G19 |
| G33 | Misc parity & finishing sweep | Finishing | AdminFightCalculator, AdminRepairChar, AdminPForge, AdminMissingHtmls, AdminPcCondOverride, `//geosave` serializer | (last) |

¹ G17's occupation *quests* need G22, but the class-change *mechanics* can land
first; nobless status can be admin-set before the nobless quest exists.

**Out of scope (present in the datapack, not Interlude Classic):**
`AdminGraciaSeeds`, ADMIN HELLBOUND, `AdminElement` (Gracia/Hellbound/elemental
attributes are Kamael-era content). Also out: `tools/` ports, MariaDB/Postgres,
Swing UI (per [PLAN_GAME_SERVER.md](PLAN_GAME_SERVER.md) §11).

**Scope gate — Classic-client & custom systems (2026-07 audit).** This build is
an Interlude/Classic hybrid: the client protocol and datapack carry Classic-era
systems the original scope never ruled in or out. Decisions:

- **In scope, folded into existing milestones:** mentoring (`MentorManager`,
  `config/MentorCoins.xml`) → G17; secondary password (`SecondaryAuthData`,
  `RequestEx2ndPassword*`) → G31; contact list
  (`RequestExAddContactToContactList` family) → G30; party adena distribution
  (`adenadistribution` packets) → G30; teleport bookmarks
  (`RequestTeleportBookMark`/bookmark-slot packets) → G15.5.
- **Deferred until an operator asks** (disabled on this dist or cash-shop
  flavored): attendance rewards (`EnableAttendanceRewards = False`), training
  camp (`TrainingCampEnable = False`), daily missions, beauty shop, prime shop,
  lucky game, item commission, item compounding (`CombinationItems.xml`),
  appearance stones, auto-play/auto-potion (`Custom/AutoPlay.ini`,
  `Custom/AutoPotions.ini`).
- **Out of scope:** the sayune/shuttle/airship packet families (post-Interlude
  movement, inert on Interlude maps) and the Mobius `config/Custom/*` features
  (offline trade/play, sell-buffs, scheme buffer, faction system, fake players,
  champion monsters, banking, class master, wedding, delevel manager, the
  seasonal `scripts/events/*`) except any the operator explicitly enables —
  G33 includes a one-time audit of `Custom/*.ini` enable flags to finalize this.

---

## Track A — Foundations (high leverage; do first)

### G14 — Item stats & equipment combat accuracy
Item `<stats>` parsing + weapon/armor bonus application (P/M-Atk, P/M-Def,
accuracy, evasion, crit, attack speed, jewelry HP/MP) **were already done** in an
earlier commit (the `ItemStats` side-map + `EquippedBonuses` in
`Player::recalculate_stats`). ✅ **Shields** (`calcShldUse`) now landed —
`sDef`/`rShld` parse, block rate (× CON, ×1.3 for bows, back-arc gated), normal
block adds shield def to pDef, perfect block → 1 dmg, "shield defense succeeded"
SM. ✅ **`//setparam`/`//unsetparam`** — a `StatModifiers.fixed` override map the
finalizers honor (and buff recomputes preserve). **Deferred:** `ArmorSetData`
(set bonuses + `getArmorMinEnchant`) → **G19** (sets grant *skills*); the
`SHOTS_BONUS` dynamic stat (matters only for the `reducedSoulshot` weapon perk,
unused in the ported set). **G14 done.**

### G15 — Economy & item actions
🚧 **In progress.** ✅ `RequestDestroyItem` (0x60). ✅ **Ground items** — a
`GroundItem` world-object kind (`World.ground_item_regions`), `SpawnItem`/
`DropItem`/`GetItem` packets, `RequestDropItem` (0x17), pickup via `Action`,
visibility on enter + region-change deltas, the **auto-loot=false** death path
drops onto the ground, and **decay** (`ItemsOnGroundManager`, 600 s lifetime).
✅ **Personal warehouse** — a `Warehouse` container (newtype over `Inventory`,
persisted alongside it via `loc="WAREHOUSE"`), the `WareHouseDepositList`/
`WithdrawalList` windows, `SendWareHouse*List` (0x3B/0x3C) deposit/withdraw
(enchant-preserving transfers), and the `DepositP`/`WithdrawP` keeper bypass.
✅ **Crystallization** (`RequestCrystallizeItem` 0x2F) — `crystal_count` parsed,
`CrystalType::crystal_item_id`/`required_crystallize_level`; destroy a
crystallizable item + award its grade's crystals (1458–1462), gated on the
`Crystallize` skill (248) level vs grade (D→1 … S→5), unequip-first.
✅ **Sell to merchant** (`RequestSellItem` 0x37) — sell inventory items to a
targeted merchant for reference-price/2 adena (the buy side + sell tab already
existed; this completes the merchant shop).
✅ **Private sell store** — `PrivateStore` component + `Player.store_type`
(CharInfo/UserInfo byte, byte-test safe); manage window (0xA0), set-list (0x31),
buyer view (0xA1) on click, `PrivateStoreMsgSell` (0xA2) title, buy transaction
(0x83, items seller→buyer + adena buyer→seller, store closes when sold out),
quit (0x96). Buy/manufacture stores + package sell deferred.
✅ **Player-to-player trade** — `Trade`/`PendingTrade` components; request (0x1A)
→ `SendTradeRequest` (0x70), answer (0x55) → `TradeStart` (0x14) both sides, add
item (0x1B) → `TradeOwnAdd`/`TradeOtherAdd` (resets confirms), confirm/cancel
(0x1C) → press-ok echoes (0x53/0x82), and on both-confirm the offered items swap
(`TradeDone`). One active trade per player; enchant preserved.

✅ **Enchant chance engine** (`data/enchant_data.rs`) — ports
`EnchantItemGroupsData` + `EnchantItemData`: the `<enchantRateGroup>` chance
ladders, the scroll group `0` rate-item bindings (slot mask + magic-weapon +
item-id whitelist → named rate group), and the branded scrolls
(`targetGrade`/`maxEnchant`/`safeEnchant`/`bonusRate`/whitelist). `base_chance`
mirrors `EnchantScroll.getChance`: scroll-group resolution → `EnchantItemGroup`
ladder → `safeEnchant` short-circuit to 100 → `+bonusRate` capped at 100 (7
tests against real dist data covering armor vs full-armor divergence, the weapon
ladder, safe/bonus, and the `-1` error sentinel).

✅ **Enchant scroll client flow** (`game_loop/enchant.rs`) — the full Ex-packet
handshake on the engine above: `EnchantScrolls` item handler opens the window
(`EnchantRequest` component + `ChooseInventoryItem`) →
`RequestExAddEnchantScrollItem` (0xE3) → `ExPutEnchantScrollItemResult` →
`RequestExTryToPutEnchantTargetItem` (0x49) validates & acks
`ExPutEnchantTargetItemResult` → `RequestEnchantItem` (0x5F) destroys the
scroll, rolls (`base_chance` + `roll_f64`), and applies the outcome —
`+1` on success, and on failure safe-retain / blessed-reset / blessed-down-1 /
destroy+crystallize, each with the right `EnchantResult` code;
`RequestExCancelEnchantItem` (0x4B) closes it. Item side gained the
`etcitem_type` parse (`EtcItemType`: weapon/blessed/blessed-down/safe
classification), `enchant_enabled`/`enchant_limit`/`is_magic_weapon`, and
`ItemTemplate::is_enchantable`. `EnchantData::is_target_valid` ports
`EnchantScroll.isValid` + `AbstractEnchantItem.isValid` (whitelist / other-scroll
claim / type2 / grade / range). End-to-end test: use scroll → +0→+1 guaranteed
success, then a forced fail at +4 destroys the sword and returns crystals.
**Not modelled (documented TODOs):** support items
(`RequestExTryToPutEnchantSupportItem` + random-enchant ranges + support bonus),
the 2-second anti-autoenchant timestamp guard, milestone announce/firework, and
on-enchant armor skills.

✅ **Clan warehouse** (`game_loop/warehouse.rs` refactor + `ClanWarehouse`
bypass) — a shared container on `Clan` in `world.clans` (vs. the per-player
personal one), routed by a new `ActiveWarehouse` component the keeper bypass
sets (`depositc`/`withdrawc`, since the deposit/withdraw client packets carry no
type). Gates ported from `ClanWarehouse.useBypass`: clan membership + clan level
≥ 1, and `CL_VIEW_WAREHOUSE` (leader-short-circuit) for withdraw; deposit needs
membership only. `whType = CLAN(2)` in the list packets. **Persisted**: loaded at
boot in `load_clans` (`owner_id = clan_id`, `loc = "CLANWH"`) and flushed on
every change via the fire-and-forget `StoreClanWarehouse` DB command
(delete-then-reinsert, like the player item save). Test: leader deposits
(persist asserted), an unprivileged member is denied the withdraw window, the
leader withdraws — shared container throughout.

✅ **Freight — withdraw side** (`Freight` container + `package_withdraw`
bypass) — the account-package warehouse, a per-player `Freight` component (like
`Warehouse`) persisted in the player rows (`loc="FREIGHT"`). Completes the
warehouse family: `ActiveWarehouse` now routes three targets (private/clan/
freight) through one `container_ref`/`transfer`, whType per Java (FREIGHT=1).
Test: seed the freight, `package_withdraw`, withdraw part, confirm the
`loc="FREIGHT"` persistence. **Remaining (the send half):** `package_deposit`
→ `PackageToList` (needs the account's character list, an async DB enumeration
the in-game session doesn't hold) → `RequestPackageSendableItemList` /
`RequestPackageSend` (writes to a possibly-offline recipient's freight rows).

✅ **Augmentation roll engine** (`data/variation_data.rs`) — ports
`VariationData` (`Variations.xml`): per-mineral `<optionGroup>` (warrior/mage ×
order 0/1) of weighted `<optionCategory>` → `<option>`/`<optionRange>` pools,
plus the `<itemGroups>`/`<fees>` cost map. `generate(mineral, is_magic, rng)`
mirrors `generateRandomVariation`: a weighted category pick then an option pick
per group (`OptionDataGroup`/`OptionDataCategory.getRandom*`), producing the two
option ids; `fee`/`cancel_fee` give the gemstone/adena costs. Tests against real
dist data: load count, deterministic-roll option ids for warrior vs mage routes,
fee lookup, unknown-mineral `None`. **Remaining for the full feature:** what each
rolled option *does* — the 390k-line `stats/augmentation/options/*` effect set
(stat bonuses / granted skills), which ties into the stat+skill systems — and
the refine Ex-packet client flow (`Augment` bypass → `ExShowVariationMakeWindow`
→ `RequestConfirmRefinerItem`/`RequestRefine`/`RequestRefineCancel` +
`ExVariationResult`), the `VariationInstance` on the item, and the augment
display bytes in the item packets.

✅ **Augmentation refine flow** (`game_loop/augment.rs`) — the make/cancel
vertical on the roll engine: `Augment 1`/`2` bypass → `ExShowVariation{Make,
Cancel}Window` → `RequestConfirmRefinerItem` (validate weapon + life stone,
echo the gemstone fee via `ExPutIntensiveResultForVariationMake`) →
`RequestRefine` (roll the two options through `World::roll_augment`, consume the
life stone + gemstones, stamp the variation, `ExVariationResult`) →
`RequestRefineCancel` (charge the adena cancel fee, strip the augment). Augment
is stored on `ItemInstance` (mineral + two option ids) and shown via
`paperdoll_augmentation` → `ExUserInfoEquipSlot`. Test: augment a Crimson Sword
(confirm → refine → options rolled, materials consumed) then cancel (adena fee
charged, augment removed).

✅ **Augmentation persistence** (`item_variations`) — the augment now rides the
item rows: `ItemRow` carries the mineral + two option ids, `Inventory::to_rows`/
`from_rows` round-trip them, `load_items` populates them from `item_variations`,
and `store_player` delete-then-reinserts the owner's augmented rows (in the save
transaction). Verified via a `build_save_data` → `from_rows` round-trip. **Not
yet:** the option *effects* (the 390k-line `stats/augmentation/options/*`
stat/skill bonuses — a dedicated milestone) and the item-list mask display bit.

✅ **Enchant support items** (`RequestExTryToPutEnchantSupportItem` /
`RequestExRemoveEnchantSupportItem`) — the `<support>` half of the enchant flow:
`EnchantData` now loads `EnchantSupport` (grade/min/max/bonusRate/randomEnchant
range) and validates them (`is_support_valid` ports `EnchantScroll.isValid`'s
support branch — weapon/blessed/giant agreement + grade/range); `EtcItemType`
gained the `INC_PROP` support classification. Put-support (0x4A) validates &
acks `ExPutEnchantSupportItemResult`; remove (0xE4) clears it. `RequestEnchantItem`
consumes the support, adds its bonus to the chance, and (on success) applies its
`randomEnchant` step instead of the scroll's +1. Test: a +20 support flips a
66.67% roll to success at +3 and is consumed.

**Next slices — each a dedicated milestone:** augmentation option effects (the
stat/skill bonuses each option grants), the freight send half (needs
account-char enumeration plumbing). Remaining ground-item TODOs: enchant carried
through pickup (stackables only for now), owner-based loot protection.

The itemcontainer breadth G5 deferred: private/clan warehouse + freight; private
stores (sell/buy/manufacture/package) + offline stores; player-to-player trade;
ground drop/pickup (`ItemsOnGroundManager`, herbs); `multisell`/`sell` bypasses;
crystallization; enchant scrolls (safe/normal/blessed + `EnchantResult`);
augmentation / life stones + variation; the rest of `handlers/itemhandlers/*`
(dyes/scrolls/`<cond>` gating). **Gate:** warehouse round-trip, a private store
sells to another client, trade completes, an item enchants and can break, loot
drops to the ground and is picked up. **Deps:** G14 (enchant/augment stat
effects).

**Audit additions (2026-07):** multisell (`MultisellData` + `MultiSellChoose` —
82 dist lists, the concrete deliverable behind the "multisell/sell bypasses"
line above); refund/buyback (`RequestRefundItem`, `AllowRefund = True`); item
try-on (`RequestPreviewItem`, `AllowWear = True`); `RequestBuySellUIClose`;
inventory-order persistence (`RequestSaveInventoryOrder`); and the item-
maintenance task managers — `ItemLifeTimeTaskManager` (time-limited items),
`ItemManaTaskManager` (shadow items), `ItemsAutoDestroyTaskManager` (ground-item
cleanup breadth).

### G15.5 — Teleporters & user commands
*(2026-07 audit addition.)* Two small, high-playability systems no milestone
covered:

- **Gatekeepers:** `TeleporterData` (`data/teleporters/` — town/castle/
  clanhall/fortress/others) + the teleport bypass family: list windows, adena
  pricing (incl. the free-teleport-under-level config), noble/hunting-ground
  lists. Also home for teleport bookmarks (scope gate).
- **User commands:** `BypassUserCmd` → `handler/usercommandhandlers/*` —
  `/unstuck` (escape cast + town teleport) first, then `/loc`, `/time`,
  `/partyinfo`, and the rest.

**Gate:** a player pays a gatekeeper to teleport between towns; `/unstuck`
returns a stuck character to town. **Deps:** none (teleport/movement primitives
exist) — the cheapest playability win on the board.

🚧 **Landed (2026-07-16):** `data/teleporter_data.rs` (all dist lists, incl.
`<npcs>` aliases) + `game_loop/teleporter.rs` (`showTeleports`/
`showTeleportsHunting`/`teleport`/`showNoblesSelect` bypass verbs, fee
suffix + adena charge, free ≤ `MaxFreeTeleportLevel`, karma gate);
`BypassUserCmd` (0xB3) → `game_loop/user_commands.rs` with `/unstuck`
(30 s forced-hit-time cast of 2099, GM 2100, via the new
`SkillEffect::EscapeToTown` — also fixes escape *items* once their handlers
land) and `/loc` (map-region `locId` + coords). Loader/dist-XML/synthetic-
world tests throughout. **Remaining:** teleport bookmarks, the rest of the
user-command family, the Mon/Tue fee discount (needs wall clock), nobles
lists (G17), siege gates (G24).

### G15.7 — Crafting & recipes
*(2026-07 audit addition — `CraftingEnabled = True` on this dist.)*
`RecipeData` (`Recipes.xml`), the recipe book (`character_recipebook` persist,
`RequestRecipeBookOpen`/`Destroy`, common vs dwarven split), craft execution
(`RequestRecipeItemMakeInfo`/`MakeSelf` — MP cost, material consume, success
roll), and the private manufacture store (`RequestRecipeShop*` — reuses G15's
private-store plumbing, `ManufactureItem` price list). **Gate:** learn a
recipe, craft an item from materials, and buy a craft from another player's
manufacture store. **Deps:** G15.

✅ **Landed (2026-07-19)** — plan [PLAN_G15_7_CRAFTING.md](PLAN_G15_7_CRAFTING.md).
`data/recipe_data.rs` (`RecipeData` — all 631 `Recipes.xml` recipes, ingredients
+ MP/HP `statUse` + `productionRare`) and a `RecipeBook` component
(dwarven/common recipe-list ids) loaded from `character_recipebook` and persisted
in the store transaction. `game_loop/crafting.rs` is the **synchronous**
`RecipeItemMaker` (this dist is `AltGameCreation = False`, so no staged multi-pass
craft, craft animation, crafting XP/SP, or HP/MP rest-wait): recipe learning via
the `Recipes` item handler (craft-skill/level/limit gates), book open/destroy,
self-craft (`RequestRecipeItemMakeInfo`/`MakeSelf` — material + MP/HP consume,
`Rnd.get(100) < successRate` roll, masterwork `productionRare` roll), and the
manufacture store (`RequestRecipeShop{ManageList,MessageSet,ListSet,ManageQuit,
MakeInfo,MakeItem}` — store byte MANUFACTURE, click→`RecipeShopSellList`,
customer buys a craft with the adena fee going crafter-ward, materials/MP off the
right party). Server packets `RecipeBookItemList`/`RecipeItemMakeInfo`/
`RecipeShop{ManageList,SellList,ItemInfo,Msg}` (0xDC–0xE1). **Deferred:**
`AltGameCreation` staged crafting + XP/SP (config-off), manufacture-store
persistence (`StoreRecipeShopList = False`), the `Stat.CRAFT_RATE` /
`CRAFTING_CRITICAL` / `RECIPE_DWARVEN/COMMON` modifiers (no source in the ported
set → identity).

### G16 — Character variables, premium & vitality
`GlobalVariablesManager` + a per-character key/value store (`character_variables`
table). On top of it: premium accounts (+ `ExVitalityEffectInfo` bonuses),
PC-café points, prime points, full vitality (points ↔ level, peace-zone regen,
item consumption), henna/dye symbols on the character sheet. **Gate:** a
premium flag and vitality level survive relog; henna changes stats.
**Unblocks:** `//premium*`, `//pccafepoints`, `//primepoints`,
`//set_vitality_level`.

✅ **Henna slice landed (2026-07-19)** — plan [PLAN_G16_HENNA.md](PLAN_G16_HENNA.md).
`data/henna_data.rs` (`HennaData` — 372 dyes) + a `HennaSlots` component whose
dye stat bonuses are folded into `BaseStats` (`= template + Σ worn dyes`,
recomputed on draw/remove, so the finalizers + UserInfo panel pick henna up with
no special-casing). `character_hennas` load/persist; the `RequestHenna*` packet
family (equip/remove/item-info/item-remove-info/item-list/remove-list) +
`HennaInfo`/`HennaEquipList`/`HennaRemoveList`/`HennaItemDrawInfo`/
`HennaItemRemoveInfo`; the SymbolMaker `Draw`/`Remove` bypass; `HennaInfo` in the
enter-world burst; empty-slot count from class level (`*_CLASS_GROUP` → 0/2/3).
Interlude dyes are permanent (`duration=-1`), so the timed-henna scheduler +
`HennaDuration` variables + LUC/CHA + dye skills are out of scope.

✅ **Vitality + variables + premium effects — G16 complete (2026-07-19)** —
plan [PLAN_G16_VITALITY.md](PLAN_G16_VITALITY.md). The `character_variables`
key/value store (`PlayerVariables` component, loaded at char load, flushed in
the store transaction); the vitality pool in `game_loop/vitality.rs` (clamped
`0..=140_000`, `setVitalityPoints`'s four notices + `ExVitalityPointInfo` +
party-window field, `updateVitalityPoints` through the gain/lost rates with the
`isLucky` exemption); the ×2 exp/sp bonus, folded in via a new `use_bonuses`
argument on `add_exp_and_sp` so quest/admin grants opt out exactly as Java's
two-argument overload does, with the surplus reported in the acquisition
SystemMessage's bonus slots; per-kill consumption
(`Attackable.getVitalityPoints`) on both the solo and party reward branches; a
real `Custom/PremiumSystem.ini` loader replacing the inlined
`PREMIUM_SYSTEM_ENABLED`, plus `hasPremiumStatus` and `PremiumRateXp`/`Sp` on
the reward path; real `ExVitalityEffectInfo` fields; and
`StartingVitalityPoints` at creation. **Deferred:** the daily/weekly refills
(`DailyTaskManager`, `TODO(G33)` — they need the wall-clock daily-task
scheduler, so **vitality currently only drains**), vitality-restoring *items*
(the `VITALITY_ITEMS_USED` counter is stored and reported but nothing
increments it), `PC_CAFE_RETAIL_LIKE` per-kill points, and the unmodelled
`VITALITY_CONSUME_RATE`/`BONUS_EXP`/`BONUS_SP` stats (`TODO(G19)`).

---

## Track B — Progression & clans

### G17 — Sub-classes, class change & nobless
Occupation change (1st/2nd/3rd) through the village-master flow; subclass
add/change/level with the class-skill retable; certification skills; nobless
status + tiara. The class-change *mechanic* + admin set can land before the
occupation *quests* (G22). **Gate:** a character changes class and gets the new
skill tree; a subclass can be added and switched. **Unblocks:** `//setnoble`,
fuller `//setclass`, `//setsubclass`.

**Audit additions (2026-07):** mentoring (`MentorManager`, `mentoring` packets,
`config/MentorCoins.xml` — graduation triggers off class change), per the
scope gate.

### G18 — Clans (full) ✅ COMPLETE — see [PROGRESS.md](PROGRESS.md) and [PLAN_G18_CLANS.md](PLAN_G18_CLANS.md) for the full 8-slice breakdown
Everything past G11's creation slice: invite/join/leave/oust/dissolve; clan
level-up + reputation; sub-pledges (royal guard / order of knights) + academy;
clan skills + `PledgeSkillList`; crests (pledge/ally/large); notices; clan
warehouse; clan wars; alliances; the `PledgeInfo`/`PledgeStatusChanged`/RELATION
breadth. **Gate:** form a clan, invite members, level it, learn a clan skill,
declare war, form an ally. **Unblocks:** `//clan_*`, `//pledge`,
`//add_clan_skill`/`//give_clan_skills`. **Deps:** G15 (clan warehouse).

**Audit additions (2026-07):** clan recruitment/entry (`ClanEntryManager` — the
`RequestPledgeRecruit*`/draft-list/waiting-list packet family) and the Classic
pledge-bonus rewards (`ClanRewardData`, `pledgebonus` packets). The five
clan-window queries (ex 0xD3/0xD4/0xD8/0xDC/0xDE → `ExPledgeRecruitInfo`/
`ExPledgeRecruitBoardSearch`/`ExPledgeDraftListSearch`/
`ExPledgeRecruitApplyInfo`) are already answered
with faithful empty-registry responses (`game_loop/clans.rs`); this milestone
adds the registry itself.

---

## Track C — Combat, skills & AI breadth

### G19 — Skills & effects breadth
Grow `EFFECT_REGISTRY` toward the 369 Java effect classes and the 230-entry
`Stat` enum on demand; toggle-type skills; the remaining `AcquireSkillType`s
(pledge/transform/transfer/subclass/collect/…); ~~`calcMagicSuccess`
(`ALT_GAME_MAGICFAILURES`)~~ (done — magic damage is now resisted against
out-of-level targets); AoE affect scopes (only `SINGLE` resolves today);
buffs/effects on NPC targets; the **abnormal-visual-effect** runtime + per-
creature team / targetable / display-effect state; `ExAbnormalStatusUpdateFrom
Target`. **Gate:** a debuff lands on a mob, an AoE nuke hits a cluster, a toggle
skill switches on. **Unblocks:** the AdminEffects AVE subset (`//ave_abnormal`,
`//setteam`, `//settargetable`, `//para*`, `//bighead`, `//playmovie`,
`//set_displayeffect`, `//event_trigger`), `//switch_gm_buffs`.

**Audit additions (2026-07):** skill enchanting (`EnchantSkillGroupsData` +
`RequestExEnchantSkill`/`Info`/`InfoDetail` — the level-76+ skill-enchant flow).

🚧 **Affect scopes + toggles landed (2026-07-19)** — plan
[PLAN_G19_EFFECTS.md](PLAN_G19_EFFECTS.md). The two structural gaps that stopped
whole categories of skill from working: **affect scopes**
(`game_loop/skills/affect.rs` — `SINGLE`/`RANGE`/`POINT_BLANK`/`PARTY`/`PLEDGE`
sweeps with the `NOT_FRIEND`/`FRIEND`/`CLAN`/`ALL` object filters, the
`affectLimit` cap incl. Java's `min + Rnd.get(max)` roll quirk, dead-skip,
"range skills don't hit you unless you're the main target", the peace-zone leg
and the target-centred LOS check), with `handle_skill_finish` fanned out over
the affected list so effects, PvP flagging and monster hate apply per target;
and **toggles** (recast switches off, `toggleGroupId` mutual exclusion, instant
cast via `SkillCaster.run`'s short circuit, the new `targetType NONE`). Until
this landed only `SINGLE` resolved, so all ~1900 area skills in the datapack hit
exactly one creature and all 104 toggles were unreachable.

🚧 **Abnormal-state flags + crowd control (2026-07-19)** — plan
[PLAN_G19_ABNORMAL_STATES.md](PLAN_G19_ABNORMAL_STATES.md). A datapack survey
put 41 % of effect instances (11 698 / 28 259) on unported effects; this slice
takes the pair that were *inert rather than merely absent* — `BlockActions`
(540 uses: stun/sleep/paralyze) and `Root` (79). Java's `EffectFlag` mask is
ported as flags stamped on each `ActiveBuff` and folded on read
(`game_loop/abnormal.rs`), rather than Java's cached-and-invalidated mask, since
buffs mutate from several places here. Gates: no attacking, casting or moving
while stunned; no moving while rooted; a stunned mob's AI goes quiet; a rooted
one stays put. Landing a stun also interrupts the victim mid-action (abort cast,
then freeze movement — that order matters). Before this a stun landed, showed
its icon, and the victim carried on fighting.

🚧 **Abnormal resistance, blocking & probabilistic dispel (2026-07-19)** — plan
[PLAN_G19_ABNORMAL_RESIST.md](PLAN_G19_ABNORMAL_RESIST.md). The other half of the
CC system: `ResistAbnormalByCategory` (Guts, Touch of Life/Death) pumping a
multiplier on incoming debuff chance that `calc_effect_land_rate` now applies as
Java's `buffDebuffMod` (multiplied before the clamp); `ResistDispelByCategory`
(Ultimate Defense) pumping `ResistDispelBuff`, correctly left *unconsumed* since
Java reads it only in `calcCancelSuccess` (the unported `Cancel` family);
`BlockAbnormalSlot` (the Prophecies' mutual exclusion) refusing blocked abnormal
types via the same stamp-and-fold pattern as the CC flags; and
`DispelBySlotProbability` (the Bane family) rolling its rate **per buff**.

**Method note for future slices:** rank unported effects by *learnable-skill*
usage, not raw instance count. `StatUp` tops the raw list (887 instances) but is
only 9 learnable skills — its footprint is almost all talisman/Freya/agathion
content outside Interlude's reach.

🚧 **Periodic HP/MP effects, healing modifiers & CP (2026-07-19)** — plan
[PLAN_G19_PERIODIC_EFFECTS.md](PLAN_G19_PERIODIC_EFFECTS.md). The top of the
learnable-usage ranking, and one coherent family: `HealOverTime` (11) and
`ManaDamOverTime` (10) join the existing DoT tick chain rather than adding
schedulers; `HealEffect` (9) scales healing on the *recipient*; `Cp` (7) is an
instant CP change. `HealOverTime` is **not heal-only** — its power is routinely
negative, which is how the upkeep toggles (Fury Fists, Arcane Wisdom) pay their
HP cost, floored at 1 so it never kills. An MP tick that exceeds current MP
switches a **toggle** off with SM 140 (Java's `false` return, honoured only for
toggles), closing the loop on the toggles ported in the first G19 slice — until
now they were free.

**Recurring trap:** effects carrying no stat modifier are silently dropped by
`apply_skill_effects`' empty-effects guard — the buff never lands, so nothing
happens at all. This has now bitten three slices running (stun/root,
`BlockAbnormalSlot`, both periodic effects). The guard now reads as three
categories — *periodic*, *icon-only*, *state flag* — and any new modifier-less
effect must join one.

🚧 **CC breadth — mute, debuff-block, control-block, target-cancel
(2026-07-19)** — plan [PLAN_G19_CC_BREADTH.md](PLAN_G19_CC_BREADTH.md).
Completes the crowd-control family: each is a flag on the existing
`effect_flag` mask plus the one gate Java puts it behind — `MUTED` /
`PHYSICAL_MUTED` refusing magic vs non-magic skills in `checkDoCastConditions`
(static skills exempt), `DEBUFF_BLOCK` bailing on incoming debuffs outright
ahead of the resistance multiplier, `BLOCK_CONTROL` refusing item use, and
`TargetCancel` dropping the victim's target and aborting their attack and cast
on a chance roll. A landing mute also aborts the victim's in-flight cast —
**except on raid bosses**, which is what stops one silence from neutering a
raid.

`Fear` (9 learnable skills) is the deliberate hold-out: it needs forced flee
movement in the AI rather than a flag, so it belongs with **G21**'s NPC AI
breadth. `AbnormalShield` has no ported source, and Java's wider
`BLOCK_CONTROL` (summon/mob control) waits on G29.

🚧 **Abnormal visual effects (2026-07-19)** — plan
[PLAN_G19_ABNORMAL_VISUALS.md](PLAN_G19_ABNORMAL_VISUALS.md). The cosmetic
counterpart to the CC/periodic slices: stun, root, silence, poison and bleed all
worked mechanically but the client drew **nothing** on the victim, because
`CharInfo` hard-coded an abnormal-visual count of 0. The enum→client-id map,
`<abnormalVisualEffect>` parsing, and a stamp-and-fold visual set now feed both
`CharInfo` and `ExUserInfoAbnormalVisualEffect` — pushed *only when the set
changes*, as Java does (an unconditional refresh is both chattier than retail
and observable, since several tests assert exact packet order). `//ave_abnormal`
lands on top, toggling a GM-pinned visual held in a new `AdminVisuals` component.

Two scoping checks made here: the **geometric affect scopes** deferred in the
first G19 slice are only **5 learnable skills**, confirming that deferral; and
the rest of the AdminEffects AVE subset (`//setteam`, `//settargetable`,
`//set_displayeffect`, `//playmovie`, `//event_trigger`) is now *unblocked* but
each needs its own per-creature state and packet field.

🚧 **Transformation (2026-07-20)** — plan
[PLAN_G19_TRANSFORMATION.md](PLAN_G19_TRANSFORMATION.md). The next rung on the
learnable-skill ranking after ruling out `DefenceAttribute` (33 learnable
skills, but Kamael-era elemental attributes are out of scope): `Transformation`
(32 learnable skills) backs the "Transform &lt;Monster&gt;" scroll family
(Grail Apostle, Unicorn, Doom Wraith, Zaken, …). Wires the skill-cast path into
the G13.B `//transform` admin runtime's existing `Player.transform_id`/
`TransformData` plumbing rather than building a second one — `admin::transforms`
split into state-only and state+broadcast halves so the buff-landing path can
mutate transform state and fold the transform-specific extras
(`ExUserInfoAbnormalVisualEffect` + refreshed `SkillList`) onto the `UserInfo`
broadcast the buff already sends, instead of a duplicate one. Reverts on
`BuffExpire` like any timed buff, which — since the death path already routes
every stripped buff through the same removal function — also covers death with
no extra hook. Cast-time gate ports `ConditionPlayerCanTransform`'s
already-transformed/in-water/cursed-weapon-equipped legs (a horse/bike mount
collapses into "already transformed" here, since mounts are themselves
transforms on this port); the sitting and registered-on-event legs have no
modeled state yet.

🚧 **MpConsumePerLevel (2026-07-20)** — plan
[PLAN_G19_MP_CONSUME_PER_LEVEL.md](PLAN_G19_MP_CONSUME_PER_LEVEL.md). Next
after `Transformation`, skipping `Summon`/`SummonCubic`/`SummonNpc` (G29) and
the already-written-off `StatUp`/`Fear`: the MP-upkeep half of the core
fighter-class toggles (Accuracy 256, Guard Stance 288, Vicious Stance 312,
Parry/Riposte Stance 339/340, War Frenzy 424, Super Haste 7029, …), each of
which already carries a real `StatModifier` that lands correctly — this was
the *other* effect on the same skill, silently dropped, so every one of these
toggles has been a free, uncosted buff on this port. A datapack survey found
every instance (all 19, not just the 11 learnable ones) is a toggle with no
`abnormalTime`, which collapses Java's formula to exactly
`ManaDamOverTime`'s `power * getTicksMultiplier()` — so it shares that
effect's tick-chain arm (periodic drain, self-deactivate + SM 140 on
insufficient MP) rather than duplicating it; the level-scaled `abnormalTime >
0` branch is unexercised by this datapack and left as a TODO. Broke
`admin_tests::admin_superhaste_applies_and_persists` as a side effect — Java's
`AdminSuperHaste` casts through the real `applyEffects` path, so `//superhaste`
is correctly now subject to the same drain, and the test's zero-MP setup
(bare `skill_data` override, not the full datapack) needed the same
`GameData::load_from` fix `Transformation`'s own test needed.

🚧 **ShieldDefence / ShieldDefenceRate (2026-07-20)** — plan
[PLAN_G19_SHIELD_DEFENCE.md](PLAN_G19_SHIELD_DEFENCE.md). Next after
`MpConsumePerLevel`, setting `EnergyAttack` (9 learnable) aside — it needs the
Dwarf Force/Charges resource, unmodeled on this port, a bigger lift than one
effect. `ShieldDefence` (8 learnable) is cheap: a single-stat
`AbstractStatEffect` like a dozen already-ported ones. The headline skill is
**Shield Mastery (153)**, a passive every shield-using class can learn —
`ShieldDefenceRate` turned out to already be *parsed* (an earlier slice put it
in `EFFECT_REGISTRY`) but never actually *read*: `game_loop::combat::
shield_stats` computed the block rate straight off the equipped shield's raw
`rShld`, bypassing `StatModifiers` entirely; `ShieldDefence` wasn't parsed at
all. So every shield-using character's real block chance and block-defence
bonus were flatly wrong the moment they learned their class's own core shield
passive. Both fold through `model::finalize` (bumped to `pub(crate)`) — the
same `base * mul + add` Java's `ShieldDefenceFinalizer`/
`ShieldDefenceRateFinalizer` use — over the shield's own `sDef`/`rShld`, gated
behind the existing "no shield equipped" early return so a buff like
Residence Shield Defense (603, +225 DIFF) still contributes nothing without an
actual shield, matching `Formulas.calcShldUse`'s short-circuit order.

🚧 **HealPercent (2026-07-20)** — plan
[PLAN_G19_HEAL_PERCENT.md](PLAN_G19_HEAL_PERCENT.md). Next after
`ShieldDefence`, setting `AttackTrait` (7 learnable) aside — it needs a whole
`TraitType` attacker-bonus/weakness system unmodeled on this port, a bigger
lift than one effect. `HealPercent` (5 learnable, 138 instances) is cheap —
the same `instant()` shape as the already-ported `Heal` — and every one of
its five learnable instances is core priest kit: **Miracle (1426)**,
**Benediction (1271)**, **Restore Life (1258)**, **Revival (181)**, **Touch
of Life (341)**. All five parsed to an empty effect list before this slice,
so casting any of them healed nothing. New match arm mirrors `Heal`'s
NPC-silent/player-with-SM split and overheal clamp, but computes the amount
as a max-HP percentage and skips `Heal`'s recipient `HealEffect`/
`HealEffectAdd` scaling, matching Java's real asymmetry; the negative-power
(damage) branch is ported for parity even though no learnable instance uses
it. Surfaced an unrelated gap while testing: `TargetType::EnemyNot` isn't
modeled at all (34 instances, 4 learnable, including Restore Life itself) —
falls through to `Other`, which `use_magic_on` silently no-ops on.

🚧 **TargetType::EnemyNot (2026-07-20)** — plan
[PLAN_G19_ENEMY_NOT_TARGET.md](PLAN_G19_ENEMY_NOT_TARGET.md). Closed the gap
`HealPercent` surfaced: `targethandlers/EnemyNot.java` is "any friendly
selected target" — the precise inverse of `Enemy`/`EnemyOnly`'s
`is_auto_attackable` gate, self always allowed, no force-use (ctrl)
override — plus an explicit exemption from the general dead-target rejection
("works on dead targets or doors as well", for a heal landing on a fresh
corpse ahead of a resurrection). Small in scope (34 instances) but it was
quietly capping two of `HealPercent`'s five learnable skills — Restore Life
and Touch of Life, the two that heal someone *other* than the caster — since
the other three are self-target and worked regardless. New `TargetType`
variant + parse arm + `resolve_cast_target` case, reusing the same
`is_auto_attackable` helper `Enemy`/`EnemyOnly` already call, just inverted.

🚧 **Force/charges — FocusMomentum + EnergyAttack (2026-07-20)** — plan
[PLAN_G19_FORCE_CHARGES.md](PLAN_G19_FORCE_CHARGES.md). Unblocks the
`EnergyAttack` slice set aside twice before: builds the warrior "Force"
resource (`Player.charges`, transient, never persisted — matches Java) and
both effects that touch it. Not niche — **Sonic Focus → Sonic Blaster/
Buster** (and the Orc/Dark Elf Force Burst/Storm/Blaster equivalents) are
core early warrior skills; 9 `EnergyAttack` + 6 `FocusMomentum` learnable
skills all parsed to empty effect lists before this, so the Force-builders
did nothing and the Force-spenders were silent no-ops. `FocusMomentum` gains
`amount` charges capped at `max_charges.min(8)` — Java's `MAX_MOMENTUM` stat
is never set anywhere in this datapack, so `8` (`FocusMomentum.java`'s own
hardcoded fallback) is the *real* cap on this build. `EnergyAttack` shares
`PhysicalAttack`'s damage core and simplifications (no trait/weakness/
attribute/PvP-PvE terms — none of those are modeled anywhere on this port)
times a new `1 + charge·0.1` boost; `chargeConsume` is a **skill-level** tag,
not a child of the effect element, the one field this port needed pulled
from outside the `<effect>` block. `EtcStatusUpdate` (0xF9) now carries the
real charge count instead of a hardcoded 0. Deferred: Java's 10-minute
charge-decay task; `GetMomentum` (dead code in this datapack — nothing sets
`MAX_MOMENTUM`, so its own `0` fallback caps it at zero regardless); wiring
the charge bonus into `PhysicalSoulAttack`/`MagicalSoulAttack`/`SoulBlow`'s
existing `×1` stand-ins (their own TODOs, follow-on work).

🚧 **Lethal (2026-07-20)** — plan [PLAN_G19_LETHAL.md](PLAN_G19_LETHAL.md).
`AttackTrait` set aside a third time — needs the `TraitType` system, a
cross-cutting project (attacker trait map + wiring trait/weakness bonuses
into every physical damage formula), not a slice. `Lethal` (9 learnable) was
already flagged in `SkillEffect::Blow`'s own doc comment as a TODO — every
learnable instance pairs it with an already-ported damage effect (Backstab
30, Lethal Blow 344, Deadly Blow 263, Critical Blow 409, Lethal Shot 343,
Turn/Banish Undead/Seraph 1400/405/450), so those skills' damage landed but
the bonus instant-kill/half-kill chance they're *named* for never rolled.
Level gate (`skill.magicLevel < target.level - 6`) and raid-boss immunity
(reusing `Mute`'s own `is_raid()` check) ported; full-lethal/half-lethal
rolls set a player's CP (and HP, on a full lethal) to 1, or halve a monster's
HP, with `chanceMultiplier` at `1.0` (no trait/attribute math anywhere on
this port). `INSTANT_KILL_RESIST` isn't rolled at all — like `MAX_MOMENTUM`
before it, nothing in this datapack ever sets it, so Java's own roll against
it always loses. Deferred: `DamageBlock`'s `BLOCK_HP` gate,
`calcCounterAttack`'s reflect, `GrandBoss`/`Door` lethal-immunity.

🚧 **AttackTrait (2026-07-20)** — plan
[PLAN_G19_ATTACK_TRAIT.md](PLAN_G19_ATTACK_TRAIT.md). The last item on the
learnable-skill ranking that started this run of G19 slices, investigated
properly this time instead of deferred a fourth time. Turned out smaller
than feared — all 7 learnable instances (Detect Insect/Beast/Animal/Dragon/
Plant Weakness 75/80/87/88/104, Eye of Hunter/Slayer 359/360) use only the
`*_WEAKNESS` category of `TraitType`, not the weapon-type or status-resist
halves — **and** the consuming formula, `Formulas.calcWeaknessBonus`, turns
out to be inert on the real Java server too: it needs a matching NPC-side
`DefenceTrait`, and grepping the whole Java tree + datapack found exactly one
call site for `mergeDefenceTrait` — its own definition. No NPC ever gets one.
So this lands as an icon-only buff (unit variant, no per-trait data — nothing
would ever read it) alongside `DefenceTrait`/`VampiricAttack`, closing a real
regression (the effect name wasn't recognized at all, so the buff didn't even
land) without needing to invent damage-formula wiring for a bonus that's
provably inert either way. Collateral: `NpcTemplate.race`/`Race` extended
from the six playable races to the full 26-member enum Java actually shares
between players and creature categories (`UNDEAD`, `BEAST`, …) — costs
nothing today, but the day NPC-side trait data lands, the race data
`calcWeaknessBonus` would need is already there. Verified safe: the only
other consumer (the Newbie Guide's own-race gate) only ever compares against
guide NPCs, always a real playable race, and player/monster race ordinals
(0-6 vs 7-25) never overlap.

🚧 **DamageBlock (2026-07-20)** — plan
[PLAN_G19_DAMAGE_BLOCK.md](PLAN_G19_DAMAGE_BLOCK.md). Next on a fresh ranking
sweep (5 learnable, 84 skills, 162 instances — highest raw count left, since
a skill carries two `<effect>` elements, one per block kind). Already flagged
by two existing TODOs (`HealPercent`/`Lethal` both noted "Java also skips
this while `isHpBlocked()` — not gated, since that effect isn't ported yet").
The five learnable instances (Celestial Shield 1418, Flames of Invincibility
1427, Dance of Medusa 367, Sonic/Force Barrier 442/443) are short (10-30s)
full-invulnerability shields. `HP_BLOCK` has a real single choke-point
consumer in Java (`CreatureStatus.reduceHp`, refusing essentially all
incoming HP damage except a DoT tick or a skill's own HP cost) — matched here
by adding an `is_dot: bool` parameter + an early return to `game_loop::
combat::apply_physical_damage`, already the one function every damage path on
this port funnels through (auto-attack, every instant-damage `SkillEffect`,
DoT ticks, damage zones), so this needed no new choke point, just gating the
existing one. `MP_BLOCK` is the same "genuinely dead code in Java too"
pattern this run of slices keeps finding (`AttackTrait`'s `MAX_MOMENTUM`,
`Lethal`'s `INSTANT_KILL_RESIST`): `isMpBlocked()` has zero callers anywhere
in the Java tree, so it's folded for completeness but wired to nothing.
Closed both existing TODOs along the way.

**EnlargeSlot** (plan: [PLAN_G19_ENLARGE_SLOT.md](PLAN_G19_ENLARGE_SLOT.md)).
Re-ran the ranking sweep with `EFFECT_REGISTRY`'s generic stat-modifier table
(`PAtk`, `MaxHp`, `ShieldDefence`, …) correctly excluded — it had been
quietly absorbing dozens of effect names and inflating earlier raw counts.
With it excluded, `EnlargeSlot` topped the list: Expand Inventory/Warehouse/
Trade/Common Craft/Dwarven Craft (5 learnable, 162 raw instances — Trade
carries two per level). A `type`-selected `Stat` passive, same shape as
`ShieldDefence`/`CriticalDamage`: 6 new `Stat` variants (`InventoryNormal`,
`StoragePrivate`, `TradeSell`, `TradeBuy`, `RecipeDwarven`, `RecipeCommon`)
folded through `model::finalize` into `UserInfo`'s INVENTORY_LIMIT block,
`ExStorageMaxCount` (which previously reported all six capacity numbers as
Java's static placeholder defaults — one literally commented
"`Stat.INVENTORY_NORMAL` not wired"), and `crafting::learn_recipe`'s
recipe-book cap, the one consumer with real enforcement behind it. Surfaced
and fixed a wider, pre-existing gap along the way: a newly learned passive
skill only took effect at the *next login* — `recompute_conditioned_passives`
(built for armor-swap re-evaluation, but generic underneath) is now also
called from `RequestAcquireSkill`, so any stat-modifier passive (this one
included) applies the moment it's learned. Deferred: warehouse deposit and
private-store listing still aren't capacity-checked anywhere on this port —
only the *number reported* to the client is accurate now, not an enforcement
gate (`TODO(G29+)`).

**Hate-manipulation effects** (plan:
[PLAN_G19_HATE_EFFECTS.md](PLAN_G19_HATE_EFFECTS.md)). A fresh ranking sweep
surfaced a tied cluster of six related effect names sharing one primitive —
an NPC's `AggroList`, already ported. Rather than take the top name alone and
defer the rest a fifth time (the `AttackTrait` pattern), bundled the four
cheap ones: `GetAgro` (Aggression, Aggression Aura, Judgment, Tribunal),
`AddHate` (Charm, Lure), `DeleteHate` (Eva's Serenade, Peace, Repose),
`DeleteHateOfMe` (Bluff, Forget, Trick) — 12 learnable-skill instances.
`GetAgro` needed the most thought: the ported AI derives its attack target
fresh from `AggroList::most_hated` every think tick rather than caching a
"current target" the way Java's AI object does, so "force intend-attack the
caster" had to become "make the caster's hate dominant" (current max + 1, not
an unbreakable magic constant) rather than a direct intention override.
`DeleteHate`/`DeleteHateOfMe` both disengage the target's AI wholesale via a
newly `pub(crate)` `npc_ai::set_active` — previously private, now shared with
`think_attack`'s own timeout/leash disengage paths rather than a duplicate.
Deferred: `TargetMe` (paired with `GetAgro` on the same 2 skills) needs a
locked-target UI concept nothing on this port gates target-changes on;
`RandomizeHate` (Confusion, Switch) needs a general nearby-visible-creatures
query that doesn't exist yet (the closest analog, `faction_call`'s neighbour
scan, only walks NPCs); `GetAgro`'s clan-mate pre-seed is left to the
already-ported `faction_call`, which recruits reactively once the taunted NPC
is actually landing hits, at most one think-tick later than Java's immediate
pre-seed.

**DispelByCategory** (plan:
[PLAN_G19_DISPEL_CATEGORY.md](PLAN_G19_DISPEL_CATEGORY.md)) — the "Cancel"
family (Cancellation, Cleanse, Purification Field, Touch of Death). Another
tied cluster at 4 learnable skills each; picked over the cheaper
`PhysicalAttackRange` (a same-shape repeat of the already-solved
`ShieldDefenceRate` pattern, no new value) because it closes a real,
previously-flagged gap: `Stat::ResistDispelBuff`, pumped by
`ResistDispelByCategory` since the earlier abnormal-resist slice, was
explicitly documented as "consumer-less until `Cancel` lands." Unlike
`DispelBySlot`/`DispelBySlotProbability` (a fixed abnormal-type list), this
steals *whatever* is up: `BUFF` slot walks dances then buffs in reverse cast
order, each gated by a ported `calcCancelSuccess` (`clamp(rate +
(casterMagicLvl - buffMagicLvl)*2 + (buffAbnormalTime/120)*
ResistDispelBuff, 25, 75)`, skipped as automatic success when `rate>=100`);
`DEBUFF` slot walks debuffs with a flatter `roll <= rate` (note the `<=`,
matching Java's operator exactly rather than this codebase's usual `<`
convention for per-item rolls). The dances-before-buffs ordering came free
from the already-ported `BuffSlot` classification (`Skill::buff_slot()`
already excludes passive/toggle/debuff from `Dance`/`Buff`, covering most of
`canBeStolen()` without new code — only the `can_be_dispelled` flag needed
an explicit check). Java's `ALL` slot is dead code — no shipped skill uses
it — and stays a no-op here too. Deferred (matching `DispelBySlotProbability`'s
own precedent): `isIrreplacableBuff()`/hero/GM/static-skill exclusions, none
of which exist on the ported `Skill` struct and none of which any learnable
skill on this dist needs.

**Still open (the milestone's continuous half):** `EFFECT_REGISTRY` growth
toward the 369 Java effect classes and the 230-entry `Stat` enum — the ~11
icon-only community-board buffs and G16's identity-valued
`VITALITY_CONSUME_RATE`/`BONUS_EXP`/`BONUS_SP` are waiting on it; the geometric
`FAN`/`FAN_PB`/`SQUARE`/`SQUARE_PB`/`RING_RANGE` scopes and `GROUND`-targeted
casts; the CC effects adjacent to the ported pair (`BlockControl` 81, `Fear` 68
— needs forced flee movement, `DebuffBlock` 115,
`TargetCancel` 101, `KnockBack` 91, and the mute/disarm family); ~~`calcMagicSuccess`~~ (done); the abnormal-visual-effect runtime + per-creature
team/targetable state (and the AdminEffects AVE subset it unblocks);
`ExAbnormalStatusUpdateFromTarget`; the remaining `AcquireSkillType`s; skill
enchanting; and `TargetType::EnemyNot` (34 instances, 4 learnable — found
unmodeled while testing `HealPercent`).

### G20 — Combat breadth
`PhysicalAttack`-type skills; bows/crossbows (arrows, reuse gauge); dual-weapon
split hits; polearm sweep; PvP auto-attack + the karma/PK/flag consumers; overhit
XP; the `SHOTS_BONUS` dynamic value; the rest of `isMovementDisabled`
(root/immobilize). **Gate:** a bow attack consumes an arrow, a polearm hits a
line, PvP flagging drives auto-attack, a physical skill lands. **Deps:** G14, G19.

**Audit additions (2026-07):** duels (`DuelManager`/`Duel` — 1v1 and party
duels, `RequestDuelStart`/`AnswerStart`/`Surrender`, end conditions + the arena
teleport variant). G25's olympiad matches reuse this shape, so duels land here.

🚧 **Ranged attacks landed (2026-07-19)** — plan
[PLAN_G20_RANGED.md](PLAN_G20_RANGED.md). The gate's first clause: bows and
crossbows now require **ammunition** matched to the weapon's crystal grade and
auto-equipped into the left hand, spend **MP** per shot, consume an arrow, and
arm a **reload delay** (`900000 / pAtkSpd`) shown as a red `SetupGauge`; running
out of arrows or MP refuses the swing. Bow *range* already worked — bows declare
`pAtkRange` 500, fed to `CombatStats` since G14.

Ammunition needs its own `Inventory::equip_ammunition`: the ordinary equip path
refuses `Etc` items (arrows are `Etc`) *and* its `SLOT_L_HAND` branch displaces
a two-handed weapon — it would unequip the bow. Java sidesteps the same problem
the same way (`setPaperdollItem` directly).

🚧 **Multi-hit melee landed (2026-07-19)** — plan
[PLAN_G20_MELEE_VARIANTS.md](PLAN_G20_MELEE_VARIANTS.md). Completes the
`doAttack` variant family: **dual** weapons strike the main target twice at half
damage, and the **polearm sweep** adds a hit per extra target inside the
weapon's radius (66 for a polearm vs 40 for a sword, from `damage_range`) and
its 120° arc. The `Attack` packet, which hard-coded "0 additional hits", now
carries the whole list, each hit scheduled through the normal victim-side path.

**The sweep is gated on `ATTACK_COUNT_MAX`, a stat — not on the weapon type.**
Holding a polearm sweeps nothing; **Polearm Mastery 216** (`HitNumber` 5) is
what enables it. A first pass at this slice wrongly concluded the whole feature
was dead on this dist, because `CreatureStat`'s angle default is 0 and a bad
regex missed skill 216 — but `PlayableStat` overrides radius/angle from the
weapon, and 216 is perfectly ordinary. Verify a "this is dead" conclusion as
carefully as a "this is live" one.

✅ **G20's gate is met (2026-07-19)** — plan
[PLAN_G20_PVP_KILLS.md](PLAN_G20_PVP_KILLS.md). The last clause needed the
*consumers*, not the targeting: `is_player_auto_attackable` already handled
peace/arena/siege zones, PK and flag, but killing a player moved nothing —
`player_do_die` carried a literal `let _ = killer_oid;`. Ported Java's three
branches (lawful PvP kill → `pvp_kills++`; positive-reputation first offence →
reset to 0; otherwise `calculateKarmaGain` + `pk_kills++`), with the PVP-zone
"do nothing" short-circuit. Found alongside: the death XP penalty was applied
unconditionally, where Java skips it inside PVP **or siege** zones — arena and
siege deaths are now free.

🚧 **Over-hit landed (2026-07-19)** — plan
[PLAN_G20_OVERHIT.md](PLAN_G20_OVERHIT.md). A killing blow from an `<overHit>`
skill banks the damage by which it overshot, paid as bonus XP capped at 25 % of
the share. 59 learnable skills carry it.

**`<overHit>` is an *effect* parameter, not a skill field** — it sits inside
`<effect>` and each damage handler reads `params.getBoolean("overHit")`. The
first implementation read it at skill level; the behaviour tests passed anyway
(their fixtures set it wherever the code looked) and only the parse assertion
against the real datapack caught it. Assert against real data, not just
fixtures.

🚧 **Duels (1v1) landed (2026-07-19)** — plan
[PLAN_G20_DUELS.md](PLAN_G20_DUELS.md). The last feature G20 names, and the one
**G25's olympiad reuses the shape of**: challenge → ask → accept → 5 s countdown
→ fight → end on death / surrender / 120 s timeout / drifting >1600 apart, with
the `canDuel` gates and the five `ExDuel*` packets. **A duel never kills** — the
losing blow is capped at 1 HP and ends the duel instead, so no death penalty and
no karma or PvP counters move.

Scoped to **1v1**: party duels teleport both parties into an arena instance,
which needs G27, so a party request is refused rather than half-handled.
Condition restore is simplified to a full heal rather than Java's pre-duel
snapshot (`TODO(G20)`); `canDuel` already requires ≥50 % HP/MP, so the gap is
small.

✅ **Death item drops — G20 complete (2026-07-19)** — plan
[PLAN_G20_DEATH_DROPS.md](PLAN_G20_DEATH_DROPS.md). A PK past
`MinimumPKRequiredToDrop` killed by a player scatters part of their inventory —
the **karma penalty**, not general looting: killing a *clean* player takes
nothing. A **monster** kill uses the separate, gentler `Player*` rates, so an
ordinary death to a mob can still cost an item. Adena/quest items never drop,
equipped items unequip first and roll the equip/weapon percentage, arena deaths
and GMs are exempt.

**Why this closes G20.** Checking the leftovers against *this dist* rather than
the list: `SHOTS_BONUS` is **provably dead** here — zero items declare
`reducedSoulshot`, so the stat has no source and porting it is a verified no-op;
karma decay is **blocked** on a per-level `KarmaData` table absent from
`data/`; party duels are **blocked** on G27's arena instances. Only the three
ranged scraps remain (bow peace-zone check, `CHEAPSHOT`, NPC-archer reuse), none
reachable in normal play.

**Scoping note:** several G20 line items were already done — `PhysicalAttack`
skills (instant-damage slice) and the root/immobilize half of
`isMovementDisabled` (G19 CC slice). Still open: polearm sweep, the melee half
of PvP auto-attack, dual-weapon split hits, overhit XP, `SHOTS_BONUS`, duels.

### G20.5 — Recommendations
*(2026-07 audit addition.)* The evaluation/recommendation system: rec-have/
rec-left counters on the character, `RequestVoteNew` (evaluate a target), the
`TaskRecom` daily reset, and the UserInfo/CharInfo fields they feed. **Gate:**
a given rec survives relog and the counters reset daily. **Deps:** G16
(character variables).

### G21 — NPC AI & world-content breadth
NPC skill casting (`AISkillScope` lists); minions; guard/faction/clan-help aggro
(needs karma); NPC pathfinding (chase/return-home + closest-reachable grid, the
G7.85 worker for NPCs) and NPC regen; ground drops + spoil/sweep; `DBSpawnManager`
persistence (raid HP across restart); `HtmCache`; walker routes; the other ~33
zone types (damage/effect/boss/jail/water-breath/no-store/arena…) + fence checks
+ the `ValidatePosition` door-exploit tail. **Gate:** a mob casts, a guard aggros
a PK, a spoiled corpse can be swept, a boss keeps its HP across restart.
**Deps:** G20.

**Audit additions (2026-07):** fences (`FenceData` — unblocks `AdminFence`),
NPC random social animations (`RandomAnimationTaskManager`), and
`CreatureSeeTaskManager` (the on-creature-see AI trigger scripts rely on).

**Progress:** NPC skill casting landed (`PLAN_G21_NPC_CASTING.md`) — the
`AISkillScope` buckets + the `thinkAttack` cast ladder, covering the "a mob
casts" gate clause. 4831 templates carry castable skills; 73 % of those
attachments resolve to fully-ported effects. Town-guard PK aggro + faction
help calls landed (`PLAN_G21_GUARD_AGGRO.md`) — the `<clanList>` faction data
(3760 templates) wasn't parsed at all before, so mobs fought alone. Remaining
gate clause: raid HP across restart (`DBSpawnManager`); minions parse but
never spawn. Raid-boss persistence landed (`PLAN_G21_BOSS_PERSISTENCE.md`) —
`DBSpawnManager`/`npc_respawns`, covering the last gate clause, so **G21's gate
is met**. Minions landed (`PLAN_G21_MINIONS.md`) — 460 leaders, 3289 escorts placed.
EffectZones landed (`PLAN_G21_EFFECT_ZONES.md`) — 218 zones, plus per-zone
`type=` parsing which recovered 20 zones that were missing entirely (605 → 843).
Note `ConditionZone` (1080) is ~99% inert on Interlude (`NoBookmark`).
NPC regen landed (`PLAN_G21_NPC_REGEN.md`) — 14855 templates' `hpRegen` was
parsed but unused, so no NPC ever healed. Remaining breadth:
`DamageZone`/`SwampZone` (only 15 live between them; the rest are siege-gated),
`DamageZone`/`SwampZone` landed (`PLAN_G21_DAMAGE_SWAMP_ZONES.md`) and walker
routes landed (`PLAN_G21_WALKER_ROUTES.md`), so **G21 is complete**. The only
untouched items are empty or blocked on this dist: `HtmCache` (caching only),
`CreatureSeeTaskManager` (needs a script engine), `FenceData` (one "demo"
fence). NPC pathfinding
landed (`PLAN_G21_NPC_PATHFINDING.md`) — mobs consulted no geodata at all
before, so chases walked through walls. `skillTargetReconsider` landed
(`PLAN_G21_TARGET_RECONSIDER.md`) — support mobs (1040 buffers / 305 healers)
now help their pack. Note `FenceData` is a single "demo" fence on this dist.

---

## Track D — Content

### G22 — Quest & script breadth
The remaining ~188 quests, ~14 village-master scripts and ~81 `ai/` scripts;
daily quests (`restartTime`); the tutorial (Q00255); ~~`onFirstTalk`~~ (✅ —
the hook and its first users, NewbieGuide + NpcLocationInfo); the
quest-window guards; `validateHtmlAction`; the remaining bypass families
(multisell/sell already partly in G15). Script hot-reload backs `//reload`.
**Gate:** the quest/AI parity checklist is green; a representative quest of each
kind (one-time, repeatable, daily, class-transfer, instance) completes.
**Unblocks:** `//quest_info`/`//quest_reload`/`//script_load`/`//script_unload`,
`//charquestmenu`/`//setcharquest`, `//reload`. **Deps:** G17, G19.

**Audit additions (2026-07):** the tutorial packet family
(`RequestTutorialLinkHtml`/`QuestionMark`/`ClientEvent`/`PassCmdToServer`)
backing Q00255.

---

## Track E — End-game systems (each unblocks a C-group handler)

### G23 — Grand bosses & raid bosses
Boss zones + entry conditions; respawn windows (`GrandBossManager` /
`RaidBossSpawnManager`); boss AI (chaos target swaps, raid curse, minion waves);
raid points; DB persistence of boss state/HP. **Gate:** a raid boss spawns on
schedule, applies raid curse, and its state persists. **Unblocks:**
`//grandboss`. **Deps:** G21.

### G24 — Castles, sieges, clan halls & territory war
Castle ownership + taxes + functions; the siege engine (registration, siege
zones, control towers, flags, siege guards/mercenaries); fort sieges; clan-hall
auction + siege; territory war. **Gate:** a siege can be scheduled, fought, and
change castle ownership; a clan hall can be bought at auction. **Unblocks:**
AdminFortSiege (`//siege*`), `//castle`, `//clanhall`, territory war commands.
**Deps:** G18 (clans), G21.

### G24.5 — Boats
*(2026-07 audit addition — `AllowBoat = True` on this dist.)* `BoatManager` +
the four `vehicles/` route scripts (Talking–Gludin, Giran–Talking, Innadril
tour, Rune–Primeval): the `Boat` world object following `VehiclePathPoint`
routes, board/disembark (`RequestGetOnVehicle`/`GetOffVehicle`), in-vehicle
movement/validation packets, and ticket collection. **Gate:** ride a scheduled
ferry between two harbors. **Deps:** movement engine (done); independent of
the other end-game systems.

### G25 — Olympiad & hero
Olympiad registration/matches/points/rank; the hero system (monthly heroes, hero
skills/weapons/aura, monument). **Gate:** register for Olympiad, run a match,
compute heroes at period end. **Unblocks:** AdminOlympiad, `//saveolymp`,
`//endolympiad`, `//sethero`/`//givehero`/`//settruehero`. **Deps:** G17
(nobless).

**Audit additions (2026-07):** olympiad observer mode (`ObserverReturn`, the
arena observe teleports, observer state in UserInfo).

### G26 — Seven Signs, Manor & Mammon
Seven Signs cycle (competition/seal periods, Festival of Darkness) + its castle
and dungeon effects; the manor system (seed sowing, crop harvest, castle manor
production/procure); the Mammon merchants (Blacksmith/Merchant of Mammon).
**Gate:** a manor seed can be sown and harvested; the Seven Signs period
advances. **Unblocks:** `//manor`, `//mammon_find`/`//mammon_respawn`. **Deps:**
G24 (siege/castle tie-in), G15 (manor economy).

### G26.5 — Lottery & Monster Race
*(2026-07 audit addition.)* `instancemanager/games/Lottery` (ticket purchase,
weekly draw, prize-claim dialogs) and `MonsterRace` (the Race Track: race
ticks, betting, `MonRaceInfo`). Niche end-game content — schedule last within
the track. **Deps:** G15 (economy).

### G27 — Instances
`InstanceManager` + instance worlds; instance zones; reenter timers; instance-
scoped spawns/doors/reset; the party-enter flow. **Gate:** a party enters an
instance, clears it, and is bound by the reenter timer. **Unblocks:**
AdminInstance, AdminInstanceZone (`//instance*`, `//instancezone`). **Deps:** G21.

### G28 — Events engine & cursed weapons
The event framework (`AbstractEvent` + `EventManager`) with a representative
event (TvT); cursed weapons (Zariche/Akamanah) lifecycle via
`CursedWeaponsManager` (drop, pickup, transformation, karma, decay). **Gate:** a
TvT event runs start-to-finish; a cursed weapon can be dropped and equipped.
**Unblocks:** AdminEvents, `//tvt_*`, AdminCursedWeapons. **Deps:** G20.

**Audit additions (2026-07):** `EventDropManager`/`EventShrineManager`, with
the seven seasonal `scripts/events/*` (SquashEvent, MerrySquashmas, …) as the
breadth list — subject to the scope gate (customs default out).

---

## Track F — Social, comms, moderation & support

### G29 — Summons, pets, servitors, cubics, agathions
Summon skills + servitor AI; pets (summon items, food/feed, pet inventory,
persistence, evolution); cubics; agathions; the pet/servitor party-window
packets. **Gate:** summon a servitor that follows and attacks; summon a pet, feed
it, and it persists. **Unblocks:** AdminEditChar `//summon_info`/`//show_pet_inv`/
`//summon_setlvl`/`//unsummon`, `//fullfood`. **Deps:** G19, G20.

### G30 — Mail, community board & party matching
`MailManager` (compose/read/attachments/return); `communitybbs` (the BBS pages);
party matching rooms; command channels (MPCC); block list (wired into every
whisper/trade/invite check); tactical signs. **Gate:** send mail with an
attachment; open the community board; create a matching room. **Unblocks:**
AdminBBS. **Deps:** G18 (clan board).

**Community board progress:** the custom board (`CustomCommunityBoard = True`)
is live — home/navigation, `_bbsheal`/`_bbsteleport`/`_bbsbuff`, `_bbspremium`
(account-premium buy), and the scheme buffer (`_bbs_buff_scheme_*` +
`buffer_schemes` persistence) have landed. The three handlers that sit
outside `HomeBoard` are now ported too: the `FavoriteBoard`
(`_bbsgetfav`/`bbs_add_fav`/`_bbsdelfav_`, backed by the `bbs_favorites` table,
memory-first mirror + write-through like the buffer schemes), the
`HomepageBoard` (`_bbslink` → `homepage.html`), and the `DropSearchBoard`
(`_bbs_search_item`/`_bbs_search_drop`/`_bbs_npc_trace` — item-name search over
a lazily-built drop index, the per-item drop/spoil list at server rates, and a
new `RadarControl` (0xF1) world-map trace; item icons parsed into an `ItemData`
side-map). The **merchant multisell** now works too: `MultisellData` +
`MultiSellList` (0xD0) + `MultiSellChoose` (0xB0) drive `_bbsmultisell` /
`_bbsexcmultisell` (adena/items → product exchange; see the G14 audit note).
Still open on the custom board: `_bbssell` (needs buylist 423, absent on this
dist and unreachable from the shipped htmls) and the enchant/chance/special-item
multisell branches (no CB list uses them). The retail forum boards +
`communitybbs` core stay deferred (the custom nav never links to them).

**Audit additions (2026-07):** the contact list
(`RequestExAddContactToContactList` family) and party adena distribution
(`adenadistribution` packets), per the scope gate.

### G30.5 — Item auction
*(2026-07 audit addition.)* `ItemAuctionManager` + `ItemAuctions.xml` (the
auctioneer NPCs), `RequestBidItemAuction`/`RequestInfoItemAuction`, scheduled
auction periods, winner delivery. **Deps:** G15 (economy); G30 if delivery
goes via mail.

### G31 — Moderation, accounts, petitions & HWID
Per-client IP plumbing (needed by several); punishment/jail (`PunishmentManager`
+ chat/jail/ban types) + say filter/chat bans; petitions (`PetitionManager`);
account/login admin control (the login-link `//setaccess`/ban relay, `//gm*`
account ops); HWID tracking; fake players. **Gate:** jail a player, file and
answer a petition, ban via the login link. **Unblocks:** AdminPunishment,
AdminLogin, AdminHwid, AdminPetition, AdminFakePlayers, and editchar
`//find_ip`/`//find_dualbox`/`//tracert`. **Deps:** IP plumbing.

**Audit additions (2026-07):** GM chat snoop (`//snoop`, `SnoopQuit`),
`AntiFeedManager`, bot-report punishments (`BotReportPunishments.xml`), and
the secondary password (`SecondaryAuthData`, `RequestEx2ndPassword*`) per the
scope gate.

### G32 — Fishing
Fishing skill + rods/lures/bait; the fishing minigame (`FishingManager`); fish
tables; the fishing championship. **Gate:** cast, hook, and land a fish.
**Deps:** G19.

---

## Track G — Finishing

### G33 — Misc parity & finishing sweep
The residuals: game-time clock (CharSelected/UserInfo use 0 today); the
wall-clock **`DailyTaskManager`** and the resets riding on it — notably the
vitality daily (+25 %) / weekly (full) refills deferred from G16, without which
**vitality only ever drains** (`reco.rs`'s `schedule_initial_daily_reset` is the
pattern); `AutoSaveManager` periodic save cadence; precautionary/scheduled restart +
deadlock detector; offline-trader restore; the `//geosave` binary-region
serializer; `NpcNameLocalisationData`/multilang; remaining packets and the last
data loaders; the niche admin tools (AdminFightCalculator, AdminRepairChar,
AdminPForge, AdminMissingHtmls, AdminPcCondOverride); Dockerfile parity. Close
with the file-by-file parity checklist ([PLAN_GAME_SERVER.md](PLAN_GAME_SERVER.md)
§8), plus a mechanical diff of the Java `network/clientpackets` handler list
(298 files) against the Rust opcode table so any packet family that slipped
every milestone surfaces here, and the one-time `Custom/*.ini` enable-flag
audit from the scope gate (2026-07 audit backstop). **Gate:** parity checklist
complete.

---

## Suggested sequencing

The tracks are ordered by leverage and dependency, but a few notes:

1. **G14 first, always.** Item `<stats>` unblock accurate combat, shields, sets,
   and the enchant/augment effects G15 needs — everything downstream is more
   faithful once it lands.
2. **G16 is cheap and unblocks 4 admin handlers** (premium/vitality/points) — a
   good quick win alongside G14/G15.
3. **G19 (effects breadth) is the long pole for combat and content** — grow it
   continuously; G20/G21/G22/G28/G29 all pull from it.
4. **End-game (G23–G28) can be reordered by product priority** — they're
   independent of each other (only shared dep is G18/G21). Pick by what the
   server operator wants live first (sieges vs olympiad vs instances).
5. **G31 needs IP plumbing** — a small cross-cut worth doing early if dualbox/
   moderation tooling is wanted sooner.
6. **G15.5 is the cheapest playability win** — gatekeepers + `/unstuck` are
   small ports that unblock normal play; slot it alongside G15's tail.
