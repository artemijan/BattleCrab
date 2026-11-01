//! Port of `data/xml/KarmaLossData` — `data/stats/players/karmaLoss.xml`
//! (`<modifier level val>`), the per-level divisor in
//! `Formulas.calculateKarmaLost`.
//!
//! One number per level, and it is what makes karma decay *slower* the higher a
//! PK is: the level-1 divisor is 0.77 and the level-110 one is 4249.78.
//!
//! Java sizes the table by `Config.PLAYER_MAXIMUM_LEVEL` (the ini value **plus
//! one**), fills it with `1.0`, and drops every row at or above that bound;
//! `getModifier` answers a level at or past the bound with the last slot.

use crate::data::xml;
use quick_xml::events::Event;
use tracing::info;

pub const KARMA_LOSS_FILE: &str = "data/stats/players/karmaLoss.xml";

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct KarmaLossData {
    /// Java `_levelModifiers`, `PLAYER_MAXIMUM_LEVEL` slots.
    modifiers: Vec<f64>,
}

impl Default for KarmaLossData {
    fn default() -> Self {
        Self::empty()
    }
}

impl KarmaLossData {
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
        let full_path = format!("{file_path}{KARMA_LOSS_FILE}");
        let content = std::fs::read_to_string(&full_path).unwrap_or_else(|e| {
            let cwd = std::env::current_dir().unwrap();
            panic!("KarmaLossData: cannot read {full_path}: {e}, CWD: {cwd:?}")
        });
        let mut modifiers = vec![1.0; player_maximum_level.max(1) as usize];
        for event in xml::events(&content) {
            match event {
                Event::Start(e) | Event::Empty(e) if e.name().as_ref() == "modifier" => {
                    let (Some(level), Some(val)) =
                        (xml::attr_i32(&e, "level"), xml::attr_f64(&e, "val"))
                    else {
                        continue;
                    };
                    if level < 0 || level >= player_maximum_level {
                        continue;
                    }
                    modifiers[level as usize] = val;
                }
                _ => {}
            }
        }
        info!("KarmaLossData: Loaded {} modifiers.", modifiers.len() - 1);
        Self { modifiers }
    }

    /// Java `KarmaLossData.getModifier(level)`: at or past the bound, the last
    /// slot.
    pub fn modifier(&self, level: i32) -> f64 {
        let last = self.modifiers.len() - 1;
        self.modifiers[(level.max(0) as usize).min(last)]
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            modifiers: vec![1.0],
        }
    }

    #[cfg(test)]
    pub fn insert_for_test(&mut self, level: i32, val: f64) {
        let level = level.max(0) as usize;
        if self.modifiers.len() <= level {
            self.modifiers.resize(level + 1, 1.0);
        }
        self.modifiers[level] = val;
    }
}

#[cfg(test)]
mod tests {
    use crate::data::dist;

    /// `karmaLoss.xml` rows 1 (`0.77`), 40 (`13.16`) and 110 (`4249.78`),
    /// verbatim; past the table's end the last slot stands in.
    #[test]
    fn real_dist_karma_table_loads() {
        let d = dist::karma();
        assert_eq!(d.modifier(1), 0.77);
        assert_eq!(d.modifier(40), 13.16);
        assert_eq!(d.modifier(110), 4249.78);
        assert_eq!(d.modifier(150), 4249.78);
    }
}
