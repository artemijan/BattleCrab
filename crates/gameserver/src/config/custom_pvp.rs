//! `Custom/PvpRewardItem.ini`, `Custom/PvpTitleColor.ini`,
//! `Custom/RandomSpawns.ini`, `Custom/ChatModeration.ini` and
//! `Custom/NoblessMaster.ini` — the moderate tier of the G33 `Custom/*.ini`
//! audit (`docs/PLAN_G33_CUSTOM_INI_AUDIT.md`). All five are enabled on this
//! dist and live in Java.

use std::collections::HashSet;

use commons::config::PropertiesParser;

pub const PVP_REWARD_ITEM_CONFIG_FILE: &str = "config/Custom/PvpRewardItem.ini";
pub const PVP_TITLE_COLOR_CONFIG_FILE: &str = "config/Custom/PvpTitleColor.ini";
pub const RANDOM_SPAWNS_CONFIG_FILE: &str = "config/Custom/RandomSpawns.ini";
pub const CHAT_MODERATION_CONFIG_FILE: &str = "config/Custom/ChatModeration.ini";
pub const NOBLESS_MASTER_CONFIG_FILE: &str = "config/Custom/NoblessMaster.ini";

/// `Custom/PvpRewardItem.ini` — an item paid to the killer on a PvP or PK kill.
/// **300 000 adena per PvP kill** on this dist; the PK arm ships off.
#[derive(Debug, Clone)]
pub struct PvpRewardConfig {
    /// `RewardPvpItem` (True here) — pay out when the victim was **flagged**.
    pub reward_pvp: bool,
    pub pvp_item_id: i32,
    pub pvp_item_amount: i64,
    /// `RewardPvpItemMessage` — send the "you obtained" line with it.
    pub pvp_message: bool,
    /// `RewardPkItem` (False here) — pay out when the victim was *not* flagged,
    /// i.e. the killer is a PK.
    pub reward_pk: bool,
    pub pk_item_id: i32,
    pub pk_item_amount: i64,
    pub pk_message: bool,
    /// `DisableRewardsInInstances` / `DisableRewardsInPvpZones` (both True) —
    /// one shared guard covering *both* rewards, checked before either.
    pub disable_in_instances: bool,
    pub disable_in_pvp_zones: bool,
}

impl Default for PvpRewardConfig {
    fn default() -> Self {
        // Java `Config`'s defaults for an absent file.
        Self {
            reward_pvp: false,
            pvp_item_id: 57,
            pvp_item_amount: 1000,
            pvp_message: false,
            reward_pk: false,
            pk_item_id: 57,
            pk_item_amount: 500,
            pk_message: false,
            disable_in_instances: false,
            disable_in_pvp_zones: false,
        }
    }
}

impl PvpRewardConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(
            root,
            PVP_REWARD_ITEM_CONFIG_FILE,
        ))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            reward_pvp: p.get_bool("RewardPvpItem", d.reward_pvp),
            pvp_item_id: p.get_int("RewardPvpItemId", d.pvp_item_id),
            pvp_item_amount: p.get_long("RewardPvpItemAmount", d.pvp_item_amount),
            pvp_message: p.get_bool("RewardPvpItemMessage", d.pvp_message),
            reward_pk: p.get_bool("RewardPkItem", d.reward_pk),
            pk_item_id: p.get_int("RewardPkItemId", d.pk_item_id),
            pk_item_amount: p.get_long("RewardPkItemAmount", d.pk_item_amount),
            pk_message: p.get_bool("RewardPkItemMessage", d.pk_message),
            disable_in_instances: p.get_bool("DisableRewardsInInstances", d.disable_in_instances),
            disable_in_pvp_zones: p.get_bool("DisableRewardsInPvpZones", d.disable_in_pvp_zones),
        }
    }
}

/// One rung of the PvP title ladder: at `kills` or more (and below the next
/// rung's), wear `title` in `color`.
#[derive(Debug, Clone)]
pub struct PvpRank {
    pub kills: i32,
    /// Java stores the colour as an int decoded from a **BBGGRR** hex string —
    /// the client's byte order, not RGB.
    pub color: i32,
    pub title: String,
}

/// `Custom/PvpTitleColor.ini` — a five-rung ladder that renames and recolours a
/// player's title as their PvP count climbs.
#[derive(Debug, Clone, Default)]
pub struct PvpTitleColorConfig {
    /// `EnablePvPColorSystem` (True here).
    pub enabled: bool,
    /// The five rungs, ascending. Java hard-codes exactly five.
    pub ranks: Vec<PvpRank>,
}

impl PvpTitleColorConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(
            root,
            PVP_TITLE_COLOR_CONFIG_FILE,
        ))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let ranks = (1..=5)
            .map(|i| PvpRank {
                kills: p.get_int(&format!("PvpAmount{i}"), i32::MAX),
                color: parse_hex_color(&p.get_string(&format!("ColorForAmount{i}"), "")),
                title: p.get_string(&format!("PvPTitleForAmount{i}"), ""),
            })
            .collect();
        Self {
            enabled: p.get_bool("EnablePvPColorSystem", false),
            ranks,
        }
    }

    /// The rung `pvp_kills` earns, or `None` below the first. Java's chain of
    /// `>= amountN && < amountN+1` tests, with the last one open-ended.
    pub fn rank_for(&self, pvp_kills: i32) -> Option<&PvpRank> {
        if !self.enabled {
            return None;
        }
        self.ranks
            .iter()
            .rev()
            .find(|r| pvp_kills >= r.kills && r.kills != i32::MAX)
    }
}

