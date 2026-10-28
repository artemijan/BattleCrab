# Salvation config keys with no Rust reader

S1 inventory (2026-10-28). These keys are **new in Salvation**: they were not in
any `interlude:dist/*/config/*.ini`, and no Rust source names them. Keys
inherited from Interlude keep the status they had there. Each group below has a
disposition and the milestone that owns it. The defaults ship exactly as
Salvation has them, with no operator re-tuning.

How the list was built: every `Key =` line in `dist/{game,login}/config/*.ini`,
minus the keys found as a string literal in `crates/**/*.rs`, minus the keys
already present on `interlude`. Files whose keys are built at run time (the
`IntervalOf<Boss>Spawn` family, `FloodProtector`, `Siege`) were checked by hand.

| File | Keys | Disposition | Owner |
|---|---|---|---|
| `Network.ini` (game + login) | `ThreadPriority`, `BufferPool.*` (20 per file) | **Not applicable.** These tune Java NIO's buffer pool and thread priority, and tokio has no equivalent. | none |
| `IdManager.ini` | `FirstObjectId`, `LastObjectId`, `InitialCapacity`, `ResizeThreshold`, `ResizeMultiplier` | **Not applicable as written.** The Rust id allocator keeps its own range. Read `First`/`LastObjectId` if the range ever has to match Java's. | S3 |
| `Development.ini` | `LogServerLoadTimes` | Unread. Rust logs load times unconditionally. | none |
| `GeoEngine.ini` | `AvoidObstructedPathNodes` | Unread. Port it with the p140 pathfinding pass. | geodata (D5) |
| `Player.ini` | `DeathPenaltyChance`, `ShowEffectMessagesOnLogin`, `AltStoreToggles`, `EnableAlterSkills`, `MaxPSkillCritRate`, `EnableFameSystem`, `ShowIntroVideo` | Unread. Port each one with the system that consumes it. | S5–S7 |
| `Player.ini` | `AltDualClassWithoutQuest`, `Hardin*` (7), `AbilityPointsResetSP` | Unread. These are Ertheia dual-class, awakening and ability systems. | S8 |
| `Rates.ini` | `RateQuestRewardFP`, `MonsterExpMaxLevelDifference`, `LuckyChanceMultiplier` | Unread. These are the fame-point reward, the exp level-gap cap and lucky NPCs. | S6 / S12 |
| `PVP.ini` | `AnnounceGainakSiege` | Unread. Gainak is a Salvation zone. | S12 |
| `Feature.ini` | `ClanContribution*` (4) | Unread. This is the pledge-contribution system. | S10 |
| `General.ini` | `ExaltedForGloryItemMax`, `ExaltedForHonorItemMax`, `MultisellAmountLimit` | Unread. Port `MultisellAmountLimit` with the multisell rework, and the Exalted caps with their quests. | S10 / S12 |
| `General.ini` | `AllowPartyInSameEvent`, `EnableBlockCheckerEvent`, `BlockCheckerMinTeamMembers`, `HBCEFairPlay` | Deferred. These are the Block Checker minigame. | D7 |
| `GrandBoss.ini` | `*RecognizeHero`, and the Beleth, Kelbim, Anakim, Lilith, Trasken, Lindvior, Ramona and Helios windows and caps (39) | Unread. Each boss's keys arrive with its AI. | S12 |
| `BalthusKnights.ini` | `BalthusKnightsEnabled`, `BalthusKnightsLevel` | Deferred. This is the Balthus Knights start. | S12 |
| `CeremonyOfChaos.ini` | `CoC*` (10) | Deferred. | D7 |
| `UndergroundColiseum.ini` | 5 keys | Deferred. | D7 |

## Keys the port stopped reading in S1

Salvation removed these, so the Rust fields are now fixed at the value Salvation
behaves as:

