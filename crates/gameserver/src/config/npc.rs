//! `NPC.ini` — port of the `NPC_CONFIG_FILE` block of `Config.java`, scoped
//! to the keys the G9 combat/AI slice consumes.

use super::character::CHARACTER_CONFIG_FILE;
use commons::config::PropertiesParser;

pub const NPC_CONFIG_FILE: &str = "config/NPC.ini";
/// The random-animation bounds live in `General.ini` in Java, not `NPC.ini`.
pub const GENERAL_CONFIG_FILE: &str = "config/General.ini";

#[derive(Debug, Clone)]
pub struct NpcConfig {
    /// `DefaultCorpseTime` (seconds) — decay delay for NPCs whose template
    /// carries no `<corpseTime>`.
    pub default_corpse_time: i32,
    /// `MaxDriftRange` — how far a monster may wander/chase from its spawn
    /// before AI walks it back home.
    pub max_drift_range: i32,
    /// `Min/MaxNpcAnimation` (seconds) — random social-animation interval for
    /// non-attackable NPCs. `MaxNpcAnimation <= 0` disables animations
    /// entirely (Java `hasRandomAnimation`).
    pub min_npc_animation: i32,
    pub max_npc_animation: i32,
    /// `Min/MaxMonsterAnimation` (seconds) — same, for attackable NPCs.
    pub min_monster_animation: i32,
    pub max_monster_animation: i32,
    /// `AltGameViewNpc` — when set, a *non-GM* player shift-clicking an NPC
    /// opens the `NpcViewMod` info window (Java `Action` case 1 →
    /// `Npc.onActionShift`) instead of a plain re-target.
    pub alt_game_view_npc: bool,
    /// `ShowNpcLevel` — prefix every monster's title with `Lv <level>`
    /// (Java `Creature.getTitle()`'s custom-title branch). True on this dist.
    pub show_npc_level: bool,
    /// `ShowNpcAggression` — append `[A]` (aggressive) / `[G]` (calls clan
    /// help) to every monster's title. True on this dist.
    pub show_npc_aggression: bool,
    /// `AggroDistanceCheckEnabled` — the chase leash (`AttackableAI.thinkAttack`):
    /// a monster dragged farther than the range below from its spawn drops
    /// aggro and returns home. Disabled by default on this dist.
    pub aggro_distance_check_enabled: bool,
    /// `AggroDistanceCheckRange` — leash radius for a normal monster.
    pub aggro_distance_check_range: i32,
    /// `AggroDistanceCheckRaids` — apply the leash to raid bosses too.
    pub aggro_distance_check_raids: bool,
    /// `AggroDistanceCheckRaidRange` — leash radius when the monster is a raid.
    pub aggro_distance_check_raid_range: i32,
    /// `AggroDistanceCheckRestoreLife` — heal to full HP/MP on returning home.
    pub aggro_distance_check_restore_life: bool,
    /// `VitalityConsumeByMob` / `VitalityConsumeByBoss` — the divisors in
    /// `Attackable.getVitalityPoints` (2250 / 1125 on this dist). Only used at
    /// level 85+; below that Java hard-codes 1000, so on an Interlude server
    /// these are effectively dormant — ported for faithfulness.
    pub vitality_consume_by_mob: i32,
    pub vitality_consume_by_boss: i32,
    /// `RaidMinionRespawnTime` (ms, 300000 here) — how long a *raid* boss's
    /// minion stays dead. A non-raid leader's minions don't come back at all
    /// unless overridden below (Java's `respawnTime < 0 ? isRaid ? cfg : 0`).
    pub raid_minion_respawn_time: i32,
    /// `CustomMinionsRespawnTime` — per-npc-id overrides in *seconds*
    /// (`22450,30;22371,120;…`, 23 entries on this dist). Present ids bypass
    /// the raid-only rule entirely, so a `0` here means "never respawn" even
    /// for a raid minion.
    pub custom_minions_respawn_time: std::collections::HashMap<i32, i32>,
    /// `ForceDeleteMinions` (False here) — despawn a leader's minions when it
    /// dies even if it isn't a raid boss.
    pub force_delete_minions: bool,
    /// `RaidHpRegenMultiplier` / `RaidMpRegenMultiplier` as fractions (Java
    /// divides the percentage by 100). Both 100 % on this dist.
    pub raid_hp_regen_multiplier: f64,
    pub raid_mp_regen_multiplier: f64,
    /// `HpRegenMultiplier` / `MpRegenMultiplier` from `Character.ini`, applied
    /// to non-raid NPCs (Java reads the same globals for players and NPCs).
    pub hp_regen_multiplier: f64,
    /// `PetHpRegenMultiplier` / `PetMpRegenMultiplier` — Java's
    /// `RegenHPFinalizer`/`RegenMPFinalizer` pet branch applies these to the
    /// pet's own per-level regen, not the NPC multipliers above. Both 100
    /// (×1.0) on this dist, so they only matter to a server that retunes them —
    /// which is exactly why they should not be silently inlined as 1.0.
    /// `DisableRaidCurse` — turns the anti-farming raid curse off entirely.
    /// **False** on this dist (and in Java's default), so the curse is live.
    pub disable_raid_curse: bool,
    pub pet_hp_regen_multiplier: f64,
    pub pet_mp_regen_multiplier: f64,
    pub mp_regen_multiplier: f64,
    /// `MinNPCLevelForMagicPenalty` (78) — the Gracia rule "when a character is
    /// 3+ levels below a level-78+ monster, that monster resists magic more
    /// often". Below this NPC level `Formulas.calcMagicSuccess` leaves its
    /// `targetModifier` at 1.0, so on Interlude content the bare
    /// `1.3^levelDiff` term is the whole penalty.
    pub min_npc_level_for_magic_penalty: i32,
    /// `SkillChancePenaltyForLvLDifferences` — multiplier on the magic-failure
    /// term for -3 … -6 level differences, indexed by
    /// `targetLevel - casterLevel - 2` and clamped to the last entry.
    pub skill_chance_penalty_for_lvl_differences: Vec<f64>,
}

