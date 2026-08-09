//! Port of `data/xml/MultisellData` — every `data/multisell/*.xml` exchange
//! list (plus `data/multisell/custom/*` when `CustomMultisellLoad=True`, which
//! it is on this dist: those are the `6000xx` community-board shop lists). The
//! file name is the list id.
//!
//! Scoped to what the live community-board shop exercises (adena/regular
//! ingredients → regular products, per-list ingredient/product multipliers).
//! Deliberately **not** ported (documented deviations, none reached by the
//! `-1`/CB lists on this dist):
//!
//! - `Config.CORRECT_PRICES` price-flooring and the `totalPrice`/chance sum
//!   warnings (load-time diagnostics only).
//! - `Config.MAX_EQUIPABLE_ITEM_GRADE` product filtering and the
//!   `EnchantItemGroup` weapon/armor grade clamp on enchanted products.
//! - `SpecialItemType` ingredients/products (negative client ids) — parsed as
//!   valid but refused at exchange time (`MultiSellChoose`), like Java's
//!   "non-implemented special item" branch.

use std::collections::{HashMap, HashSet};

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::{info, warn};

use super::item_data::ItemData;
use crate::data::xml::attr_str;

pub const MULTISELL_DIR: &str = "data/multisell";
/// `MultisellData.PAGE_SIZE` — the client shows this many entries per page and
/// `MultiSellList` is sent once per page.
pub const PAGE_SIZE: usize = 40;

/// One `<ingredient>` of a `<item>` entry.
#[derive(Debug, Clone)]
pub struct Ingredient {
    pub id: i32,
    pub count: i64,
    /// `enchantmentLevel` — the ingredient must be at least this enchanted.
    pub enchant_level: i16,
    /// `maintainIngredient` — kept (not consumed) on exchange.
    pub maintain: bool,
}

/// One `<production>` of a `<item>` entry.
#[derive(Debug, Clone)]
pub struct Product {
    pub id: i32,
    pub count: i64,
    /// `chance` — `None` when unset (display-only; a chance multisell rolls
    /// among the products that declare one).
    pub chance: Option<f64>,
    pub enchant_level: i16,
}

/// A `<item>` entry: what you give (`ingredients`) for what you get
/// (`products`).
#[derive(Debug, Clone)]
pub struct MultisellEntry {
    pub ingredients: Vec<Ingredient>,
    pub products: Vec<Product>,
    /// `MultisellEntryHolder._stackable`: true only when every product is a
    /// stackable template (drives whether `amount > 1` is allowed).
    pub stackable: bool,
}

/// A static multisell list (`MultisellListHolder`).
#[derive(Debug, Clone)]
pub struct MultisellList {
    pub list_id: i32,
    pub is_chance_multisell: bool,
    pub apply_taxes: bool,
    pub maintain_enchantment: bool,
    pub ingredient_multiplier: f64,
    pub product_multiplier: f64,
    pub entries: Vec<MultisellEntry>,
    /// `_npcsAllowed`: `None` = no `<npcs>` tag = every NPC allowed and the
    /// list is not npc-only; `Some(set)` = only those ids, and the list is
    /// npc-only. The CB lists carry the sentinel `-1`.
    pub npcs_allowed: Option<HashSet<i32>>,
}

impl MultisellList {
    /// `MultisellListHolder.isNpcAllowed`.
    pub fn is_npc_allowed(&self, npc_id: i32) -> bool {
        self.npcs_allowed
            .as_ref()
            .is_none_or(|s| s.contains(&npc_id))
    }

    /// `MultisellListHolder.isNpcOnly` — true when a `<npcs>` allow-list exists.
    pub fn is_npc_only(&self) -> bool {
        self.npcs_allowed.is_some()
    }

    /// `PreparedMultisellListHolder.getIngredientCount` for the no-npc/no-tax
    /// community-board case (tax rate 0): apply the ingredient multiplier.
    pub fn ingredient_count(&self, ing: &Ingredient) -> i64 {
        self.ingredient_count_taxed(ing, 0.0)
    }

