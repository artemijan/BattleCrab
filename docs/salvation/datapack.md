# Interlude Classic (p110) -> Salvation (p140): datapack, DB schema, config, client

Trees compared:
- **I** = Java Interlude reference `/Users/artem/dev/l2/interlude_classic/dist`
- **S** = `/Users/artem/dev/l2/gitlab/L2J_Mobius/L2J_Mobius_05.0_Salvation/dist`
- **R** = Rust port `/Users/artem/dev/l2/l2r_interlude/dist` + `crates/`


## TL;DR

1. **The DB schema is nearly the same.** The two installers share 93 of their tables. Of those, only 3 change a column. S adds 6 tables and drops 3. This is the cheapest axis: one new migration of about 6 tables.
2. **The XML format is mostly the same Mobius dialect.** The parsers under `crates/gameserver/src/data` will mostly load S files. There are about 10 concrete format breaks, listed below. The bigger change is the **content**: 422 of 423 shared item files differ, all 243 skill files differ, and 186 of 187 NPC files differ. All derived logic, tests and hard-coded ids are therefore suspect.
3. **Scripts are a different game.** Only 32 quest directories overlap (I has 202, S has 562). 170 I quests are gone, including all 2nd/3rd class-transfer and Saga quests. That leaves about 530 new quests. AI/areas grows from 17 to 61, bosses from 11 to 31, others from 39 to 79. Instance scripts grow from 0 real ones to 50 (and from 6 to 175 instance XMLs). The Rust port ships 195 quest modules, and **only 28 of them exist in S**.
4. **Handlers (compiled into Rust) change little in kind.** S reorganised the directories. Effects go from 369 to 385, with 19 removed or renamed (mostly `*Attack` -> `*Damage`) and 35 new. Skill conditions go from 121 to 122. Targets stay at 52 (one rename). Items go from 30 to 32.
5. **Config:** `Character.ini` becomes `Player.ini` (168 keys move). `Server.ini`/`LoginServer.ini` split into `Database.ini`, `Threads.ini`, `IdManager.ini` and `Development.ini`. The 35 `AltOly*` keys become `Olympiad*`. There are 13 new ini files. The protocol changes from 110 to 140 and `ServerListType` from `Classic` to `Normal`.
6. **Client:** the vendored `.dat` schema already includes `28_salvation.xml`. However, every tool defaults to the `*_Classic-eu.dat` files, which a mainline Salvation client does not use. The system-message table in `commons` is generated from I's `SystemMessageId.java`: 1,248 ids have new names and 226 ids are new. The client bundle and geodata must be replaced, because S ships **no geodata**.
7. **CUSTOM_DIST_DEVIATIONS.md:** 4 of the 7 entries need re-applying, 1 is already fixed upstream, and 2 lapse.

---

## 1. dist/game/data XML

### 1.1 xsd/ (87 in I -> 97 in S)

- **Only in S (14):** AgathionData, AlchemyData, ClanLevelData, ClanMasteryData, ClanShop, DynamicExpRates, ensoulFees, ensoulOptions, ensoulStones, experienceLoss, karmaLoss, KrateisCube, MonsterBook, UndergroundColiseum, playerTemplate.
- **Only in I (5):** charTemplate (renamed to playerTemplate), FakePlayerVisualData (folded into npcs.xsd `<fakePlayer>`), LevelUpCrystalData (soul crystals removed), pcKarmaIncrease (renamed to karmaLoss), playerXpPercentLost (renamed to experienceLoss).
- **Changed:** the raw diff is large because of CRLF and BOM. Ignoring whitespace and line endings, 22 files really change:
  - skills.xsd: 539 lines. Adds `fromSubLevel`/`toSubLevel` value ranges, `irreplaceableBuff` (spelling fix of `irreplacableBuff`), `replace`/`replacementSkillId`, `skillId10..27`, and drops `cond`.
  - Recipes.xsd: 113, a full restructure.
  - npcs.xsd: 36, the `<fakePlayer>` element.
  - statBonus.xsd: 24 (CHA/LUC).
  - items.xsd: 16, skillTrees.xsd: 10, transformations.xsd: 8, experience.xsd: 8, classList.xsd: 6.
  - The rest change 1 to 4 lines each.

