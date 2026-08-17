//! `Character.ini` — port of the `CHARACTER_CONFIG_FILE` block of `Config.java`.
//!
//! **Every key this file ships is now accounted for**, one of three ways: read
//! into a field below, listed as dead in Java just under this note, or —
//! `MaximumPlayerLevel` alone — left open pending the level-cap decision
//! (`docs/PORTING_STATUS.md` row 19).
//!
//! # Parsed by Java and read by nothing
//!
//! These eight are assigned to a `Config` field and then consulted by no code
//! outside `Config.java`, in Java as much as here. They are named here rather
//! than in a field, because a field would imply something reads it:
//!
//! * `AbilityMaxPoints`, `AbilityPointsResetAdena` — the ability-point system
//!   (Ertheia), whose fields nothing references.
//! * `FeeDeleteSubClassSkills`, `FeeDeleteDualClassSkills`,
//!   `FeeDeleteTransferSkills` — skill-reset fees with no reader.
//! * `MaxNewbieBuffLevel` — the newbie-buffer ceiling; `newbie_guide` enforces
//!   its own, and Java's field is unused.
//! * `MentorPenaltyForMenteeComplete`, `MentorPenaltyForMenteeLeave` — the
//!   mentor system, and dead twice over: **both keys assign to the same
//!   field** (`MENTOR_PENALTY_FOR_MENTEE_COMPLETE`), so the "leave" penalty has
//!   never done anything in any Mobius build.

use std::collections::HashMap;

use crate::config::common::parse_tuples_separated_by_semicolon;
use crate::model::MAX_VITALITY_POINTS;
use commons::config::PropertiesParser;

pub const CHARACTER_CONFIG_FILE: &str = "config/Character.ini";
/// `CharacterDataStoreInterval` lives in Java's `General.ini`, not `Character.ini`.
const GENERAL_CONFIG_FILE: &str = "config/General.ini";

