//! `Custom/PremiumSystem.ini` — port of the `PREMIUM_SYSTEM_CONFIG_FILE` block
//! of `Config.java`. Premium is an account-scoped flag (`account_premium`,
//! cached in `World.premium`); this carries what that flag *does*: the reward
//! multipliers `Attackable.doItemDrop`/the XP path apply to a premium killer.
//!
//! It also carries the **PC-café (PA) point** block, which the same ini owns —
//! `game_loop::character::pc_cafe` is the subsystem that reads it.
//!
//! Only the keys the ported subsystems read are loaded. Not here (no backing
//! subsystem yet): `PremiumOnlyFishing` (G32).

use commons::config::PropertiesParser;

pub const PREMIUM_SYSTEM_CONFIG_FILE: &str = "config/Custom/PremiumSystem.ini";

#[derive(Debug, Clone)]
pub struct PremiumConfig {
    /// `EnablePremiumSystem` — master switch (True on this dist). When false
    /// `Player.hasPremiumStatus()` is always false and every rate below is
    /// inert.
    pub enabled: bool,
    /// `PremiumRateXp` / `PremiumRateSp` — multipliers applied to a premium
    /// killer's exp/sp reward, *before* the vitality/skill bonus multiplier
    /// (`Attackable.onKill`). Both **2** on this dist.
    pub rate_xp: f64,
    pub rate_sp: f64,
    /// `PremiumRateDropChance` / `PremiumRateDropAmount` — drop-roll
    /// multipliers for a premium killer (1 / 2 on this dist).
    pub rate_drop_chance: f64,
    pub rate_drop_amount: f64,
    /// `PremiumRateSpoilChance` / `PremiumRateSpoilAmount` (1 / 2).
    pub rate_spoil_chance: f64,
    pub rate_spoil_amount: f64,
    /// `PremiumRateQuestXp` / `PremiumRateQuestSp` — quest turn-in rewards for a
    /// premium character (**1 / 1** on this dist, so inert here).
    pub rate_quest_xp: f64,
    pub rate_quest_sp: f64,
    /// `PremiumRateDropChanceByItemId` / `PremiumRateDropAmountByItemId` — the
    /// per-item overrides that **replace** the flat rate above rather than
    /// stacking with it. The dist declares both as
    /// `57,2;6656,1;…;10314,1`, so adena doubles while the listed jewels are
    /// pinned to **×1** — a premium killer gets *no* bonus on them, which the
    /// flat ×2 amount would otherwise have given.
    pub rate_drop_chance_by_id: std::collections::HashMap<i32, f64>,
    pub rate_drop_amount_by_id: std::collections::HashMap<i32, f64>,

    // --- PC-café (PA) points -------------------------------------------
    /// `PcCafeEnabled` — master switch for earning PA points. **False on this
    /// dist**, so nothing below fires until an operator turns it on.
    pub pc_cafe_enabled: bool,
    /// `PcCafeOnlyPremium` (**True** here) — only premium accounts earn.
    pub pc_cafe_only_premium: bool,
    /// `PcCafeRetailLike` (**True** here) — which of the two earning modes is
    /// live. Retail-like pays a flat amount on a timer and *disables* the
    /// exp-proportional path entirely (`givePcCafePoint` returns immediately
    /// when this is set), so the two are mutually exclusive.
    pub pc_cafe_retail_like: bool,
    /// `PcCafeRewardTime` in ms (**300 000** = 5 min here) — the retail-like
    /// timer's period.
    ///
    /// **Java never reads this key.** `Config.PC_CAFE_REWARD_TIME` is declared
    /// and never assigned, so it stays 0 and
    /// `ThreadPool.scheduleAtFixedRate(…, 0, 0)` throws
    /// `IllegalArgumentException` ("period must be > 0"), which `ThreadPool`
    /// swallows and logs — the reference server's retail-like timer therefore
    /// never starts at all. The dist ini is the specification here, so the port
    /// honours the 300 000 it declares rather than reproducing the omission.
    pub pc_cafe_reward_time: i32,
    /// `MaxPcCafePoints` (200 000; a negative value is clamped to 0).
    pub pc_cafe_max_points: i32,
    /// `DoublingAcquisitionPoints` (**True** here) / its percent chance
    /// (`DoublingAcquisitionPointsChance`, 1; anything outside 0..=100 falls
    /// back to 1).
    pub pc_cafe_enable_double_points: bool,
    pub pc_cafe_double_points_chance: i32,
    /// `AcquisitionPointsRetailLikePoints` (10) — the flat retail-like award.
    pub acquisition_pc_cafe_retail_like_points: i32,
    /// `AcquisitionPointsRate` (1.0; negative falls back to 1) — the
    /// exp-proportional multiplier in `exp * 0.0001 * rate`.
    pub pc_cafe_point_rate: f64,
    /// `AcquisitionPointsRandom` (False) — award `Rnd.get(points/2, points)`
    /// instead of the flat amount.
    pub pc_cafe_random_point: bool,
    /// `RewardLowExpKills` (True) / `RewardLowExpKillsChance` (50, clamped to
    /// 0..=100): a kill whose exp rounds the award down to 0 still pays 1
    /// point, this often.
    pub pc_cafe_reward_low_exp_kills: bool,
    pub pc_cafe_low_exp_kills_chance: i32,
}

