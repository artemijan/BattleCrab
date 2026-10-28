//! Port of `data/xml/ExperienceData` — cumulative XP required per level from
//! `data/stats/players/experience.xml`, and the level cap that comes out of it.
//!
//! # The cap is two numbers multiplied by an off-by-one
//!
//! Java never treats the table's `maxLevel` as the cap. `parseDocument` reads
//! it and immediately adds one — `MAX_LEVEL = maxLevel + 1` — then clamps that
//! to `Config.PLAYER_MAXIMUM_LEVEL`, which `Config.load` has *also* already
//! incremented. Both `+ 1`s are deliberate: the exp cap is
//! `getExpForLevel(MAX_LEVEL) - 1`, one point short of the row **above** the
//! highest attainable level, so the table needs that extra row to exist and
//! `MAX_LEVEL` names it. Java's own boot log says as much — it prints
//! `MAX_LEVEL - 1` as "Max Player Level".
//!
//! On this dist that is `min(110 + 1, 110 + 1) = 111`, so the highest level a
//! character reaches is **110** — the Salvation cap. [`ExperienceData::max_level`]
//! is Java's `MAX_LEVEL`, so every consumer spells the cap `max_level - 1`
//! exactly as the Java expressions do.
//!
//! Two consequences carried over with it: rows above `PLAYER_MAXIMUM_LEVEL` or
//! above `MAX_LEVEL` are **not loaded** (the file's level-111 row is the one
//! above the cap, so it is read), and `getExpForLevel` of anything past the cap
//! answers with the last row that was.
//!
//! # `maxPetLevel` is parsed by Java and consumed by almost nothing
//!
//! `MAX_PET_LEVEL = min(maxPetLevel + 1, MAX_LEVEL + 1)` — 111 here. Its only readers are
//! `Summon.getExpForThisLevel`/`getExpForNextLevel`, and the port's pet exp
//! bar reads the species table instead (`servitor::exp::max_pet_level`), which
//! is the binding constraint on both sides. Named here rather than given a
//! field, because a field would imply something consults it.

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

pub const EXPERIENCE_FILE: &str = "data/stats/players/experience.xml";

/// This dist's `MaximumPlayerLevel + 1` — the value [`ExperienceData::load_from`]
/// assumes, so the cached test snapshot is the shipped configuration. The
/// server passes the real one through `DataOptions`.
pub const DIST_PLAYER_MAXIMUM_LEVEL: i32 = 111;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ExperienceData {
    /// `exp_for_level[level]` = cumulative XP to reach `level` (Java `tolevel`).
    exp_for_level: Vec<i64>,
    /// Java `MAX_LEVEL`: one **above** the highest attainable level. See the
    /// module header — `max_level - 1` is the cap a player can reach.
    pub max_level: u8,
}