#[derive(Debug, Clone)]
pub struct CharacterConfig {
    /// `CastleZoneFameTaskFrequency` (seconds) — how often a registered
    /// participant standing in a castle siege zone is paid fame.
    pub castle_zone_fame_task_frequency: i32,
    /// `CastleZoneFameAquirePoints` — how much each payment is worth. **0 on
    /// this dist**, which is what makes the whole task inert here; Java gates
    /// the task on `giveFame() && frequency > 0`, so a 0 amount still arms the
    /// task and pays nothing, exactly as ported.
    pub castle_zone_fame_acquire_points: i32,
    /// `FameForDeadPlayers` (False on this dist) — whether a corpse lying in
    /// the zone keeps earning.
    pub fame_for_dead_players: bool,
    /// `RemoveCastleCirclets` (True on this dist) — strip the castle's circlet
    /// from a clan's members when it loses the castle, and from a member who
    /// leaves the clan. Java gates both call sites on this one flag.
    pub remove_castle_circlets: bool,
    /// `DeleteCharAfterDays`: 0 = delete immediately, else mark with a timer.
    pub delete_days: i32,
    /// `StartingAdena`: adena a freshly created character receives.
    pub starting_adena: i64,
    /// `MaxAdena` (Java `Config.MAX_ADENA` → `Inventory.MAX_ADENA`): the ceiling
    /// on a single adena pile. **9 999 999 999 999 on this dist**, not the Java
    /// default 99 900 000 000; a negative value means `Long.MAX_VALUE`. Read by
    /// the castle treasury's clamp (`Castle.addToTreasuryNoTax`) — the shop and
    /// private-store paths still carry their own hard-coded ceilings.
    pub max_adena: i64,
    /// `MaximumWarehouseSlotsForClan` (**200** here; Java's default is 150) —
    /// `Config.WAREHOUSE_SLOTS_CLAN`, the clan warehouse's slot ceiling, read by
    /// `ClanWarehouse.validateCapacity` when the manor checks whether its next
    /// period's crops would fit.
    pub warehouse_slots_clan: i32,
    /// `AltKarmaPlayerCanUseWareHouse` (**True** here, Java's default) —
    /// whether a negative-reputation character may use the warehouse/freight.
    pub alt_karma_player_can_use_warehouse: bool,
    /// `FreightPrice` (**1000** here) — adena charged per item *slot* sent
    /// through the freight (Java `Config.ALT_FREIGHT_PRICE`).
    pub freight_price: i32,
    /// `MaximumFreightSlots` (**200** here) — the recipient freight's ceiling
    /// (Java `PlayerFreight.validateCapacity`).
    pub freight_slots: i32,
    /// `RestorePetOnReconnect` / `RestoreServitorOnReconnect` — a summon that
    /// was out at logout comes back on the next login. **Both True on this
    /// dist**, so the reconnect path is live content, not an opt-in.
    /// `KeyboardMovement` (True on this dist) — gates the two rotation
    /// packets the client sends while turning with the keyboard. Java returns
    /// from both handlers when it is off, so a false here makes a turning
    /// player's heading stop propagating to onlookers.
    pub keyboard_movement: bool,
    pub restore_pet_on_reconnect: bool,
    pub restore_servitor_on_reconnect: bool,
    /// `AutoLoot`: monster drops go straight to the killer's inventory (the
    /// ground-drop path is not ported yet — see G9 notes).
    pub auto_loot: bool,
    /// `AutoLootRaids`: the raid counterpart of `AutoLoot` — **off** on this
    /// dist, so raid drops fall to the ground even though `AutoLoot` is on.
    pub auto_loot_raids: bool,
    /// `DisableTutorial`: skips the Q255 newbie tutorial login hook (False on
    /// this dist).
    pub disable_tutorial: bool,
    /// `RaidLootRightsInterval` (seconds): how long a raid drop stays owned by
    /// the privileged command channel's leader.
    pub raid_loot_rights_interval: u64,
    /// `RaidLootRightsCCSize`: the minimum command-channel member count that
    /// earns raid looting rights.
    pub raid_loot_rights_cc_size: i32,
    /// `RespawnRestoreCP/HP/MP` (percent of max on revive).
    pub respawn_restore_cp: f64,
    pub respawn_restore_hp: f64,
    pub respawn_restore_mp: f64,
    /// `MaxPvtStoreBuySlotsDwarf` / `MaxPvtStoreBuySlotsOther` (5 / 4 here) —
    /// how many wanted lines a private *buy* store may carry
    /// (`Player.getPrivateBuyStoreLimit`, race-dependent).
    pub max_pvtstore_buy_slots_dwarf: i32,
    pub max_pvtstore_buy_slots_other: i32,
    /// `MaxPvtStoreSellSlots{Dwarf,Other}` (4 / 3 on this dist) — the sell
    /// twin (`Player.getPrivateSellStoreLimit`).
    pub max_pvtstore_sell_slots_dwarf: i32,
    pub max_pvtstore_sell_slots_other: i32,
    /// `AltPartyRange`: also the max reward distance from a killed monster.
    pub alt_party_range: i32,
    /// `Delevel` + `DelevelMinimum`: whether death XP loss can drop a level,
    /// and the floor it can't drop below.
    pub player_delevel: bool,
    pub delevel_minimum: i32,
    /// `RandomRespawnInTownEnabled`: pick a random town respawn point instead
    /// of the first.
    pub random_respawn_in_town: bool,
    /// `AltPartyMaxMembers` (9 on this dist, Java default 7).
    pub alt_party_max_members: usize,
    /// `BlowRateChanceLimit`: the cap (%) on a dagger blow's land chance
    /// (`Formulas.calcBlowSuccess`). 100 on this dist, Java default 80.
    pub blow_rate_chance_limit: f64,
    /// `AltLeavePartyLeader`: leader leaving transfers lead instead of
    /// disbanding (True on this dist).
    pub alt_leave_party_leader: bool,
    /// `EnableVitality`: master switch for the vitality system (True on this
    /// dist). Java gates `PlayerStat.updateVitalityPoints` and the daily/weekly
    /// resets on it.
    pub enable_vitality: bool,
    /// `StartingVitalityPoints`: vitality a freshly created character gets
    /// (0 on this dist; Java's default is `MAX_VITALITY_POINTS`).
    pub starting_vitality_points: i32,
    /// `PetitioningAllowed`: whether players may file GM petitions (True). Java
    /// `PetitionManager.isPetitioningAllowed` (G31).
    pub petitioning_allowed: bool,
    /// `MaxPetitionsPerPlayer`: petitions one player may submit per period (5).
    pub max_petitions_per_player: i32,
    /// `MaxPetitionsPending`: total pending petitions the queue holds (25).
    pub max_petitions_pending: i32,
    /// `RaidbossUseVitality`: whether raid-boss kills move vitality at all
    /// (False on this dist, so boss kills neither consume nor grant points).
    pub raidboss_use_vitality: bool,
    /// `PartyXpCutoffMethod` (+ its per-method tuning): which rewarded
    /// members share the party XP split, and — for "highfive" (this dist) —
    /// the per-member level-gap percentage table.
    pub party_xp_cutoff_method: String,
    pub party_xp_cutoff_level: i32,
    pub party_xp_cutoff_percent: f64,
    pub party_xp_cutoff_gaps: Vec<(i32, i32)>,
    pub party_xp_cutoff_gap_percents: Vec<i32>,
    /// `MaximumSlotsForNoDwarf`/`MaximumSlotsForDwarf`: the ordinary
    /// inventory-slot cap (`Player.getInventoryLimit`). GM/belt bonuses
    /// aren't wired — no access-level or `Stat.INVENTORY_NORMAL` on the live
    /// player model yet.
    /// `AltWeightLimit` — a multiplier on Java's CON-derived carry limit.
    /// **3** on this dist.
    pub alt_weight_limit: f64,
    pub inventory_max_no_dwarf: i32,
    pub inventory_max_dwarf: i32,
    /// `MaximumSlotsForGMPlayer` — GMs get their own, larger cap.
    pub inventory_max_gm: i32,
    /// `MaximumSlotsForQuestItems` (`Player.getQuestInventoryLimit`): quest
    /// items are checked against this separate cap, never the ordinary one
    /// (`PlayerInventory.validateCapacity`'s `questItem` branch).
    pub inventory_max_quest_items: i32,
    /// `CraftingEnabled`: master switch for the crafting subsystem (recipe
    /// registration + item creation). True on this dist.
    pub crafting_enabled: bool,
    /// `DwarfRecipeLimit` / `CommonRecipeLimit`: max recipes registrable in each
    /// book (`Player.getDwarfRecipeLimit` / `getCommonRecipeLimit`, before the
    /// `Stat.RECIPE_DWARVEN/COMMON` modifiers — no source grants those here).
    pub dwarf_recipe_limit: i32,
    pub common_recipe_limit: i32,
    /// `CraftMasterwork` + `CraftMasterworkChance`: whether a recipe's rare
    /// (`productionRare`) output can roll, and the fallback rarity when the
    /// recipe omits its own.
    pub craft_masterwork: bool,
    /// `AltGameCreation` — the staged multi-pass craft: the create skill
    /// animates per pass, materials are "equipped" in grabs, and the finish
    /// awards XP/SP. `False` on this dist (crafts finish inline).
    pub alt_game_creation: bool,
    /// `AltSubClassWithoutQuests` — skip the Fate's Whisper + Mimir's Elixir
    /// completion gate on adding a subclass. **True on this dist**, so the
    /// quest gate is ported but inert.
    pub alt_sub_class_without_quests: bool,
    /// `AltGameCreationSpeed` — per-pass delay multiplier (1 here).
    pub alt_game_creation_speed: f64,
    /// `AltGameCreationXpRate` / `AltGameCreationSpRate` /
    /// `AltGameCreationRareXpSpRate` — the staged craft's reward scaling.
    pub alt_game_creation_xp_rate: f64,
    pub alt_game_creation_sp_rate: f64,
    pub alt_game_creation_rare_xpsp_rate: f64,
    pub craft_masterwork_chance: i32,
    /// `AutoLearnSkills`: when true, `Player.rewardSkills` grants every class
    /// skill reachable at the player's level (not just autoGet skills), on
    /// enter-world and every level-up (Java `giveAvailableSkills`).
    pub auto_learn_skills: bool,
    /// `AutoLearnSkillsWithoutItems` (Java `giveAvailableSkills`'
    /// `includeRequiredItems`): when true, `AutoLearnSkills` also grants class
    /// skills that normally require a consumable book (e.g. Divine Inspiration);
    /// when false those are skipped by the auto-learn path.
    pub auto_learn_skills_without_items: bool,
    /// `AutoLearnDivineInspiration`: Divine Inspiration (skill 1405) is excluded
    /// from `AutoLearnSkills` unless this is set (or the learner is a GM) — Java
    /// `getAvailableSkills`' explicit `CommonSkill.DIVINE_INSPIRATION` guard.
    pub auto_learn_divine_inspiration: bool,
    /// `DivineInspirationSpBookNeeded` (Java `Config.DIVINE_SP_BOOK_NEEDED`,
    /// default `true`, **`False` on this dist**): whether learning Divine
    /// Inspiration costs its Ancient Book. When false, `checkPlayerSkill`
    /// returns early for skill 1405 — which skips the book *and*, because that
    /// return sits above the SP deduction, makes the skill free of SP too (the
    /// earlier "enough SP?" gate still applies). Java quirk, kept verbatim.
    pub divine_inspiration_sp_book_needed: bool,
    /// `ExpertisePenalty`: when true, equipping a weapon/armor whose grade
    /// exceeds the character's expertise level applies the grade-penalty debuff
    /// skills (Java `Player.refreshExpertisePenalty`, gated on this flag).
    pub expertise_penalty: bool,
    /// `DecreaseSkillOnDelevel`: when true, a skill whose learn level the
    /// character has dropped below (on delevel, or found out of range at login)
    /// is downgraded to the highest still-reachable level, or removed if none
    /// remains (Java `Player.checkPlayerSkills`).
    pub decrease_skill_level: bool,
    /// `StrictDelevelSkillRemoval`: drop the 9-level grace Java's
    /// `checkPlayerSkills` normally applies, so a skill is downgraded/removed
    /// the moment the character's level falls below its learn level (level-exact
    /// matching, same rule Java uses for Expertise).
    ///
    /// **Ships and defaults to `false`, i.e. the Java-faithful grace.** A port
    /// extension with no upstream key, so the default deliberately sits on the
    /// retail branch: an operator who has never heard of this knob gets the
    /// reference behaviour, and turning it on is the explicit choice.
    pub strict_delevel_skill_removal: bool,
    /// `CharacterDataStoreInterval` (General.ini, minutes → game ticks): the
    /// period of the staggered per-player autosave flush (Java
    /// `PlayerAutoSaveTaskManager` / `CHAR_DATA_STORE_INTERVAL`). Character state
    /// is otherwise memory-only until logout/shutdown; this bounds how much a
    /// crash can lose. Expressed in 100 ms ticks (`minutes * 600`).
    pub character_data_store_interval_ticks: u64,
    /// Stat finalizer ceilings + the flat `RunSpeedBoost` (`MaxPAtk`,
    /// `MaxMAtk`, `MaxPCritRate`, `MaxMCritRate`, `MaxPAtkSpeed`,
    /// `MaxMAtkSpeed`, `MaxEvasion`, `RunSpeedBoost`). Consumed at boot into
    /// `GameData::combat_caps`, which the stat engine clamps/offsets with.
    /// Defaults are this dist's Character.ini values.
    pub run_spd_boost: f64,
    pub max_p_atk: f64,
    pub max_m_atk: f64,
    pub max_p_crit_rate: f64,
    pub max_m_crit_rate: f64,
    pub max_p_atk_speed: f64,
    pub max_m_atk_speed: f64,
    pub max_evasion: f64,
    /// `MaxRunSpeed`: `SpeedFinalizer`'s player move-speed ceiling (300 on
    /// this dist); GMs bypass it via the MAX_STATS_VALUE cond override.
    pub max_run_speed: f64,
    /// `MaxRunSpeedSummon`: `SummonStat`'s own ceiling, separate from the
    /// player's — Java's comment at the site: *"In retail maximum run speed is
    /// 350 for summons and 300 for players"*.
    pub max_run_speed_summon: f64,
    /// `MaxHP`: `MaxHpFinalizer`'s player HP ceiling (`min(maxHp,
    /// MAX_HP * mul + add)`). NPCs are uncapped — that branch is player-only.
    pub max_hp: f64,
    /// `MaxSp`: `PlayableStat.addExpAndSp`'s SP ceiling. Java reads it as
    /// `getLong(...) >= 0 ? value : Long.MAX_VALUE`, so a **negative value
    /// means unlimited** rather than "no SP allowed"; [`Self::sp_ceiling`]
    /// applies that.
    pub max_sp: i64,
    /// `MinAbnormalStateSuccessRate` / `MaxAbnormalStateSuccessRate` — the
    /// `constrain(rate, minChance, maxChance)` bounds every debuff's land rate
    /// passes through in `Formulas.calcEffectLandRate`. 10/90 here, which is
    /// why nothing on this dist is ever a guaranteed or impossible debuff
    /// (bar the two paths that bypass the clamp — see the formula's notes).
    pub min_abnormal_state_success_rate: f64,
    pub max_abnormal_state_success_rate: f64,
    /// `MaximumWarehouseSlotsForDwarf` / `...ForNoDwarf` — the private
    /// warehouse's base slot count before `Stat::StoragePrivate` (Expand
    /// Warehouse) is finalized on top. The clan and freight ceilings already
    /// live above as `warehouse_slots_clan` / `freight_slots`.
    pub warehouse_slots_dwarf: i32,
    pub warehouse_slots_no_dwarf: i32,
    /// `AltMaxNumOfClansInAlly` — how many clans one alliance may hold.
    pub max_num_of_clans_in_ally: usize,
    /// `AltClanMembersForWar` — the member count both sides need before a clan
    /// war may be declared (alongside clan level 3).
    pub clan_members_for_war: usize,
    /// `MaxEquipableItemGrade` — the highest crystal grade a shop, multisell
    /// or recipe list will offer. Java parses it as a `CrystalType` name and
    /// defaults to `EVENT` (i.e. no filtering); this dist ships **S**, which
    /// is what makes the filter bite at all.
    pub max_equipable_item_grade: crate::data::item_data::CrystalType,
    /// `MaxBuffAmount`: the good-buff slot cap (Java `Config.BUFFS_MAX_AMOUNT` →
    /// `getMaxBuffCount`; 24 on this dist). When exceeded the oldest buff is
    /// dropped (`EffectList.addActive`).
    pub max_buff_count: i32,
    /// `MaxSubclass` (5) — how many subclass slots a character may hold.
    pub max_subclass: i32,
    /// `MaxDanceAmount`: the dance/song slot cap (Java `DANCES_MAX_AMOUNT`; 12
    /// on this dist). Dances/songs are counted separately from buffs.
    pub max_dance_count: i32,
    /// `VampiricAttackWorkWithSkills` (**False** on this dist) — whether the
    /// `VampiricAttack` HP absorb fires on skill damage as well as melee. Java
    /// gates it as `skill == null || VAMPIRIC_ATTACK_WORKS_WITH_SKILLS`, so with
    /// it off Vampiric Rage only feeds off auto-attacks.
    pub vampiric_attack_works_with_skills: bool,
    /// `VampiricAttackAffectsPvP` — lives in **PVP.ini**, not Character.ini
    /// (**True** here). With it off, `isPlayable()` attacker + playable target
    /// absorbs nothing.
    pub vampiric_attack_affects_pvp: bool,
    /// `MpVampiricAttackWorkWithMelee` (**False** on this dist) — the MP twin
    /// of `VampiricAttackWorkWithSkills`, and note it is the *inverse* shape:
    /// Java's gate is `(skill != null) || MP_VAMPIRIC_ATTACK_WORKS_WITH_MELEE`,
    /// so MP vampirism works with **skills** by default and only reaches melee
    /// when this is on.
    pub mp_vampiric_attack_work_with_melee: bool,
    /// `MpVampiricAttackAffectsPvP` (PvP.ini; **false** default, and unset on
    /// this dist).
    pub mp_vampiric_attack_affects_pvp: bool,
    /// `PlayerReflectPercentLimit` / `NonPlayerReflectPercentLimit` (**100**
    /// each) — the ceiling `Creature.doAttack` clamps
    /// `REFLECT_DAMAGE_PERCENT` to, chosen by whether the *reflecting* side is
    /// a player.
    pub player_reflect_percent_limit: f64,
    pub non_player_reflect_percent_limit: f64,
    /// `DanceConsumeAdditionalMP` (Java `DANCE_CONSUME_ADDITIONAL_MP`): each
    /// dance already running adds `ceil(mpConsume / 2)` to the next dance's MP
    /// cost. **False on this dist**, so the surcharge is off — but
    /// `CreatureStat.getMpConsume` reads the flag, so the port does too.
    pub dance_consume_additional_mp: bool,
    /// `StoreSkillCooltime`: persist active buffs *and* skill reuse cooldowns to
    /// `character_skills_save` on flush and restore them on login (Java
    /// `Player.storeEffect`/`restoreEffects` — the one flag gates both halves).
    /// True on this dist.
    pub store_skill_cooltime: bool,
    /// `AltStoreDances`: whether dances/songs survive a relog. Off in retail
    /// (and Java's default) — `storeEffect` drops them at logout; this dist's
    /// Character.ini turns them on.
    pub alt_store_dances: bool,
    /// `DanceCancelBuff`: whether a dance/song may be stripped by the client's
    /// alt+click buff-cancel (`RequestDispel`). Java default False; this dist's
    /// Character.ini sets it True.
    pub dance_cancel_buff: bool,
    /// `MaxFreeTeleportLevel`: gatekeeper NORMAL/HUNTING teleports are free at
    /// or below this level (40 on this dist, Java default 99).
    pub max_free_teleport_level: i32,
    /// `AltKarmaPlayerCanUseGK`: whether a negative-reputation character may
    /// use gatekeepers (False — Java default and this dist).
    pub alt_karma_player_can_use_gk: bool,
    /// `AltKarmaPlayerCanShop` (**False** here) — whether a player with
    /// negative reputation may open a merchant's or fisherman's dialog at all.
    /// Java refuses by showing `<npcId>-pk.htm` in place of the normal page,
    /// so it only bites where that file exists: 92 merchants and one fisherman
    /// on this dist.
    pub alt_karma_player_can_shop: bool,

