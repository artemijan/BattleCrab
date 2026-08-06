//! `Character.ini` — port of the `CHARACTER_CONFIG_FILE` block of `Config.java`.
//! Only the keys needed so far are loaded (grown per milestone).

use std::collections::HashMap;

use commons::config::PropertiesParser;

use crate::model::MAX_VITALITY_POINTS;

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
    /// matching, same rule Java uses for Expertise). Off = Java-faithful grace.
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
            craft_masterwork_chance: 10,
            auto_learn_skills: false,
            auto_learn_skills_without_items: true,
            auto_learn_divine_inspiration: false,
            divine_inspiration_sp_book_needed: true,
            expertise_penalty: true,
            decrease_skill_level: true,
            strict_delevel_skill_removal: true,
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

impl CharacterConfig {
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
            party_xp_cutoff_gaps: parse_gaps(&p.get_string("PartyXpCutoffGaps", "0,9;10,14;15,99")),
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
                parse_skill_duration_list(&p.get_string("SkillDurationList", ""))
            } else {
                HashMap::new()
            },
        }
    }
}

/// `SkillDurationList`: `skillId,seconds;skillId2,seconds2;…`. Malformed
/// entries are skipped, mirroring Java's per-entry try/catch (which just logs).
fn parse_skill_duration_list(raw: &str) -> HashMap<i32, i32> {
    let mut out = HashMap::new();
    for entry in raw.split(';') {
        let mut it = entry.split(',');
        if let (Some(id), Some(secs)) = (it.next(), it.next())
            && let (Ok(id), Ok(secs)) = (id.trim().parse::<i32>(), secs.trim().parse::<i32>())
        {
            out.insert(id, secs);
        }
    }
    out
}

/// `PartyXpCutoffGaps`: `from,to;from,to;…` pairs.
fn parse_gaps(raw: &str) -> Vec<(i32, i32)> {
    raw.split(';')
        .filter_map(|pair| {
            let (a, b) = pair.split_once(',')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_duration_list_parses_id_second_pairs() {
        // The multi-line dist form (backslash continuations are already joined
        // by the properties parser) with trailing `;` and stray whitespace.
        let m = parse_skill_duration_list("1078,7200;1085,7200; 264,3600 ;bad;309,");
        assert_eq!(m.get(&1078), Some(&7200));
        assert_eq!(m.get(&1085), Some(&7200));
        assert_eq!(m.get(&264), Some(&3600));
        assert_eq!(m.get(&309), None, "missing value is skipped, not defaulted");
        assert_eq!(m.len(), 3);
    }
}
