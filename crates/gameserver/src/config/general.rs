//! `General.ini` — port of the GM-startup / hero-aura keys of the
//! `GENERAL_CONFIG_FILE` block of `Config.java`. Only the keys the admin
//! login flow needs so far are loaded (grown per milestone, like the other
//! config sections).

use commons::config::PropertiesParser;

pub const GENERAL_CONFIG_FILE: &str = "config/General.ini";

/// The GM login-state settings applied in `EnterWorld.runImpl` plus the hero
/// aura toggle read by CharInfo/UserInfo (`Config.GM_*`).
#[derive(Debug, Clone, Default)]
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

    /// `AllowManor`: whether the castle manor (seed sowing / crop harvest) runs.
    /// The dist ships `False`; the manor data + packets exist regardless so the
    /// feature works the moment an operator enables it.
    pub allow_manor: bool,
    /// `AltManorSaveAllActions`: persist every manor setup change immediately
    /// (vs. a periodic save). `False` on this dist — the owner's seed/crop setup
    /// lives in memory until the periodic `storeMe` (unported), so with the dist
    /// default nothing is written per-action.
    pub alt_manor_save_all_actions: bool,
    /// `AltManorRefreshTime` (hour, dist 20) — when the daily manor cycle rolls:
    /// the `APPROVED → MAINTENANCE` change (production rollover) fires at
    /// `refresh_time:refresh_min`.
    pub alt_manor_refresh_time: i32,
    /// `AltManorRefreshMin` (minute, dist 0).
    pub alt_manor_refresh_min: i32,
    /// `AltManorMaintenanceMin` (dist 6) — the maintenance window length; the
    /// `MAINTENANCE → MODIFIABLE` change fires at
    /// `refresh_time:(refresh_min + maintenance_min)`.
    pub alt_manor_maintenance_min: i32,
    /// `AltManorApproveTime` (hour, dist 4) — when the owner's edit window
    /// closes: the `MODIFIABLE → APPROVED` change fires at
    /// `approve_time:approve_min`.
    pub alt_manor_approve_time: i32,
    /// `AltManorApproveMin` (minute, dist 30).
    pub alt_manor_approve_min: i32,

    /// `AllowLottery`: whether the weekly Lucky Lottery runs (G26.5). Dist ships
    /// `False`; the round engine + persistence exist regardless so it works the
    /// moment an operator enables it.
    pub allow_lottery: bool,
    /// `AltLotteryPrize` (dist 50000): the starting jackpot of a fresh round.
    pub alt_lottery_prize: i64,
    /// `AltLotteryTicketPrice` (dist 2000): adena charged per ticket.
    pub alt_lottery_ticket_price: i64,
    /// `AltLottery5NumberRate` (dist 0.6): share of the pot paid to 5-match
    /// (first-prize) winners.
    pub alt_lottery_5_number_rate: f64,
    /// `AltLottery4NumberRate` (dist 0.2): share paid to 4-match winners.
    pub alt_lottery_4_number_rate: f64,
    /// `AltLottery3NumberRate` (dist 0.2): share paid to 3-match winners.
    pub alt_lottery_3_number_rate: f64,
    /// `AltLottery2and1NumberPrize` (dist 200): flat adena for a 2-or-1 match.
    pub alt_lottery_2and1_number_prize: i64,

    /// `AllowRace`: whether the Monster Race Track runs (G26.5). Dist ships
    /// `False`; the race engine exists regardless so an operator can enable it.
    pub allow_race: bool,
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
            gm_give_special_aura_skills: p
                .get_bool("GMGiveSpecialAuraSkills", d.gm_give_special_aura_skills),
            autodestroy_item_after: p.get_int("AutoDestroyDroppedItemAfter", 0).max(0) as u64,
            destroy_dropped_player_item: p
                .get_bool("DestroyPlayerDroppedItem", d.destroy_dropped_player_item),
            destroy_equipable_player_item: p
                .get_bool("DestroyEquipableItem", d.destroy_equipable_player_item),
            // Java parses a comma-separated id list; the dist ships a lone `0`.
            protected_items: p
                .get_string("ListOfProtectedItems", "")
                .split(',')
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect(),
            allow_manor: p.get_bool("AllowManor", d.allow_manor),
            alt_manor_save_all_actions: p
                .get_bool("AltManorSaveAllActions", d.alt_manor_save_all_actions),
            alt_manor_refresh_time: p.get_int("AltManorRefreshTime", 20),
            alt_manor_refresh_min: p.get_int("AltManorRefreshMin", 0),
            alt_manor_maintenance_min: p.get_int("AltManorMaintenanceMin", 6),
            alt_manor_approve_time: p.get_int("AltManorApproveTime", 4),
            alt_manor_approve_min: p.get_int("AltManorApproveMin", 30),
            allow_lottery: p.get_bool("AllowLottery", d.allow_lottery),
            alt_lottery_prize: p.get_int("AltLotteryPrize", 50000) as i64,
            alt_lottery_ticket_price: p.get_int("AltLotteryTicketPrice", 2000) as i64,
            alt_lottery_5_number_rate: p.get_float("AltLottery5NumberRate", 0.6) as f64,
            alt_lottery_4_number_rate: p.get_float("AltLottery4NumberRate", 0.2) as f64,
            alt_lottery_3_number_rate: p.get_float("AltLottery3NumberRate", 0.2) as f64,
            alt_lottery_2and1_number_prize: p.get_int("AltLottery2and1NumberPrize", 200) as i64,
            allow_race: p.get_bool("AllowRace", d.allow_race),
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
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/config/General.ini"
        );
        let g = GeneralConfig::from_parser(&PropertiesParser::load(path));
        assert!(g.gm_hero_aura, "GMHeroAura=True");
        assert!(g.gm_startup_builder_hide, "GMStartupBuilderHide=True");
        assert!(g.gm_startup_invulnerable, "GMStartupInvulnerable=True");
        assert!(g.gm_startup_invisible, "GMStartupInvisible=True");
        assert!(g.gm_startup_silence, "GMStartupSilence=True");
        assert!(!g.gm_startup_auto_list, "GMStartupAutoList=False");
        assert!(!g.gm_startup_diet_mode, "GMStartupDietMode=False");
        assert!(!g.gm_give_special_skills, "GMGiveSpecialSkills=False");
        assert!(!g.allow_manor, "AllowManor=False on this dist");
        // Manor cutover times (guards the key names against a config rename).
        assert_eq!(g.alt_manor_refresh_time, 20, "AltManorRefreshTime=20");
        assert_eq!(g.alt_manor_refresh_min, 0, "AltManorRefreshMin=0");
        assert_eq!(g.alt_manor_maintenance_min, 6, "AltManorMaintenanceMin=6");
        assert_eq!(g.alt_manor_approve_time, 4, "AltManorApproveTime=4");
        assert_eq!(g.alt_manor_approve_min, 30, "AltManorApproveMin=30");
        // Lottery (G26.5): disabled on the dist, but the economics keys load.
        assert!(!g.allow_lottery, "AllowLottery=False on this dist");
        assert_eq!(g.alt_lottery_prize, 50000, "AltLotteryPrize=50000");
        assert_eq!(
            g.alt_lottery_ticket_price, 2000,
            "AltLotteryTicketPrice=2000"
        );
        // Parsed via get_float (f32) → f64, so compare with tolerance.
        assert!(
            (g.alt_lottery_5_number_rate - 0.6).abs() < 1e-6,
            "AltLottery5NumberRate=0.6"
        );
        assert_eq!(
            g.alt_lottery_2and1_number_prize, 200,
            "AltLottery2and1NumberPrize=200"
        );
        assert!(!g.allow_race, "AllowRace=False on this dist");
    }

    /// The dist ground-item auto-destroy block: NPC drops decay after 600 s,
    /// but player drops are kept (`DestroyPlayerDroppedItem=False`).
    #[test]
    fn loads_dist_ground_item_autodestroy_values() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/config/General.ini"
        );
        let g = GeneralConfig::from_parser(&PropertiesParser::load(path));
        assert_eq!(
            g.autodestroy_item_after, 600,
            "AutoDestroyDroppedItemAfter=600"
        );
        assert!(
            !g.destroy_dropped_player_item,
            "DestroyPlayerDroppedItem=False"
        );
        assert!(
            !g.destroy_equipable_player_item,
            "DestroyEquipableItem=False"
        );
        assert_eq!(g.protected_items, vec![0], "ListOfProtectedItems=0");
    }
}
