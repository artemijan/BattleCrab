//! `Custom/ChampionMonsters.ini` — port of the
//! `CUSTOM_CHAMPION_MONSTERS_CONFIG_FILE` block of `Config.java`.
//!
//! A champion is an ordinary monster that rolled a lottery at spawn time
//! (`Attackable.onRespawn`) and came up as a beefed-up, better-paying version
//! of itself: a red team aura, a `Champion `-prefixed title, damage taken
//! divided by [`hp`](ChampionConfig::hp), and multiplied attack / attack-speed
//! / HP-regen / exp / drop rates.
//!
//! **This dist ships `ChampionEnable = True`**, which is why the feature is
//! ported at all — the ROADMAP files champion monsters under the Mobius
//! `config/Custom/*` block that is out of scope "except any the operator
//! explicitly enables". The G33 `Custom/*.ini` audit found this one enabled.
//!
//! `ChampionEnableInInstances` is parsed and honoured, but the instance id is
//! always 0 on the spawn paths that exist today, so it only ever reads as
//! "allowed" — kept so the gate is already right when instanced spawns land.

use commons::config::PropertiesParser;

pub const CHAMPION_MONSTERS_CONFIG_FILE: &str = "config/Custom/ChampionMonsters.ini";

/// The `(item id, count)` pairs of `ChampionRewardItems`.
pub type ItemHolder = (i32, i64);

#[derive(Debug, Clone)]
pub struct ChampionConfig {
    /// `ChampionEnable` — the master gate. Every other field is inert without
    /// it, and Java re-checks it at each consumer rather than at the roll only.
    pub enable: bool,
    /// `ChampionPassive` — champions never seed hate from the aggro scan, so
    /// they stand still until attacked.
    pub passive: bool,
    /// `ChampionFrequency` — percent chance, rolled once per spawn. `0`
    /// disables the roll even when `enable` is on.
    pub frequency: i32,
    /// `ChampionTitle` — prefixed onto the mob's title.
    pub title: String,
    /// `ChampionAura` — show the red team aura (`Team.RED`).
    pub aura: bool,
    /// `ChampionMinLevel` / `ChampionMaxLevel` — inclusive level window the
    /// monster's own level must fall in to be eligible.
    pub min_level: i32,
    pub max_level: i32,
    /// `ChampionHp` — **damage divisor**, not an HP multiplier: Java leaves
    /// max HP alone and divides every incoming hit by this instead
    /// (`Creature.reduceCurrentHp`). `0` disables the division.
    pub hp: i32,
    /// `ChampionHpRegen` — multiplies the finalized HP regen.
    pub hp_regen: f64,
    /// `ChampionRewardsExpSp` — multiplies exp **and** sp.
    pub rewards_exp_sp: f64,
    /// `ChampionRewardsChance` / `ChampionRewardsAmount` — multiply the
    /// non-adena drop chance / amount.
    pub rewards_chance: f64,
    pub rewards_amount: f64,
    /// `ChampionAdenasRewardsChance` / `ChampionAdenasRewardsAmount` — the
    /// adena-only equivalents. **Java applies the chance one only inside the
    /// `RATE_DROP_CHANCE_BY_ID` arm**, i.e. only when adena carries a per-id
    /// rate; see `roll_drops`.
    pub adenas_rewards_chance: f64,
    pub adenas_rewards_amount: f64,
    /// `ChampionAtk` — multiplies finalized P.Atk and M.Atk.
    pub atk: f64,
    /// `ChampionSpdAtk` — multiplies finalized P.Atk speed and M.Atk speed.
    pub spd_atk: f64,
    /// `ChampionRewardLowerLvlItemChance` / `ChampionRewardHigherLvlItemChance`
    /// — percent chances that **suppress** the guaranteed reward item when the
    /// champion is below / above the killer's level. See
    /// [`suppresses_reward_items`](Self::suppresses_reward_items) for why the
    /// polarity is the opposite of what the ini comment claims.
    pub reward_lower_level_item_chance: i32,
    pub reward_higher_level_item_chance: i32,
    /// `ChampionRewardItems` — `id,count;id,count` pairs appended to a
    /// champion's drop list.
    pub reward_items: Vec<ItemHolder>,
    /// `ChampionEnableVitality` — whether a champion kill consumes vitality.
    /// Java's `Attackable.useVitalityRate()` is `!champion || this`.
    pub enable_vitality: bool,
    /// `ChampionEnableInInstances` — allow the roll inside an instance.
    pub enable_in_instances: bool,
}

