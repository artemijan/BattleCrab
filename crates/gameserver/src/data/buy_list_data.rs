//! Port of `data/xml/BuyListData` — every `data/buylists/*.xml` merchant
//! list plus `data/buylists/custom/*.xml` (the GM-shop lists opened via
//! `//buy`; Java parses both directories). The file name is the list id.
//!
//! Three of Java's rules live in the **`Product` constructor** rather than in
//! `parseDocument`, which is easy to miss when reading the loader alone:
//!
//! - **An undeclared `price` is not "no price".** `_price = (price < 0) ?
//!   item.getReferencePrice() : price` — a bare `<item id="6902" />` sells at
//!   the item's own reference price. 3079 of the 8198 product lines on the
//!   npc-served lists declare no price, so treating -1 as unbuyable takes 38 %
//!   of the merchant catalogue off the shelves.
//! - `restock_delay` is in **minutes** (`_restockDelay = restockDelay *
//!   60000`).
//! - Limited stock is `_maxCount > -1`, so `count="0"` is a stocked product
//!   with nothing left — not an unlimited one.
//!
//! `CorrectPrices` (General.ini, **True** here) floors a declared price at the
//! item's sell value (reference price / 2), but only `if … &&
//! (buyList.getNpcsAllowed() != null)` — the GM-shop lists under `custom/`
//! have no `<npcs>` block, and that is what keeps their `price="0"` lines free.
//!
//! The mutable half of a `Product` — how many are left and when they restock —
//! is **not** here: `world.data` is shared and immutable, so the counts live on
//! the `World` (`buy_list_stock`) and the rules in `game_loop/shop.rs`.

use crate::data::item_data::CrystalType;

use std::collections::HashMap;

use quick_xml::events::Event;
use tracing::{info, warn};

use super::item_data::ItemData;
use crate::data::xml;
use crate::data::xml::attr_str;

pub const BUYLISTS_DIR: &str = "data/buylists";

/// `Config.MAX_EQUIPABLE_ITEM_GRADE` (`Character.ini`, **S** on this dist).
/// Products above it — but below `EVENT`, which is the escape hatch for event
/// gear that carries no real grade — never reach the list. Held as a constant
/// rather than read from config because `GameData::load_from` takes none;
/// `recipe_data` states the same value for the same filter.
const MAX_EQUIPABLE_ITEM_GRADE: CrystalType = CrystalType::S;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Product {
    pub item_id: i32,
    /// Adena price per unit. Never negative: an undeclared `price` was
    /// resolved to the item's reference price at load, exactly as Java's
    /// `Product` constructor does.
    pub price: i64,
    /// `baseTax` percent (the packet/charge multiply by `1 + base_tax/100`).
    pub base_tax: i32,
    /// `count` — the full stock. **-1 = unlimited**, which is what
    /// `has_limited_stock` tests; `0` is a real (empty) stock.
    pub max_count: i64,
    /// `restock_delay` converted from minutes to ms, like Java's constructor.
    /// -60000 when undeclared, which is unreachable: every one of this dist's
    /// 1928 limited-stock lines declares one.
    pub restock_delay_ms: i64,
}

impl Product {
    /// `Product.hasLimitedStock()`.
    pub fn has_limited_stock(&self) -> bool {
        self.max_count > -1
    }

    /// `Product.getPrice()` — the stored price, scaled by
    /// `Config.RATE_SIEGE_GUARDS_PRICE` when the item is a `CASTLE_GUARD`
    /// (the mercenary posting tickets, 11 of them on this dist). The rate
    /// ships as 1, so this is an identity here; it is a getter in Java and
    /// stays a getter here rather than being folded in at load, because the
    /// rate is config and the load is not.
    pub fn price_at(&self, template: &crate::data::item_data::ItemTemplate, rate: f64) -> i64 {
        if template.etc_item_type == crate::data::item_data::EtcItemType::CastleGuard {
            return (self.price as f64 * rate) as i64;
        }
        self.price
    }

    /// Test hook: the shape 10 134 of this dist's 12 062 product lines have.
    pub fn unlimited(item_id: i32, price: i64, base_tax: i32) -> Self {
        Self {
            item_id,
            price,
            base_tax,
            max_count: -1,
            restock_delay_ms: -60_000,
        }
    }

