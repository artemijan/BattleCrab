//! Port of `data/xml/KarmaData` — `data/stats/chars/pcKarmaIncrease.xml`, the
//! per-level divisor in `Formulas.calculateKarmaLost`.
//!
//! One number per level, and it is what makes karma decay *slower* the higher a
//! PK is: the level-1 divisor is 1.14 and the level-79 one is 190, so the same
//! experience buys a level-79 character roughly a 170th of the redemption it
//! buys a beginner. Without this table the port had no karma decay at all — a
//! PK could never work their reputation off by hunting, only by dying.

use crate::data::xml;
use quick_xml::events::Event;
use std::collections::HashMap;
use tracing::info;

pub const KARMA_FILE: &str = "data/stats/chars/pcKarmaIncrease.xml";

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct KarmaData {
    by_level: HashMap<i32, f64>,
}

impl KarmaData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let full_path = format!("{file_path}{KARMA_FILE}");
        let content = std::fs::read_to_string(&full_path).unwrap_or_else(|e| {
            let cwd = std::env::current_dir().unwrap();
            panic!("KarmaData: cannot read {full_path}: {e}, CWD: {cwd:?}")
        });
        let mut by_level = HashMap::new();
        for event in xml::events(&content) {
            match event {
                Event::Start(e) | Event::Empty(e) if e.name().as_ref() == b"increase" => {
                    let attr = |key: &[u8]| xml::attr_str(&e, key);
                    let (Some(level), Some(val)) = (
                        attr(b"lvl").and_then(|v| v.parse::<i32>().ok()),
                        attr(b"val").and_then(|v| v.parse::<f64>().ok()),
                    ) else {
                        continue;
                    };
                    by_level.insert(level, val);
                }
                _ => {}
            }
        }
        info!("KarmaData: Loaded {} karma modifiers.", by_level.len());
        Self { by_level }
    }

    /// Java `KarmaData.getMultiplier(level)`.
    ///
    /// **Two deliberate departures, both at the top of the table.** Java stops
    /// parsing at `lvl >= MaximumPlayerLevel` (80 here), so its map holds levels
    /// 1–79 and `getMultiplier` — which unboxes a `Double` straight out of the
    /// map — would throw for anything past that. It never does, because Java's
    /// `ExperienceData` clamps the attainable cap to `MaximumPlayerLevel` and
    /// stops a character one level short of it, at 79.
    ///
    /// This port does neither: it keeps every row the file ships (1–99), and its
    /// own attainable cap is higher than Java's because `ExperienceData` reads
    /// `maxLevel` raw and nothing reads `MaximumPlayerLevel` at all (recorded
    /// separately in PORTING_STATUS.md's measured gaps). Answering from the row
    /// the file actually declares is the only reading that keeps decay working
    /// across that whole range; falling back to the highest row below covers a
    /// level past the file's end.
    pub fn multiplier(&self, level: i32) -> Option<f64> {
        self.by_level.get(&level).copied().or_else(|| {
            self.by_level
                .iter()
                .filter(|(l, _)| **l <= level)
                .max_by_key(|(l, _)| **l)
                .map(|(_, v)| *v)
        })
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            by_level: HashMap::new(),
        }
    }

    #[cfg(test)]
    pub fn insert_for_test(&mut self, level: i32, val: f64) {
        self.by_level.insert(level, val);
    }
}

#[cfg(test)]
mod tests {
    use crate::data::dist;

    /// The shipped table loads, and the divisor climbs steeply with level —
    /// which is the whole mechanic: redemption by hunting gets slower the
    /// deeper a PK is.
    #[test]
    fn real_dist_karma_table_loads() {
        let d = dist::karma();
        assert_eq!(
            d.multiplier(1),
            Some(1.13511094305),
            "the level-1 divisor, verbatim from the file"
        );
        assert_eq!(d.multiplier(99), Some(284.804739660698), "and the last row");

        let (low, high) = (d.multiplier(20).unwrap(), d.multiplier(70).unwrap());
        assert!(
            high > low * 10.0,
            "the divisor grows by more than 10x over 50 levels ({low} → {high})"
        );

        // Past the file's end, the top row stands in rather than vanishing.
        assert_eq!(d.multiplier(150), d.multiplier(99));
    }
}
