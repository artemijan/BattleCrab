//! Port of `data/xml/ExperienceData` — cumulative XP required per level from
//! `data/stats/experience.xml`.

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

pub const EXPERIENCE_FILE: &str = "data/stats/experience.xml";

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ExperienceData {
    /// `exp_for_level[level]` = cumulative XP to reach `level` (Java `tolevel`).
    exp_for_level: Vec<i64>,
    pub max_level: u8,
}

impl ExperienceData {
    pub fn load() -> Self {
        Self::load_from("")
    }
    pub fn load_from(file_path: &str) -> Self {
        let full_path = format!("{file_path}{EXPERIENCE_FILE}");
        let content = std::fs::read_to_string(&full_path).unwrap_or_else(|e| {
            let cwd = std::env::current_dir().unwrap();
            panic!("ExperienceData: cannot read {full_path}: {e}, CWD: {cwd:?}")
        });
        let mut reader = Reader::from_str(&content);
        let mut max_level = 0u8;
        let mut entries: Vec<(u8, i64)> = Vec::new();

        loop {
            match reader.read_event() {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) => match e.name().as_ref() {
                    b"table" => {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"maxLevel" {
                                max_level = attr_str(&a.value).parse().unwrap_or(0);
                            }
                        }
                    }
                    b"experience" => {
                        let mut level = 0u8;
                        let mut tolevel = 0i64;
                        for a in e.attributes().flatten() {
                            match a.key.as_ref() {
                                b"level" => level = attr_str(&a.value).parse().unwrap_or(0),
                                b"tolevel" => tolevel = attr_str(&a.value).parse().unwrap_or(0),
                                _ => {}
                            }
                        }
                        entries.push((level, tolevel));
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(e) => panic!("ExperienceData: parse error in {EXPERIENCE_FILE}: {e}"),
                _ => {}
            }
        }

        let top = entries.iter().map(|(l, _)| *l).max().unwrap_or(0) as usize;
        let mut exp_for_level = vec![0i64; top + 2];
        for (level, tolevel) in entries {
            exp_for_level[level as usize] = tolevel;
        }
        info!("ExperienceData: Loaded {} levels.", top);
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

    /// `getExpForLevel(level)`; out-of-range clamps to the ends (as Java does).
    pub fn exp_for_level(&self, level: i32) -> i64 {
        let idx = level.clamp(1, self.exp_for_level.len() as i32 - 1) as usize;
        self.exp_for_level[idx]
    }
}

fn attr_str(v: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(v)
}
