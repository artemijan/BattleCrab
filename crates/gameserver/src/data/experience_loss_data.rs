//! Port of `data/xml/ExperienceLossData` — per-level XP percentage lost on
//! death, from `data/stats/players/experienceLoss.xml` (`<experience level
//! val>`).
//!
//! Java sizes the table by `Config.PLAYER_MAXIMUM_LEVEL` (the ini value **plus
//! one**), fills it with `1.0`, and drops every row at or above that bound. A
//! level the file does not name therefore reads `1.0`, and a request at or past
//! the bound answers with the table's last slot.

use crate::data::xml;
use quick_xml::events::Event;
use tracing::info;

pub const EXPERIENCE_LOSS_FILE: &str = "data/stats/players/experienceLoss.xml";

/// Java's `Arrays.fill(_levelPercentLost, 1d)`.
const DEFAULT_PERCENT_LOST: f64 = 1.0;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ExperienceLossData {
    /// `percent[level]`, `PLAYER_MAXIMUM_LEVEL` slots (Java `_levelPercentLost`).
    percent: Vec<f64>,
}

impl ExperienceLossData {
    /// The table under this dist's shipped `MaximumPlayerLevel`.
    pub fn load_from(file_path: &str) -> Self {
        Self::load_from_with(
            file_path,
            crate::data::experience::DIST_PLAYER_MAXIMUM_LEVEL,
        )
    }

    /// `player_maximum_level` is Java's `PlayerConfig.PLAYER_MAXIMUM_LEVEL`,
    /// already incremented.
    pub fn load_from_with(file_path: &str, player_maximum_level: i32) -> Self {
        let size = player_maximum_level.max(1) as usize;
        let mut percent = vec![DEFAULT_PERCENT_LOST; size];
        if let Ok(content) = std::fs::read_to_string(format!("{file_path}{EXPERIENCE_LOSS_FILE}")) {
            for event in xml::events(&content) {
                match event {
                    Event::Empty(e) | Event::Start(e) if e.name().as_ref() == "experience" => {
                        let (Some(level), Some(val)) =
                            (xml::attr_i32(&e, "level"), xml::attr_f64(&e, "val"))
                        else {
                            continue;
                        };
                        if level < 0 || level >= player_maximum_level {
                            continue;
                        }
                        percent[level as usize] = val;
                    }
                    _ => {}
                }
            }
        }
        info!("ExperienceLossData: Loaded {} levels.", percent.len() - 1);
        Self { percent }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            percent: vec![DEFAULT_PERCENT_LOST],
        }
    }

    /// Java `getPercentLost(level)`: at or above the bound, the last slot.
    pub fn percent_lost(&self, level: i32) -> f64 {
        let last = self.percent.len() - 1;
        let idx = (level.max(0) as usize).min(last);
        self.percent[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `experienceLoss.xml`: level 1 is `10.0`, level 40 is `5.125`
    /// (10 − 39·0.125), and levels 100–111 are `0.5`. Under the shipped
    /// `MaximumPlayerLevel = 110` the table holds 111 slots (0..=110), so the
    /// level-111 row is never read.
    #[test]
    fn loads_real_dist_table() {
        let data = ExperienceLossData::load_from(crate::data::DIST_GAME);
        assert_eq!(data.percent_lost(1), 10.0);
        assert_eq!(data.percent_lost(40), 5.125);
        assert_eq!(data.percent.len(), 111);
        // Past the bound: Java warns and answers with the last slot.
        assert_eq!(data.percent_lost(110), 0.5);
        assert_eq!(data.percent_lost(500), 0.5);
    }
}
