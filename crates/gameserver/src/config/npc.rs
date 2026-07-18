//! `NPC.ini` — port of the `NPC_CONFIG_FILE` block of `Config.java`, scoped
//! to the keys the G9 combat/AI slice consumes.

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
            aggro_distance_check_enabled: true,
            aggro_distance_check_range: 1500,
            aggro_distance_check_raids: false,
            aggro_distance_check_raid_range: 3000,
            aggro_distance_check_restore_life: true,
        }
    }
}

impl NpcConfig {
    pub fn load() -> Self {
        let p = PropertiesParser::load(NPC_CONFIG_FILE);
        let g = PropertiesParser::load(GENERAL_CONFIG_FILE);
        let d = Self::default();
        Self {
            default_corpse_time: p.get_int("DefaultCorpseTime", d.default_corpse_time),
            max_drift_range: p.get_int("MaxDriftRange", d.max_drift_range),
            min_npc_animation: g.get_int("MinNpcAnimation", d.min_npc_animation),
            max_npc_animation: g.get_int("MaxNpcAnimation", d.max_npc_animation),
            min_monster_animation: g.get_int("MinMonsterAnimation", d.min_monster_animation),
            max_monster_animation: g.get_int("MaxMonsterAnimation", d.max_monster_animation),
            alt_game_view_npc: p.get_bool("AltGameViewNpc", d.alt_game_view_npc),
            aggro_distance_check_enabled: p
                .get_bool("AggroDistanceCheckEnabled", d.aggro_distance_check_enabled),
            aggro_distance_check_range: p
                .get_int("AggroDistanceCheckRange", d.aggro_distance_check_range),
            aggro_distance_check_raids: p
                .get_bool("AggroDistanceCheckRaids", d.aggro_distance_check_raids),
            aggro_distance_check_raid_range: p
                .get_int("AggroDistanceCheckRaidRange", d.aggro_distance_check_raid_range),
            aggro_distance_check_restore_life: p
                .get_bool("AggroDistanceCheckRestoreLife", d.aggro_distance_check_restore_life),
        }
    }
}