impl Default for NpcConfig {
    fn default() -> Self {
        Self {
            default_corpse_time: 7,
            max_drift_range: 300,
            min_npc_animation: 5,
            max_npc_animation: 60,
            min_monster_animation: 5,
            max_monster_animation: 60,
            alt_game_view_npc: false,
            show_npc_level: false,
            show_npc_aggression: false,
            aggro_distance_check_enabled: true,
            aggro_distance_check_range: 1500,
            aggro_distance_check_raids: false,
            aggro_distance_check_raid_range: 3000,
            aggro_distance_check_restore_life: true,
            vitality_consume_by_mob: 2250,
            raid_minion_respawn_time: 300_000,
            custom_minions_respawn_time: std::collections::HashMap::new(),
            force_delete_minions: false,
            disable_raid_curse: false,
            pet_hp_regen_multiplier: 1.0,
            pet_mp_regen_multiplier: 1.0,
            raid_hp_regen_multiplier: 1.0,
            raid_mp_regen_multiplier: 1.0,
            hp_regen_multiplier: 1.0,
            mp_regen_multiplier: 1.0,
            vitality_consume_by_boss: 1125,
            min_npc_level_for_magic_penalty: 78,
            // Java's `Config` default for this key is the string "unknown",
            // which `parseConfigLine` turns into an empty array; this dist sets
            // it explicitly, so mirror the dist value as the default.
            skill_chance_penalty_for_lvl_differences: vec![2.5, 3.0, 3.25, 3.5],
        }
    }
}

