//! Port of `BaseStat`'s stat-bonus table from `data/stats/statBonus.xml`.
//! Each base stat (STR/DEX/CON/INT/WIT/MEN) maps a stat value to a multiplier;
//! HP uses CON, MP uses MEN (see `MaxHpFinalizer`/`MaxMpFinalizer`).

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

pub const STAT_BONUS_FILE: &str = "data/stats/statBonus.xml";

pub struct StatBonus {
    /// stat name → (value → bonus multiplier).
    bonus: HashMap<String, Vec<f64>>,
}

impl StatBonus {
    pub fn load() -> Self {
        Self::load_from("")
    }
    pub fn load_from(file_path: &str) -> Self {
        let content = std::fs::read_to_string(format!("{file_path}{STAT_BONUS_FILE}"))
            .unwrap_or_else(|e| panic!("StatBonus: cannot read {STAT_BONUS_FILE}: {e}"));
        let mut reader = Reader::from_str(&content);
        let mut bonus: HashMap<String, Vec<f64>> = HashMap::new();
        let mut current: Option<String> = None;

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                    if name != "list" {
                        current = Some(name);
                    }
                }
                Ok(Event::Empty(e)) if e.name().as_ref() == b"stat" => {
                    if let Some(stat) = &current {
                        let mut value = 0usize;
                        let mut b = 1.0f64;
                        for a in e.attributes().flatten() {
                            match a.key.as_ref() {
                                b"value" => {
                                    value = String::from_utf8_lossy(&a.value).parse().unwrap_or(0)
                                }
                                b"bonus" => {
                                    b = String::from_utf8_lossy(&a.value).parse().unwrap_or(1.0)
                                }
                                _ => {}
                            }
                        }
                        let arr = bonus.entry(stat.clone()).or_default();
                        if arr.len() <= value {
                            arr.resize(value + 1, 1.0);
                        }
                        arr[value] = b;
                    }
                }
                Ok(Event::End(e)) => {
                    if current.as_deref() == Some(&String::from_utf8_lossy(e.name().as_ref())) {
                        current = None;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => panic!("StatBonus: parse error: {e}"),
                _ => {}
            }
        }
        info!("StatBonus: Loaded bonus tables for {} stats.", bonus.len());
        Self { bonus }
    }

    fn get(&self, stat: &str, value: i32) -> f64 {
        // Java `calcBonus`: value < 1 → 1.0; else the table entry.
        if value < 1 {
            return 1.0;
        }
        self.bonus
            .get(stat)
            .and_then(|a| a.get(value as usize))
            .copied()
            .unwrap_or(1.0)
    }

    pub fn con_bonus(&self, con: i32) -> f64 {
        self.get("CON", con)
    }
    pub fn men_bonus(&self, men: i32) -> f64 {
        self.get("MEN", men)
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            bonus: HashMap::new(),
        }
    }
}