impl ExperienceData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    /// The table under this dist's shipped `MaximumPlayerLevel`.
    pub fn load_from(file_path: &str) -> Self {
        Self::load_from_with(file_path, DIST_PLAYER_MAXIMUM_LEVEL)
    }

    /// `player_maximum_level` is Java's `Config.PLAYER_MAXIMUM_LEVEL` — the
    /// `Character.ini` value **already incremented**, as
    /// [`crate::config::CharacterConfig::maximum_player_level`] stores it.
    pub fn load_from_with(file_path: &str, player_maximum_level: i32) -> Self {
        let full_path = format!("{file_path}{EXPERIENCE_FILE}");
        let content = std::fs::read_to_string(&full_path).unwrap_or_else(|e| {
            let cwd = std::env::current_dir().unwrap();
            panic!("ExperienceData: cannot read {full_path}: {e}, CWD: {cwd:?}")
        });
        let mut reader = Reader::from_str(&content);
        let mut table_max_level = 0i32;
        let mut max_level = 0i32;
        let mut entries: Vec<(u8, i64)> = Vec::new();

        loop {
            match reader.read_event() {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) => match e.name().as_ref() {
                    "table" => {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == "maxLevel" {
                                table_max_level = a.value.parse().unwrap_or(0);
                            }
                        }
                        // `MAX_LEVEL = maxLevel + 1`, clamped to
                        // `PLAYER_MAXIMUM_LEVEL` — before the rows, which it bounds.
                        max_level = (table_max_level + 1).min(player_maximum_level);
                    }
                    "experience" => {
                        let mut level = 0i32;
                        let mut tolevel = 0i64;
                        for a in e.attributes().flatten() {
                            match a.key.as_ref() {
                                "level" => level = a.value.parse().unwrap_or(0),
                                "tolevel" => tolevel = a.value.parse().unwrap_or(0),
                                _ => {}
                            }
                        }
                        // Java `return`s from the row lambda: a row above
                        // either bound is skipped.
                        if level > player_maximum_level || level > max_level {
                            continue;
                        }
                        entries.push((level as u8, tolevel));
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(e) => panic!("ExperienceData: parse error in {EXPERIENCE_FILE}: {e}"),
                _ => {}
            }
        }

        let max_level = max_level.max(0) as u8;
        let top = entries.iter().map(|(l, _)| *l).max().unwrap_or(0) as usize;
        let mut exp_for_level = vec![0i64; top + 1];
        for (level, tolevel) in entries {
            exp_for_level[level as usize] = tolevel;
        }
        info!("ExperienceData: Loaded {} levels.", top);
        info!("ExperienceData: Max Player Level is {}.", max_level - 1);
        Self {
            exp_for_level,
            max_level,
        }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            exp_for_level: vec![0, 0],
            max_level: 0,
        }
    }

    /// Synthetic cumulative table for unit tests: `table[level] = tolevel`.
    #[doc(hidden)]
    pub fn from_table(exp_for_level: Vec<i64>, max_level: u8) -> Self {
        Self {
            exp_for_level,
            max_level,
        }
    }

    /// `getExpForLevel(level)`. Java answers anything past
    /// `PLAYER_MAXIMUM_LEVEL` with that level's row, and the table stops there
    /// — so clamping to the last row loaded is the same answer.
    pub fn exp_for_level(&self, level: i32) -> i64 {
        let idx = level.clamp(1, self.exp_for_level.len() as i32 - 1) as usize;
        self.exp_for_level[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIST: &str = crate::data::DIST_GAME;

    /// The dist's own numbers: `experience.xml` declares `maxLevel="110"` and
    /// carries rows up to 111, `Player.ini` says `MaximumPlayerLevel = 110`.
    /// Java's arithmetic — `(110 + 1)` clamped to `(110 + 1)` — makes the cap
    /// 111, which is level **110** to a player.
    #[test]
    fn the_dist_table_is_clamped_to_the_configured_maximum() {
        let data = ExperienceData::load_from(DIST);
        assert_eq!(data.max_level, 111, "MAX_LEVEL = min(110 + 1, 110 + 1)");

        // Row 111 is the cap's own threshold, verbatim from the file.
        let at_cap = data.exp_for_level(111);
        assert_eq!(at_cap, 46_239_023_912_827_500);
        assert_eq!(data.exp_for_level(110), 32_274_410_352_357_300);
        assert_eq!(data.exp_for_level(999), at_cap);
    }

    /// The table's own `maxLevel + 1` is a ceiling too: with the config set
    /// higher, rows above 111 would still be dropped (there are none), and the
    /// cap stays 111.
    #[test]
    fn the_table_keeps_its_own_maximum_when_the_config_allows() {
        let data = ExperienceData::load_from_with(DIST, 200);
        assert_eq!(data.max_level, 111, "110 + 1, unclamped");
        assert!(
            data.exp_for_level(111) > data.exp_for_level(110),
            "and the top row is loaded"
        );
    }

    /// A lower `MaximumPlayerLevel` moves both halves: the cap and how much of
    /// the table is read.
    #[test]
    fn a_lower_maximum_player_level_truncates_the_table() {
        let data = ExperienceData::load_from_with(DIST, 41);
        assert_eq!(data.max_level, 41, "a level-40 server");
        let at_cap = data.exp_for_level(41);
        assert_eq!(data.exp_for_level(42), at_cap, "nothing above 41 loaded");
        assert!(at_cap > data.exp_for_level(40));
    }
}