    // --- The karma gates and the arrival/teleport protection window ---------
    /// `AltKarmaPlayerCanBeKilledInPeaceZone` — **False**. Java's use is
    /// `if (ALT_GAME_KARMA_PLAYER_CAN_BE_KILLED_IN_PEACEZONE) { … }` inside
    /// `Creature.onForcedAttack`'s peace-zone refusal, so with it off the
    /// refusal stands for everyone and a PK is as safe in town as anyone else.
    /// The port's peace-zone gate is already unconditional, which is the same
    /// behaviour.
    pub alt_karma_player_can_be_killed_in_peace_zone: bool,
    /// `AltKarmaPlayerCanTeleport` — **True**, and Java's guards are all
    /// `if (!ALT_GAME_KARMA_PLAYER_CAN_TELEPORT && reputation < 0)`, so with it
    /// on a criminal teleports freely. Inert at this value; carried beside its
    /// two siblings so the trio reads as one rule.
    pub alt_karma_player_can_teleport: bool,
    /// `AltKarmaPlayerCanTrade` — **True**, same shape: the `TradeRequest` and
    /// `RequestGiveItemToPet` refusals never fire.
    pub alt_karma_player_can_trade: bool,
    /// `PlayerSpawnProtection` (seconds) — how long after entering the world a
    /// character is **protected from aggressive monsters**. Not
    /// invulnerability: Java's only real consumer is `Attackable.getHating`,
    /// which drops a protected player from the aggro list, plus
    /// `Summon.isInvul`, which does make the *pet* invulnerable meanwhile. The
    /// window ends at the player's first deliberate action
    /// (`Player.onActionRequest`), so the 600 here is a ceiling and not ten
    /// minutes of safety.
    pub player_spawn_protection: i32,
    /// `PlayerTeleportProtection` (seconds) — the same idea after a teleport,
    /// except that this one *is* real invulnerability
    /// (`Player.isInvul() = super.isInvul() || isTeleportProtected()`).
    /// **0 on this dist**, so it never arms; the port parses it and does not
    /// wire the invulnerability, because that branch cannot fire here.
    pub player_teleport_protection: i32,
    /// `OffsetOnTeleportEnabled` / `MaxOffsetOnTeleport` — scatter a teleport
    /// arrival inside a radius instead of stacking everyone on the exact point
    /// (`Creature.teleToLocation`'s `randomOffset`).
    pub offset_on_teleport_enabled: bool,
    pub max_offset_on_teleport: i32,
    /// `DisconnectAfterDeath` — **False**. Java would kick the player when the
    /// "to village" window is dismissed and shorten the corpse decay to an
    /// hour; neither fires at this value.
    pub disconnect_after_death: bool,

    // --- What may be enchanted, and what may be augmented ------------------
    /// `EnchantBlackList` — item ids that refuse enchanting outright. Java
    /// ANDs it into `ItemTemplate.isEnchantable()`
    /// (`binarySearch(ENCHANT_BLACKLIST, id) < 0 && _enchantable`), so it is a
    /// veto on top of the template's own flag.
    pub enchant_black_list: Vec<i32>,
    /// `AugmentationBlackList` — the same idea for `AbstractRefinePacket`'s
    /// target check, which is its last gate.
    pub augmentation_black_list: Vec<i32>,
    /// `DisableOverEnchanting` — **True**. Refuses a scroll whose target is
    /// already at the scroll's own ceiling or at the item's `enchantLimit`.
    /// The port already enforced that unconditionally inside `accepts_target`;
    /// the key now gates it.
    pub disable_over_enchanting: bool,
    /// `OverEnchantProtection` — **True**: scan the inventory on login,
    /// destroy anything enchanted past the ceilings derived from
    /// `EnchantItemGroups.xml`, and punish the owner.
    ///
    /// **Read the deviation note before changing this.** On this dist the
    /// derived accessory ceiling is **0**, because Java infers the three
    /// ceilings from enchant-group *names* and none of this dist's four groups
    /// matches its accessory patterns. Taken literally that destroys every
    /// enchanted ring, earring and necklace on the server and jails the owner.
    /// See `docs/CUSTOM_DIST_DEVIATIONS.md`.
    pub over_enchant_protection: bool,
    /// `OverEnchantPunishment` — what the scan above does to the owner.
    /// `JAIL` here; shares the `Util.handleIllegalPlayerAction` plumbing with
    /// `General.ini`'s `DefaultPunish`.
    pub over_enchant_punishment: crate::model::punishment::IllegalActionPunishment,
    /// `AltAllowAugmentPvPItems` — **False**, and **unreachable on this dist**:
    /// the gate is `item.isPvp() && !config`, and no item in
    /// `data/stats/items` declares `is_pvp` at all.
    pub alt_allow_augment_pvp_items: bool,
    /// `AltAllowAugmentTrade` — **True**. Java's `Item.isTradeable`,
    /// `isSellable` and `isDropable` each open with
    /// `if (config && isAugmented()) return true`, so at this value an
    /// augmented item trades *regardless of its template's own flag*. The port
    /// applies no augmentation gate on those paths, which is the same answer;
    /// turning it off would need one added at each.
    pub alt_allow_augment_trade: bool,
    /// `AltAllowAugmentDestroy` — **True**, so `Item.isDestroyable`'s
    /// `if (!config && isAugmented()) return false` never fires.
    pub alt_allow_augment_destroy: bool,

    // --- Clan and alliance timers -----------------------------------------
    /// `DaysBeforeJoinAClan` — the rejoin penalty stamped on a member who
    /// leaves or is ousted, and on the ousting clan.
    pub alt_clan_join_days: i32,
    /// `DaysBeforeCreateAClan` — how long a dissolved clan's leader waits
    /// before founding another. **10** here, the one day-key that is not 1.
    pub alt_clan_create_days: i32,
    /// `DaysToPassToDissolveAClan` — the delay between requesting dissolution
    /// and it taking effect.
    pub alt_clan_dissolve_days: i32,
    /// The four alliance penalties, one per `ally_penalty_type`:
    /// `DaysBeforeJoinAllyWhenLeaved` (the clan that left),
    /// `DaysBeforeJoinAllyWhenDismissed` (the clan that was dismissed),
    /// `DaysBeforeAcceptNewClanWhenDismissed` (the leader clan that did the
    /// dismissing) and `DaysBeforeCreateNewAllyWhenDissolved` (the leader clan
    /// after dissolving). All 1 here, which is why one shared constant passed
    /// unnoticed.
    pub alt_ally_join_days_when_leaved: i32,
    pub alt_ally_join_days_when_dismissed: i32,
    pub alt_accept_clan_days_when_dismissed: i32,
    pub alt_create_ally_days_when_dissolved: i32,
    /// `AltMembersCanWithdrawFromClanWH` — **False**, and the two branches are
    /// not "privilege vs. no check": with it **on** the gate is the
    /// `CL_VIEW_WAREHOUSE` privilege, with it **off** only the *clan leader*
    /// may withdraw at all.
    pub alt_members_can_withdraw_from_clan_wh: bool,
    /// `AltClanLeaderInstantActivation` — **False**, so nominating a successor
    /// only records `new_leader_id` and the handover happens on the daily
    /// task. With it on, `Clan.setNewLeader` runs immediately.
    pub alt_clan_leader_instant_activation: bool,
    /// `AltClanMembersTimeForBonus` (millis; the ini writes `30mins`) — how
    /// long a member must have been online before `ClanMember.getOnlineStatus`
    /// reports `2` rather than `1`. The port does not track per-member online
    /// time, so nothing reads this yet; parsed so the value is visible.
    pub alt_clan_members_time_for_bonus_ms: i64,
    /// `AltCommandChannelFriends` — **True**: two parties in the same command
    /// channel cannot attack each other.
    pub alt_command_channel_friends: bool,
    /// `LifeCrystalNeeded` — **True**, and inert: it gates whether a pledge
    /// skill's required items are consumed and shown, and no entry in this
    /// dist's pledge tree declares any.
    pub life_crystal_needed: bool,