    /// Test hook: the other 1928 — `count` in stock, restocking `minutes`
    /// after the first sale.
    pub fn limited(item_id: i32, price: i64, count: i64, minutes: i64) -> Self {
        Self {
            max_count: count,
            restock_delay_ms: minutes * 60_000,
            ..Self::unlimited(item_id, price, 0)
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
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

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
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
                        // `Config.MAX_EQUIPABLE_ITEM_GRADE` — Java `break`s out
                        // of the item node, dropping the line entirely.
                        let grade = template.crystal_type.level();
                        if grade > MAX_EQUIPABLE_ITEM_GRADE.level()
                            && grade < CrystalType::Event.level()
                        {
                            continue;
                        }
                        let declared: i64 =
                            attr(b"price").and_then(|v| v.parse().ok()).unwrap_or(-1);
                        // `Config.CORRECT_PRICES` (True on this dist): never
                        // sell below the item's own sell value — but only on a
                        // list an npc serves. The `getNpcsAllowed() != null`
                        // half is what leaves the GM shop's `price="0"` lines
                        // free, and it reads the block parsed *so far*, so it
                        // is document order that decides, like Java's.
                        let sell_price = template.sell_price();
                        let mut price = declared;
                        if !list.npcs.is_empty() && declared > -1 && sell_price > declared {
                            warn!(
                                "BuyListData: buy price {declared} is less than sell price \
                                 {sell_price} for ItemID:{item_id} of buylist {list_id}."
                            );
                            price = sell_price;
                        }
                        // `Product`'s constructor, not the parser: an
                        // undeclared price is the item's reference price.
                        if price < 0 {
                            price = template.price;
                        }
                        let base_tax = attr(b"baseTax")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(default_base_tax);
                        list.products.push(Product {
                            item_id,
                            price,
                            base_tax,
                            max_count: attr(b"count").and_then(|v| v.parse().ok()).unwrap_or(-1),
                            // `restockDelay * 60000` — the attribute is minutes.
                            restock_delay_ms: attr(b"restock_delay")
                                .and_then(|v| v.parse::<i64>().ok())
                                .unwrap_or(-1)
                                * 60_000,
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
    use crate::data::dist;

    #[test]
    fn loads_real_dist_files() {
        let root = crate::data::DIST_GAME;
        let items = dist::items();
        let data = BuyListData::load_from(root, items);
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

        // CorrectPrices floors an npc-served list at the sell value…
        for list in data.by_id.values().filter(|l| !l.npcs.is_empty()) {
            for p in &list.products {
                let Some(t) = items.get(p.item_id) else {
                    continue;
                };
                assert!(
                    p.price >= t.sell_price(),
                    "buylist {} item {}",
                    list.list_id,
                    p.item_id
                );
            }
        }
        // …and leaves the GM shop alone, which is the only reason `//buy`
        // hands out free gear. 9964 is the weapon shop; every line is 0.
        let gm_weapons = data.get(9964).expect("GM weapon buylist 9964");
        assert!(gm_weapons.npcs.is_empty());
        assert_eq!(gm_weapons.product(893).expect("line 893").price, 0);
        assert!(
            items.get(893).expect("item 893").sell_price() > 0,
            "the floor would have been non-zero had it applied"
        );

        // No price attribute means the item's *reference* price, not "no
        // sale" — Java resolves it in `Product`'s constructor. 3079 of the
        // npc-served lines rely on this.
        let cooper = data.get(3082900).expect("buylist 3082900");
        let canine = cooper.product(2505).expect("Iron Canine line");
        assert_eq!(canine.price, items.get(2505).expect("item 2505").price);
        assert!(canine.price > 0);
        for list in data.by_id.values() {
            for p in &list.products {
                assert!(p.price >= 0, "buylist {} item {}", list.list_id, p.item_id);
            }
        }

        // Limited stock: 1928 lines across 147 files, every one with a
        // restock delay, and the minutes→ms conversion applied.
        let mut limited = 0;
        let mut limited_files = std::collections::HashSet::new();
        for list in data.by_id.values() {
            for p in &list.products {
                if p.has_limited_stock() {
                    limited += 1;
                    limited_files.insert(list.list_id);
                    assert!(
                        p.restock_delay_ms > 0,
                        "buylist {} item {} has stock but no restock delay",
                        list.list_id,
                        p.item_id
                    );
                }
            }
        }
        assert_eq!(limited, 1928, "limited-stock product lines");
        assert_eq!(limited_files.len(), 147, "files declaring limited stock");
        let hall = data.get(3538400).expect("buylist 3538400");
        let soe = hall.product(1829).expect("Scroll of Escape: Clan Hall");
        assert_eq!(soe.max_count, 5);
        assert_eq!(soe.restock_delay_ms, 60 * 60_000, "60 minutes, in ms");
        let shield = hall.product(6902).expect("Pledge Shield");
        assert!(!shield.has_limited_stock(), "no count attribute");
        assert_eq!(shield.max_count, -1);

        // `MaxEquipableItemGrade = S` drops the five S80 lines on the GM
        // armour list, and nothing else on this dist.
        let gm_armour = data.get(9917).expect("GM armour buylist 9917");
        for id in [10170, 16025, 16026, 21712, 22175] {
            assert!(
                gm_armour.product(id).is_none(),
                "S80 item {id} should be filtered out"
            );
        }
        for list in data.by_id.values() {
            for p in &list.products {
                let grade = items.get(p.item_id).expect("template").crystal_type.level();
                assert!(
                    grade <= CrystalType::S.level() || grade >= CrystalType::Event.level(),
                    "buylist {} item {} is above grade S",
                    list.list_id,
                    p.item_id
                );
            }
        }
    }
}