    /// `PreparedMultisellListHolder.getIngredientCount` — the castle's buy tax
    /// rides on the **adena** ingredient only, and only for a list that declares
    /// `applyTaxes` (Java's `getTaxRate()` returns 0 otherwise, whatever the
    /// NPC's castle charges).
    pub fn ingredient_count_taxed(&self, ing: &Ingredient, tax_rate: f64) -> i64 {
        let tax = if self.apply_taxes { tax_rate } else { 0.0 };
        if ing.id == super::item_data::ADENA_ID {
            (ing.count as f64 * self.ingredient_multiplier * (1.0 + tax)).round() as i64
        } else {
            (ing.count as f64 * self.ingredient_multiplier).round() as i64
        }
    }

    /// `PreparedMultisellListHolder.getProductCount`.
    pub fn product_count(&self, prod: &Product) -> i64 {
        (prod.count as f64 * self.product_multiplier).round() as i64
    }
}

#[derive(Default)]
pub struct MultisellData {
    by_id: HashMap<i32, MultisellList>,
}

impl MultisellData {
    pub fn load_from(file_path: &str, items: &ItemData) -> Self {
        let mut by_id = HashMap::new();
        // Retail lists first, then the custom overlay (`CustomMultisellLoad`).
        for sub in ["", "/custom"] {
            let dir = format!("{file_path}{MULTISELL_DIR}{sub}");
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
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
                    continue;
                };
                if let Some(list) = parse_file(&path, list_id, items) {
                    by_id.insert(list_id, list);
                }
            }
        }
        info!("MultisellData: Loaded {} multisell lists.", by_id.len());
        Self { by_id }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Test hook.
    pub fn insert_for_test(&mut self, list: MultisellList) {
        self.by_id.insert(list.list_id, list);
    }

    pub fn get(&self, list_id: i32) -> Option<&MultisellList> {
        self.by_id.get(&list_id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// `MultisellData.itemExists`: negative client ids are special items (accepted
/// here, refused at exchange); otherwise the template must exist and the count
/// must fit stackability.
fn item_exists(id: i32, count: i64, items: &ItemData) -> bool {
    if id < 0 {
        return true;
    }
    match items.get(id) {
        Some(t) if t.is_stackable => count >= 1,
        Some(_) => count == 1,
        None => false,
    }
}

fn parse_file(path: &std::path::Path, list_id: i32, items: &ItemData) -> Option<MultisellList> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut reader = Reader::from_str(&content);

    let mut list = MultisellList {
        list_id,
        is_chance_multisell: false,
        apply_taxes: false,
        maintain_enchantment: false,
        ingredient_multiplier: 1.0,
        product_multiplier: 1.0,
        entries: Vec::new(),
        npcs_allowed: None,
    };
    let mut cur: Option<MultisellEntry> = None;
    let mut npcs: Option<HashSet<i32>> = None;
    let mut in_npc = false;

    while let Ok(event) = reader.read_event() {
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let attr = |key: &[u8]| attr_str(&e, key);
                match e.name().as_ref() {
                    b"list" => {
                        let b = |k: &[u8]| {
                            attr(k)
                                .map(|v| v.eq_ignore_ascii_case("true"))
                                .unwrap_or(false)
                        };
                        list.is_chance_multisell = b(b"isChanceMultisell");
                        list.apply_taxes = b(b"applyTaxes");
                        list.maintain_enchantment = b(b"maintainEnchantment");
                        list.ingredient_multiplier = attr(b"ingredientMultiplier")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(1.0);
                        list.product_multiplier = attr(b"productMultiplier")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(1.0);
                    }
                    b"npcs" => npcs = Some(HashSet::new()),
                    b"npc" => in_npc = true,
                    b"item" => {
                        cur = Some(MultisellEntry {
                            ingredients: Vec::new(),
                            products: Vec::new(),
                            stackable: true,
                        })
                    }
                    b"ingredient" => {
                        if let Some(entry) = cur.as_mut() {
                            let Some(id) = attr(b"id").and_then(|v| v.parse::<i32>().ok()) else {
                                continue;
                            };
                            let count = attr(b"count")
                                .and_then(|v| v.parse::<i64>().ok())
                                .unwrap_or(0);
                            let enchant_level = attr(b"enchantmentLevel")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0);
                            let maintain = attr(b"maintainIngredient")
                                .map(|v| v.eq_ignore_ascii_case("true"))
                                .unwrap_or(false);
                            if item_exists(id, count, items) {
                                entry.ingredients.push(Ingredient {
                                    id,
                                    count,
                                    enchant_level,
                                    maintain,
                                });
                            } else {
                                warn!(
                                    "MultisellData: invalid ingredient id {id} count {count} in list {list_id}"
                                );
                            }
                        }
                    }
                    b"production" => {
                        if let Some(entry) = cur.as_mut() {
                            let Some(id) = attr(b"id").and_then(|v| v.parse::<i32>().ok()) else {
                                continue;
                            };
                            let count = attr(b"count")
                                .and_then(|v| v.parse::<i64>().ok())
                                .unwrap_or(0);
                            let chance = attr(b"chance").and_then(|v| v.parse::<f64>().ok());
                            let enchant_level = attr(b"enchantmentLevel")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0);
                            if let Some(c) = chance
                                && !(0.0..=100.0).contains(&c)
                            {
                                warn!(
                                    "MultisellData: invalid chance {c} for item {id} in list {list_id}"
                                );
                                continue;
                            }
                            if item_exists(id, count, items) {
                                // A product is non-stackable if its template is
                                // missing or non-stackable (Java `MultisellEntryHolder`).
                                if id < 0 || !items.get(id).is_some_and(|t| t.is_stackable) {
                                    entry.stackable = false;
                                }
                                entry.products.push(Product {
                                    id,
                                    count,
                                    chance,
                                    enchant_level,
                                });
                            } else {
                                warn!(
                                    "MultisellData: invalid product id {id} count {count} in list {list_id}"
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(t) if in_npc => {
                if let (Some(set), Ok(id)) = (
                    npcs.as_mut(),
                    String::from_utf8_lossy(&t.into_inner())
                        .trim()
                        .parse::<i32>(),
                ) {
                    set.insert(id);
                }
            }
            Event::End(e) => match e.name().as_ref() {
                b"npc" => in_npc = false,
                b"item" => {
                    if let Some(entry) = cur.take() {
                        list.entries.push(entry);
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    list.npcs_allowed = npcs;
    Some(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist() -> String {
        crate::data::DIST_GAME.to_string()
    }

    #[test]
    fn loads_real_dist_lists() {
        let root = dist();
        let items = ItemData::load_from(&root);
        let data = MultisellData::load_from(&root, &items);
        assert!(data.len() > 100, "loaded {} lists", data.len());

        // 600026.xml (custom CB belt shop): 4 adena→belt entries, npc -1.
        let belts = data.get(600026).expect("cb belt list 600026");
        assert_eq!(belts.entries.len(), 4);
        assert!(belts.is_npc_only(), "the <npcs> tag makes it npc-only");
        assert!(belts.is_npc_allowed(-1), "the CB sentinel -1 is allowed");
        assert!(!belts.is_npc_allowed(30001), "an arbitrary merchant is not");
        let first = &belts.entries[0];
        assert_eq!(first.ingredients.len(), 1);
        assert_eq!(first.ingredients[0].id, 57, "adena");
        assert_eq!(first.ingredients[0].count, 50_000_000);
        assert_eq!(first.products.len(), 1);
        assert_eq!(first.products[0].id, 13894, "Cloth Belt");
        assert_eq!(first.products[0].count, 1);
    }

    #[test]
    fn multipliers_default_to_one() {
        let root = dist();
        let items = ItemData::load_from(&root);
        let data = MultisellData::load_from(&root, &items);
        let list = data.get(600026).expect("list");
        assert_eq!(list.ingredient_multiplier, 1.0);
        assert_eq!(list.product_multiplier, 1.0);
        let ing = &list.entries[0].ingredients[0];
        assert_eq!(list.ingredient_count(ing), 50_000_000);
    }
}