    // --- Interruption and fake death --------------------------------------
    /// `AltGameCancelByHit`, split the way Java splits it: one string key
    /// (`bow` / `cast` / `all` / anything else) into two booleans. **`cast`**
    /// here, so a hit can interrupt a *cast* but never a bow shot.
    /// `Formulas.calcAtkBreak` sets its `init` to 15 when the matching branch
    /// applies and returns `false` outright when `init <= 0` — so with both
    /// off nothing is ever interrupted by damage.
    pub alt_game_cancel_cast: bool,
    pub alt_game_cancel_bow: bool,
    /// `BreakStun` — **True**: a hit on a stunned target has a 1-in-14 chance
    /// to free them (`Formulas.calcStunBreak`). Java's own default is `false`,
    /// so this is one of the few keys where the dist opts *into* behaviour.
    pub alt_game_stun_break: bool,
    /// `FakeDeathDamageStand` — **True**: any damage while playing dead stands
    /// the player back up.
    pub fake_death_damage_stand: bool,
    /// `FakeDeathUntarget` — **False**, so the sweep that clears the feigning
    /// player out of everyone else's target slot never runs. The port has no
    /// such sweep, which is the same behaviour.
    pub fake_death_untarget: bool,
    /// `PlayerFakeDeathUpProtection` (seconds) — a grace window against
    /// aggressive monsters after *standing up* from fake death, the sibling of
    /// `PlayerSpawnProtection`. **0**, so it never arms.
    pub player_fake_death_up_protection: i32,
    /// `EffectTickRatio` (ms) — the period of one effect "tick", so a
    /// `ticks="N"` effect fires every `N × ratio` ms and each tick is worth
    /// `power × N × ratio / 1000` (`AbstractEffect.getTicksMultiplier`).
    pub effect_tick_ratio_ms: i64,
    /// `MaxTriggeredBuffAmount` — `EffectList.isLimitExceeded`'s cap on
    /// concurrently active **trigger** buffs. Parsed and not wired: the port's
    /// buff list does not classify a buff as `SkillBuffType.TRIGGER`, so there
    /// is no separate count to cap. Wiring it means adding that
    /// classification first.
    pub triggered_buffs_max_amount: i32,

    // --- Cooldowns and what survives a session ----------------------------
    /// `ArmorSetEquipActiveSkillReuse` (ms) — completing an armour set grants
    /// its active skills, and Java stamps a reuse on them straight away so the
    /// set cannot be re-equipped to refire them. The skill's own reuse wins
    /// when it has one; this is the fallback for the ones that do not.
    /// `0` disables the stamp entirely.
    pub armor_set_equip_active_skill_reuse_ms: i32,
    /// `ItemEquipActiveSkillReuse` (ms) — the same rule for a *single item's*
    /// `ON_EQUIP` actives. Parsed and unwired: the port grants no per-item
    /// equip skills (only armour-set ones), so there is nothing to stamp. It
    /// becomes live the moment `ItemSkillType.NORMAL` grants land.
    pub item_equip_active_skill_reuse_ms: i32,
    /// `StoreCharUiSettings` — whether a character's saved keybindings are
    /// replayed on `RequestKeyMapping` and accepted on `RequestSaveKeyMapping`.
    pub store_ui_settings: bool,
    /// `EnableModifySkillReuse` / `SkillReuseList` — an override map keyed by
    /// skill id that replaces a skill's declared reuse. **The flag is False and
    /// the list is empty**, and Java's guard is
    /// `if (ENABLE_MODIFY_SKILL_REUSE && SKILL_REUSE_LIST.containsKey(id))`, so
    /// neither half can fire. The list is parsed so a value put there is
    /// visible; honouring it would go in `Skill`'s reuse lookup.
    pub enable_modify_skill_reuse: bool,
    pub skill_reuse_list: std::collections::HashMap<i32, i32>,
    /// `SubclassStoreSkillCooltime` — Java's `setActiveClass` calls
    /// `store(SUBCLASS_STORE_SKILL_COOLTIME)` to flush cooldowns *before*
    /// `resetTimeStamps()` wipes them. The port saves memory-first on its own
    /// interval and clears `Reuses` at the switch (same end state), so there is
    /// no distinct "save with or without cooltimes" moment for this to gate —
    /// a persistence-model difference, like `General.ini`'s save keys.
    pub subclass_store_skill_cooltime: bool,
    /// `SummonStoreSkillCooltime` — whether a pet/servitor's effects and skill
    /// cooldowns survive an unsummon (`Pet.storeEffect`). The port does not
    /// persist summon effects at all, so nothing reads this yet.
    pub summon_store_skill_cooltime: bool,
    /// `StoreRecipeShopList` — whether a manufacture store's contents survive a
    /// logout. **False**, which is exactly the port's behaviour: manufacture
    /// stores are transient. Turning it on would need the persistence added.
    pub store_recipe_shop_list: bool,

    // --- Character creation, and what a kill drops in your lap ------------
    /// `StartingLevel` / `StartingSP` — what a freshly created character
    /// starts on. Java guards both (`> 1`, `> 0`), and this dist ships the
    /// neutral 1 and 0, so neither add fires.
    pub starting_level: i32,
    pub starting_sp: i64,
    /// `InitialEquipmentEvent` — picks `initialEquipmentEvent.xml` over
    /// `initialEquipment.xml` for a new character's gear. **True** here, and
    /// the two files are byte-identical on this dist, so it changes nothing
    /// today; wired anyway, because editing the event file is the whole point
    /// of having it.
    pub initial_equipment_event: bool,
    /// `ForbiddenNames` — substrings a character name may not contain, matched
    /// case-insensitively. The shipped list is announcement lookalikes
    /// (`annou`, `ammou`, …), which stop a player naming themselves something
    /// that reads as a server message in chat.
    pub forbidden_names: Vec<String>,
    /// `AutoLootHerbs` — **False**. Herbs are the `ex_immediate_effect` items
    /// that fire on contact, and Java deliberately excludes them from ordinary
    /// `AutoLoot`: that arm is gated on `!hasExImmediateEffect()`, so only this
    /// key can auto-loot one. With it off they fall to the ground to be walked
    /// over, which is the mechanic that makes a herb a herb.
    pub auto_loot_herbs: bool,
    /// `AutoLootItemIds` — item ids always auto-looted, whatever the other
    /// flags say. Ships as `0`, which is not an item id, so the set is
    /// effectively empty.
    pub auto_loot_item_ids: std::collections::HashSet<i32>,
    /// `AutoLootSlotLimit` — **True**, which reduces
    /// `PlayerInventory.validateCapacity` to "quest items against the quest
    /// limit, everything else against the normal one" — what the port already
    /// does. The `False` branch is Java's quirk of checking a zero-slot add
    /// against the *quest* limit.
    pub auto_loot_slot_limit: bool,

    // --- Subclasses, and what may be learned ------------------------------
    /// `BaseSubclassLevel` — the level (and matching exp) a newly added
    /// subclass starts on.
    pub base_subclass_level: i32,
    /// `MaxSubclassLevel` — a subclass's own ceiling, which Java takes as
    /// `min(MaxSubclassLevel, experienceTable.maxLevel - 1)`. Separate from the
    /// main class's cap: a subclass stops at 80 here while the base class does
    /// not.
    pub max_subclass_level: i32,
    /// `BaseDualclassLevel` — the dual-class starting level. Dual class is an
    /// Ertheia system with no Interlude counterpart, so nothing reaches this.
    pub base_dualclass_level: i32,
    /// `AltSubclassEverywhere` — **True**, so `VillageMaster.checkVillageMaster`
    /// returns `true` outright and *any* master will add a subclass, rather
    /// than only one matching the class's race and teach type. The port has no
    /// such gate, which is the same behaviour.
    pub alt_game_subclass_everywhere: bool,
    /// `AutoLearnForgottenScrollSkills` — whether the `AutoLearnSkills` grant
    /// also hands out Forgotten Scroll skills. Reachable here (`AutoLearnSkills`
    /// is **True**) but empty-handed: this dist's base-class trees carry no
    /// forgotten-scroll entries for it to include.
    pub auto_learn_fs_skills: bool,
    /// `AltTransformationWithoutQuest` — **False**, so Java requires
    /// `Q00136_MoreThanMeetsTheEye` completed before a transformation skill can
    /// be learned. The port parses no `transformSkillTree.xml`, so no
    /// transformation skill is learnable and the gate has nothing to guard.
    pub allow_transform_without_quest: bool,

