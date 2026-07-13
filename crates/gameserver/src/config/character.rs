//! `Character.ini` — port of the `CHARACTER_CONFIG_FILE` block of `Config.java`.
//! Only the keys needed so far are loaded (grown per milestone).

use commons::config::PropertiesParser;

pub const CHARACTER_CONFIG_FILE: &str = "config/Character.ini";
/// `CharacterDataStoreInterval` lives in Java's `General.ini`, not `Character.ini`.
const GENERAL_CONFIG_FILE: &str = "config/General.ini";

#[derive(Debug, Clone)]
pub struct CharacterConfig {
    /// `DeleteCharAfterDays`: 0 = delete immediately, else mark with a timer.
    pub delete_days: i32,
    /// `StartingAdena`: adena a freshly created character receives.
    pub starting_adena: i64,
    /// `AutoLoot`: monster drops go straight to the killer's inventory (the
    /// ground-drop path is not ported yet — see G9 notes).
    pub auto_loot: bool,
    /// `RespawnRestoreCP/HP/MP` (percent of max on revive).
    pub respawn_restore_cp: f64,
    pub respawn_restore_hp: f64,
    pub respawn_restore_mp: f64,
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
    /// `AltLeavePartyLeader`: leader leaving transfers lead instead of
    /// disbanding (True on this dist).
    pub alt_leave_party_leader: bool,
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
    pub inventory_max_no_dwarf: i32,
    pub inventory_max_dwarf: i32,
    /// `MaximumSlotsForQuestItems` (`Player.getQuestInventoryLimit`): quest
    /// items are checked against this separate cap, never the ordinary one
    /// (`PlayerInventory.validateCapacity`'s `questItem` branch).
    pub inventory_max_quest_items: i32,
    /// `AutoLearnSkills`: when true, `Player.rewardSkills` grants every class
    /// skill reachable at the player's level (not just autoGet skills), on
    /// enter-world and every level-up (Java `giveAvailableSkills`).
    pub auto_learn_skills: bool,
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
}

impl Default for CharacterConfig {
    /// Java `Config` defaults (used by tests via `CombatConfig::default`).
    fn default() -> Self {
        Self {
            delete_days: 1,
            starting_adena: 0,
            auto_loot: false,
            respawn_restore_cp: 0.0,
            respawn_restore_hp: 65.0,
            respawn_restore_mp: 0.0,
            alt_party_range: 1500,
            player_delevel: true,
            delevel_minimum: 85,
            random_respawn_in_town: true,
            alt_party_max_members: 7,
            alt_leave_party_leader: false,
            party_xp_cutoff_method: "level".to_string(),
            party_xp_cutoff_level: 20,
            party_xp_cutoff_percent: 3.0,
            party_xp_cutoff_gaps: vec![(0, 9), (10, 14), (15, 99)],
            party_xp_cutoff_gap_percents: vec![100, 30, 0],
            inventory_max_no_dwarf: 80,
            inventory_max_dwarf: 100,
            inventory_max_quest_items: 100,
            auto_learn_skills: false,
            expertise_penalty: true,
            decrease_skill_level: true,
            strict_delevel_skill_removal: true,
            character_data_store_interval_ticks: 15 * 600,
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

    pub fn load() -> Self {
        let p = PropertiesParser::load(CHARACTER_CONFIG_FILE);
        let general = PropertiesParser::load(GENERAL_CONFIG_FILE);
        let d = Self::default();
        Self {
            delete_days: p.get_int("DeleteCharAfterDays", 1),
            starting_adena: p.get_int("StartingAdena", 0) as i64,
            auto_loot: p.get_bool("AutoLoot", d.auto_loot),
            respawn_restore_cp: p.get_float("RespawnRestoreCP", 0.0) as f64,
            respawn_restore_hp: p.get_float("RespawnRestoreHP", 65.0) as f64,
            respawn_restore_mp: p.get_float("RespawnRestoreMP", 0.0) as f64,
            alt_party_range: p.get_int("AltPartyRange", d.alt_party_range),
            player_delevel: p.get_bool("Delevel", d.player_delevel),
            delevel_minimum: p.get_int("DelevelMinimum", d.delevel_minimum),
            random_respawn_in_town: p.get_bool("RandomRespawnInTownEnabled", d.random_respawn_in_town),
            alt_party_max_members: p.get_int("AltPartyMaxMembers", 7).max(2) as usize,
            alt_leave_party_leader: p.get_bool("AltLeavePartyLeader", d.alt_leave_party_leader),
            party_xp_cutoff_method: p.get_string("PartyXpCutoffMethod", "level").to_lowercase(),
            party_xp_cutoff_level: p.get_int("PartyXpCutoffLevel", 20),
            party_xp_cutoff_percent: p.get_float("PartyXpCutoffPercent", 3.0) as f64,
            party_xp_cutoff_gaps: parse_gaps(&p.get_string("PartyXpCutoffGaps", "0,9;10,14;15,99")),
            party_xp_cutoff_gap_percents: p
                .get_string("PartyXpCutoffGapPercent", "100;30;0")
                .split(';')
                .filter_map(|v| v.trim().parse().ok())
                .collect(),
            inventory_max_no_dwarf: p.get_int("MaximumSlotsForNoDwarf", d.inventory_max_no_dwarf),
            inventory_max_dwarf: p.get_int("MaximumSlotsForDwarf", d.inventory_max_dwarf),
            inventory_max_quest_items: p.get_int("MaximumSlotsForQuestItems", d.inventory_max_quest_items),
            auto_learn_skills: p.get_bool("AutoLearnSkills", d.auto_learn_skills),
            expertise_penalty: p.get_bool("ExpertisePenalty", d.expertise_penalty),
            decrease_skill_level: p.get_bool("DecreaseSkillOnDelevel", d.decrease_skill_level),
            strict_delevel_skill_removal: p.get_bool("StrictDelevelSkillRemoval", d.strict_delevel_skill_removal),
            character_data_store_interval_ticks: general.get_int("CharacterDataStoreInterval", 15).max(1) as u64 * 600,
        }
    }
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
