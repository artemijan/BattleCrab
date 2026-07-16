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
        }
    }
}

impl GeneralConfig {
    pub fn load() -> Self {
        Self::from_parser(&PropertiesParser::load(GENERAL_CONFIG_FILE))
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
        assert!(!g.gm_startup_invisible, "GMStartupInvisible=False");
        assert!(!g.gm_startup_silence, "GMStartupSilence=False");
        assert!(!g.gm_startup_auto_list, "GMStartupAutoList=False");
        assert!(!g.gm_startup_diet_mode, "GMStartupDietMode=False");
        assert!(!g.gm_give_special_skills, "GMGiveSpecialSkills=False");
    }
}
