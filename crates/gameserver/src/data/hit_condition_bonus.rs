//! Port of `data/xml/HitConditionBonusData` — the auto-attack hit-chance
//! modifiers from `data/stats/hitConditionBonus.xml`. The `dark` (night) and
//! `rain` terms are parsed but never applied: there is no game-time clock or
//! weather yet (Java's rain check is dead code upstream too).

use quick_xml::Reader;
use quick_xml::events::Event;

pub const HIT_CONDITION_BONUS_FILE: &str = "data/stats/hitConditionBonus.xml";

#[derive(Debug, Clone)]
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
            let mut reader = Reader::from_str(&content);
            while let Ok(event) = reader.read_event() {
                let e = match event {
                    Event::Empty(e) | Event::Start(e) => e,
                    Event::Eof => break,
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

    /// `getConditionBonus`, minus the night/rain terms (no game clock or
    /// weather): 100 base, ± elevation (z-diff > 50), + position bonus,
    /// as a multiplier (Java divides by 100), floored at 0.
    pub fn condition_bonus(
        &self,
        attacker_z: i32,
        target_z: i32,
        position: crate::model::movement::Position,
    ) -> f64 {
        use crate::model::movement::Position;
        let mut modifier = 100.0;
        if attacker_z - target_z > 50 {
            modifier += self.high_bonus;
        } else if attacker_z - target_z < -50 {
            modifier += self.low_bonus;
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
        // Level ground, front: ×1.0. Back + high ground: ×1.13. Low ground
        // from the side: ×1.02.
        assert!((data.condition_bonus(0, 0, Position::Front) - 1.0).abs() < 1e-9);
        assert!((data.condition_bonus(60, 0, Position::Back) - 1.13).abs() < 1e-9);
        assert!((data.condition_bonus(-60, 0, Position::Side) - 1.02).abs() < 1e-9);
    }
}