impl Default for PremiumConfig {
    /// Java `Config` defaults — the system off, but the ×2 rates it would use.
    fn default() -> Self {
        Self {
            enabled: false,
            rate_xp: 2.0,
            rate_sp: 2.0,
            rate_drop_chance: 2.0,
            rate_drop_amount: 1.0,
            rate_spoil_chance: 2.0,
            rate_spoil_amount: 1.0,
            rate_quest_xp: 1.0,
            rate_quest_sp: 1.0,
            rate_drop_chance_by_id: std::collections::HashMap::new(),
            rate_drop_amount_by_id: std::collections::HashMap::new(),
            pc_cafe_enabled: false,
            pc_cafe_only_premium: false,
            pc_cafe_retail_like: true,
            // Java's field default is 0 (never assigned); the dist ini's value.
            pc_cafe_reward_time: 300_000,
            pc_cafe_max_points: 200_000,
            pc_cafe_enable_double_points: false,
            pc_cafe_double_points_chance: 1,
            acquisition_pc_cafe_retail_like_points: 10,
            pc_cafe_point_rate: 1.0,
            pc_cafe_random_point: false,
            pc_cafe_reward_low_exp_kills: true,
            pc_cafe_low_exp_kills_chance: 50,
        }
    }
}

impl PremiumConfig {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(
            root,
            PREMIUM_SYSTEM_CONFIG_FILE,
        ))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            enabled: p.get_bool("EnablePremiumSystem", d.enabled),
            rate_xp: p.get_float("PremiumRateXp", d.rate_xp as f32) as f64,
            rate_sp: p.get_float("PremiumRateSp", d.rate_sp as f32) as f64,
            rate_drop_chance: p.get_float("PremiumRateDropChance", d.rate_drop_chance as f32)
                as f64,
            rate_drop_amount: p.get_float("PremiumRateDropAmount", d.rate_drop_amount as f32)
                as f64,
            rate_spoil_chance: p.get_float("PremiumRateSpoilChance", d.rate_spoil_chance as f32)
                as f64,
            rate_spoil_amount: p.get_float("PremiumRateSpoilAmount", d.rate_spoil_amount as f32)
                as f64,
            rate_quest_xp: p.get_float("PremiumRateQuestXp", d.rate_quest_xp as f32) as f64,
            rate_quest_sp: p.get_float("PremiumRateQuestSp", d.rate_quest_sp as f32) as f64,
            rate_drop_chance_by_id: super::rates::parse_id_multiplier_list(
                &p.get_string("PremiumRateDropChanceByItemId", ""),
            ),
            rate_drop_amount_by_id: super::rates::parse_id_multiplier_list(
                &p.get_string("PremiumRateDropAmountByItemId", ""),
            ),
            pc_cafe_enabled: p.get_bool("PcCafeEnabled", d.pc_cafe_enabled),
            pc_cafe_only_premium: p.get_bool("PcCafeOnlyPremium", d.pc_cafe_only_premium),
            pc_cafe_retail_like: p.get_bool("PcCafeRetailLike", d.pc_cafe_retail_like),
            pc_cafe_reward_time: p.get_int("PcCafeRewardTime", d.pc_cafe_reward_time),
            // Java: `if (PC_CAFE_MAX_POINTS < 0) PC_CAFE_MAX_POINTS = 0;`
            pc_cafe_max_points: p.get_int("MaxPcCafePoints", d.pc_cafe_max_points).max(0),
            pc_cafe_enable_double_points: p
                .get_bool("DoublingAcquisitionPoints", d.pc_cafe_enable_double_points),
            // Java falls back to 1 (not to the bound) when out of range.
            pc_cafe_double_points_chance: match p.get_int(
                "DoublingAcquisitionPointsChance",
                d.pc_cafe_double_points_chance,
            ) {
                v @ 0..=100 => v,
                _ => 1,
            },
            acquisition_pc_cafe_retail_like_points: p.get_int(
                "AcquisitionPointsRetailLikePoints",
                d.acquisition_pc_cafe_retail_like_points,
            ),
            // Java: `if (PC_CAFE_POINT_RATE < 0) PC_CAFE_POINT_RATE = 1;`
            pc_cafe_point_rate: {
                let v = p.get_float("AcquisitionPointsRate", d.pc_cafe_point_rate as f32) as f64;
                if v < 0.0 { 1.0 } else { v }
            },
            pc_cafe_random_point: p.get_bool("AcquisitionPointsRandom", d.pc_cafe_random_point),
            pc_cafe_reward_low_exp_kills: p
                .get_bool("RewardLowExpKills", d.pc_cafe_reward_low_exp_kills),
            // This one *is* clamped to the bounds rather than reset.
            pc_cafe_low_exp_kills_chance: p
                .get_int("RewardLowExpKillsChance", d.pc_cafe_low_exp_kills_chance)
                .clamp(0, 100),
        }
    }
}