### 1.2 Layout moves (these break Rust's hard-coded paths)

R reads these paths, from greps in `crates/gameserver/src/data` and `config`:

| I path (read by R) | S path |
|---|---|
| `data/skillTrees/**` (99 files) | `data/stats/players/skillTrees/**` (163 files) |
| `data/stats/chars/baseStats/*` (89) | `data/stats/players/templates/{StartingClass,1stClass..4rthClass,Ertheia,BalthusKnights}/*` (155) |
| `data/stats/chars/classList.xml` | `data/stats/players/classList.xml` |
| `data/stats/experience.xml` (lvl 86) | `data/stats/players/experience.xml` (lvl 111) |
| `data/stats/initialEquipment.xml`, `initialShortcuts.xml` | `data/stats/players/…` |
| `data/stats/initialEquipmentEvent.xml` | **gone** |
| `data/stats/chars/pcKarmaIncrease.xml` | `data/stats/players/karmaLoss.xml` (`<modifier level val>`) |
| `data/stats/chars/playerXpPercentLost.xml` | `data/stats/players/experienceLoss.xml` (`<experience level val>`) |
| `data/LevelUpCrystalData.xml` (`soul_crystal_data.rs`) | **gone**; soul crystals are not in Salvation |
| `data/FakePlayerVisualData.xml` | gone; it is now `<fakePlayer …>` inside the npc xml |
| `data/skillTrees/subPledgeSkillTree.xml` | gone |
| `config/ClanReward.xml` | gone (ClanShop.xml added) |
| `data/geodata/*.l2j` (227 tiles) | **empty; S ships no geodata** |

### 1.3 Core data: counts and structure

Counts use `<x id=` occurrences. "Shape" is the set of distinct element and attribute names, from `xmlshape.py`.

| Data | files I -> S | entities I -> S | shape change |
|---|---|---|---|
| stats/items | 439 -> 432 (1 identical) | 49,103 -> 49,980 items | `<cond msgId addName>` renamed `<conditions msgId addName>` (2,126 -> 2,196 uses). New attributes `player@MaximumVitalityPoints` and `skill@subLevel` |
| stats/skills | 246 -> 248 (0 identical) | 20,601 -> 21,300 skills | Same `cond` -> `conditions` rename. **Conflict:** S skills now use `<conditions>` for two things: 3,676 skill-condition blocks and 180 legacy `msgId` blocks. R's `skill_data/mod.rs` treats `"conditions"` as the skill-condition scope, so the parser must tell the two apart by the `msgId` attribute. SubLevel ranges go from 2,961 to 69,264 refs (R already parses `fromSubLevel`/`toSubLevel`). New elements `irreplaceableBuff`, `replace`, `replacementSkillId`. Effect names used go from 335 to 354, with 38 names only in S (below) |
| stats/npcs | 191 -> 196 (1 identical) | 15,828 -> 15,747 npcs | New `<fakePlayer classId equip* face hair…>` replaces `status@fakePlayer` and FakePlayerVisualData. New `<lucky>` element |
| spawns | 154 -> 209 (1 identical, 117 I-only files) | 27,155 -> 44,799 `<npc>` | S uses `<locations><location chance>` (2), `<minions>` (1), `<banned_territory>` (4), `spawnByDefault` (46). `spawn_data.rs` explicitly skips locations, minions and banned_territory ("0 occurrences"), so a small port is needed |
| instances | 6 -> 175 | 238 -> 17,829 nodes | Adds `parameters`, `skill`, `territories`/`territory`, `banned_territory`, `minions`, `node`, `location@name/heading`, `npc@count/respawnTime/spawnAnimation`, `reenter@apply`, `reset@time`, `condition@showMessageAndHtml`. `instance_data.rs` covers only what the 6 Interlude files use |
| teleporters | 116 -> 142 | | New `location@feeId`, `@name`, `@questZoneId` |
| multisell | 101 -> 498 | 33k -> 128k nodes | New `ingredient@enchantmentLevel` |
| buylists | 481 -> 893 | | no shape change |
| zones | 40 -> 44 (29 identical) | | no shape change. New files: balthus_knight, ctf_event_zone, sayune_zones, teleportzones |
| residences | castles 9/9, clanHalls 9 -> 10 | | no shape change |
| skillTrees | 98 -> 163 | 14.6k -> 32.8k nodes | New `removeSkill`, `skill@row/column/pointsRequired/treeId/learnedByFS/getDualClassLevel`, `skillTree@race/subType`. New trees: ability, alchemy, awakeningSave, dualClass, revelation, BalthusKnight, Ertheia, 4rthClass |
| player templates | 89 -> 155 | | New `baseCHA`, `baseLUC`, `basePSkillCritRate` |
| Recipes.xml | 1 file, 5,956 -> 23,487 nodes | | **Full restructure**: `<item recipeId craftLevel…><ingredient/><production/>` becomes `<recipe id itemId level isCommonRecipe…><materials/><product/><npcFee/><stat/>`. `recipe_data.rs` needs a rewrite |
| hennaList.xml | | 13.8k -> 20.7k nodes | New `<duration time>`, `<skill>`, `henna@isPremium` |
| DoorData.xml | | 11.7k -> 14.8k nodes | New `status@isCheckCollision`, `@isStealth` |
| CategoryData / EnchantItemData / EnchantItemGroups / ActionData / classList / initialEquipment / initialShortcuts / armorsets / pets / cubics (21 -> 25) / transformations / augmentation | | | no shape change; content only. EnchantItemGroups still ships only the 4 groups, so the accessory-ceiling deviation still applies |
| EnchantSkillGroups.xml | | 481 -> 321 nodes | Same shape, but it is keyed by sub-level in S (enchant routes become skill sub-levels 1001+) |
| DailyMission.xml / PrimeShop.xml | | empty -> 169 / 222 nodes | Now populated; R has no loaders for them |
| html | 2,712 -> 3,703 files (423 identical) | | Script html grows from 6,984 to 10,881 files |

