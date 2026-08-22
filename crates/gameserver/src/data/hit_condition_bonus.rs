//! Port of `data/xml/HitConditionBonusData` — the auto-attack hit-chance
//! modifiers from `data/stats/hitConditionBonus.xml`. The `rain` term is
//! parsed but never applied: there is no weather (Java's rain check is
//! commented out upstream too). The `dark` term **is** applied — the caller
//! passes `game_time::is_night_at`, the same clock the night spawns use.

use crate::data::xml;
use quick_xml::events::Event;

pub const HIT_CONDITION_BONUS_FILE: &str = "data/stats/hitConditionBonus.xml";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HitConditionBonusData {
    pub front_bonus: f64,
    pub side_bonus: f64,
    pub back_bonus: f64,
    pub high_bonus: f64,
    pub low_bonus: f64,
    pub dark_bonus: f64,
}

impl Default for HitConditionBonusData {
    /// The stock XML values, doubling as the no-file default.
    fn default() -> Self {
        Self {
            front_bonus: 0.0,
            side_bonus: 5.0,
            back_bonus: 10.0,
            high_bonus: 3.0,
            low_bonus: -3.0,
            dark_bonus: -10.0,
        }
    }
}

impl HitConditionBonusData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut out = Self::default();
        if let Ok(content) =
            std::fs::read_to_string(format!("{file_path}{HIT_CONDITION_BONUS_FILE}"))
        {
            for event in xml::events(&content) {
                let e = match event {
                    Event::Empty(e) | Event::Start(e) => e,
                    _ => continue,
                };
                let val = e
                    .attributes()
                    .flatten()
                    .find(|a| a.key.as_ref() == b"val")
                    .and_then(|a| String::from_utf8_lossy(&a.value).parse::<f64>().ok());
                let Some(val) = val else { continue };
                match e.name().as_ref() {
                    b"front" => out.front_bonus = val,
                    b"side" => out.side_bonus = val,
                    b"back" => out.back_bonus = val,
                    b"high" => out.high_bonus = val,
                    b"low" => out.low_bonus = val,
                    b"dark" => out.dark_bonus = val,
                    _ => {}
                }
            }
        }
        out
    }

    /// `getConditionBonus`, minus the rain term (no weather): 100 base,
    /// ± elevation (z-diff > 50), + `dark` at night, + position bonus, as a
    /// multiplier (Java divides by 100), floored at 0.
    ///
    /// `is_night` is Java's `GameTimeTaskManager.isNight()`, passed in rather
    /// than read here so the data layer stays clock-free.
    pub fn condition_bonus(
        &self,
        attacker_z: i32,
        target_z: i32,
        position: crate::model::movement::Position,
        is_night: bool,
    ) -> f64 {
        use crate::model::movement::Position;
        let mut modifier = 100.0;
        if attacker_z - target_z > 50 {
            modifier += self.high_bonus;
        } else if attacker_z - target_z < -50 {
            modifier += self.low_bonus;
        }
        if is_night {
            modifier += self.dark_bonus;
        }
        modifier += match position {
            Position::Side => self.side_bonus,
            Position::Back => self.back_bonus,
            Position::Front => self.front_bonus,
        };
        (modifier / 100.0).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::movement::Position;

    #[test]
    fn loads_real_dist_values_and_combines() {
        let data = HitConditionBonusData::load_from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/"
        ));
        assert_eq!(data.back_bonus, 10.0);
        assert_eq!(data.side_bonus, 5.0);
        assert_eq!(data.high_bonus, 3.0);
        assert_eq!(data.dark_bonus, -10.0);
        // Level ground, front: ×1.0. Back + high ground: ×1.13. Low ground
        // from the side: ×1.02.
        assert!((data.condition_bonus(0, 0, Position::Front, false) - 1.0).abs() < 1e-9);
        assert!((data.condition_bonus(60, 0, Position::Back, false) - 1.13).abs() < 1e-9);
        assert!((data.condition_bonus(-60, 0, Position::Side, false) - 1.02).abs() < 1e-9);
    }

    /// Java adds `darkBonus` for the whole in-game night, before the position
    /// term — a flat −10 on the 100 base, so every swing in the dark is 10
    /// points less likely to land.
    #[test]
    fn night_costs_ten_points_of_hit_chance() {
        let data = HitConditionBonusData::default();
        assert!((data.condition_bonus(0, 0, Position::Front, true) - 0.9).abs() < 1e-9);
        assert!((data.condition_bonus(60, 0, Position::Back, true) - 1.03).abs() < 1e-9);
        assert!((data.condition_bonus(-60, 0, Position::Side, true) - 0.92).abs() < 1e-9);
    }
}