impl Default for ChampionConfig {
    /// Java `Config` defaults (the file absent): the feature off, the tuning
    /// values at the literals `Config.java` hard-codes.
    fn default() -> Self {
        Self {
            enable: false,
            passive: false,
            frequency: 0,
            title: "Champion".to_string(),
            aura: true,
            min_level: 20,
            max_level: 60,
            hp: 7,
            hp_regen: 1.0,
            rewards_exp_sp: 8.0,
            rewards_chance: 8.0,
            rewards_amount: 1.0,
            adenas_rewards_chance: 1.0,
            adenas_rewards_amount: 1.0,
            atk: 1.0,
            spd_atk: 1.0,
            reward_lower_level_item_chance: 0,
            reward_higher_level_item_chance: 0,
            // Java's default string is "4356,10", parsed the same way.
            reward_items: vec![(4356, 10)],
            enable_vitality: false,
            enable_in_instances: false,
        }
    }
}

impl ChampionConfig {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(
            root,
            CHAMPION_MONSTERS_CONFIG_FILE,
        ))
    }

    fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            enable: p.get_bool("ChampionEnable", d.enable),
            passive: p.get_bool("ChampionPassive", d.passive),
            frequency: p.get_int("ChampionFrequency", d.frequency),
            title: p.get_string("ChampionTitle", &d.title),
            aura: p.get_bool("ChampionAura", d.aura),
            min_level: p.get_int("ChampionMinLevel", d.min_level),
            max_level: p.get_int("ChampionMaxLevel", d.max_level),
            hp: p.get_int("ChampionHp", d.hp),
            hp_regen: p.get_float("ChampionHpRegen", d.hp_regen as f32) as f64,
            rewards_exp_sp: p.get_float("ChampionRewardsExpSp", d.rewards_exp_sp as f32) as f64,
            rewards_chance: p.get_float("ChampionRewardsChance", d.rewards_chance as f32) as f64,
            rewards_amount: p.get_float("ChampionRewardsAmount", d.rewards_amount as f32) as f64,
            adenas_rewards_chance: p.get_float(
                "ChampionAdenasRewardsChance",
                d.adenas_rewards_chance as f32,
            ) as f64,
            adenas_rewards_amount: p.get_float(
                "ChampionAdenasRewardsAmount",
                d.adenas_rewards_amount as f32,
            ) as f64,
            atk: p.get_float("ChampionAtk", d.atk as f32) as f64,
            spd_atk: p.get_float("ChampionSpdAtk", d.spd_atk as f32) as f64,
            reward_lower_level_item_chance: p.get_int(
                "ChampionRewardLowerLvlItemChance",
                d.reward_lower_level_item_chance,
            ),
            reward_higher_level_item_chance: p.get_int(
                "ChampionRewardHigherLvlItemChance",
                d.reward_higher_level_item_chance,
            ),
            reward_items: parse_reward_items(&p.get_string("ChampionRewardItems", "4356,10")),
            enable_vitality: p.get_bool("ChampionEnableVitality", d.enable_vitality),
            enable_in_instances: p.get_bool("ChampionEnableInInstances", d.enable_in_instances),
        }
    }

    /// Java `Attackable.useVitalityRate()`: `!_champion || CHAMPION_ENABLE_VITALITY`.
    pub fn uses_vitality_rate(&self, champion: bool) -> bool {
        !champion || self.enable_vitality
    }

    /// The champion-extra-drop gate of `NpcTemplate.calculateDrops`, ported
    /// **behaviour-first** because the behaviour contradicts the key names.
    ///
    /// Java reads:
    /// ```text
    /// if (victim.level < killer.level && Rnd.get(100) < LOWER)  return drops; // no reward item
    /// if (victim.level > killer.level && Rnd.get(100) < HIGHER) return drops; // no reward item
    /// drops.addAll(CHAMPION_REWARD_ITEMS);
    /// ```
    /// Both arms **skip** the reward, so each key is a *suppression* chance
    /// even though the ini documents it as a "% Chance to obtain". On this
    /// dist that inverts the intent exactly: `LOWER = 0` means a champion
    /// *below* your level always pays out, and `HIGHER = 100` means one
    /// *above* your level never does. Equal levels always pay.
    ///
    /// `roll` is the caller's `Rnd.get(100)` so the RNG stays on the world's
    /// stream; it is only consumed on the arm whose level test matched, as in
    /// Java (`&&` short-circuits).
    pub fn suppresses_reward_items(&self, victim_level: i32, killer_level: i32, roll: i32) -> bool {
        if victim_level < killer_level {
            return roll < self.reward_lower_level_item_chance;
        }
        if victim_level > killer_level {
            return roll < self.reward_higher_level_item_chance;
        }
        false
    }
}

