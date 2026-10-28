# Engine / model core: Classic Interlude → Salvation (protocol 140)

Scope: `java/org/l2jmobius/gameserver/**` minus `network/`, minus datapack scripts (effect/condition/target handler sets are noted because they are engine contract).

- **I** = `/Users/artem/dev/l2/interlude_classic/java/org/l2jmobius/gameserver` (the user's tree: Mobius Classic Interlude snapshot from 2023-10-16 (`1fa7580b`) plus 162 local commits. Those commits changed 313 engine files, +13,058/−10,095 lines, including the DB dialect layer.)
- **S** = `/Users/artem/dev/l2/gitlab/L2J_Mobius/L2J_Mobius_05.0_Salvation/java/org/l2jmobius/gameserver` (upstream HEAD `368d1d560b0`, 2026-07-01)

## 0. Headline finding: most of the delta is upstream drift, not the chronicle change

The I tree and S tree are not "Interlude vs Salvation" in the Java sense. Both run the same GoD-era Mobius engine. Classic Interlude is built on the Classic/Salvation engine. Most of the textual difference comes from about 2.7 years of upstream Mobius refactoring between the 2023 snapshot and 2026 HEAD:

| Comparison (engine, excluding network/) | Shared files | Normalized changed lines | Only-in-Salvation files |
|---|---|---|---|
| **I (2023 + local) vs S (2026)** | 676 by path + 387 moved | **20,767** | 559 by path (most are moves) |
| current upstream Classic_1.0 vs S | 1,168 | 3,676 | 67 |
| current upstream Classic_2.0_Saviors vs S | 1,184 | 3,697 | 51 |
| current upstream Classic_3.0_TheKamael vs S | 1,184 | 5,259 | 51 |

"Normalized" means the comparison ignores CRLF, license header, imports, package lines, comments, braces and whitespace. It also maps the systematic renames: `XxxConfig.` → `Config.`, `getActingPlayer`→`asPlayer`, `ClassId`→`PlayerClass`, `CtrlIntention.AI_INTENTION_X`→`Intention.X`, `CtrlEvent.EVT_`→`Action.`, `Util`/`CommonUtil`→`LocationUtil`/`ArrayUtil`/`MathUtil`, `isBad`→`hasNegativeEffect`, `ShortCuts`→`Shortcuts`, `QuestManager`→`ScriptManager`, and so on.

**So:**
- The real chronicle delta is about 50 new engine files and about 3.7k changed lines in about 130 files.
- About 80% of the I→S diff is upstream drift. Most of that drift is renames and package moves, which the Rust port does not mirror.
- A minority of the drift changes behavior, notably Formulas crit math, shots, AI parameterized casts, recipes and skill hashing. If Salvation Java becomes the new spec, those behavior changes must still be ported.
- Recommended diff base for planning: `cdiff_Classic_2.0_Saviors.txt` for the chronicle delta, plus a targeted pass over the behavioral drift listed in §2.

## 1. File-level classification

### 1.1 Counts (engine only)
- I engine files: 1,207. S engine files: 1,235.
- Interlude-only paths: **531**
  - **387** are the same class name relocated inside the engine.
  - **24** moved into S `network/` (enums/holders).
  - About **120** are renamed or merged equivalents or were genuinely removed (see 1.3).
- Salvation-only paths: **559**
  - **387** are relocations.
  - **64** come from the Config split (`config/*.java` 21, `config/custom/*.java` 43).
  - About **108** are renames or new features (see 1.4).

### 1.2 Package moves (pure relocation, same basename). Upstream refactor, not chronicle.
| From (I) | To (S) | # |
|---|---|---|
| model/events/impl/creature/player | model/events/holders/actor/player | 60 |
| instancemanager | managers | 35 |
| taskmanager | taskmanagers | 21 |
| model/events/impl/creature/npc, /creature | model/events/holders/actor/npc, /creature | 19+15 |
| model/holders | data/holders, model/item/holders, model/actor/holders/{player,npc,creature}, model/skill/holders | 18+9+7+7+2+4 |
| enums (113 files) | model/actor/enums/{player,creature,npc}, model/skill/enums, model/item/enums, model/clan/enums, model/siege, data/enums, model/groups, network/enums | 18+9+6+15+7+5+6+3+… |
| model/quest | model/script | 7 |
| model/itemauction | model/item/auction | 7 |
| model/matching, model/Party, CommandChannel | model/groups(/matching) | 6 |
| model/events/timers | model/script/timers | 3 |

### 1.3 Interlude-only files that are not plain moves

**Renamed or merged equivalents (the function still exists in S):**
- `enums/ClassId` → `model/actor/enums/player/PlayerClass`. Same 153 entries plus `EVISCERATOR_BALTHUS(190)` and `SAYHA_SEER_BALTHUS(191)`. **I already contains Ertheia and all awakened classes (ids 139–189).**
- `ai/Ctrl`, `CtrlIntention`, `CtrlEvent` → `ai/Intention` (IDLE, ACTIVE, …) and `ai/Action` (THINK, ATTACKED, …). Every `onEvtX` became `onActionX` in AbstractAI, CreatureAI, AttackableAI, PlayerAI and SummonAI.
- `handler/ActionHandler`, `IActionHandler`, `PlayerActionHandler`, `IPlayerActionHandler` → `ActionClickHandler`/`IActionClickHandler` and `ActionUserHandler`/`IActionUserHandler`.
- `model/ShortCuts` → `model/actor/holders/player/Shortcuts`.
- `instancemanager/QuestManager` → `managers/ScriptManager`.
- `scripting/ScriptEngineManager`, `ManagedScript`, `scripting/java/*` (4) → `scripting/engine/{ScriptExecutor,ScriptClassLoader,ScriptClassData,ScriptFileManager}`.
- `instancemanager/DBSpawnManager` → `managers/DatabaseSpawnManager`.
- `instancemanager/DailyTaskManager` → `managers/DailyResetManager`.
- `IdManager` has its DB cleanup split out into `managers/DatabaseIdManager`.
- `instancemanager/MapRegionManager` + `model/MapRegion` → `data/xml/MapRegionData`.
- `data/xml/KarmaData` → `KarmaLossData`.
- `PlayerXpPercentLostData` → `ExperienceLossData`.
- `model/clan/ClanPrivilege` → `ClanAccess` / `ClanPrivileges`.
- `AdminCommandAccessRight` → `data/holders/AccessRight`.
- `util/Util` → `LocationUtil`, `ArrayUtil`, `FormatUtil`, `MapUtil`.
- `LinePointIterator(3D)` → `geoengine/util/GridLineIterator2D/3D`.
- `model/Territory` → `util/PolygonTerritory`.
- `PrimeFinder` → `PrimeCapacityAllocator`.
- `NpcWalkerNode` → `WalkNode`.
- `FortSiegeSpawn` → `FortSpawnHolder`.
- `model/item/PlayerItemTemplate` → `InitialEquipment`.
- `instancemanager/games/MonsterRace` → `managers/games/MonsterRaceManager`.
- `model/events/AbstractScript` is merged into `model/script/Quest`. A new `Script`/`InstanceScript` base sits on top.
- `model/RecipeList`, `instancemanager/RecipeManager`, `RecipeStatHolder`, `TempItem`, `ManufactureItem` → `data/holders/RecipeHolder` (now holds `doCraft`, `checkNecessaryIngredients`, …).
- `util/OfflineTradeUtil` → `OfflineTraderTable.enteredOfflineMode`.
- `util/Evolve` → `PetManager.doEvolve`.
- `CharSelectInfoPackage` → `network/holders/CharacterInfoHolder`.
- Geoengine: `geodata/GeoData` is merged into `GeoEngine`. The two pathfinders (`cellnodes/*` 4 files, `geonodes/*` 2 files, `AbstractNode*`, `BinaryNodeHeap`) are replaced by a single A* (`pathfinding/{PathFinding,NodeBuffer,GeoNode,GeoLocation}`). `GeoEngine` gains `loadRegion`, `reloadRegion` and `isCompletelyBlocked`.
- `ui/frmAbout` → `AboutFrame`.
- `org/l2jmobius/Config.java` (3,621 lines) is split into 64 classes under `gameserver/config` and `config/custom`, plus `commons/config/{DatabaseConfig,ThreadConfig,InterfaceConfig}`.

**Genuinely removed in S** (Classic- or Interlude-specific features, or legacy engine pieces):
- **Lottery**: `instancemanager/games/Lottery`, `ALLOW_LOTTERY` and the `AltLottery*` keys, `lottery.sql`.
- **Mob groups**: `MobGroup`, `MobGroupTable`, `ControllableMob`, `ai/ControllableMobAI` (the `//mobgroup` admin command).
- **Race-gated village masters**: `VillageMasterDElf/Dwarf/Fighter/Mystic/Orc/Priest`. `VillageMaster` also loses all its subclass menus (`getSubClassMenu`, `isValidNewSubClass`, `checkVillageMasterRace`, …). In S, `Player.addSubClass` is called only from the datapack script `ai/areas/TalkingIsland/Raina/Raina.java`.
- **Classic clan reward / hunting points**: `ClanRewardData`, `ClanRewardBonus`, `ClanRewardType`, plus `Clan.addHuntingPoints`/`getHuntingPoints`/`canClaimBonusReward`/`resetClanBonus`/`addMemberOnlineTime`, `ClanMember` online-time and reward fields, and `config/ClanReward.xml`.
- **Global task scheduler**: `taskmanager/TaskManager`, `Task`, `TaskTypes`, `tasks/TaskBirthday`, `TaskCleanUp`, `TaskRecom`, `TaskRestart`, `TaskShutdown`, and the `global_tasks.sql` table. Their work is folded into `DailyResetManager` and `ServerRestartManager`.
- **Mob soul absorption**: `model/AbsorberInfo` and `Attackable.absorbSoul`/`addAbsorber`/`isAbsorbed`.
- **GM condition overrides**: `PlayerCondOverride` and `add/removeOverrideCond`/`canOverrideCond` on Creature and Player. S uses plain `player.isGM()`.
- **Item-grade condition**: `ConditionPlayerGrade`, plus the `MaxEquipableItemGrade` and `MaxNewbieBuffLevel` keys.
- **Marker interfaces**: `model/interfaces/I*` (IIdentifiable, INamable, IUniqueId, ISpawnable, IDeletable, IRestorable, IStorable, IDecayable, IUpdatable, ISkillsHolder, IParserUtils, IParserAdvUtils). `WorldObject` gains `getId` and `asX()` casts instead.
- **Additional-stat API**: `model/stats/StatHolder` and `CreatureStat.add/mulAdditionalStat`.
- **Old date and script parser**: `script/{DateRange,Parser,ParserFactory,ScriptDocument,EngineInterface,ParserNotCreatedException}`. `LongTimeEvent` now parses dates itself.
- **Small leftovers**: `BuilderUtil`, `BypassBuilder`, `BypassParser`, `TimeAmountInterpreter`, `EnumIntBitmask`, `KeyValuePair`, `GroupSpawn`, `ai/VehicleAI`, `SpeedType`, `MissionResetType`, `EffectDurationHolder`, `PlayerSkillHolder`, `QuestSoundHtmlHolder`.
- **Classic crafting extras**: `AltGameCreation*`, `CraftMasterwork*`.
- **Classic clan-level config**: the `ClanLevel6..11Cost/Requirement` and `LevelUpXXReputationScore` keys. These moved to `ClanLevelData` XML in S.
- **Interlude-only DB layer** (local commits): `commons/database/dialect/DatabaseDialect` and `commons/database/queries/{Sql,SqlQueryLoader,SqlQueryManager}`.

### 1.4 Salvation-only files that are not plain moves, by subsystem
| Subsystem | Files (S paths) | Present in current upstream Classic? |
|---|---|---|
| **Balthus Knights (Ertheia/awakening pathway)** | managers/BalthusKnightManager, model/zone/type/BalthusKnightZone, config/BalthusKnightsConfig; CategoryType +ALLOWED_BALTHUS_CLASSES, BALTHUS_FIGHTER/MAGE_CLASSES; PlayerClass 190/191 | no |
| **Ability points / awakening skills** | model/actor/request/AbilityLearnRequest; SkillTreeData.getAllRevelationSkills; awaken checks via `CategoryType.SIXTH_CLASS_GROUP` | no |
| **Dual class** | Player: `KNOWN_DUAL_SKILLS_VAR`, `DUAL_CLASS_SKILLS`, `restoreDualSkills`, `get/setOriginalClass`; config `AltDualClassWithoutQuest`, `Hardin*` (7 keys) | partial |
| **Agathion slots** | data/xml/AgathionData, model/item/holders/AgathionSkillHolder, `Inventory.equipAgathion`/`getAgathionSlots`, `PlayerStat.getAgathionSlots`, `Stat.AGATHION_SLOTS`, `BodyPart.AGATHION(0x3000000000L)`, effect `AgathionSlot` | no |
| **Alchemy** | data/xml/AlchemyData, model/alchemy/AlchemyCraftData | no |
| **Ensoul / special abilities** | data/xml/EnsoulData, model/ensoul/{EnsoulFee,EnsoulOption,EnsoulStone}; `Item.*SpecialAbilit*` (13 methods); `ItemTemplate.getEnsoulSlots`; table `item_special_abilities` | yes (Classic has ensoul) |
| **Clan mastery / clan levels / clan shop / contribution** | data/xml/{ClanLevelData,ClanMasteryData,ClanShopData}, data/holders/{ClanMasteryHolder,ClanShopProductHolder}; `Clan.addMastery`/`getTotalDevelopmentPoints`/…; `Player`/`ClanMember.getClanContribution*`; `FeatureConfig.CLAN_CONTRIBUTION_*` | no |
| **Monster Book** | data/xml/MonsterBookData + 2 holders; `Player.updateMonsterBook`/`rewardMonsterBook` | no |
| **Ceremony of Chaos** | config/CeremonyOfChaosConfig, events/holders/ceremonyofchaos/OnCeremonyOfChaosMatchResult (event logic lives in datapack) | no |
| **Underground Coliseum** | model/undergroundColiseum/* (7), instance/{UCHelper,UCManager,UCTower}, managers/games/UndergroundColiseumManager, config; table `underground_coliseum`; `Party.get/setUCState`; Player UC fields | no |
| **Krateis Cube** | model/krateisCube/* (4), instance/{KrateisCubeManager,KrateisMatchManager}, managers/games/KrateisCubeManager | no |
| **Handy's Block Checker** | instance/Block, managers/{HandysBlockCheckerManager,games/BlockCheckerManager}, ArenaParticipantsHolder | no |
| **Gracia Seeds** | managers/GraciaSeedsManager, config/GraciaSeedsConfig | no |
| **Wedding** | managers/CoupleManager, config/custom/WeddingConfig; table `mods_wedding`; Player marriage fields | yes |
| **Faction / ShilensBreath / alter skills** | model/script/Faction; `Player.*Faction*`, `*ShilensBreath*`, `*ReplacedSkill*`, `*AlterSkill*` | no |
| **Misc infrastructure** | managers/{ItemManager (create/destroy extracted from ItemData), ZoneBuildManager (admin zone builder), CaptchaManager}, model/captcha/*, data/sql/{OfflinePlayTable,PartyMatchingHistoryTable}, MatchingRoomHistory, DynamicExpRateData (+config/DynamicExpRates.xml), ConditionMaximumVitalityPoints, model/skill/TriggerCastInfo, ai/DistrustAI, PSkillCritRateFinalizer, ItemProcessType (enum replaces `String process` everywhere), BodyPart (enum replaces `long SLOT_*`), ChatBroadcastType, PartyExpType | mostly yes (upstream drift) |
| **New events** | OnPlayerCheatDeath, OnItemAttributeAdd, OnItemEnchantAdd, OnItemSoulCrystalAdd, OnDailyReset, OnCeremonyOfChaosMatchResult (EventType 115→121) | partial |

**Skill handler sets** (datapack, but engine contract). Salvation keeps them under `handlers/skill/{effects,conditions,targets}`.
- **Effects**: I has 369, S has 385, 350 shared.
  - About 19 are renames: `MagicalAttack*`→`MagicalDamage*`, `PhysicalAttack*`→`PhysicalDamage*`, `EnergyAttack`→`EnergyDamage`, `TriggerSkillByAttack/ByDamage/BySkillAttack`→`ByDamageDealt/ByDamageReceived/BySkillDamage`. These matter only if Rust keys effects by XML name; the XML must then be checked.
  - Genuinely new in S: `AgathionSlot`, `ApplySkillEffects`, `BonusRaidPoints`, `CheatDeath`, `CreateCommonItem`, `CreateItem`, `Crystallize`, `DispelCaster`, `Distrust`, `FlipBlock`, `LimitSkill`, `MagicalSuccessResistance`, `RemoveEquipPenalty`, `ReplaceSkillBySkill`, `Ride`, `TrackLimitedSkill`, `TriggerSkillWithDelay`.
  - Removed: `MAtk`, `PAtk`, `ResistDDMagic`.
- **Skill conditions**: 121 vs 122 (+`OpNotInEventSkillCondition`).
- **Targets/affect handlers**: 52 vs 52 (`BalakasScope`→`ValakasScope`, a typo fix).

## 2. Shared files: biggest changes

Metric: normalized changed lines (`<` + `>`), I vs S. **Drift** means the same change is already present in current upstream Classic_2.0 (so it is not chronicle-specific). **Chronicle** means it differs from current Classic as well.

| # | File (S path) | Δ | Note |
|---|---|---|---|
| 1 | model/actor/Player.java | 1,592 | Chronicle: dual class (original class, dual skills), noble level, UC/Krateis/BlockChecker state, marriage, faction, Shilen's Breath, clan contribution, monster book, alter/replaced skills, create-item/crystallize levels, expertise via `CrystalType.plusLevel`, agathion equip. Drift: `ShortCuts`→`Shortcuts`, `processQuestEvent`→`processScriptEvent`, online-time tracking removed, `setClassId`→`setPlayerClass`, raw MySQL `REPLACE INTO` replaces I's named queries. Δ vs Classic 2.0: 610. |
| 2 | model/script/Quest.java (was model/quest) | 1,502 | Mostly drift: `AbstractScript` merged in. All `notifyX` dispatchers removed and replaced by listener registration. `onAttack`, `onKill` and `onSpawn` now return **void** (they returned String in I). Δ vs Classic: 42. |
| 3 | model/actor/Creature.java | 596 | Mostly drift: override-cond API removed, `asCreature`, `broadcastSkillPacket`, `isAttackingOrCastingNow`. Chronicle: `getCHA`/`getLUC`, block-checker attack guard. |
| 4 | model/itemcontainer/Inventory.java | 553 | **Chronicle: paperdoll layout 32→37 slots, AGATHION1-5 inserted at 17-21, shifting DECO1..BROOCH_JEWEL6 up by 5.** Drift: `BodyPart` enum replaces `long` masks (mask values unchanged), `getArmorSetEnchant`. Δ vs Classic: 112. |
| 5 | model/actor/instance/VillageMaster.java | 515 | Chronicle: subclass, sub-pledge and pledge-skill menus removed (moved to scripts). Δ vs Classic: 511. |
| 6 | model/item/instance/Item.java | 423 | Ensoul special abilities, transmog id, `ItemProcessType`. Mostly drift: Classic has ensoul. |
| 7 | model/clan/Clan.java | 420 | Chronicle: hunting points and clan bonus replaced by mastery and development points; level-up via `ClanLevelData` instead of `ClanLevelNCost` config. Δ vs Classic: 349. |
| 8 | data/xml/SkillData.java | 377 | Drift: skill-hash helpers moved out; new expression evaluator for level/sub-level formulas in XML (`evaluatePostfix`, `matchesLevelAndSubLevel`). Affects the Rust skill XML parser if S skill XML uses expressions. |
| 9 | model/actor/instance/SchemeBuffer.java | 370 | Drift (custom buffer rewrite, auto-buff). |
| 10 | geoengine/GeoEngine.java | 351 | Drift: GeoData merged in, lazy region load and reload, `isCompletelyBlocked`. |
| 11 | GameServer.java | 347 | Boot order: new managers, ConfigLoader, DB pool. |
| 12 | managers/CastleManorManager.java | 327 | Drift (MySQL upserts, refactor). |
| 13 | data/xml/SpawnData.java | 290 | Drift: `addNewSpawn`/`deleteSpawn` moved here from SpawnTable. |
| 14 | data/xml/SkillTreeData.java | 288 | Drift plus chronicle: `getCompleteClassSkillTree` cached, revelation skills, awaken filter `SIXTH_CLASS_GROUP && (race != ERTHEIA or dual active)`. Δ vs Classic: 20. |
| 15 | ai/AttackableAI.java | 284 | Drift: `onEvt`→`onAction`; new NPC-template "parameterized cast" gating (`allowsParameterizedCast`, `findParameterizedSkillSlot`). |
| 16 | data/xml/ItemData.java | 256 | Drift: createItem/destroyItem moved into `managers/ItemManager`. |
| 17 | data/SchemeBufferTable.java | 245 | Drift. |
| 18 | model/actor/templates/NpcTemplate.java | 236 | Drift: fake-player info refactor. |
| 19 | ai/AbstractAI.java | 202 | Drift: Ctrl → Intention/Action rename, `getClientMovingToPawnOffset`. |
| 20 | managers/IdManager.java | 192 | Drift: configurable FIRST/LAST object id (`IdManager.ini`), prime-growing BitSet, cleanup moved to `DatabaseIdManager`. |
| 21 | model/actor/Attackable.java | 185 | Soul absorption removed; `asAttackable`. Δ vs Classic: 59. |
| 22 | managers/ZoneManager.java | 180 | Drift. |
| 23 | data/SpawnTable.java | 174 | Drift. |
| 24 | model/spawns/NpcSpawnTemplate.java | 168 | Drift (`getHighZ`). |
| 25 | model/actor/instance/PetManager.java | 167 | Pet evolve/restore inlined. |
| 26 | model/actor/holders/npc/WalkInfo.java | 166 | Drift. |
| 27 | ai/CreatureAI.java | 161 | Drift (rename). |
| 28 | model/actor/Npc.java | 159 | Drift. |
| 29 | data/xml/RecipeData.java (+ RecipeHolder 121) | 156 | Chronicle plus drift: recipe model replaced (RecipeList → RecipeHolder with craft logic). Δ vs Classic: 146+121. |
| 30 | **model/stats/Formulas.java** | 154 | Drift (also in Classic HEAD), but **behavioral**, listed below. Chronicle-only: dragon-weapon defence multiplier in PvP bonus. |
| 31 | model/actor/stat/PlayerStat.java | 154 | Fishing exp from `FishingData` rod multipliers; `getAgathionSlots`; level-up broadcasts. |
| 32 | util/FloodProtectorAction.java | 152 | Drift. |
| 33 | model/actor/status/PlayerStatus.java | 139 | Drift (cosmetic). |
| 34 | model/olympiad/Olympiad.java | 131 | **Hero class set `FOURTH_CLASS_GROUP`→`SIXTH_CLASS_GROUP`** (awakened classes); MySQL upserts; config rename `AltOly*`→`Olympiad*`. |
| 35 | data/xml/InitialShortcutData.java | 130 | Drift. |
| 36 | model/script/LongTimeEvent.java | 126 | Drift: own date parsing, drop periods. |
| 37 | model/skill/SkillCaster.java | 119 | Reuse-message threshold 3000→1000 ms; `BodyPart.DECO`; fame → clan reputation message; `broadcastSkillPacket`. |
| 38 | model/groups/Party.java | 114 | UC state. |
| 39 | model/World.java | 111 | `calculateRegionZ`, `getNpc`. |
| 40 | model/WorldObject.java | 101 | `asX()` casts, `getId`, trigger-cast queue (`TriggerCastInfo`). |
| 41 | model/clan/ClanMember.java | 100 | Online time and bonus replaced by clan contribution. |
| 42 | model/actor/stat/CreatureStat.java | 94 | **Stat recalculation is now debounced** (scheduled task plus broadcast flag) instead of synchronous; additional-stat API removed; `getPSkillCriticalRate`, `getCHA`, `getLUC`. |
| 43 | model/item/ItemTemplate.java | 89 | `BodyPart`, ensoul slots, `isQuestUsableItem`. |
| 44 | data/xml/ExperienceData.java | 72 | Arrays instead of maps. **Path is now `data/stats/players/experience.xml`; maxLevel 85→110, maxPetLevel 86→110.** |
| 45 | data/xml/PlayerTemplateData.java | 90 | Path is now `data/stats/players/templates`; level-up gain auto-generation up to the max level. |

**Smaller but mechanically important**
- **Stat finalizers.**
  - Chronicle: `MaxHpFinalizer` and `PAttackFinalizer` multiply by a **CHA base-stat bonus** for players. `CreatureTemplate` reads `baseCHA` and `baseLUC`.
  - Drift: `ShotsBonusFinalizer` raises the weapon enchant bonus from 0.3 to 0.7 per level, and mage casters use the sapphire jewel. Also `RegenHP`/`RegenMP`, `PAttackSpeed`, `MAttackSpeed`, `MaxCp`, `MaxMp`, `PDef`, `MDef` (3–36 lines each). There is a new `PSkillCritRateFinalizer`.
- **`Stat` enum.** I has 182 constants, S has 186: +`AGATHION_SLOTS`, `BONUS_RAID_POINTS`, `PVE_DAMAGE_TAKEN_MONSTER`, `PVE_DAMAGE_TAKEN_RAID`. I already has `STAT_CHA`, `STAT_LUC` and `DRAGON_WEAPON_DEFENCE`.
- **`Formulas` behavioral drift** (versus I):
  - P-crit, M-crit and P-skill-crit move to a 1000 scale and are clamped to `[30, MAX_*CRIT_RATE]`. A new `MAX_PSKILLCRIT_RATE` key exists. Defence crit-rate and crit-add are applied differently.
  - Crit damage becomes `1 + critDmg·bal·max(0.2, 2-defCritDmg)` instead of `2·critDmg·defCritDmg·bal`.
  - Soulshot and spiritshot bonuses become **additive** (`2 + SHOTS_BONUS/100`) instead of multiplicative (`2·SHOTS_BONUS`).
  - Magic damage gets a `critMagicAdd` term. Minimum damage rises from 0 to 1.
  - `activateRate > 99` means a guaranteed land. `KarmaLossData` is renamed.
- **`AbnormalType`**: 535 → 550. Adds DISTRUST, SAYHA_BUFF, ARCHER/WIZARD/SUMMON_BUFF1-4, RESIST_CHEAT_DEATH and others. Removes AGATHION_SONG_DANCE, the herb fruits and HERB_*.
- **`CommonSkill`**: +SHILENS_BREATH, LUCKY_CLOVER, transforms, KNIGHT_OF_*. Removes ABILITY_OF_LIGHT and ABILITY_OF_DARKNESS.
- **Unchanged** (0 enum diffs): `EffectFlag` (38), `EffectType` (40), `TraitType` (64), `TargetType` (21), `AffectObject` (11), `Race` (S adds `FRIEND` at the end), `CategoryType` (+3 Balthus).
- **Siege, fort, castle, instance, zone**: each 13–62 normalized lines, mostly MySQL upserts and casts. These are low-risk.

## 3. Infrastructure

### 3.1 Threading
- `commons/threads/ThreadPool`: I 206 lines, S 302 lines. Same model (scheduled pool, high-priority scheduled pool, instant pool). S reads sizes from `ThreadConfig`/`Threads.ini` (`ScheduledThreadPoolSize=-1` means auto) and adds `purge`. `RejectedExecutionHandlerImpl` and `RunnableWrapper` are removed.
- S `CreatureStat.recalculateStats(broadcast)` is now **debounced through `ThreadPool.schedule`**. On the Rust single game thread this becomes "mark dirty, recalc next tick". It is a visible ordering change: UserInfo or StatusUpdate can arrive one tick later.
- **Rust impact is minimal.** The Rust tick loop and `scheduler.rs` replace all of this.

### 3.2 Config
- I: one `Config.java` (3,621 lines, 1,081 `public static` keys).
- S: `ConfigLoader` plus 21 `gameserver/config/*Config` classes, 43 `config/custom/*Config` classes, and 3 commons configs (1,138 keys).
- **Java field names**: 134 are only in I and 191 only in S. Many are renames (`ALT_OLY_*`→`OLYMPIAD_*`, `ALT_DEV_*`→`NO_QUESTS`/`NO_SPAWNS`/`SHOW_*_LOAD_IN_LOGS`, `DEADLOCK_DETECTOR`→`DEADLOCK_WATCHER`).
- **What matters for Rust is the .ini key names.** I has 1,311 keys and S has 1,424; **100 are I-only and 213 are S-only.**
  - I-only: `AltOly*` (31, renamed `Olympiad*` in S), Lottery (8), `AltGameCreation*`/`CraftMasterwork*`, `ClanLevel6..11Cost/Requirement` and `LevelUpXX..ReputationScore` (26), `AbilityMaxPoints`, `MaxEquipableItemGrade`, `MaxNewbieBuffLevel`, `AltSubclassEverywhere`, Zaken spawn, `DeadLockDetector`, `UseCachedThreadPool`, `FairnessBuckets`.
  - S-only: `Olympiad*`, `CoC*` (11), `UC*`, Balthus (2), `Hardin*` (7), grand-boss keys for Anakim/Lilith/Beleth/Helios/Kelbim/Lindvior/Ramona/Trasken (about 35), `ClanContribution*` (4), Transmog, Captcha, Wedding (8), `BufferPool.*` (network), `FirstObjectId`/`LastObjectId`/`InitialCapacity` (IdManager), `MaxPSkillCritRate`, `KillCounter*`, `AllowErtheia`/`AllowKamael`, `AltDualClassWithoutQuest`, `EnableFameSystem`, `EnableAlterSkills`, and others.
- **Config files**:
  - S adds `BalthusKnights.ini`, `CeremonyOfChaos.ini`, `Database.ini`, `Development.ini`, `IdManager.ini`, `Threads.ini`, `UndergroundColiseum.ini`, `ClanShop.xml`, `DynamicExpRates.xml`, and under `Custom/`: `CancelReturn`, `Captcha`, `FreeMounts`, `StartingTitle`, `Transmog`, `Wedding`.
  - `Character.ini` is renamed **`Player.ini`**. The Rust `config/character.rs` hard-codes `config/Character.ini`.
  - S drops `ClanReward.xml` and `Custom/PcCafe.ini`.

### 3.3 Database layer
- I (local commits) uses `DriverManager` without a pool and a `DatabaseDialect` with named queries via `SqlQueryManager` (`Sql.` used in 26 files). It ships MariaDB, PostgreSQL and SQLite schemas (94 game tables each).
- S uses a **HikariCP** pool and MySQL/MariaDB only. **589 inline SQL string literals** in `java/`. **MySQL-specific upserts in 24 engine files**: `REPLACE INTO` in 16, `ON DUPLICATE KEY UPDATE` in 9, `INSERT IGNORE` in 1.
  - Files: Player, Clan, Item, Olympiad, Fort, Castle, Instance, Quest, ItemAuction, ClanHallAuction, AbstractResidence, Shortcuts, ContactList, Servitor, Product, CharSummonTable, ClanTable, ClanEntryManager, GlobalVariablesManager, PremiumManager, MonsterRaceManager, AbstractDailyMissionHandler, SecondaryPasswordAuth, and loginserver RequestTempBan.
  - `LIMIT` appears in 6 files in both trees. No `NOW()` or `UNIX_TIMESTAMP`.
- **Schema** (`dist/sql/game`, 97 tables vs I's 94):
  - S adds `character_offline_play`, `character_offline_play_group`, `character_transmogs`, `item_special_abilities`, `mods_wedding`, `underground_coliseum`.
  - S drops `custom_teleport`, `global_tasks`, `lottery`.
  - **Every shared table has an identical column list** (checked by name, for example `characters` has 57 columns in both). The only schema migration needed is the 6 new tables. Data migration is separate: paperdoll `loc_data` must be remapped for slots ≥17.
- **Rust** uses SeaORM with SQLite/Postgres and is unaffected by the MySQL dialect. Each S upsert just needs a SeaORM `on_conflict` equivalent when ported.

### 3.4 IdManager
- S adds `IdManager.ini` (FirstObjectId, LastObjectId, InitialCapacity, ResizeThreshold, ResizeMultiplier) and a BitSet that grows at prime sizes. Orphan cleanup SQL moved to `DatabaseIdManager`.
- I uses hard-coded `0x10000000..0x7FFFFFFF`.
- Rust: a small config-only change.

### 3.5 Geoengine
- Geodata blocks and regions are the same format. `MultilayerBlock` changes: `getNswe` → `getNearestNswe`.
- Pathfinding: the I cell and geo node finders are replaced by one A* with `NodeBuffer`, post-filtering (`applyPostFiltering`) and an `AvoidObstructedPathNodes` key.
- Lazy and reloadable regions. Line iterators renamed.
- Rust `geo/` (7 files, 2.7k LOC) is already its own implementation, so only path-quality parity is affected.

## 4. Impact on the Rust port

Rust gameserver sizes: `model/` 64 files / 18.8k LOC, `data/` 78 / 26.0k, `game_loop/` 561 / 262k (138k of it tests), `config/` 29 / 8.0k, `scripts/` 255 / 50.4k, `geo/` 7 / 2.7k, `db/` 28 / 8.5k.

| Rust area | Impact | What |
|---|---|---|
| `model/inventory.rs` (+ `db/`, `network/enter_world.rs`, `server_packets/npc.rs`) | **High, foundational** | `PAPERDOLL_TOTAL_SLOTS` 32→37 and AGATHION 17–21, which shifts deco/cloak/belt/brooch indexes. `BodyPart::AGATHION` mask `0x3000000000`. Agathion equip and slot-count stat. Existing DB `items.loc_data` needs a remap migration. |
| `enums.rs`, `model/class_names.rs`, `data/player_template.rs`, `data/experience.rs` | **High, small** | Class ids 190/191; `CategoryType` Balthus groups; max level 85→110 (pets 110); datapack paths `data/stats/players/{experience.xml,templates}`; `baseCHA`/`baseLUC` template values. |
| `model/stats.rs`, `model/stat_finalize.rs`, `model/formulas/*` (≈2.1k LOC) | **High** | CHA multiplier in MaxHp/PAtk; 4 new `Stat` keys; crit 1000-scale and clamps; crit-damage formula; additive shots bonus; min damage 1; dragon-weapon PvP defence; ShotsBonus finalizer; P-skill crit finalizer. Ported values are pinned by tests, so expect many `game_loop/tests` (combat, skills) to need new golden numbers. |
| `model/skill/*`, `data/skill_data`, `data/skill_tree.rs`, `game_loop/skills` (16k) | **High** | About 17 new effect handlers and about 19 renamed effect names; `AbnormalType` +22/−7; `CommonSkill` changes; skill XML expression evaluator (level/sub-level formulas); revelation and dual-class skill trees; awaken filter; `TriggerCastInfo` queue; alter/replaced skills. |
| `config/` | **Medium** | Rename `Character.ini`→`Player.ini`; `AltOly*`→`Olympiad*`; drop Lottery/Craft/ClanLevel/ClanReward keys; add 7 new .ini files and 6 Custom ones; `IdManager.ini`; `MaxPSkillCritRate`. The "parsed key with no reader" rule means each new key needs a consumer. |
| `model/clan.rs`, `game_loop/clans` (7.3k) | **Medium–High** | Replace Classic hunting points and clan bonus with ClanLevelData, mastery and development points, clan shop and clan contribution. |
| `game_loop/character`, `game_loop/npc` village-master flows | **Medium–High** | Subclass and dual-class flow moves to the Raina script; original-class tracking; noble level; ability points; awakening. Rust code that ports `VillageMaster` subclass menus becomes dead. |
| `model/olympiad.rs`, `game_loop/olympiad` | **Low–Medium** | Hero class set is SIXTH_CLASS_GROUP; config key renames. |
| `data/recipe_data.rs`, `game_loop/items` crafting | **Medium** | RecipeHolder craft model; no masterwork or creation-rate keys. |
| `model/item*`, `game_loop/items` | **Medium** | Ensoul special abilities (`item_special_abilities` table), transmog, crystallize and create-item levels. |
| `scripts/` (50k) | **Medium, wide but mechanical** | Script hooks `onAttack`, `onKill` and `onSpawn` return void in S. The quest base gains AbstractScript helpers. Script content itself belongs to the datapack axis. |
| `game_loop/npc/ai` | **Low–Medium** | NPC-template parameterized cast gating; DistrustAI. Intention/Action renames do not matter to Rust. |
| `model/lottery.rs`, `model/mob_group.rs`, `game_loop/admin` mobgroup, soul-crystal absorb (`data/soul_crystal_data.rs`, verify), Classic clan reward, global_tasks | **Delete or gate** | Removed in S. |
| New subsystems (`game_loop/activities`, `events`) | **Optional, large** | Ceremony of Chaos, Underground Coliseum, Krateis Cube, Block Checker, Monster Book, Alchemy, Balthus Knights, Wedding, Captcha, Faction. Each is 0.5–2 weeks; they can be deferred behind config. |
| `geo/` | **Low** | Optional A* post-filter parity. |
| `db/` + `crates/migration` | **Low–Medium** | 6 new tables, 3 dropped, paperdoll remap migration. No column changes in shared tables. |
| `scheduler.rs`, threading | **Negligible** | Stat recalc debounce becomes dirty-flag-next-tick; decide on parity. |

### Recommended order (engine side)
1. **Spec decision.** Treat Salvation HEAD as the new spec. Plan from the Classic_2.0→Salvation delta (about 3.7k lines and 51 files), plus an explicit list of behavioral drift items (Formulas, finalizers, SkillCaster reuse threshold, CreatureStat debounce, recipe model). Ignore rename and package drift.
2. **Foundations (about 1–2 weeks).**
   - Class ids, CategoryType, level cap 110 and exp table, player template paths.
   - 37-slot paperdoll and BodyPart AGATHION, plus the DB remap migration.
   - Config renames and new ini files.
   - These change shared constants that everything else reads, so they come first.
3. **Stats and combat (about 1–2 weeks).** Stat keys, CHA/LUC, finalizers, Formulas. Re-baseline the combat tests.
4. **Skills (about 2–3 weeks).** Effect-handler deltas, abnormal types, the skill XML expression parser, skill trees (revelation, dual, awaken).
5. **Class progression (about 2–3 weeks).** Awakening, dual class (Raina flow), ability points, noble level, Olympiad hero group.
6. **Items and clan (about 3–4 weeks).** Ensoul, agathion, recipes, crystallize and create-item effects; clan levels, mastery, shop and contribution. Remove the Classic clan reward.
7. **Removals.** Lottery, mob groups, race village masters, mob soul absorb, global tasks. These can run in parallel with steps 3–6.
8. **Optional activity systems.** CoC, UC, Krateis, Block Checker, Monster Book, Alchemy, Balthus, Wedding, Captcha. Do them later and gate by config.

Engine-core total, excluding packets, datapack scripts and optional activities: roughly **10–15 developer-weeks**, with the largest risk in combat and skill parity re-baselining.
