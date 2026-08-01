//! `Custom/AutoPlay.ini` — the `.play` auto-hunt panel and its loops.
//!
//! **This dist has no Classic auto-hunt packet family** (`ExClientPackets`
//! registers no `ExAutoPlay*` opcode); the whole feature hangs off a voiced
//! command and an html panel. See `docs/PLAN_G33_AUTO_PLAY.md`.

use std::collections::HashSet;

use commons::config::PropertiesParser;

pub const AUTO_PLAY_CONFIG_FILE: &str = "config/Custom/AutoPlay.ini";

#[derive(Debug, Clone, Default)]
pub struct AutoPlayConfig {
    /// `EnableAutoPlay` (True here) — the master gate; Java registers the
    /// voiced command only when it is on.
    pub enabled: bool,
    /// `EnableAutoPotion` / `EnableAutoSkill` / `EnableAutoItem` — which of the
    /// three sub-panels the main page offers (all True here). They gate the
    /// *buttons*, so a disabled one simply cannot be configured.
    pub potion: bool,
    pub skill: bool,
    pub item: bool,
    /// `ResumeAutoPlay` (**False** here) — whether logging in restarts a loop
    /// that was running at logout. The settings are restored either way.
    pub resume: bool,
    /// `AssistLeader` (**False** here) — a party member targets whatever the
    /// leader targets, and follows them when the leader has no target.
    pub assist_leader: bool,
    /// `AutoPlayPremium` (True here) — restrict the feature to premium
    /// accounts.
    pub premium_only: bool,
    /// `DisabledSkillIds` / `DisabledItemIds` — both empty here.
    pub disabled_skills: HashSet<i32>,
    pub disabled_items: HashSet<i32>,
    /// `AutoPlayLoginMessage` — an announcement on login; empty here, and Java
    /// skips the packet entirely when it is.
    pub login_message: String,
}

impl AutoPlayConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, AUTO_PLAY_CONFIG_FILE))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let ids = |key: &str| {
            p.get_string(key, "")
                .split(',')
                .filter_map(|v| v.trim().parse().ok())
                .collect()
        };
        Self {
            enabled: p.get_bool("EnableAutoPlay", false),
            potion: p.get_bool("EnableAutoPotion", true),
            skill: p.get_bool("EnableAutoSkill", true),
            item: p.get_bool("EnableAutoItem", true),
            resume: p.get_bool("ResumeAutoPlay", false),
            assist_leader: p.get_bool("AssistLeader", false),
            premium_only: p.get_bool("AutoPlayPremium", false),
            disabled_skills: ids("DisabledSkillIds"),
            disabled_items: ids("DisabledItemIds"),
            login_message: p.get_string("AutoPlayLoginMessage", ""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dist_values_load() {
        let cfg =
            AutoPlayConfig::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        assert!(cfg.enabled, "EnableAutoPlay = True");
        assert!(cfg.potion && cfg.skill && cfg.item, "all three sub-panels");
        assert!(!cfg.resume, "a logout stops the loop");
        assert!(!cfg.assist_leader, "party assist ships off");
        assert!(cfg.premium_only, "AutoPlayPremium = True");
        assert!(cfg.disabled_skills.is_empty() && cfg.disabled_items.is_empty());
        assert!(cfg.login_message.is_empty(), "no announcement configured");
    }
}