    // --- The last of Character.ini ----------------------------------------
    /// `MaxPersonalFamePoints` — the ceiling `Player.setFame` clamps to
    /// (`[0, max]`). **0 on this dist**, which does not mean "no limit": it
    /// means fame is *disabled*, because every award is clamped straight back
    /// to zero. The port paid castle-zone fame with no clamp at all, so players
    /// accumulated fame Java would have zeroed.
    pub max_personal_fame_points: i32,
    /// `FortressZoneFameTaskFrequency` / `FortressZoneFameAquirePoints` — the
    /// castle-zone fame pair's fortress twin (`FortSiege.startFameTask`).
    /// Fortresses are off-chronicle here, and the award is **0** besides.
    pub fortress_zone_fame_task_frequency: i32,
    /// See [`Self::fortress_zone_fame_task_frequency`].
    pub fortress_zone_fame_acquire_points: i32,
    /// `MaxExpBonus` / `MaxSpBonus` — ceilings on the *bonus* multiplier
    /// `PlayerStat` applies (vitality, premium, …). Both **0**, and Java's
    /// guard is `if (MAX_BONUS_EXP > 0)`, so neither clamp runs.
    pub max_bonus_exp: f64,
    /// See [`Self::max_bonus_exp`].
    pub max_bonus_sp: f64,
    /// `NpcTalkBlockingTime`, in **milliseconds** (Java multiplies the key by
    /// 1000 at parse) — how long a player is pinned in place after opening an
    /// NPC dialog. **0**, and every use site is guarded on `> 0`.
    pub player_movement_block_time_ms: i32,
    /// `SilenceModeExclude` — whether silence mode keeps a per-player
    /// exclusion list (friends still get through). **False**, and the port
    /// models no player-facing silence mode at all — only the GM startup flag
    /// in `General.ini` — so there is no list to exclude from.
    pub silence_mode_exclude: bool,
    /// `TeleportWhileSiegeInProgress`: may a gatekeeper send anyone to (or from)
    /// a castle town whose siege is running? **False** on this dist (Java's
    /// default is true), so both gates in `TeleportHolder.doTeleport` are live.
    pub teleport_while_siege_in_progress: bool,
    /// `UnstuckInterval` (seconds): the `/unstuck` escape cast time (30 on
    /// this dist, Java default 300 = the stock 5-minute escape skill).
    pub unstuck_interval: i32,
    /// `TeleportWatchdogTimeout` (seconds → 100 ms game ticks, **0 = off**):
    /// how long a character may sit in the teleporting state before the server
    /// finishes the teleport for them (Java `Config.TELEPORT_WATCHDOG_TIMEOUT`
    /// / `TeleportWatchdogTask`).
    ///
    /// A teleport only completes when the client answers
    /// `ExTeleportToLocationActivate` with `Appearing`; until then the
    /// character is decayed out of the world and invisible to everyone. A
    /// client that never answers — hung zone load, dropped packet, crash
    /// mid-teleport — leaves a ghost that only a relog clears. The watchdog is
    /// the escape hatch. Off by default (Java's default and this dist), i.e.
    /// the client is trusted; the ini warns against values below ~60 s,
    /// because firing before a slow client finishes loading spawns the
    /// character in early and desyncs instead of curing anything.
    pub teleport_watchdog_timeout_ticks: u64,
    /// `CalculateMagicSuccessBySkillMagicLevel`: when true (dist default), the
    /// magic-hit level modifier in `Formulas.calcMagicSuccess` uses the skill's
    /// own `magicLevel` instead of the caster's level. Drives the Spoil landing
    /// roll and the magic-damage failure roll.
    pub calculate_magic_success_by_skill_magic_level: bool,
    /// `MagicFailures` (`ALT_GAME_MAGICFAILURES`, True on this dist): gates the
    /// `Formulas.calcMagicDam`/`calcManaDam` resist branch. With it off, magic
    /// damage always lands at full strength regardless of the level gap.
    pub magic_failures: bool,
    /// `EnableModifySkillDuration` + `SkillDurationList` (`skillId,seconds;…`):
    /// when enabled, a landed buff/debuff's `abnormalTime` is overridden by the
    /// list value at skill-load time (Java `Skill` constructor), overriding the
    /// XML `abnormalTime`. On this dist it stretches songs/dances/buffs to 2h.
    /// Toggles (`operateType=T`) are exempt; enchanted levels (100–139) add the
    /// override to the base time instead of replacing it.
    pub enable_modify_skill_duration: bool,
    pub skill_duration_list: HashMap<i32, i32>,
}

impl Default for CharacterConfig {
    /// Java `Config` defaults (used by tests via `CombatConfig::default`).
    fn default() -> Self {
        Self {
            // Java's own defaults (300 / 125 / true); the dist overrides the
            // amount to 0 and dead players to false.
            castle_zone_fame_task_frequency: 300,
            castle_zone_fame_acquire_points: 125,
            fame_for_dead_players: true,
            remove_castle_circlets: true,
            delete_days: 1,
            starting_adena: 0,
            max_adena: 99_900_000_000,
            warehouse_slots_clan: 150,
            alt_karma_player_can_use_warehouse: true,
            freight_price: 1000,
            freight_slots: 200,
            keyboard_movement: true,
            restore_pet_on_reconnect: true,
            restore_servitor_on_reconnect: true,
            auto_loot: false,
            auto_loot_raids: false,
            disable_tutorial: false,
            raid_loot_rights_interval: 900,
            raid_loot_rights_cc_size: 45,
            respawn_restore_cp: 0.0,
            respawn_restore_hp: 65.0,
            respawn_restore_mp: 0.0,
            alt_party_range: 1500,
            max_pvtstore_buy_slots_dwarf: 5,
            max_pvtstore_buy_slots_other: 4,
            max_pvtstore_sell_slots_dwarf: 4,
            max_pvtstore_sell_slots_other: 3,
            player_delevel: true,
            delevel_minimum: 85,
            random_respawn_in_town: true,
            alt_party_max_members: 7,
            blow_rate_chance_limit: 80.0,
            alt_leave_party_leader: false,
            // Java `Config` defaults: vitality off, but full points when it is
            // switched on.
            enable_vitality: false,
            starting_vitality_points: MAX_VITALITY_POINTS,
            petitioning_allowed: true,
            max_petitions_per_player: 5,
            max_petitions_pending: 25,
            raidboss_use_vitality: false,
            party_xp_cutoff_method: "level".to_string(),
            party_xp_cutoff_level: 20,
            party_xp_cutoff_percent: 3.0,
            party_xp_cutoff_gaps: vec![(0, 9), (10, 14), (15, 99)],
            party_xp_cutoff_gap_percents: vec![100, 30, 0],
            alt_weight_limit: 1.0,
            inventory_max_no_dwarf: 80,
            inventory_max_dwarf: 100,
            inventory_max_gm: 250,
            inventory_max_quest_items: 100,
            crafting_enabled: true,
            dwarf_recipe_limit: 100,
            common_recipe_limit: 100,
            craft_masterwork: true,
            alt_game_creation: false,
            alt_sub_class_without_quests: false,
            alt_game_creation_speed: 1.0,
            alt_game_creation_xp_rate: 1.0,
            alt_game_creation_sp_rate: 1.0,
            alt_game_creation_rare_xpsp_rate: 1.0,
            craft_masterwork_chance: 10,
            auto_learn_skills: false,
            auto_learn_skills_without_items: true,
            auto_learn_divine_inspiration: false,
            divine_inspiration_sp_book_needed: true,
            expertise_penalty: true,
            decrease_skill_level: true,
            // The retail branch — see the field doc. Flipping this default is a
            // behaviour change for every test world, not just a fallback tweak.
            strict_delevel_skill_removal: false,
            character_data_store_interval_ticks: 15 * 600,
            run_spd_boost: 35.0,
            max_p_atk: 999_999.0,
            max_m_atk: 999_999.0,
            max_p_crit_rate: 500.0,
            max_m_crit_rate: 200.0,
            max_p_atk_speed: 1500.0,
            max_m_atk_speed: 1999.0,
            max_evasion: 250.0,
            max_run_speed: 300.0,
            max_run_speed_summon: 350.0,
            max_hp: 150_000.0,
            max_sp: 50_000_000_000,
            min_abnormal_state_success_rate: 10.0,
            max_abnormal_state_success_rate: 90.0,
            warehouse_slots_dwarf: 120,
            warehouse_slots_no_dwarf: 100,
            max_num_of_clans_in_ally: 3,
            clan_members_for_war: 15,
            max_equipable_item_grade: crate::data::item_data::CrystalType::S,
            max_buff_count: 24,
            max_subclass: 5,
            max_dance_count: 12,
            vampiric_attack_works_with_skills: true,
            vampiric_attack_affects_pvp: false,
            mp_vampiric_attack_work_with_melee: false,
            mp_vampiric_attack_affects_pvp: false,
            player_reflect_percent_limit: 100.0,
            non_player_reflect_percent_limit: 100.0,
            dance_consume_additional_mp: false,
            store_skill_cooltime: true,
            alt_store_dances: false,
            dance_cancel_buff: false,
            max_free_teleport_level: 99,
            alt_karma_player_can_use_gk: false,
            alt_karma_player_can_shop: false,
            alt_karma_player_can_be_killed_in_peace_zone: false,
            alt_karma_player_can_teleport: true,
            alt_karma_player_can_trade: true,
            player_spawn_protection: 600,
            player_teleport_protection: 0,
            offset_on_teleport_enabled: true,
            max_offset_on_teleport: 50,
            disconnect_after_death: false,
            enchant_black_list: Vec::new(),
            augmentation_black_list: Vec::new(),
            disable_over_enchanting: true,
            over_enchant_protection: true,
            over_enchant_punishment: crate::model::punishment::IllegalActionPunishment::Jail,
            alt_allow_augment_pvp_items: false,
            alt_allow_augment_trade: true,
            alt_allow_augment_destroy: true,
            alt_clan_join_days: 1,
            alt_clan_create_days: 10,
            alt_clan_dissolve_days: 1,
            alt_ally_join_days_when_leaved: 1,
            alt_ally_join_days_when_dismissed: 1,
            alt_accept_clan_days_when_dismissed: 1,
            alt_create_ally_days_when_dissolved: 1,
            alt_members_can_withdraw_from_clan_wh: false,
            alt_clan_leader_instant_activation: false,
            alt_clan_members_time_for_bonus_ms: 30 * 60 * 1000,
            alt_command_channel_friends: true,
            life_crystal_needed: true,
            alt_game_cancel_cast: true,
            alt_game_cancel_bow: false,
            alt_game_stun_break: true,
            fake_death_damage_stand: true,
            fake_death_untarget: false,
            player_fake_death_up_protection: 0,
            effect_tick_ratio_ms: 666,
            triggered_buffs_max_amount: 12,
            armor_set_equip_active_skill_reuse_ms: 60_000,
            item_equip_active_skill_reuse_ms: 300_000,
            store_ui_settings: true,
            enable_modify_skill_reuse: false,
            skill_reuse_list: std::collections::HashMap::new(),
            subclass_store_skill_cooltime: true,
            summon_store_skill_cooltime: true,
            store_recipe_shop_list: false,
            starting_level: 1,
            starting_sp: 0,
            initial_equipment_event: true,
            forbidden_names: Vec::new(),
            auto_loot_herbs: false,
            auto_loot_item_ids: std::collections::HashSet::new(),
            auto_loot_slot_limit: true,
            base_subclass_level: 40,
            max_subclass_level: 80,
            base_dualclass_level: 80,
            alt_game_subclass_everywhere: true,
            auto_learn_fs_skills: true,
            allow_transform_without_quest: false,
            max_personal_fame_points: 0,
            fortress_zone_fame_task_frequency: 300,
            fortress_zone_fame_acquire_points: 0,
            max_bonus_exp: 0.0,
            max_bonus_sp: 0.0,
            player_movement_block_time_ms: 0,
            silence_mode_exclude: false,
            teleport_while_siege_in_progress: true,
            unstuck_interval: 300,
            teleport_watchdog_timeout_ticks: 0,
            calculate_magic_success_by_skill_magic_level: true,
            magic_failures: true,
            enable_modify_skill_duration: false,
            skill_duration_list: HashMap::new(),
        }
    }
}