/// Java `Integer.decode("0x" + value)` on a `BBGGRR` string.
fn parse_hex_color(raw: &str) -> i32 {
    i32::from_str_radix(raw.trim(), 16).unwrap_or(0)
}

/// `Custom/RandomSpawns.ini` — jitter a monster's spawn point so a camp is not
/// pinned to the same coordinates every respawn.
#[derive(Debug, Clone, Default)]
pub struct RandomSpawnsConfig {
    /// `EnableRandomMonsterSpawns` (True here).
    pub enabled: bool,
    /// `MaxSpawnMobRange` (100). Java's minimum is its negative, so the offset
    /// is symmetric about the spawn point.
    pub max_range: i32,
    /// `MobsSpawnNotRandom` — ids that keep their exact coordinates (raid
    /// minions, quest chests, the Four Sepulchers' furniture …).
    pub never_random: HashSet<i32>,
}

impl RandomSpawnsConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, RANDOM_SPAWNS_CONFIG_FILE))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        Self {
            enabled: p.get_bool("EnableRandomMonsterSpawns", false),
            max_range: p.get_int("MaxSpawnMobRange", 150),
            never_random: p
                .get_string("MobsSpawnNotRandom", "")
                .split(',')
                .filter_map(|id| id.trim().parse().ok())
                .collect(),
        }
    }
}

/// `Custom/ChatModeration.ini` + `Custom/NoblessMaster.ini`.
#[derive(Debug, Clone)]
pub struct CustomNpcConfig {
    /// `ChatAdmin` (True here) — the `.banchat`/`.unbanchat` voiced commands,
    /// gated on the same access table as the `//` form.
    pub chat_admin: bool,
    /// `Enabled` — the Noblesse Master NPC grants nobless for a level.
    ///
    /// **Reachable only if spawned.** The npc template (1003000, "Kadmos")
    /// ships in `stats/npcs/custom/`, but *no spawn file places it* — so on an
    /// untouched dist the only way to meet him is `//spawn 1003000`. Java is in
    /// exactly the same position; the feature is ported so that a GM spawn or
    /// an operator's own spawn file behaves the same in both.
    pub nobless_master_enabled: bool,
    pub nobless_master_npc_id: i32,
    pub nobless_master_level: i32,
    /// `RewardTiara` — also hand over the Noblesse Tiara (7694).
    pub nobless_master_tiara: bool,
}

impl Default for CustomNpcConfig {
    fn default() -> Self {
        Self {
            chat_admin: false,
            nobless_master_enabled: false,
            nobless_master_npc_id: 1003000,
            nobless_master_level: 80,
            nobless_master_tiara: true,
        }
    }
}

impl CustomNpcConfig {
    pub fn load_from(root: &str) -> Self {
        let d = Self::default();
        let chat = PropertiesParser::load_rel(root, CHAT_MODERATION_CONFIG_FILE);
        let nobless = PropertiesParser::load_rel(root, NOBLESS_MASTER_CONFIG_FILE);
        Self {
            chat_admin: chat.get_bool("ChatAdmin", d.chat_admin),
            nobless_master_enabled: nobless.get_bool("Enabled", d.nobless_master_enabled),
            nobless_master_npc_id: nobless.get_int("NpcId", d.nobless_master_npc_id),
            nobless_master_level: nobless.get_int("LevelRequirement", d.nobless_master_level),
            nobless_master_tiara: nobless.get_bool("RewardTiara", d.nobless_master_tiara),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

    #[test]
    fn dist_values_load() {
        let pvp = PvpRewardConfig::load_from(DIST);
        assert!(pvp.reward_pvp, "RewardPvpItem = True");
        assert_eq!((pvp.pvp_item_id, pvp.pvp_item_amount), (57, 300_000));
        assert!(!pvp.reward_pk, "the PK arm ships off");
        assert!(pvp.disable_in_instances && pvp.disable_in_pvp_zones);

        let npc = CustomNpcConfig::load_from(DIST);
        assert!(npc.chat_admin && npc.nobless_master_enabled);
        assert_eq!(npc.nobless_master_level, 80);

        let spawns = RandomSpawnsConfig::load_from(DIST);
        assert!(spawns.enabled);
        assert_eq!(spawns.max_range, 100);
        // The multi-line `\`-continued list parses whole.
        assert_eq!(spawns.never_random.len(), 88, "every id on the list");
        assert!(spawns.never_random.contains(&18812));
        assert!(spawns.never_random.contains(&31487), "the last one too");
    }

    /// The ladder: below the first rung nothing applies; each threshold is
    /// inclusive; the top rung is open-ended.
    #[test]
    fn the_pvp_ladder_picks_the_right_rung() {
        let cfg = PvpTitleColorConfig::load_from(DIST);
        assert!(cfg.enabled);
        assert!(cfg.rank_for(499).is_none(), "below the first rung");
        assert_eq!(cfg.rank_for(500).unwrap().title, "Sergeant");
        assert_eq!(cfg.rank_for(999).unwrap().title, "Sergeant");
        assert_eq!(cfg.rank_for(1000).unwrap().title, "Lieutenant");
        assert_eq!(cfg.rank_for(4999).unwrap().title, "Major");
        assert_eq!(cfg.rank_for(1_000_000).unwrap().title, "General");
        // `9C9C9C` is read as the client's BBGGRR int, not as RGB.
        assert_eq!(cfg.rank_for(500).unwrap().color, 0x9C_9C_9C);
    }

    /// Disabled → no rung at any count, so nothing overwrites a player's title.
    #[test]
    fn a_disabled_ladder_never_applies() {
        let mut cfg = PvpTitleColorConfig::load_from(DIST);
        cfg.enabled = false;
        assert!(cfg.rank_for(1_000_000).is_none());
    }
}