| Former key | Now | Note |
|---|---|---|
| `CraftMasterwork`, `CraftMasterworkChance`, `AltGameCreation*` | `false` / defaults | The Interlude crafting paths stay inert until S10 rewrites recipes. |
| `MaxEquipableItemGrade` | `CrystalType::Event` (no filter) | Remove the buylist grade filter after the S2 merge. |
| `InitialEquipmentEvent` | `false` | Remove the event-equipment loader after the S2 merge. |
| `AltSubclassEverywhere` | field removed | The port never had the gate. |
| `StrictDelevelSkillRemoval` | field removed; callers pass `false` | This was a port-only deviation. Remove the `strict` parameter from `delevel_skill_changes` after the S2 merge. |
| `LevelObtainedReputationScoreMultiplier` (Feature) | removed | Salvation has no clan reputation for a level-up. |
| `HideBypassRemoval`, `CustomTeleportTable`, `HtmlActionCacheDebug`, `DatabaseCleanUp` (General) | removed | |
| `AllowLottery`, `AltLottery*` (General) | removed; the lottery is hard-wired off | Lottery deletion is S9. |
| `AltDev*`, `Debug*Packets`, `ExcludedPacketList` (General) | moved to `Development.ini` under Salvation's names | |
| Zaken's `GrandBoss.ini` window | removed | Salvation's Zaken is the Cavern of the Pirate Captain instance (S12). The generic-lifecycle Zaken has been dropped. |

## Value-parity audit (2026-10-28)

Every shipped key was dumped from Salvation's Java config classes (1204
fields) and compared, value by value, with what the Rust loaders produce from
the same `dist`. What remains after the fixes below are unit or shape
differences only: milliseconds against ticks or seconds, and lists against sets.

Fixed:

| Key | Was | Now |
|---|---|---|
| Any value with trailing whitespace | trimmed | Parsed the Java way. `Properties` keeps the trailing space, so `Boolean.parseBoolean("True ")` is false and `Integer.parseInt("5 ")` falls back to the default. Only `Float.parseFloat` trims. The shipped `FakeDeathUntarget = True ` is therefore **false**. |
| `RateInstance{Xp,Sp,PartyXp,PartySp}` | `-1` taken literally | `-1` resolves to the matching base rate. `RateInstancePartySp` now has its own key; Interlude's Java read it from the `PartyXp` key. |
| `CommunityCombatDisabled`, `CommunityBoardPeaceOnly` | unread; combat check always on | Both are read and gate the board. The combat check also refuses a player in store mode. |
| `IgnoredAutoPickItems` | unread | Auto-play leaves these items (8190, 8689) on the ground. |
| `RaidbossExcludedFromSpawnAnnouncements`, `RaidbossExcludedFromDefeatAnnouncements` | unread | Read. The spawn exclusion is applied, and so would the defeat one be if that line were ported. |
| `BufferMaxSchemesPerChar` | hardcoded 5 | Read from `SchemeBuffer.ini` (4). `BufferItemId` and `BufferStaticCostPerBuff` are read but inert, because the SchemeBuffer NPC dialog is not ported. |

Matched, with notes:

- **`Chat` enum keys**: Java's `Enum.valueOf` is case-sensitive and throws on a bad
  value. Rust accepts any case and falls back to the default. Every shipped value
  is valid, so the effective values agree.
- **Karma non-droppable lists**: Java lists 7694 twice. Rust dedups it. No
  behavioural difference.
- **`NoblessMaster` `LevelRewarded = 1`**: the same as the existing plain-noble
  grant.
- **`CustomDepositableEnabled`, `DepositableQuestItems`** (both `False`): unread.
  At `False` neither changes `isDepositable`, so the warehouse already behaves
  as Salvation's does.
- **`DelevelRemoveAbilities`**: read, but inert until abilities exist (S8).
- **`MimimumDelevel`** (sic): not applicable, because the Rust port has no Delevel Manager.

The unread keys that belong to unported systems are listed in the first table,
under their milestones.