/// Java's `ChampionRewardItems` split: `id,count` pairs separated by `;`,
/// empty segments skipped. A malformed segment is dropped rather than
/// panicking — Java would throw and abort the whole config load, which is a
/// worse outcome for a cosmetic custom feature.
fn parse_reward_items(raw: &str) -> Vec<ItemHolder> {
    raw.split(';')
        .filter(|s| !s.trim().is_empty())
        .filter_map(|s| {
            let mut parts = s.split(',');
            let id = parts.next()?.trim().parse::<i32>().ok()?;
            let count = parts.next()?.trim().parse::<i64>().ok()?;
            Some((id, count))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist(rel: &str) -> String {
        format!("{}/../../dist/game/{rel}", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn loads_the_dist_file() {
        let p = PropertiesParser::load(dist(CHAMPION_MONSTERS_CONFIG_FILE));
        let c = ChampionConfig::from_parser(&p);
        // The audit finding that motivated the port: the operator has this on.
        assert!(c.enable, "ChampionEnable = True on this dist");
        assert!(c.passive, "ChampionPassive = True");
        assert_eq!(c.frequency, 1);
        assert_eq!(c.title, "Champion");
        assert!(c.aura);
        assert_eq!((c.min_level, c.max_level), (30, 85));
        assert_eq!(c.hp, 10);
        assert_eq!(c.hp_regen, 2.0);
        assert_eq!(c.rewards_exp_sp, 10.0);
        assert_eq!(c.rewards_chance, 5.0);
        assert_eq!(c.rewards_amount, 5.0);
        assert_eq!(c.adenas_rewards_chance, 10.0);
        assert_eq!(c.adenas_rewards_amount, 10.0);
        assert_eq!(c.atk, 4.0);
        assert_eq!(c.spd_atk, 2.0);
        assert_eq!(c.reward_items, vec![(6393, 1)]);
        assert_eq!(c.reward_lower_level_item_chance, 0);
        assert_eq!(c.reward_higher_level_item_chance, 100);
        assert!(!c.enable_vitality);
        assert!(!c.enable_in_instances);
    }

    #[test]
    fn parses_multiple_reward_items() {
        // The format the ini's own comment documents.
        assert_eq!(
            parse_reward_items("6393,1;57,5000"),
            vec![(6393, 1), (57, 5000)]
        );
    }

    #[test]
    fn skips_empty_and_malformed_reward_segments() {
        // A trailing `;` is the common hand-edit; it must not cost the whole list.
        assert_eq!(parse_reward_items("6393,1;"), vec![(6393, 1)]);
        assert_eq!(parse_reward_items("nonsense;57,10"), vec![(57, 10)]);
    }

    #[test]
    fn reward_item_suppression_keeps_javas_inverted_polarity() {
        // Dist values: lower = 0, higher = 100.
        let c = ChampionConfig {
            reward_lower_level_item_chance: 0,
            reward_higher_level_item_chance: 100,
            ..Default::default()
        };
        // Champion below the killer: `roll < 0` is never true → always pays.
        assert!(!c.suppresses_reward_items(30, 40, 0));
        assert!(!c.suppresses_reward_items(30, 40, 99));
        // Champion above the killer: `roll < 100` is always true → never pays,
        // despite the ini calling 100 a "% Chance to obtain".
        assert!(c.suppresses_reward_items(40, 30, 0));
        assert!(c.suppresses_reward_items(40, 30, 99));
        // Equal level is not covered by either arm → always pays.
        assert!(!c.suppresses_reward_items(35, 35, 0));
    }

    #[test]
    fn vitality_rate_follows_java() {
        let mut c = ChampionConfig::default();
        assert!(c.uses_vitality_rate(false), "a normal mob always consumes");
        assert!(
            !c.uses_vitality_rate(true),
            "a champion does not, with the flag off"
        );
        c.enable_vitality = true;
        assert!(c.uses_vitality_rate(true), "…and does with it on");
    }
}
