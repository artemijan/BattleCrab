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
| G16 | Character variables, premium & vitality | Foundations | `//premium*` `//pccafepoints` `//primepoints` `//set_vitality_level` | — |
| G17 | Sub-classes, class change & nobless | Progression | `//setnoble` `//setsubclass` (editchar) | G22¹ |
| G18 | Clans — full | Progression | `//clan_*` `//pledge` `//add_clan_skill` | G15 |
| G19 | Skills & effects breadth | Combat | `//ave_abnormal` `//setteam` `//settargetable` `//para` `//playmovie` … (AdminEffects) | — |
| G20 | Combat breadth | Combat | — | G14, G19 |
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

🚧 **Henna slice landed (2026-07-19)** — plan [PLAN_G16_HENNA.md](PLAN_G16_HENNA.md).
`data/henna_data.rs` (`HennaData` — 372 dyes) + a `HennaSlots` component whose
dye stat bonuses are folded into `BaseStats` (`= template + Σ worn dyes`,
recomputed on draw/remove, so the finalizers + UserInfo panel pick henna up with
no special-casing). `character_hennas` load/persist; the `RequestHenna*` packet
family (equip/remove/item-info/item-remove-info/item-list/remove-list) +
`HennaInfo`/`HennaEquipList`/`HennaRemoveList`/`HennaItemDrawInfo`/
`HennaItemRemoveInfo`; the SymbolMaker `Draw`/`Remove` bypass; `HennaInfo` in the
enter-world burst; empty-slot count from class level (`*_CLASS_GROUP` → 0/2/3).
Interlude dyes are permanent (`duration=-1`), so the timed-henna scheduler +
`HennaDuration` variables + LUC/CHA + dye skills are out of scope. **Remaining
for G16:** `character_variables` store, full vitality, premium gameplay effects.

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

### G18 — Clans (full)
Everything past G11's creation slice: invite/join/leave/oust/dissolve; clan
level-up + reputation; sub-pledges (royal guard / order of knights) + academy;
clan skills + `PledgeSkillList`; crests (pledge/ally/large); notices; clan
warehouse; clan wars; alliances; the `PledgeInfo`/`PledgeStatusChanged`/RELATION
breadth. **Gate:** form a clan, invite members, level it, learn a clan skill,
declare war, form an ally. **Unblocks:** `//clan_*`, `//pledge`,
`//add_clan_skill`/`//give_clan_skills`. **Deps:** G15 (clan warehouse).

**Audit additions (2026-07):** clan recruitment/entry (`ClanEntryManager` — the
`RequestPledgeRecruit*`/draft-list/waiting-list packet family) and the Classic
pledge-bonus rewards (`ClanRewardData`, `pledgebonus` packets).

---

## Track C — Combat, skills & AI breadth

### G19 — Skills & effects breadth
Grow `EFFECT_REGISTRY` toward the 369 Java effect classes and the 230-entry
`Stat` enum on demand; toggle-type skills; the remaining `AcquireSkillType`s
(pledge/transform/transfer/subclass/collect/…); `calcMagicSuccess`
(`ALT_GAME_MAGICFAILURES`); AoE affect scopes (only `SINGLE` resolves today);
buffs/effects on NPC targets; the **abnormal-visual-effect** runtime + per-
creature team / targetable / display-effect state; `ExAbnormalStatusUpdateFrom
Target`. **Gate:** a debuff lands on a mob, an AoE nuke hits a cluster, a toggle
skill switches on. **Unblocks:** the AdminEffects AVE subset (`//ave_abnormal`,
`//setteam`, `//settargetable`, `//para*`, `//bighead`, `//playmovie`,
`//set_displayeffect`, `//event_trigger`), `//switch_gm_buffs`.

**Audit additions (2026-07):** skill enchanting (`EnchantSkillGroupsData` +
`RequestExEnchantSkill`/`Info`/`InfoDetail` — the level-76+ skill-enchant flow).

### G20 — Combat breadth
`PhysicalAttack`-type skills; bows/crossbows (arrows, reuse gauge); dual-weapon
split hits; polearm sweep; PvP auto-attack + the karma/PK/flag consumers; overhit
XP; the `SHOTS_BONUS` dynamic value; the rest of `isMovementDisabled`
(root/immobilize). **Gate:** a bow attack consumes an arrow, a polearm hits a
line, PvP flagging drives auto-attack, a physical skill lands. **Deps:** G14, G19.

**Audit additions (2026-07):** duels (`DuelManager`/`Duel` — 1v1 and party
duels, `RequestDuelStart`/`AnswerStart`/`Surrender`, end conditions + the arena
teleport variant). G25's olympiad matches reuse this shape, so duels land here.

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

---

## Track D — Content

### G22 — Quest & script breadth
The remaining ~188 quests, ~14 village-master scripts and ~81 `ai/` scripts;
daily quests (`restartTime`); the tutorial (Q00255); `onFirstTalk`; the
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
The residuals: game-time clock (CharSelected/UserInfo use 0 today);
`AutoSaveManager` periodic save cadence; precautionary/scheduled restart +
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