**Effect names used only in S's skills/items (38):** AgathionSlot, BonusDropAdena, BonusRaidPoints, CallLearnedSkill, CheatDeath, CreateCommonItem, CreateItem, Crystallize, Distrust, EnchantRate, EnergyDamage, FishingExpSpBonus, FlipBlock, LimitSkill, MagicalAbnormalDispelDamage, MagicalDamage, MagicalDamageByAbnormal, MagicalDamageByAbnormalSlot, MagicalDamageMp, MagicalDamageOverTime, MagicalDamageRange, MagicalSoulDamage, MagicalSuccessResistance, PhysicalDamage, PhysicalDamageHpLink, PhysicalDamageMute, PhysicalDamageSaveHp, PhysicalDamageWeaponBonus, PhysicalSoulDamage, RemoveEquipPenalty, ReplaceSkillBySkill, Ride, TrackLimitedSkill, TriggerSkill, TriggerSkillByDamageDealt, TriggerSkillByDamageReceived, TriggerSkillBySkillDamage, WorldChatPoints. About 17 of these are renames of I's `*Attack`/`TriggerSkillBy*` handlers. Skill-condition names go from 93 to 94; targetType (21) and affectScope (20) are unchanged.

### 1.4 New data files only in S

| File | What it is |
|---|---|
| `AgathionData.xml` (3,773 lines) | Agathion charm items: per-enchant `mainSkill`/`subSkill`. Goes with the AgathionSlot effect and agathion paperdoll slots |
| `AlchemyData.xml` (1,220) | Alchemy (Ertheia-era crafting) skill recipes: ingredients and products per alchemy skill |
| `ClanLevelData.xml` (18) | Clan level -> exp and member caps. Replaces Feature.ini's `ClanLevelNCost/Requirement` and `LevelUpNNReputationScore` keys; clan level-up becomes exp-based |
| `ClanMasteryData.xml` (34) | Clan "Specialty" mastery tree (Salvation clan rework): skills unlocked by clan level and reputation |
| `MonsterBook.xml` (961) | Monster Collection cards: kills -> exp/sp/points per faction. Goes with the MonsterBook AI and new packets |
| `KrateisCube.xml` (271) | Krateis Cube PvP event arenas: doors, wait and spawn points, level brackets |
| `UndergroundColiseum.xml` (57) | Underground Coliseum arenas: towers, spawn doors, level brackets. Plus `config/UndergroundColiseum.ini` and the `underground_coliseum` table |
| `stats/ensoul/{ensoulOptions,ensoulStones,ensoulFees}.xml` | Soul crystal "ensoul" system (replaces I's LevelUpCrystal); the `item_special_abilities` table |
| `config/DynamicExpRates.xml`, `config/ClanShop.xml` | Per-level exp/sp multipliers; clan fame shop |

---

## 2. dist/game/data/scripts

| subdir | I java files (dirs) | S java files (dirs) | overlap |
|---|---|---|---|
| quests | 217 (202) | 586 (562) | **32 dirs overlap**: 170 I-only, ~530 S-only. All 32 shared ones changed (e.g. Q620 FourGoblets, 1,219 diff lines) |
| ai/areas | 17 | 61 | 15 shared; I-only: DwarvenVillage, LairOfAntharas |
| ai/bosses | 11 | 31 | 10 shared; I-only: DrChaos. New: Anais, Anakim, Balok, Baylor, Beleth, Freya, Helios, Istina, Kelbim, Lilith, Lindvior, Octavis, Ramona, Spezion, Tauti, Teredor, Tiat, Trasken… |
| ai/others | 39 | 79 | 35 shared; I-only: NewbieGuide, PolymorphingAngel, RandomWalkingGuards, ToIVortex. Only 3 shared AI dirs are byte-identical (DragonValley, Zaken, Mammons); bosses differ by 450–1,130 lines each |
| ai total | 83 | 341 | |
| instances | 1 (AbstractInstance only) | 75 (50 dirs) | Kamaloka, Pailakas, SSQ*, Fortuna, Nursery, ChambersOfDelusion, CeremonyOfChaos, KartiasLabyrinth, … |
| events | 7 | 20 | 3 shared (EveTheFortuneTeller, HappyHours, RudolphsBlessing, ThePowerOfLove) |
| village_master | 17 | 5 | All the `*Change1/2` class-transfer and FirstClassTransferTalk scripts are gone; ProofOfCourage and ProofOfJustice are new |
| custom | 14 | 18 | I-only: EchoCrystals, NpcLocationInfo. S-only: ShadowWeapons, Transmog |
| features | 0 | 1 | SkillTransfer |
| vehicles | 4 | 9 | Gludin/Talking/Giran boats removed or renamed; airships, Keucereus, SoD and SoI controllers added |
| handlers | 788 | 807 | see below |

**I-only quests (170)** include Q070–Q100 (all 31 Sagas), Q211–Q235 (2nd-class trials and testimonies), Q255 Tutorial, the Q257–Q350 newbie and mid-level repeatables, Q127 Fishing, and others. **R implements 195 quest modules (`crates/gameserver/src/scripts/q*`), and only 28 of those ids exist in S.** The remaining ~167 Rust quests, plus `_SagaShared` and the class-change village masters, become dead code. Roughly 530 S quests would need porting.

### Handlers (compiled into Rust)

S reorganised the directories but changed little else:

| Interlude dir | Salvation dir | I count | S count | delta |
|---|---|---|---|---|
| effecthandlers | skill/effects | 369 | 385 | 350 shared names; 19 I-only (EnergyAttack, MagicalAttack*, PhysicalAttack*, PAtk, MAtk, MagicalSoulAttack, PhysicalSoulAttack, ResistDDMagic, TriggerSkillByAttack/ByDamage/BySkillAttack); 35 S-only (the `*Damage` renames + AgathionSlot, ApplySkillEffects, BonusRaidPoints, CheatDeath, CreateItem/CreateCommonItem, Crystallize, DispelCaster, Distrust, FlipBlock, LimitSkill, MagicalSuccessResistance, RemoveEquipPenalty, ReplaceSkillBySkill, Ride, TrackLimitedSkill, TriggerSkillWithDelay, TriggerSkillByDamageDealt/Received, TriggerSkillBySkillDamage) |
| skillconditionhandlers | skill/conditions | 121 | 122 | +OpNotInEventSkillCondition |
| targethandlers | skill/targets | 52 | 52 | BalakasScope renamed ValakasScope |
| itemhandlers | items | 30 | 32 | +EventItem, +PaulinasSupportBox |
| admin/user/voicedcommandhandlers | chat/commands | 109 | 111 | −AdminBBS, AdminMobGroup, AdminPcCondOverride, SetVCmd; +AdminAugment, AdminGoto, AdminHelp, AdminRegions, ClanHandler, Wedding; AdminZones renamed AdminZoneBuild, CastleVCmd renamed CastleHandler |
| chathandlers | chat/channels | 14 | 14 | |
| bypasshandlers + communityboard | bypass/npc + bypass/communityboard | 24 + 9 | 21 + 9 | −ChangePlayerName, Loto, QuestLink, SupportBlessing, SupportMagic; +EnsoulWindow, ScriptLink |
| actionhandlers / actionshifthandlers / playeractions | actions/click, shiftclick, user | 10/6/24 | 10/6/24 | |
| dailymissionhandlers | dailymissions | 9 | 10 | +CeremonyOfChaos |
| punishmenthandlers | punishments | 3 | 3 | |
| conditions | conditions | 3 | 3 | |

The Rust effect implementations live in `crates/gameserver/src/game_loop/skills/effects/*` (14 modules). The renamed handlers mostly need aliases or renames plus checks for formula changes. Roughly 18 genuinely new effects are needed.

---

## 3. Database

Sources: I `dist/db_installer/sql/{mariadb,sqlite,postgresql}/{game,login}` (94 + 4 files; this is the user's fork with SQLite and Postgres variants). S `dist/sql/game` (97 files) and `dist/sql/login` (4), **MySQL only**: the Driver is `com.mysql.cj.jdbc.Driver` and `libs/` contains only mysql-connector-j 9.5.0. R's schema is `crates/migration` (baseline_login, baseline_game with 96 tables, master_accounts, grandboss_real_hp, ip_bans; 101 tables in all) plus 103 entities in `crates/models/src/entity`.

Compared with `sqldiff.py`, ignoring CRLF:
- **Tables:** 96 in I, 99 in S.
- **Dropped in S (3):** `custom_teleport`, `global_tasks`, `lottery`. R actively uses `lottery` (`game_loop/activities/lottery.rs`, `db/*/minigames.rs`) and `global_tasks` (`game_loop/tasks.rs`). Keep them. Lottery is an Interlude feature with no S counterpart.
- **New in S (6):**
  - `character_offline_play(charId, type, id)`
  - `character_offline_play_group(leaderId, charId, type)`
  - `character_transmogs(owner varchar45, itemId; PK both)`
  - `item_special_abilities(objectId, type, optionId, position; PK objectId+optionId)`, for ensoul
  - `mods_wedding(id AI, player1Id, player2Id, married, affianceDate, weddingDate)`
  - `underground_coliseum(arenaId PK, leader, wins)`
- **Changed columns (3 tables):**
  - `buffer_schemes.skills`: varchar(200) -> varchar(512)
  - `character_shortcuts.level`: mediumint -> `mediumint signed not null default 0`
  - `olympiad_fights.classed`: tinyint(4) -> tinyint(1) unsigned
- **Everything else is column-identical.** That includes `characters`, `items`, `character_skills(_save)` (both already have `skill_sub_level`), `character_subclasses` and `clan_data`. Both trees descend from the same Mobius trunk.
- **Seed data differences:**
  - `grandboss_data` gains Lindvior 29240, Trasken 29197, Beleth 29118, Anakim 25286, Lilith 25283, Kelbim 26124 and Helios 29305. Dr. Chaos 25512 is gone.
  - `fort_siege_guards` goes from 0 to 3,497 rows and `fort_spawnlist` from 0 to 350. **Fortresses are live in S.** R has the tables and entities but no fortress gameplay.
  - The `fort` row for 113 Cloud Mountain is commented out in S.
  - `gameservers` seeds server id 2.
  - R's migrations contain **no seed INSERTs**, so wherever R currently seeds castles and grand bosses, the S rows need to be added there.
- **Login:** S `accounts` has `login` as PK NOT NULL and no `is_verified`. R's `master_accounts` migration (nullable login, email masters) is a Rust-only extension, so keep R's version. Nothing else changes.
- **Effort:** one new migration (6 `CREATE TABLE IF NOT EXISTS` plus 3 column alters; SQLite needs table rebuilds, following the `grandboss_real_hp.rs` pattern), 6 entities, and parity tests in `crates/models/tests/schema_parity.rs`. MySQL versus SQLite is irrelevant to R, because R owns its schema and dialect (`migration/src/dialect.rs`). Do not adopt S's MySQL defaults.

---

## 4. Config (.ini)

### File list (game/config)

- **Only in I:** `Character.ini` (to Player.ini), `ClanReward.xml`, `Custom/PcCafe.ini` (merged into PremiumSystem.ini), `ipconfig-default.xml` (to ipconfig.xml).
- **Only in S:** BalthusKnights.ini, CeremonyOfChaos.ini, ClanShop.xml, Custom/CancelReturn.ini, Custom/Captcha.ini (in-game anti-bot captcha, separate from R's Turnstile), Custom/FreeMounts.ini, Custom/StartingTitle.ini, Custom/Transmog.ini, Custom/Wedding.ini, Database.ini, Development.ini, DynamicExpRates.xml, IdManager.ini, Player.ini, Threads.ini, UndergroundColiseum.ini.
- **login/config:** `LoginServer.ini` splits into `Server.ini` (20 keys), `Database.ini` (10 keys) and `Threads.ini` (2 keys).

### Keys (game): I has 1,311, S has 1,424, 1,211 in common

- **Moved:** Character.ini to Player.ini (168); Server.ini to Database.ini (10) and Threads.ini (3); General.ini to Development.ini (6) and IdManager.ini (1); PcCafe into PremiumSystem (9).
- **Renamed:**
  - 35 Olympiad keys `AltOly*` become `Olympiad*`
  - `DeadLockDetector`/`DeadLockCheckInterval` become `DeadlockWatcher`/`DeadlockCheckInterval`
  - `MinimumDelevel` becomes `MimimumDelevel` (upstream typo)
  - `AltDev*` become `NoQuests`/`NoSpawns`/`Show*LoadInLogs`
- **Removed (I-only, ~100):**
  - Feature.ini clan-level costs and reputation-by-level (26), now ClanLevelData.xml
  - Lottery keys (7). R implements the lottery, so keep these
  - Character `AltGameCreation*`, `CraftMasterwork*`, `MaxNewbieBuffLevel`, `MaxEquipableItemGrade`, `InitialEquipmentEvent`, `AbilityMaxPoints`
  - Network cached-thread keys
  - `RateQuestDrop`
- **New (S-only, ~210):**
  - GrandBoss.ini: 40 new-boss keys
  - Network.ini: 20 BufferPool keys
  - Player.ini: 16 keys (`Hardin*`, `AltDualClassWithoutQuest`, `EnableAlterSkills`, `EnableFameSystem`, `MaxPSkillCritRate`, `ShowIntroVideo`, …)
  - CeremonyOfChaos (10), Captcha (10), Wedding (9), UndergroundColiseum (5), Transmog (5), IdManager (5)
  - Rates: `LuckyChanceMultiplier`, `MonsterExpMaxLevelDifference`, `QuestItemDropAmountMultiplier`, `RateQuestRewardFP`
  - General: `MultisellAmountLimit`, block checker, `ExaltedFor*`
  - Feature: `ClanContribution*`
- **Changed defaults:** 173 common keys. Most notable: `AllowedProtocolRevisions` 110 -> 140, `ServerListType` Classic -> Normal, `MaximumPlayerLevel` 80 -> 110, `MaxSubclass` 5 -> 3, `BaseDualclassLevel` 80 -> 85, `AltPartyMaxMembers` 9 -> 7, `MaxBuffAmount` 24 -> 20, `StartingVitalityPoints` 0 -> 140000, `WorldChatMinLevel` 40 -> 95, and `PacketEncryption` True -> False.
  - **Most of the other ~150 differences are the operator's own customisations in I:** rates x10, champions on, autoloot, premium, banking, PvP colours, GM flags, x2 monster stats, and so on. These are not chronicle changes, and they must be re-applied by hand to S's ini set.
- **R's own config additions** (keep them, then map them onto the new file split): `Dashboard.ini`, `Logging.ini`, `Monitor.ini`, `Security.ini`, `ipconfig.xml`, `StrictDelevelSkillRemoval` (Character.ini, which becomes Player.ini), `InternalStatusBindAddress/Port` (LoginServer.ini), and the operator values below.
  - General: `HtmCache=True`, `GMStartupSilence=True`
  - NPC: `AltAttackableNpcs=False`, `AggroDistanceCheckEnabled=True`, `VitalityConsume*=0`
  - Rates: `VitalityMaxItemsAllowed=0`
  - `UnstuckInterval=300`
  - `AutoCreateAccounts=False`
- **R's config loaders** hard-code `config/Character.ini`, and `Server.ini` for the DB URL. Choose one: follow S's split (Player.ini, Database.ini, Olympiad renames), or keep R's file names and treat S's ini set as reference only. Recommendation: adopt S's names. That keeps the "dist is the spec" rule intact.

---

## 5. Client side

- `dist/client/system` holds 424 files, all `Lineage2Ver413`. It is the Classic client: 67 `*_Classic*` files plus the mainline copies (`NpcName-eu.dat`, `SystemMsg-eu.dat`, …).
- `crates/tools` dependencies:
  - The `.dat` schema set (`dist/client/structure/structure/*.xml`) already contains `28_salvation.xml` (alongside 06_interlude, 25_underground, 26_helios, 27_grand_crusade). Layout auto-detection should therefore work on a Salvation client.
  - **CLI defaults are hard-wired to Classic file names:** `SystemMsg_Classic-eu.dat` in gen_messages, sync_messages and msg_color, and `NpcName_Classic-eu.dat` in sync_npc. `dat_schema.rs:647` matches `Armorgrp_Classic(Aden)?.dat`. Salvation uses the non-Classic files, so these defaults need switching.
  - `gen-messages` reads `../interlude_classic/java/.../SystemMessageId.java`. Salvation's has 5,606 entries against 5,380. All 5,380 I ids exist in S, but **1,248 of them carry different names** and 13 names moved id. Regenerating `crates/commons/src/system_messages/generated.rs` will rename constants that R code references. Expect a compile-error sweep.
  - `sync-npc` was used to align NPC names with the Classic client. About 76 `stats/npcs` files in R differ from I only by those renames, so it has to be re-run against the Salvation client.
  - `utx`/`icon_atlas`/`SysTextures_patched` and the asset viewer read SkeletalMesh v131 (see memory). The Salvation client package versions may differ, so re-verify.
- **The launcher** (`crates/launcher/src/install.rs`) downloads `client.7z` and expects `L2_Client/system/l2.exe`, so it needs a new client bundle. The patched client edits made so far (QuestName entry for Q214, recoloured SystemMsg, SysTextures patches) must be redone or dropped.
- **Geodata:** S ships an empty `data/geodata` (readme only). The Salvation world reworked several regions (Talking Island, Gludio, plus new areas: Gainak, Atelia Fortress, Ertheia lands), so I's 227 `.l2j` tiles are not valid. New Salvation geodata has to be sourced. The roughly 176 spawn-pocket z-fixes made with `spawn-pockets` in I/R spawns do not carry over, and the tool should be re-run against S spawns with new geodata.

---

## 6. CUSTOM_DIST_DEVIATIONS.md: status on Salvation

| Deviation | S status | Action |
|---|---|---|
| Cruma Tower entrance serves 3rd floor only (`teleporters/others/CrumaTower.xml` + `html/teleporter/30483.htm`) | S still ships both destinations (index 0 = 2nd floor) | **Re-apply**, including the index trap in the html. Note that S's 3rd-floor coordinate is `17729,114808` (I's was `17726,114838`) |
| Q214 "Reinforced Gargoyle" html | **Lapses**: Q214 does not exist in S (all 2nd-class trials removed) | Drop the entry and its test |
| Advanced HQ takes half damage | S `SiegeFlagStatus.reduceHp` still has the `value/2` + `value` bug | **Re-apply** (Rust code, not data). Keep the test |
| TvT servitor thaw | **Already fixed upstream**: S `TvT.java:637` sets `summon.setInvul(false)` etc. | Entry becomes "matches upstream"; the test stays valid |
| `StrictDelevelSkillRemoval` key | S has no such key, and `DecreaseSkillOnDelevel` moved to `Player.ini` | **Re-apply** in Player.ini |
| Over-enchant: missing accessory ceiling is not zero | S `EnchantItemGroups.xml` still has only ARMOR, FULL_ARMOR, FIGHTER_WEAPON and MAGE_WEAPON groups; `OverEnchantProtection=True` and `OverEnchantPunishment=JAIL` are now in Player.ini | **Re-apply** (Rust code). Re-check that the derived weapon and armor ceilings are still 29 under S's tables |
| `HtmCache = True` | S ships `False` | **Re-apply** in General.ini |

**Undocumented R-local datapack edits** that a re-sync would also lose (from `r_vs_i.txt`, 181 files):
- 78 `html/villagemaster/*.htm`: Shadow Weapon coupon button un-commented (commit 433137aa), plus the `scripts/custom/ShadowWeapons` and `multisell/306893001-3.xml` additions. S already has `custom/ShadowWeapons`, so check its multisell ids.
- 16 quest htmls: the Q33–Q37 Formal Wear chain "restored to authentic Interlude" levels (db26b8db). These quests exist in S with their own content, so decide again.
- About 76 npc and 2 skill/item name syncs to the Classic client.
- 2 admin help htmls.

Consider adding these to the deviations doc before migrating.

---

## Effort and ordering (datapack/DB/config axis only)

1. **Copy S dist alongside, config first (S):** adopt S's ini layout (Player/Database/Threads/IdManager/Development, Olympiad renames), port R's extra keys and files into it, and re-apply the operator values plus the HtmCache and StrictDelevel deviations. About 1–2 days.
2. **DB migration (S):** 6 new tables, 3 alters, entities, seed rows (grandboss, fort). About 1 day.
3. **Loader path and format fixes (M):** the path moves in §1.2; the `cond` -> `conditions` rename with the msgId split; Recipes.xml rewrite; karmaLoss/experienceLoss; `<fakePlayer>`; henna duration/skill; teleporter `feeId`/`name`; skillTree new attributes; templates for 155 classes; drop LevelUpCrystal and soul crystals. Gate all of this behind "boot loads S dist with zero warnings". About 1 week.
4. **Effects, conditions, targets (M):** rename about 17 handlers and implement about 18 new effects. Then **re-baseline every data-driven test**, because item, skill and NPC numbers all moved.
5. **Spawns and instances (M–L):** implement `locations`/`minions`/`banned_territory` and the full instance XML feature set (175 templates).
6. **Client and tools (S–M, parallel):** new client bundle, switch tool defaults to non-Classic dats, regenerate system messages (the 1,248-name sweep), re-run sync-npc, source geodata, re-run spawn-pockets.
7. **Re-apply deviations** (Cruma, AdvHQ, over-enchant, HtmCache, StrictDelevel). Drop Q214 and reclassify TvT.
8. **Scripts (XL, open-ended):** about 530 quests, 260 AI, 50 instance scripts and new systems (agathions, alchemy, ensoul, clan mastery, monster book, Ceremony of Chaos, Krateis, Coliseum, fortresses, dual class/awakening). Of R's 195 quests, 167 become obsolete. This dwarfs everything above.
