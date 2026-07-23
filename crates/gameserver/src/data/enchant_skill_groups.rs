//! Port of `EnchantSkillGroupsData` (`data/EnchantSkillGroups.xml`) — the
//! per-enchant-level cost/chance table for **skill** enchanting (30 levels on
//! this dist; +1…+20 routes read levels 1–20, the file carries 30). The route
//! map half of Java's class lives on `SkillData` (`enchant_routes`), filled
//! during skill parse. PLAN_G19_SKILL_ENCHANT.md.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

const FILE: &str = "data/EnchantSkillGroups.xml";

/// One `<enchant level=…>` row: everything keyed by the enchant **type**
/// (`NORMAL` / `BLESSED` / `CHANGE` / `IMMORTAL` — the book families).
#[derive(Debug, Clone, Default)]
pub struct EnchantSkillCost {
    pub level: i32,
    /// `enchantFailLevel` — the sub-level step a failed NORMAL enchant drops
    /// back to (0 = lose the whole route's progress).
    pub enchant_fail_level: i32,
    /// `<sps><sp amount type/>` — the SP price.
    pub sp: HashMap<String, i64>,
    /// `<chances><chance value type/>` — success %. Types absent here
    /// (`CHANGE`, `IMMORTAL`) never fail in Java.
    pub chance: HashMap<String, i32>,
    /// `<items><item id count type/>` — the required items (Giant's Codex
    /// variants + adena).
    pub items: HashMap<String, Vec<(i32, i64)>>,
}

#[derive(Debug, Clone, Default)]
pub struct EnchantSkillGroups {
    /// Keyed by enchant level (1-based).
    levels: HashMap<i32, EnchantSkillCost>,
}

impl EnchantSkillGroups {
    pub fn load_from(file_path: &str) -> Self {
        let mut levels = HashMap::new();
        if let Ok(content) = std::fs::read_to_string(format!("{file_path}{FILE}")) {
            parse_str(&content, &mut levels);
        }
        info!(
            "EnchantSkillGroups: Loaded {} enchant levels.",
            levels.len()
        );
        Self { levels }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    /// The cost row for enchanting *to* sub-step `n` of a route (Java indexes
    /// the table by `subLevel % 1000`).
    pub fn cost_for(&self, enchant_level: i32) -> Option<&EnchantSkillCost> {
        self.levels.get(&enchant_level)
    }

    pub fn len(&self) -> usize {
        self.levels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, cost: EnchantSkillCost) {
        self.levels.insert(cost.level, cost);
    }
}

fn parse_str(content: &str, out: &mut HashMap<i32, EnchantSkillCost>) {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);
    let mut cur: Option<EnchantSkillCost> = None;
    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(e) => e,
        };
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let attr = |key: &[u8]| {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == key)
                        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
                };
                match e.name().as_ref() {
                    b"enchant" => {
                        let level = attr(b"level").and_then(|v| v.parse().ok()).unwrap_or(0);
                        cur = Some(EnchantSkillCost {
                            level,
                            enchant_fail_level: attr(b"enchantFailLevel")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                            ..Default::default()
                        });
                    }
                    b"sp" => {
                        if let (Some(c), Some(ty), Some(amount)) =
                            (cur.as_mut(), attr(b"type"), attr(b"amount"))
                        {
                            c.sp.insert(ty, amount.parse().unwrap_or(0));
                        }
                    }
                    b"chance" => {
                        if let (Some(c), Some(ty), Some(value)) =
                            (cur.as_mut(), attr(b"type"), attr(b"value"))
                        {
                            c.chance.insert(ty, value.parse().unwrap_or(0));
                        }
                    }
                    b"item" => {
                        if let (Some(c), Some(ty), Some(id)) =
                            (cur.as_mut(), attr(b"type"), attr(b"id"))
                        {
                            let count = attr(b"count").and_then(|v| v.parse().ok()).unwrap_or(0);
                            if let Ok(id) = id.parse() {
                                c.items.entry(ty).or_default().push((id, count));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::End(e) if e.name().as_ref() == b"enchant" => {
                if let Some(c) = cur.take() {
                    out.insert(c.level, c);
                }
            }
            _ => {}
        }
    }
}