impl NpcConfig {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        let p = PropertiesParser::load_rel(root, NPC_CONFIG_FILE);
        let g = PropertiesParser::load_rel(root, GENERAL_CONFIG_FILE);
        let c = PropertiesParser::load_rel(root, CHARACTER_CONFIG_FILE);
        let d = Self::default();
        Self {
            default_corpse_time: p.get_int("DefaultCorpseTime", d.default_corpse_time),
            max_drift_range: p.get_int("MaxDriftRange", d.max_drift_range),
            min_npc_animation: g.get_int("MinNpcAnimation", d.min_npc_animation),
            max_npc_animation: g.get_int("MaxNpcAnimation", d.max_npc_animation),
            min_monster_animation: g.get_int("MinMonsterAnimation", d.min_monster_animation),
            max_monster_animation: g.get_int("MaxMonsterAnimation", d.max_monster_animation),
            alt_game_view_npc: p.get_bool("AltGameViewNpc", d.alt_game_view_npc),
            show_npc_level: p.get_bool("ShowNpcLevel", d.show_npc_level),
            show_npc_aggression: p.get_bool("ShowNpcAggression", d.show_npc_aggression),
            aggro_distance_check_enabled: p
                .get_bool("AggroDistanceCheckEnabled", d.aggro_distance_check_enabled),
            aggro_distance_check_range: p
                .get_int("AggroDistanceCheckRange", d.aggro_distance_check_range),
            aggro_distance_check_raids: p
                .get_bool("AggroDistanceCheckRaids", d.aggro_distance_check_raids),
            aggro_distance_check_raid_range: p.get_int(
                "AggroDistanceCheckRaidRange",
                d.aggro_distance_check_raid_range,
            ),
            aggro_distance_check_restore_life: p.get_bool(
                "AggroDistanceCheckRestoreLife",
                d.aggro_distance_check_restore_life,
            ),
            vitality_consume_by_mob: p.get_int("VitalityConsumeByMob", d.vitality_consume_by_mob),
            vitality_consume_by_boss: p
                .get_int("VitalityConsumeByBoss", d.vitality_consume_by_boss),
            raid_minion_respawn_time: p
                .get_int("RaidMinionRespawnTime", d.raid_minion_respawn_time),
            custom_minions_respawn_time: parse_minion_respawn_overrides(
                &p.get_string("CustomMinionsRespawnTime", ""),
            ),
            force_delete_minions: p.get_bool("ForceDeleteMinions", d.force_delete_minions),
            disable_raid_curse: p.get_bool("DisableRaidCurse", false),
            pet_hp_regen_multiplier: p.get_int("PetHpRegenMultiplier", 100) as f64 / 100.0,
            pet_mp_regen_multiplier: p.get_int("PetMpRegenMultiplier", 100) as f64 / 100.0,
            raid_hp_regen_multiplier: p.get_int("RaidHpRegenMultiplier", 100) as f64 / 100.0,
            raid_mp_regen_multiplier: p.get_int("RaidMpRegenMultiplier", 100) as f64 / 100.0,
            hp_regen_multiplier: c.get_int("HpRegenMultiplier", 100) as f64 / 100.0,
            mp_regen_multiplier: c.get_int("MpRegenMultiplier", 100) as f64 / 100.0,
            min_npc_level_for_magic_penalty: p.get_int(
                "MinNPCLevelForMagicPenalty",
                d.min_npc_level_for_magic_penalty,
            ),
            skill_chance_penalty_for_lvl_differences: parse_config_line(
                &p.get_string("SkillChancePenaltyForLvLDifferences", ""),
                d.skill_chance_penalty_for_lvl_differences,
            ),
        }
    }
}

/// Java `Config.parseConfigLine` — a comma-separated float list. An unparseable
/// or empty value falls back to the default rather than failing the boot (Java
/// would hand back an empty array; an empty penalty table is indistinguishable
/// from "no penalty", which is the same outcome as the fallback on this dist).
fn parse_config_line(raw: &str, fallback: Vec<f64>) -> Vec<f64> {
    let parsed: Vec<f64> = raw
        .split(',')
        .filter_map(|v| v.trim().parse().ok())
        .collect();
    if parsed.is_empty() { fallback } else { parsed }
}

/// `CustomMinionsRespawnTime` — `id,seconds;id,seconds;…`. Malformed pairs are
/// skipped rather than failing the boot, matching Java's lenient split.
fn parse_minion_respawn_overrides(raw: &str) -> std::collections::HashMap<i32, i32> {
    raw.split(';')
        .filter_map(|pair| {
            let (id, secs) = pair.split_once(',')?;
            Some((id.trim().parse().ok()?, secs.trim().parse().ok()?))
        })
        .collect()
}
