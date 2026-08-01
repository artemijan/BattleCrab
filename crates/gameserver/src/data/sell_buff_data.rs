//! `data/SellBuffData.xml` — the whitelist of skills a player may put in their
//! buff shop (Java `SellBuffsManager.ALLOWED_BUFFS`).
//!
//! 149 ids ship; **99 of them are learnable** from this dist's class trees, so
//! the feature is genuinely reachable rather than an off-chronicle leftover
//! (the rest are later-chronicle ISS buffs a character here cannot know).

use std::collections::HashSet;

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

pub const SELL_BUFF_DATA_FILE: &str = "data/SellBuffData.xml";

#[derive(Debug, Clone, Default)]
pub struct SellBuffData {
    allowed: HashSet<i32>,
}

impl SellBuffData {
    pub fn load_from(root: &str) -> Self {
        let path = format!("{root}{SELL_BUFF_DATA_FILE}");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        let mut reader = Reader::from_str(&content);
        let mut allowed = HashSet::new();
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"skill" => {
                    if let Some(id) = e
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"id")
                        .and_then(|a| String::from_utf8_lossy(&a.value).parse::<i32>().ok())
                    {
                        allowed.insert(id);
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        info!("SellBuffData: Loaded {} allowed buffs.", allowed.len());
        Self { allowed }
    }

    /// Java `ALLOWED_BUFFS.contains(skill.getId())`.
    pub fn allows(&self, skill_id: i32) -> bool {
        self.allowed.contains(&skill_id)
    }

    pub fn len(&self) -> usize {
        self.allowed.len()
    }

    pub fn is_empty(&self) -> bool {
        self.allowed.is_empty()
    }

    #[cfg(test)]
    pub fn insert_for_test(&mut self, skill_id: i32) {
        self.allowed.insert(skill_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dist_list_loads() {
        let data =
            SellBuffData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        assert_eq!(data.len(), 149, "every `<skill id=…>` in the file");
        assert!(data.allows(264), "the lowest id");
        assert!(data.allows(11612), "and the highest");
        assert!(!data.allows(1), "an unlisted skill is not sellable");
    }
}
