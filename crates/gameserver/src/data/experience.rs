//! Port of `data/xml/ExperienceData` — cumulative XP required per level from
//! `data/stats/experience.xml`, and the level cap that comes out of it.
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
//! On this dist that is `min(85 + 1, 80 + 1) = 81`, so the highest level a
//! character reaches is **80** — the Interlude cap. [`ExperienceData::max_level`]
//! is Java's `MAX_LEVEL`, so every consumer spells the cap `max_level - 1`
//! exactly as the Java expressions do.
//!
//! Two consequences carried over with it: rows above `PLAYER_MAXIMUM_LEVEL` are
//! **not loaded** (Java `break`s out of the row loop), and `getExpForLevel` of
//! anything past the cap answers with the last row that was.
//!
//! # `maxPetLevel` is parsed by Java and consumed by almost nothing
//!
//! `MAX_PET_LEVEL = min(maxPetLevel + 1, MAX_LEVEL + 1)` — 82 here, a ceiling
//! no pet on this chronicle can approach, since the highest row in any
//! `PetData` species table is far below it. Its only readers are
//! `Summon.getExpForThisLevel`/`getExpForNextLevel`, and the port's pet exp
//! bar reads the species table instead (`servitor::exp::max_pet_level`), which
//! is the binding constraint on both sides. Named here rather than given a
//! field, because a field would imply something consults it.

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

pub const EXPERIENCE_FILE: &str = "data/stats/experience.xml";

/// This dist's `MaximumPlayerLevel + 1` — the value [`ExperienceData::load_from`]
/// assumes, so the cached test snapshot is the shipped configuration. The
/// server passes the real one through `DataOptions`.
pub const DIST_PLAYER_MAXIMUM_LEVEL: i32 = 81;

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
        let mut entries: Vec<(u8, i64)> = Vec::new();

        loop {
            match reader.read_event() {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) => match e.name().as_ref() {
                    b"table" => {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"maxLevel" {
                                table_max_level = attr_str(&a.value).parse().unwrap_or(0);
                            }
                        }
                    }
                    b"experience" => {
                        let mut level = 0i32;
                        let mut tolevel = 0i64;
                        for a in e.attributes().flatten() {
                            match a.key.as_ref() {
                                b"level" => level = attr_str(&a.value).parse().unwrap_or(0),
                                b"tolevel" => tolevel = attr_str(&a.value).parse().unwrap_or(0),
                                _ => {}
                            }
                        }
                        // Java `break`s here rather than skipping, so a row
                        // above the cap ends the load: the rows are ordered,
                        // which makes the two the same thing.
                        if level > player_maximum_level {
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

        // `MAX_LEVEL = maxLevel + 1`, clamped to `Config.PLAYER_MAXIMUM_LEVEL`.
        let max_level = (table_max_level + 1).min(player_maximum_level).max(0) as u8;
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

fn attr_str(v: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIST: &str = crate::data::DIST_GAME;

    /// The dist's own numbers: `experience.xml` declares `maxLevel="85"` and
    /// carries rows up to 86, `Character.ini` says `MaximumPlayerLevel = 80`.
    /// Java's arithmetic — `(85 + 1)` clamped to `(80 + 1)` — makes the cap 81,
    /// which is level **80** to a player.
    #[test]
    fn the_dist_table_is_clamped_to_the_configured_maximum() {
        let data = ExperienceData::load_from(DIST);
        assert_eq!(data.max_level, 81, "MAX_LEVEL = min(85 + 1, 80 + 1)");

        // The rows above the cap were never loaded, so asking for one answers
        // with the last row that was — Java's `getExpForLevel` guard.
        let at_cap = data.exp_for_level(81);
        assert!(at_cap > 0, "level 81's row is the cap's own threshold");
        assert_eq!(data.exp_for_level(86), at_cap, "row 86 was not loaded");
        assert_eq!(data.exp_for_level(999), at_cap);
    }

    /// The config is a *ceiling*, not the answer: with room to spare, the
    /// table's own `maxLevel + 1` wins. This is what the `+ 1` on each side is
    /// for — the cap names the row **above** the highest attainable level.
    #[test]
    fn the_table_keeps_its_own_maximum_when_the_config_allows() {
        let data = ExperienceData::load_from_with(DIST, 200);
        assert_eq!(data.max_level, 86, "85 + 1, unclamped");
        assert!(
            data.exp_for_level(86) > data.exp_for_level(85),
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