/// `AltGameCancelByHit` → `(ALT_GAME_CANCEL_CAST, ALT_GAME_CANCEL_BOW)`.
///
/// Java reads the key twice, once per boolean, comparing case-insensitively
/// against `cast`/`bow` with `all` setting both. Anything else leaves both
/// off, which makes `Formulas.calcAtkBreak` return `false` outright — damage
/// then interrupts nothing.
fn cancel_by_hit(raw: &str) -> (bool, bool) {
    match raw.trim().to_ascii_lowercase().as_str() {
        "cast" => (true, false),
        "bow" => (false, true),
        "all" => (true, true),
        _ => (false, false),
    }
}

/// Java's `getString(...).split(",")` → `int[]` → `Arrays.sort`. Sorted because
/// Java looks the id up with `binarySearch`, so a duplicate or out-of-order
/// entry behaves the same on both sides.
fn parse_id_list(raw: &str) -> Vec<i32> {
    let mut ids: Vec<i32> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

impl CharacterConfig {
    /// `(randomOffset) ? Config.MAX_OFFSET_ON_TELEPORT : 0`, with
    /// `OFFSET_ON_TELEPORT_ENABLED` folded in — the radius a *scattering*
    /// teleport lands within, or `0` when it should land on the exact point.
    ///
    /// Java applies this only where the caller asks for it, which on this dist
    /// is four places: the jail zone, a residence-hall teleport zone, the
    /// Olympiad observer's return to `_lastLoc`, and summons following their
    /// owner. Every other teleport — gatekeepers, quests, `//tp` — is exact,
    /// so this must never be folded into the shared teleport path.
    pub fn teleport_offset(&self) -> i32 {
        if self.offset_on_teleport_enabled {
            self.max_offset_on_teleport.max(0)
        } else {
            0
        }
    }

    /// `MAX_SP` as `PlayableStat` uses it: Java stores
    /// `getLong("MaxSp", …) >= 0 ? value : Long.MAX_VALUE`, so a **negative**
    /// configured value means "no ceiling" rather than "no SP".
    pub fn sp_ceiling(&self) -> i64 {
        if self.max_sp >= 0 {
            self.max_sp
        } else {
            i64::MAX
        }
    }

    /// `Player.getInventoryLimit()`, narrowed to the race-based base (dwarves
    /// get a bigger bag).
    pub fn inventory_limit(&self, race: i32) -> i32 {
        if race == crate::enums::Race::Dwarf as i32 {
            self.inventory_max_dwarf
        } else {
            self.inventory_max_no_dwarf
        }
    }

    /// The whole config half of `Player.getInventoryLimit()` — Java tests
    /// `isGM()` *before* the race, so a GM gets `MaximumSlotsForGMPlayer`
    /// regardless of race:
    ///
    /// ```java
    /// if (isGM()) ivlim = Config.INVENTORY_MAXIMUM_GM;
    /// else if (getRace() == Race.DWARF) ivlim = Config.INVENTORY_MAXIMUM_DWARF;
    /// else ivlim = Config.INVENTORY_MAXIMUM_NO_DWARF;
    /// ```
    ///
    /// Every place that *reports* or *enforces* the bag size goes through this
    /// so raising a `MaximumSlotsFor…` key in `Character.ini` moves all of them
    /// together; the `Stat::InventoryNormal` bonus (`EnlargeSlot`) is added on
    /// top by the caller, as Java does with `getValue(Stat.INVENTORY_NORMAL, 0)`.
    pub fn inventory_limit_for(&self, race: i32, is_gm: bool) -> i32 {
        if is_gm {
            self.inventory_max_gm
        } else {
            self.inventory_limit(race)
        }
    }

    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        let p = PropertiesParser::load_rel(root, CHARACTER_CONFIG_FILE);
        let general = PropertiesParser::load_rel(root, GENERAL_CONFIG_FILE);
        let d = Self::default();
        Self {
            delete_days: p.get_int("DeleteCharAfterDays", 1),
            starting_adena: p.get_int("StartingAdena", 0) as i64,
            // Java: `if (MAX_ADENA < 0) MAX_ADENA = Long.MAX_VALUE;`
            max_adena: match p.get_long("MaxAdena", d.max_adena) {
                v if v < 0 => i64::MAX,
                v => v,
            },
            warehouse_slots_clan: p.get_int("MaximumWarehouseSlotsForClan", d.warehouse_slots_clan),
            alt_karma_player_can_use_warehouse: p.get_bool(
                "AltKarmaPlayerCanUseWareHouse",
                d.alt_karma_player_can_use_warehouse,
            ),
            freight_price: p.get_int("FreightPrice", d.freight_price),
            freight_slots: p.get_int("MaximumFreightSlots", d.freight_slots),
            keyboard_movement: p.get_bool("KeyboardMovement", true),
            restore_pet_on_reconnect: p.get_bool("RestorePetOnReconnect", true),
            restore_servitor_on_reconnect: p.get_bool("RestoreServitorOnReconnect", true),
            castle_zone_fame_task_frequency: p.get_int(
                "CastleZoneFameTaskFrequency",
                d.castle_zone_fame_task_frequency,
            ),
            castle_zone_fame_acquire_points: p.get_int(
                "CastleZoneFameAquirePoints",
                d.castle_zone_fame_acquire_points,
            ),
            fame_for_dead_players: p.get_bool("FameForDeadPlayers", d.fame_for_dead_players),
            remove_castle_circlets: p.get_bool("RemoveCastleCirclets", d.remove_castle_circlets),
            auto_loot: p.get_bool("AutoLoot", d.auto_loot),
            auto_loot_raids: p.get_bool("AutoLootRaids", d.auto_loot_raids),
            disable_tutorial: p.get_bool("DisableTutorial", d.disable_tutorial),
            raid_loot_rights_interval: p.get_int("RaidLootRightsInterval", 900) as u64,
            raid_loot_rights_cc_size: p.get_int("RaidLootRightsCCSize", 45),
            respawn_restore_cp: p.get_float("RespawnRestoreCP", 0.0) as f64,
            respawn_restore_hp: p.get_float("RespawnRestoreHP", 65.0) as f64,
            respawn_restore_mp: p.get_float("RespawnRestoreMP", 0.0) as f64,
            max_pvtstore_buy_slots_dwarf: p
                .get_int("MaxPvtStoreBuySlotsDwarf", d.max_pvtstore_buy_slots_dwarf),
            max_pvtstore_buy_slots_other: p
                .get_int("MaxPvtStoreBuySlotsOther", d.max_pvtstore_buy_slots_other),
            max_pvtstore_sell_slots_dwarf: p
                .get_int("MaxPvtStoreSellSlotsDwarf", d.max_pvtstore_sell_slots_dwarf),
            max_pvtstore_sell_slots_other: p
                .get_int("MaxPvtStoreSellSlotsOther", d.max_pvtstore_sell_slots_other),
            alt_party_range: p.get_int("AltPartyRange", d.alt_party_range),
            player_delevel: p.get_bool("Delevel", d.player_delevel),
            delevel_minimum: p.get_int("DelevelMinimum", d.delevel_minimum),
            random_respawn_in_town: p
                .get_bool("RandomRespawnInTownEnabled", d.random_respawn_in_town),
            alt_party_max_members: p.get_int("AltPartyMaxMembers", 7).max(2) as usize,
            blow_rate_chance_limit: p.get_int("BlowRateChanceLimit", 80) as f64,
            alt_leave_party_leader: p.get_bool("AltLeavePartyLeader", d.alt_leave_party_leader),
            petitioning_allowed: p.get_bool("PetitioningAllowed", d.petitioning_allowed),
            max_petitions_per_player: p
                .get_int("MaxPetitionsPerPlayer", d.max_petitions_per_player),
            max_petitions_pending: p.get_int("MaxPetitionsPending", d.max_petitions_pending),
            enable_vitality: p.get_bool("EnableVitality", d.enable_vitality),
            starting_vitality_points: p
                .get_int("StartingVitalityPoints", d.starting_vitality_points)
                .clamp(0, MAX_VITALITY_POINTS),
            raidboss_use_vitality: p.get_bool("RaidbossUseVitality", d.raidboss_use_vitality),
            party_xp_cutoff_method: p.get_string("PartyXpCutoffMethod", "level").to_lowercase(),
            party_xp_cutoff_level: p.get_int("PartyXpCutoffLevel", 20),
            party_xp_cutoff_percent: p.get_float("PartyXpCutoffPercent", 3.0) as f64,
            party_xp_cutoff_gaps: parse_tuples_separated_by_semicolon(
                &p.get_string("PartyXpCutoffGaps", "0,9;10,14;15,99"),
            ),
            party_xp_cutoff_gap_percents: p
                .get_string("PartyXpCutoffGapPercent", "100;30;0")
                .split(';')
                .filter_map(|v| v.trim().parse().ok())
                .collect(),
            alt_weight_limit: p.get_float("AltWeightLimit", d.alt_weight_limit as f32) as f64,
            inventory_max_no_dwarf: p.get_int("MaximumSlotsForNoDwarf", d.inventory_max_no_dwarf),
            inventory_max_dwarf: p.get_int("MaximumSlotsForDwarf", d.inventory_max_dwarf),
            inventory_max_gm: p.get_int("MaximumSlotsForGMPlayer", d.inventory_max_gm),
            inventory_max_quest_items: p
                .get_int("MaximumSlotsForQuestItems", d.inventory_max_quest_items),
            crafting_enabled: p.get_bool("CraftingEnabled", d.crafting_enabled),
            dwarf_recipe_limit: p.get_int("DwarfRecipeLimit", d.dwarf_recipe_limit),
            common_recipe_limit: p.get_int("CommonRecipeLimit", d.common_recipe_limit),
            craft_masterwork: p.get_bool("CraftMasterwork", d.craft_masterwork),
            alt_game_creation: p.get_bool("AltGameCreation", d.alt_game_creation),
            alt_sub_class_without_quests: p
                .get_bool("AltSubClassWithoutQuests", d.alt_sub_class_without_quests),
            alt_game_creation_speed: f64::from(p.get_float("AltGameCreationSpeed", 1.0)),
            alt_game_creation_xp_rate: f64::from(p.get_float("AltGameCreationXpRate", 1.0)),
            alt_game_creation_sp_rate: f64::from(p.get_float("AltGameCreationSpRate", 1.0)),
            alt_game_creation_rare_xpsp_rate: f64::from(
                p.get_float("AltGameCreationRareXpSpRate", 1.0),
            ),
            craft_masterwork_chance: p.get_int("CraftMasterworkChance", d.craft_masterwork_chance),
            auto_learn_skills: p.get_bool("AutoLearnSkills", d.auto_learn_skills),
            auto_learn_skills_without_items: p.get_bool(
                "AutoLearnSkillsWithoutItems",
                d.auto_learn_skills_without_items,
            ),
            auto_learn_divine_inspiration: p.get_bool(
                "AutoLearnDivineInspiration",
                d.auto_learn_divine_inspiration,
            ),
            divine_inspiration_sp_book_needed: p.get_bool(
                "DivineInspirationSpBookNeeded",
                d.divine_inspiration_sp_book_needed,
            ),
            expertise_penalty: p.get_bool("ExpertisePenalty", d.expertise_penalty),
            decrease_skill_level: p.get_bool("DecreaseSkillOnDelevel", d.decrease_skill_level),
            strict_delevel_skill_removal: p
                .get_bool("StrictDelevelSkillRemoval", d.strict_delevel_skill_removal),
            character_data_store_interval_ticks: general
                .get_int("CharacterDataStoreInterval", 15)
                .max(1) as u64
                * 600,
            run_spd_boost: p.get_float("RunSpeedBoost", 35.0) as f64,
            max_p_atk: p.get_float("MaxPAtk", 999_999.0) as f64,
            max_m_atk: p.get_float("MaxMAtk", 999_999.0) as f64,
            max_p_crit_rate: p.get_float("MaxPCritRate", 500.0) as f64,
            max_m_crit_rate: p.get_float("MaxMCritRate", 200.0) as f64,
            max_p_atk_speed: p.get_float("MaxPAtkSpeed", 1500.0) as f64,
            max_m_atk_speed: p.get_float("MaxMAtkSpeed", 1999.0) as f64,
            max_evasion: p.get_float("MaxEvasion", 250.0) as f64,
            max_run_speed: p.get_float("MaxRunSpeed", 300.0) as f64,
            max_run_speed_summon: p.get_float("MaxRunSpeedSummon", 350.0) as f64,
            max_hp: f64::from(p.get_int("MaxHP", 150_000)),
            // Java: `getLong(..) >= 0 ? value : Long.MAX_VALUE`.
            max_sp: p.get_long("MaxSp", 50_000_000_000),
            min_abnormal_state_success_rate: f64::from(
                p.get_int("MinAbnormalStateSuccessRate", 10),
            ),
            max_abnormal_state_success_rate: f64::from(
                p.get_int("MaxAbnormalStateSuccessRate", 90),
            ),
            warehouse_slots_dwarf: p.get_int("MaximumWarehouseSlotsForDwarf", 120),
            warehouse_slots_no_dwarf: p.get_int("MaximumWarehouseSlotsForNoDwarf", 100),
            max_num_of_clans_in_ally: p.get_int("AltMaxNumOfClansInAlly", 3).max(0) as usize,
            clan_members_for_war: p.get_int("AltClanMembersForWar", 15).max(0) as usize,
            // Java's default is `EVENT` — the top of the enum, i.e. no filter.
            max_equipable_item_grade: crate::data::item_data::CrystalType::from_config_name(
                p.get_string("MaxEquipableItemGrade", "EVENT").as_str(),
            ),
            max_buff_count: p.get_int("MaxBuffAmount", 24),
            max_subclass: p.get_int("MaxSubclass", 5),
            max_dance_count: p.get_int("MaxDanceAmount", 12),
            vampiric_attack_works_with_skills: p.get_bool(
                "VampiricAttackWorkWithSkills",
                d.vampiric_attack_works_with_skills,
            ),
            // Java reads this one out of PVP.ini (`pvpConfig`), not the
            // character block — same split `karma_pk_limit` already lives with.
            vampiric_attack_affects_pvp: PropertiesParser::load_rel(root, "config/PVP.ini")
                .get_bool("VampiricAttackAffectsPvP", d.vampiric_attack_affects_pvp),
            mp_vampiric_attack_work_with_melee: p.get_bool(
                "MpVampiricAttackWorkWithMelee",
                d.mp_vampiric_attack_work_with_melee,
            ),
            mp_vampiric_attack_affects_pvp: PropertiesParser::load_rel(root, "config/PVP.ini")
                .get_bool(
                    "MpVampiricAttackAffectsPvP",
                    d.mp_vampiric_attack_affects_pvp,
                ),
            player_reflect_percent_limit: p.get_float(
                "PlayerReflectPercentLimit",
                d.player_reflect_percent_limit as f32,
            ) as f64,
            non_player_reflect_percent_limit: p.get_float(
                "NonPlayerReflectPercentLimit",
                d.non_player_reflect_percent_limit as f32,
            ) as f64,
            dance_consume_additional_mp: p
                .get_bool("DanceConsumeAdditionalMP", d.dance_consume_additional_mp),
            store_skill_cooltime: p.get_bool("StoreSkillCooltime", d.store_skill_cooltime),
            alt_store_dances: p.get_bool("AltStoreDances", d.alt_store_dances),
            dance_cancel_buff: p.get_bool("DanceCancelBuff", d.dance_cancel_buff),
            max_free_teleport_level: p.get_int("MaxFreeTeleportLevel", d.max_free_teleport_level),
            alt_karma_player_can_use_gk: p
                .get_bool("AltKarmaPlayerCanUseGK", d.alt_karma_player_can_use_gk),
            alt_karma_player_can_shop: p
                .get_bool("AltKarmaPlayerCanShop", d.alt_karma_player_can_shop),
            alt_karma_player_can_be_killed_in_peace_zone: p.get_bool(
                "AltKarmaPlayerCanBeKilledInPeaceZone",
                d.alt_karma_player_can_be_killed_in_peace_zone,
            ),
            alt_karma_player_can_teleport: p
                .get_bool("AltKarmaPlayerCanTeleport", d.alt_karma_player_can_teleport),
            alt_karma_player_can_trade: p
                .get_bool("AltKarmaPlayerCanTrade", d.alt_karma_player_can_trade),
            player_spawn_protection: p.get_int("PlayerSpawnProtection", d.player_spawn_protection),
            player_teleport_protection: p
                .get_int("PlayerTeleportProtection", d.player_teleport_protection),
            offset_on_teleport_enabled: p
                .get_bool("OffsetOnTeleportEnabled", d.offset_on_teleport_enabled),
            max_offset_on_teleport: p.get_int("MaxOffsetOnTeleport", d.max_offset_on_teleport),
            disconnect_after_death: p.get_bool("DisconnectAfterDeath", d.disconnect_after_death),
            enchant_black_list: parse_id_list(&p.get_string("EnchantBlackList", "")),
            augmentation_black_list: parse_id_list(&p.get_string("AugmentationBlackList", "")),
            disable_over_enchanting: p.get_bool("DisableOverEnchanting", d.disable_over_enchanting),
            over_enchant_protection: p.get_bool("OverEnchantProtection", d.over_enchant_protection),
            over_enchant_punishment:
                crate::model::punishment::IllegalActionPunishment::find_by_name(
                    &p.get_string("OverEnchantPunishment", "JAIL"),
                ),
            alt_allow_augment_pvp_items: p
                .get_bool("AltAllowAugmentPvPItems", d.alt_allow_augment_pvp_items),
            alt_allow_augment_trade: p.get_bool("AltAllowAugmentTrade", d.alt_allow_augment_trade),
            alt_allow_augment_destroy: p
                .get_bool("AltAllowAugmentDestroy", d.alt_allow_augment_destroy),
            alt_clan_join_days: p.get_int("DaysBeforeJoinAClan", d.alt_clan_join_days),
            alt_clan_create_days: p.get_int("DaysBeforeCreateAClan", d.alt_clan_create_days),
            alt_clan_dissolve_days: p
                .get_int("DaysToPassToDissolveAClan", d.alt_clan_dissolve_days),
            alt_ally_join_days_when_leaved: p.get_int(
                "DaysBeforeJoinAllyWhenLeaved",
                d.alt_ally_join_days_when_leaved,
            ),
            alt_ally_join_days_when_dismissed: p.get_int(
                "DaysBeforeJoinAllyWhenDismissed",
                d.alt_ally_join_days_when_dismissed,
            ),
            alt_accept_clan_days_when_dismissed: p.get_int(
                "DaysBeforeAcceptNewClanWhenDismissed",
                d.alt_accept_clan_days_when_dismissed,
            ),
            alt_create_ally_days_when_dissolved: p.get_int(
                "DaysBeforeCreateNewAllyWhenDissolved",
                d.alt_create_ally_days_when_dissolved,
            ),
            alt_members_can_withdraw_from_clan_wh: p.get_bool(
                "AltMembersCanWithdrawFromClanWH",
                d.alt_members_can_withdraw_from_clan_wh,
            ),
            alt_clan_leader_instant_activation: p.get_bool(
                "AltClanLeaderInstantActivation",
                d.alt_clan_leader_instant_activation,
            ),
            // Java `getDuration(...).toMillis()`; the ini writes `30mins`.
            alt_clan_members_time_for_bonus_ms: p
                .get_duration_secs("AltClanMembersTimeForBonus", 30 * 60)
                * 1000,
            alt_command_channel_friends: p
                .get_bool("AltCommandChannelFriends", d.alt_command_channel_friends),
            life_crystal_needed: p.get_bool("LifeCrystalNeeded", d.life_crystal_needed),
            alt_game_cancel_cast: cancel_by_hit(&p.get_string("AltGameCancelByHit", "Cast")).0,
            alt_game_cancel_bow: cancel_by_hit(&p.get_string("AltGameCancelByHit", "Cast")).1,
            alt_game_stun_break: p.get_bool("BreakStun", d.alt_game_stun_break),
            fake_death_damage_stand: p.get_bool("FakeDeathDamageStand", d.fake_death_damage_stand),
            fake_death_untarget: p.get_bool("FakeDeathUntarget", d.fake_death_untarget),
            player_fake_death_up_protection: p.get_int(
                "PlayerFakeDeathUpProtection",
                d.player_fake_death_up_protection,
            ),
            effect_tick_ratio_ms: p.get_long("EffectTickRatio", d.effect_tick_ratio_ms),
            triggered_buffs_max_amount: p
                .get_int("MaxTriggeredBuffAmount", d.triggered_buffs_max_amount),
            armor_set_equip_active_skill_reuse_ms: p.get_int(
                "ArmorSetEquipActiveSkillReuse",
                d.armor_set_equip_active_skill_reuse_ms,
            ),
            item_equip_active_skill_reuse_ms: p.get_int(
                "ItemEquipActiveSkillReuse",
                d.item_equip_active_skill_reuse_ms,
            ),
            store_ui_settings: p.get_bool("StoreCharUiSettings", d.store_ui_settings),
            enable_modify_skill_reuse: p
                .get_bool("EnableModifySkillReuse", d.enable_modify_skill_reuse),
            // Same `id,value;…` shape the Olympiad items list uses.
            skill_reuse_list: super::common::parse_tuples_separated_by_semicolon(
                &p.get_string("SkillReuseList", ""),
            ),
            subclass_store_skill_cooltime: p.get_bool(
                "SubclassStoreSkillCooltime",
                d.subclass_store_skill_cooltime,
            ),
            summon_store_skill_cooltime: p
                .get_bool("SummonStoreSkillCooltime", d.summon_store_skill_cooltime),
            store_recipe_shop_list: p.get_bool("StoreRecipeShopList", d.store_recipe_shop_list),
            starting_level: p.get_int("StartingLevel", d.starting_level),
            starting_sp: p.get_long("StartingSP", d.starting_sp),
            initial_equipment_event: p.get_bool("InitialEquipmentEvent", d.initial_equipment_event),
            forbidden_names: p
                .get_string("ForbiddenNames", "")
                .split(',')
                .map(|n| n.trim().to_ascii_lowercase())
                .filter(|n| !n.is_empty())
                .collect(),
            auto_loot_herbs: p.get_bool("AutoLootHerbs", d.auto_loot_herbs),
            auto_loot_item_ids: p
                .get_string("AutoLootItemIds", "")
                .split(',')
                .filter_map(|id| id.trim().parse().ok())
                .collect(),
            auto_loot_slot_limit: p.get_bool("AutoLootSlotLimit", d.auto_loot_slot_limit),
            base_subclass_level: p.get_int("BaseSubclassLevel", d.base_subclass_level),
            max_subclass_level: p.get_int("MaxSubclassLevel", d.max_subclass_level),
            base_dualclass_level: p.get_int("BaseDualclassLevel", d.base_dualclass_level),
            alt_game_subclass_everywhere: p
                .get_bool("AltSubclassEverywhere", d.alt_game_subclass_everywhere),
            auto_learn_fs_skills: p
                .get_bool("AutoLearnForgottenScrollSkills", d.auto_learn_fs_skills),
            allow_transform_without_quest: p.get_bool(
                "AltTransformationWithoutQuest",
                d.allow_transform_without_quest,
            ),
            max_personal_fame_points: p
                .get_int("MaxPersonalFamePoints", d.max_personal_fame_points),
            fortress_zone_fame_task_frequency: p.get_int(
                "FortressZoneFameTaskFrequency",
                d.fortress_zone_fame_task_frequency,
            ),
            fortress_zone_fame_acquire_points: p.get_int(
                "FortressZoneFameAquirePoints",
                d.fortress_zone_fame_acquire_points,
            ),
            max_bonus_exp: f64::from(p.get_float("MaxExpBonus", 0.0)),
            max_bonus_sp: f64::from(p.get_float("MaxSpBonus", 0.0)),
            // Java stores this one already in millis (`getInt(...) * 1000`).
            player_movement_block_time_ms: p.get_int("NpcTalkBlockingTime", 0) * 1000,
            silence_mode_exclude: p.get_bool("SilenceModeExclude", d.silence_mode_exclude),
            teleport_while_siege_in_progress: p.get_bool(
                "TeleportWhileSiegeInProgress",
                d.teleport_while_siege_in_progress,
            ),
            unstuck_interval: p.get_int("UnstuckInterval", d.unstuck_interval),
            // Java: `characterConfig.getInt("TeleportWatchdogTimeout", 0)`,
            // scheduled as `timeout * 1000` ms — here 10 ticks per second.
            // Negatives would wrap the `as u64`, so clamp at 0 = disabled.
            teleport_watchdog_timeout_ticks: p.get_int("TeleportWatchdogTimeout", 0).max(0) as u64
                * 10,
            calculate_magic_success_by_skill_magic_level: p.get_bool(
                "CalculateMagicSuccessBySkillMagicLevel",
                d.calculate_magic_success_by_skill_magic_level,
            ),
            magic_failures: p.get_bool("MagicFailures", d.magic_failures),
            enable_modify_skill_duration: p
                .get_bool("EnableModifySkillDuration", d.enable_modify_skill_duration),
            // Java only builds the map when the flag is set; keep it empty otherwise.
            skill_duration_list: if p
                .get_bool("EnableModifySkillDuration", d.enable_modify_skill_duration)
            {
                parse_tuples_separated_by_semicolon(&p.get_string("SkillDurationList", ""))
            } else {
                HashMap::new()
            },
        }
    }
}

