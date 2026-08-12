//! Port of `data/xml/BuyListData` — every `data/buylists/*.xml` merchant
//! list plus `data/buylists/custom/*.xml` (the GM-shop lists opened via
//! `//buy`; Java parses both directories). The file name is the list id.
//! Scoped to the G12 Buy slice:
//!
//! - `CorrectPrices` (General.ini, **True** here) is applied at load like
//!   Java: a declared price below the item's sell value (reference price /
//!   2) is floored to it.
//! - Limited stock (`count`/`restock_delay`, 3 files) is **not** tracked —
//!   those products sell as unlimited, and the DB-backed restock counts are
//!   skipped with them (deliberate deviation until something needs them).
//! - `CASTLE_GUARD` price scaling and the max-equipable-grade filter are
//!   skipped (no sieges; grade config is default off).

use std::collections::HashMap;

use quick_xml::events::Event;
use tracing::{info, warn};

use super::item_data::ItemData;
use crate::data::xml;
use crate::data::xml::attr_str;

pub const BUYLISTS_DIR: &str = "data/buylists";

#[derive(Debug, Clone)]
pub struct Product {
    pub item_id: i32,
    /// Adena price per unit; -1 = undeclared (a buy attempt is rejected).
    pub price: i64,
    /// `baseTax` percent (the packet/charge multiply by `1 + base_tax/100`).
    pub base_tax: i32,
}

#[derive(Debug, Clone, Default)]
pub struct BuyList {
    pub list_id: i32,
    /// NPC ids allowed to serve this list (empty = nobody, like Java's
    /// null `npcsAllowed` — `isNpcAllowed` fails).
    pub npcs: Vec<i32>,
    pub products: Vec<Product>,
}

impl BuyList {
    pub fn is_npc_allowed(&self, npc_id: i32) -> bool {
        self.npcs.contains(&npc_id)
    }
    pub fn product(&self, item_id: i32) -> Option<&Product> {
        self.products.iter().find(|p| p.item_id == item_id)
    }
}

#[derive(Default, Clone)]
pub struct BuyListData {
    by_id: HashMap<i32, BuyList>,
}

impl BuyListData {
    pub fn load(items: &ItemData) -> Self {
        Self::load_from("", items)
    }

    pub fn load_from(file_path: &str, items: &ItemData) -> Self {
        let mut by_id = HashMap::new();
        // Java parses "data/buylists" then "data/buylists/custom"; on an id
        // collision the later (custom) file wins.
        let dir = format!("{file_path}{BUYLISTS_DIR}");
        load_dir(&dir, items, &mut by_id);
        load_dir(&format!("{dir}/custom"), items, &mut by_id);
        info!("BuyListData: Loaded {} buy lists.", by_id.len());
        Self { by_id }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Test hook.
    pub fn insert_for_test(&mut self, list: BuyList) {
        self.by_id.insert(list.list_id, list);
    }

    pub fn get(&self, list_id: i32) -> Option<&BuyList> {
        self.by_id.get(&list_id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

fn load_dir(dir: &str, items: &ItemData, by_id: &mut HashMap<i32, BuyList>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }
        let Some(list_id) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<i32>().ok())
        else {
            warn!("BuyListData: non-numeric buylist file {}", path.display());
            continue;
        };
        if let Some(list) = parse_file(&path, list_id, items) {
            by_id.insert(list_id, list);
        }
    }
}

fn parse_file(path: &std::path::Path, list_id: i32, items: &ItemData) -> Option<BuyList> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut list = BuyList {
        list_id,
        ..Default::default()
    };
    let mut default_base_tax = 0i32;
    let mut in_npcs = false;
    let mut in_npc = false;

    for event in xml::events(&content) {
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let attr = |key: &[u8]| attr_str(&e, key);
                match e.name().as_ref() {
                    b"list" => {
                        default_base_tax =
                            attr(b"baseTax").and_then(|v| v.parse().ok()).unwrap_or(0);
                    }
                    b"npcs" => in_npcs = true,
                    b"npc" if in_npcs => in_npc = true,
                    b"item" => {
                        let Some(item_id) = attr(b"id").and_then(|v| v.parse::<i32>().ok()) else {
                            continue;
                        };
                        let Some(template) = items.get(item_id) else {
                            warn!("BuyListData: item {item_id} not found (buylist {list_id})");
                            continue;
                        };
                        let mut price: i64 =
                            attr(b"price").and_then(|v| v.parse().ok()).unwrap_or(-1);
                        // `Config.CORRECT_PRICES` (True on this dist): never
                        // sell below the item's own sell value.
                        let sell_price = template.price / 2;
                        if price > -1 && sell_price > price {
                            price = sell_price;
                        }
                        let base_tax = attr(b"baseTax")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(default_base_tax);
                        list.products.push(Product {
                            item_id,
                            price,
                            base_tax,
                        });
                    }
                    _ => {}
                }
            }
            Event::Text(t) if in_npc => {
                if let Ok(id) = String::from_utf8_lossy(&t.into_inner())
                    .trim()
                    .parse::<i32>()
                {
                    list.npcs.push(id);
                }
            }
            Event::End(e) => match e.name().as_ref() {
                b"npcs" => in_npcs = false,
                b"npc" => in_npc = false,
                _ => {}
            },
            _ => {}
        }
    }
    Some(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_real_dist_files() {
        let root = crate::data::DIST_GAME;
        let items = ItemData::load_from(root);
        let data = BuyListData::load_from(root, &items);
        // 338 regular merchant lists + 143 custom (GM shop) lists.
        assert_eq!(data.len(), 481);

        // custom/0009916.xml: GM-shop "Currency" list (`//buy 9916`). No
        // <npcs> block — only the admin path, which skips the npc check,
        // can open it.
        let gm = data.get(9916).expect("GM currency buylist 9916");
        assert!(gm.npcs.is_empty());
        assert!(gm.product(57).is_some(), "adena line");

        // 3000101.xml: Weapon Trader Lector's Gludin weapon list.
        let list = data.get(3000101).expect("buylist 3000101");
        assert!(list.is_npc_allowed(30001));
        assert!(!list.is_npc_allowed(31319));
        let p = list.product(1).expect("short sword line");
        assert_eq!(p.price, 883);
        assert_eq!(p.base_tax, 0);

        // 0000138.xml declares a list-level baseTax="50".
        let taxed = data.get(138).expect("buylist 138");
        assert!(taxed.is_npc_allowed(31319));
        let p = taxed.product(3031).expect("spirit ore line");
        assert_eq!(p.price, 400);
        assert_eq!(p.base_tax, 50, "list-level baseTax applied");

        // CorrectPrices: no product declares a price below its sell value.
        for list in data.by_id.values() {
            for p in &list.products {
                if p.price > -1
                    && let Some(t) = items.get(p.item_id)
                {
                    assert!(
                        p.price >= t.price / 2,
                        "buylist {} item {}",
                        list.list_id,
                        p.item_id
                    );
                }
            }
        }
    }
}
