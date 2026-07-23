//! Port of `data/xml/PlayerXpPercentLostData` — per-level XP percentage lost
//! on death, from `data/stats/chars/playerXpPercentLost.xml`.

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

pub const XP_LOST_FILE: &str = "data/stats/chars/playerXpPercentLost.xml";

/// Java default when a level is missing from the table.
const DEFAULT_XP_PERCENT_LOST: f64 = 1.0;

pub struct PlayerXpPercentLostData {
    /// `percent[level]` — indexed 1..=max parsed level.
    percent: Vec<f64>,
}

impl PlayerXpPercentLostData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut entries: Vec<(usize, f64)> = Vec::new();
        if let Ok(content) = std::fs::read_to_string(format!("{file_path}{XP_LOST_FILE}")) {
            let mut reader = Reader::from_str(&content);
            while let Ok(event) = reader.read_event() {
                match event {
                    Event::Empty(e) | Event::Start(e) if e.name().as_ref() == b"xpLost" => {
                        let mut level = 0usize;
                        let mut val = DEFAULT_XP_PERCENT_LOST;
                        for a in e.attributes().flatten() {
                            let v = String::from_utf8_lossy(&a.value).into_owned();
                            match a.key.as_ref() {
                                b"level" => level = v.parse().unwrap_or(0),
                                b"val" => val = v.parse().unwrap_or(DEFAULT_XP_PERCENT_LOST),
                                _ => {}
                            }
                        }
                        if level > 0 {
                            entries.push((level, val));
                        }
                    }
                    Event::Eof => break,
                    _ => {}
                }
            }
        }
        let top = entries.iter().map(|&(l, _)| l).max().unwrap_or(0);
        let mut percent = vec![DEFAULT_XP_PERCENT_LOST; top + 1];
        for (level, val) in entries {
            percent[level] = val;
        }
        if top > 0 {
            info!("PlayerXpPercentLostData: Loaded {top} levels.");
        }
        Self { percent }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            percent: Vec::new(),
        }
    }

    /// `getXpPercent(level)` — Java warns and returns 1.0 above the table.
    pub fn xp_percent(&self, level: i32) -> f64 {
        self.percent
            .get(level.max(0) as usize)
            .copied()
            .unwrap_or(DEFAULT_XP_PERCENT_LOST)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_real_dist_table() {
        let data = PlayerXpPercentLostData::load_from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/"
        ));
        assert_eq!(data.xp_percent(1), 10.0);
        assert_eq!(data.xp_percent(40), 5.125); // 10 − 39·0.125
                                                // Above the table: Java warns and falls back to 1.0.
        assert_eq!(data.xp_percent(500), 1.0);
    }
}
