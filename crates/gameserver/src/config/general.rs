//! `General.ini` — port of the GM-startup / hero-aura keys of the
//! `GENERAL_CONFIG_FILE` block of `Config.java`. Only the keys the admin
//! login flow needs so far are loaded (grown per milestone, like the other
//! config sections).

use commons::config::PropertiesParser;

pub const GENERAL_CONFIG_FILE: &str = "config/General.ini";

/// The GM login-state settings applied in `EnterWorld.runImpl` plus the hero
/// aura toggle read by CharInfo/UserInfo (`Config.GM_*`).
#[derive(Debug, Clone)]
pub struct GeneralConfig {
    /// `GMHeroAura`: give GMs the Hero glow on login (CharInfo/UserInfo hero
    /// byte = `isHero() || (isGM() && GMHeroAura)`).
    pub gm_hero_aura: bool,
    /// `GMStartupBuilderHide`: hide the GM on login (retail builder default).
    /// When set, Java **skips** the invul/invis/silence/diet block below.
    pub gm_startup_builder_hide: bool,
    /// `GMStartupInvulnerable`: auto-set invulnerable on login.
    pub gm_startup_invulnerable: bool,
    /// `GMStartupInvisible`: auto-set invisible on login (also applied at
    /// char-select, matching `CharacterSelect.java`).
    pub gm_startup_invisible: bool,
    /// `GMStartupSilence`: auto-enable silence (whisper block) on login.
    pub gm_startup_silence: bool,
    /// `GMStartupAutoList`: register the GM in the visible GM list on login
    /// (vs. hidden). Drives the `addGm` hidden flag.
    pub gm_startup_auto_list: bool,
    /// `GMStartupDietMode`: auto-enable diet mode (no weight overload) on login.
    pub gm_startup_diet_mode: bool,
    /// `GMGiveSpecialSkills` / `GMGiveSpecialAuraSkills`: grant the special GM
    /// skill sets on login. Loaded for parity but the special-skill trees are
    /// not ported yet — TODO(G14): honor these once `SkillTreeData.addSkills`
    /// has the special-skill data.
    pub gm_give_special_skills: bool,
    pub gm_give_special_aura_skills: bool,

    // --- Ground-item auto-destroy (`ItemsAutoDestroyTaskManager`) ---
    /// `AutoDestroyDroppedItemAfter` (seconds): how long a dropped ground item
    /// lies before auto-destroying. `0` = never (Java code default). Applies to
    /// NPC drops unconditionally and to player drops only when
    /// [`Self::destroy_dropped_player_item`] is set.
    pub autodestroy_item_after: u64,
    /// `DestroyPlayerDroppedItem`: whether **player**-dropped items are subject
    /// to auto-destroy at all. Java default `false` (and the dist value) — so a
    /// player's drop persists until pickup/restart, unlike an NPC drop.
    pub destroy_dropped_player_item: bool,
    /// `DestroyEquipableItem`: extends [`Self::destroy_dropped_player_item`] to
    /// equipable player drops (weapons/armor). When false, only non-equipable
    /// player drops auto-destroy.
    pub destroy_equipable_player_item: bool,
    /// `ListOfProtectedItems`: item ids never auto-destroyed on the ground
    /// (dist ships `0`, a non-existent id ⇒ effectively empty).
    pub protected_items: Vec<i32>,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        // Java `Config` defaults (the `getBoolean(key, false)` fallbacks).
        Self {
            gm_hero_aura: false,
            gm_startup_builder_hide: false,
            gm_startup_invulnerable: false,
            gm_startup_invisible: false,
            gm_startup_silence: false,
            gm_startup_auto_list: false,
            gm_startup_diet_mode: false,
            gm_give_special_skills: false,
            gm_give_special_aura_skills: false,
            autodestroy_item_after: 0,
            destroy_dropped_player_item: false,
            destroy_equipable_player_item: false,
            protected_items: Vec::new(),
        }
    }
}

impl GeneralConfig {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, GENERAL_CONFIG_FILE))
    }

    /// Parse from an already-loaded `General.ini` (split out so tests can point
    /// at the real `dist/game` file without depending on the process cwd).
    fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            gm_hero_aura: p.get_bool("GMHeroAura", d.gm_hero_aura),
            gm_startup_builder_hide: p.get_bool("GMStartupBuilderHide", d.gm_startup_builder_hide),
            gm_startup_invulnerable: p.get_bool("GMStartupInvulnerable", d.gm_startup_invulnerable),
            gm_startup_invisible: p.get_bool("GMStartupInvisible", d.gm_startup_invisible),
            gm_startup_silence: p.get_bool("GMStartupSilence", d.gm_startup_silence),
            gm_startup_auto_list: p.get_bool("GMStartupAutoList", d.gm_startup_auto_list),
            gm_startup_diet_mode: p.get_bool("GMStartupDietMode", d.gm_startup_diet_mode),
            gm_give_special_skills: p.get_bool("GMGiveSpecialSkills", d.gm_give_special_skills),
            gm_give_special_aura_skills: p.get_bool("GMGiveSpecialAuraSkills", d.gm_give_special_aura_skills),
            autodestroy_item_after: p.get_int("AutoDestroyDroppedItemAfter", 0).max(0) as u64,
            destroy_dropped_player_item: p.get_bool("DestroyPlayerDroppedItem", d.destroy_dropped_player_item),
            destroy_equipable_player_item: p.get_bool("DestroyEquipableItem", d.destroy_equipable_player_item),
            // Java parses a comma-separated id list; the dist ships a lone `0`.
            protected_items: p
                .get_string("ListOfProtectedItems", "")
                .split(',')
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real dist `General.ini` values (the GM block near the top of the
    /// file). Guards the key names against a config rename.
    #[test]
    fn loads_dist_gm_startup_values() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/config/General.ini");
        let g = GeneralConfig::from_parser(&PropertiesParser::load(path));
        assert!(g.gm_hero_aura, "GMHeroAura=True");
        assert!(g.gm_startup_builder_hide, "GMStartupBuilderHide=True");
        assert!(g.gm_startup_invulnerable, "GMStartupInvulnerable=True");
        assert!(g.gm_startup_invisible, "GMStartupInvisible=True");
        assert!(g.gm_startup_silence, "GMStartupSilence=True");
        assert!(!g.gm_startup_auto_list, "GMStartupAutoList=False");
        assert!(!g.gm_startup_diet_mode, "GMStartupDietMode=False");
        assert!(!g.gm_give_special_skills, "GMGiveSpecialSkills=False");
    }

    /// The dist ground-item auto-destroy block: NPC drops decay after 600 s,
    /// but player drops are kept (`DestroyPlayerDroppedItem=False`).
    #[test]
    fn loads_dist_ground_item_autodestroy_values() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/config/General.ini");
        let g = GeneralConfig::from_parser(&PropertiesParser::load(path));
        assert_eq!(g.autodestroy_item_after, 600, "AutoDestroyDroppedItemAfter=600");
        assert!(!g.destroy_dropped_player_item, "DestroyPlayerDroppedItem=False");
        assert!(!g.destroy_equipable_player_item, "DestroyEquipableItem=False");
        assert_eq!(g.protected_items, vec![0], "ListOfProtectedItems=0");
    }
}