#[cfg(test)]
mod tests {

    /// Java parses both blacklists with `split(",")` → `int[]` → `Arrays.sort`,
    /// and then looks ids up with `binarySearch`. The dist's own lists happen
    /// to ship in order, so only a deliberately unsorted input can show that
    /// the port sorts too — without it, `binary_search` would silently miss
    /// entries on an operator-edited list.
    /// One string key, four meanings — and the "neither" case is the one that
    /// matters, because it is what makes damage interrupt nothing at all.
    #[test]
    fn cancel_by_hit_parses_its_four_settings() {
        assert_eq!(cancel_by_hit("cast"), (true, false));
        assert_eq!(cancel_by_hit("bow"), (false, true));
        assert_eq!(cancel_by_hit("all"), (true, true));
        assert_eq!(cancel_by_hit("  ALL "), (true, true), "trimmed, any case");
        assert_eq!(cancel_by_hit("none"), (false, false));
        assert_eq!(cancel_by_hit(""), (false, false));
    }

    #[test]
    fn id_lists_are_sorted_and_deduped_like_javas() {
        assert_eq!(parse_id_list("3,1,2"), vec![1, 2, 3]);
        assert_eq!(parse_id_list("5, 5 ,4"), vec![4, 5], "trimmed and deduped");
        assert_eq!(parse_id_list(""), Vec::<i32>::new());
        assert_eq!(parse_id_list("7,,8"), vec![7, 8], "empty entries skipped");
        assert_eq!(parse_id_list("9,oops,10"), vec![9, 10], "junk skipped");
        // The property the lookup depends on.
        let ids = parse_id_list("7827,7816,7820");
        assert!(ids.binary_search(&7816).is_ok());
        assert!(ids.binary_search(&7817).is_err());
    }

    use super::*;

    #[test]
    fn skill_duration_list_parses_id_second_pairs() {
        // The multi-line dist form (backslash continuations are already joined
        // by the properties parser) with trailing `;` and stray whitespace.
        let m: HashMap<i32, i32> =
            parse_tuples_separated_by_semicolon("1078,7200;1085,7200; 264,3600 ;bad;309,");
        assert_eq!(m.get(&1078), Some(&7200));
        assert_eq!(m.get(&1085), Some(&7200));
        assert_eq!(m.get(&264), Some(&3600));
        assert_eq!(m.get(&309), None, "missing value is skipped, not defaulted");
        assert_eq!(m.len(), 3);
    }
}
