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
//! - the `totalChance > 100` warning (a load-time diagnostic only).
//! - `Config.MAX_EQUIPABLE_ITEM_GRADE` product filtering and the
//!   `EnchantItemGroup` weapon/armor grade clamp on enchanted products.
//! - `SpecialItemType` ingredients/products (negative client ids) — parsed as
//!   valid but refused at exchange time (`MultiSellChoose`), like Java's
//!   "non-implemented special item" branch.

use std::collections::{HashMap, HashSet};

use quick_xml::events::Event;
use tracing::{info, warn};

use super::item_data::ItemData;
use crate::data::xml;
use crate::data::xml::attr_str;

pub const MULTISELL_DIR: &str = "data/multisell";
/// `MultisellData.PAGE_SIZE` — the client shows this many entries per page and
/// `MultiSellList` is sent once per page.
pub const PAGE_SIZE: usize = 40;

/// One `<ingredient>` of a `<item>` entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Ingredient {
    pub id: i32,
    pub count: i64,
    /// `enchantmentLevel` — the ingredient must be at least this enchanted.
    pub enchant_level: i16,
    /// `maintainIngredient` — kept (not consumed) on exchange.
    pub maintain: bool,
}

/// One `<production>` of a `<item>` entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MultisellEntry {
    pub ingredients: Vec<Ingredient>,
    pub products: Vec<Product>,
    /// `MultisellEntryHolder._stackable`: true only when every product is a
    /// stackable template (drives whether `amount > 1` is allowed).
    pub stackable: bool,
}

/// A static multisell list (`MultisellListHolder`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct MultisellData {
    by_id: HashMap<i32, MultisellList>,
}

impl MultisellData {
    pub fn load_from(
        file_path: &str,
        items: &ItemData,
        custom: bool,
        correct_prices: bool,
    ) -> Self {
        let mut by_id = HashMap::new();
        // Retail lists first, then the custom overlay (`CustomMultisellLoad`,
        // **True** on this dist — these are the `6000xx` community-board shop
        // lists, so turning it off empties the CB shop).
        let subs: &[&str] = if custom { &["", "/custom"] } else { &[""] };
        for sub in subs {
            let dir = format!("{file_path}{MULTISELL_DIR}{sub}");
            for path in crate::data::xml::xml_files_in(&dir) {
                let Some(list_id) = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.parse::<i32>().ok())
                else {
                    continue;
                };
                if let Some(list) = parse_file(&path, list_id, items, correct_prices) {
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

fn parse_file(
    path: &std::path::Path,
    list_id: i32,
    items: &ItemData,
    correct_prices: bool,
) -> Option<MultisellList> {
    let content = std::fs::read_to_string(path).ok()?;

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

    for event in xml::events(&content) {
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
                    if let Some(mut entry) = cur.take() {
                        if correct_prices {
                            correct_entry_price(&mut entry, items, list_id);
                        }
                        list.entries.push(entry);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    list.npcs_allowed = npcs;
    Some(list)
}

/// `MultisellData`'s `Config.CORRECT_PRICES` block — the other half of the key
/// `BuyListData` reads.
///
/// Java raises the **cost** rather than lowering the reward: an entry bought
/// with a single adena ingredient may not cost less than the total sell value
/// of what it hands out, so the ingredient count is replaced with that total.
/// A chance-weighted product counts for its share (`sellValue * chance / 100`),
/// which is what makes a chance list priced on expectation rather than on the
/// jackpot.
///
/// **It corrects nothing on the shipped datapack**: of the 4971 single-
/// ingredient entries across `data/multisell`, none is underpriced. Ported
/// anyway because the key is one key — an operator who edits a list, or flips
/// `CorrectPrices` off, should get Java's behaviour in both loaders rather than
/// in one. Re-derive with the command in `docs/PORTING_STATUS.md`.
fn correct_entry_price(entry: &mut MultisellEntry, items: &ItemData, list_id: i32) {
    let [ingredient] = entry.ingredients.as_slice() else {
        return;
    };
    if ingredient.id != crate::data::item_data::ADENA_ID {
        return;
    }
    let total_price: i64 = entry
        .products
        .iter()
        .map(|p| {
            let sell_value = items.get(p.id).map(|t| t.sell_price()).unwrap_or(0) * p.count;
            // Java's `chance > 0` branch — a `chance="0"` product is priced in
            // full, like an unset one.
            match p.chance {
                Some(c) if c > 0.0 => (sell_value as f64 * (c / 100.0)) as i64,
                _ => sell_value,
            }
        })
        .sum();
    if ingredient.count >= total_price {
        return;
    }
    warn!(
        "MultisellData: buy price {} is less than sell price {total_price} in multisell {list_id}.",
        ingredient.count
    );
    // Java replaces the holder outright, keeping only `maintainIngredient` —
    // so the enchant level goes with it, which for adena is always 0 anyway.
    entry.ingredients = vec![Ingredient {
        id: crate::data::item_data::ADENA_ID,
        count: total_price,
        enchant_level: 0,
        maintain: ingredient.maintain,
    }];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::dist;

    fn dist() -> String {
        crate::data::DIST_GAME.to_string()
    }

    #[test]
    fn loads_real_dist_lists() {
        let root = dist();
        let items = dist::items();
        let data = MultisellData::load_from(&root, items, true, true);
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
        let items = dist::items();
        let data = MultisellData::load_from(&root, items, true, true);
        let list = data.get(600026).expect("list");
        assert_eq!(list.ingredient_multiplier, 1.0);
        assert_eq!(list.product_multiplier, 1.0);
        let ing = &list.entries[0].ingredients[0];
        assert_eq!(list.ingredient_count(ing), 50_000_000);
    }
}

#[cfg(test)]
mod correct_prices_tests {
    use super::*;
    use crate::data::dist;

    fn entry(adena: i64, products: &[(i32, i64, Option<f64>)]) -> MultisellEntry {
        MultisellEntry {
            ingredients: vec![Ingredient {
                id: crate::data::item_data::ADENA_ID,
                count: adena,
                enchant_level: 0,
                maintain: false,
            }],
            products: products
                .iter()
                .map(|&(id, count, chance)| Product {
                    id,
                    count,
                    chance,
                    enchant_level: 0,
                })
                .collect(),
            stackable: true,
        }
    }

    fn cost(e: &MultisellEntry) -> i64 {
        e.ingredients[0].count
    }

    /// The correction raises the **cost**, not the reward, and only for an
    /// entry bought with a single adena ingredient.
    #[test]
    fn an_underpriced_adena_entry_is_raised_to_what_it_hands_out() {
        let items = dist::items();
        // Item 1 (Short Sword) has a reference price, so a sell value.
        let sell = items.get(1).expect("item 1").sell_price();
        assert!(sell > 1, "the fixture item needs a sell value");

        let mut cheap = entry(1, &[(1, 1, None)]);
        correct_entry_price(&mut cheap, items, 0);
        assert_eq!(cost(&cheap), sell, "raised to the product's sell value");

        // Count multiplies, and a fair price is left alone.
        let mut two = entry(sell * 2, &[(1, 2, None)]);
        correct_entry_price(&mut two, items, 0);
        assert_eq!(cost(&two), sell * 2, "already fair");

        let mut cheap_two = entry(1, &[(1, 2, None)]);
        correct_entry_price(&mut cheap_two, items, 0);
        assert_eq!(cost(&cheap_two), sell * 2);
    }

    /// A chance product is priced on expectation — `sellValue * chance / 100`
    /// — which is what stops a 1 %-drop list costing the jackpot.
    #[test]
    fn a_chance_product_counts_for_its_share() {
        let items = dist::items();
        let sell = items.get(1).expect("item 1").sell_price();

        let mut half = entry(1, &[(1, 1, Some(50.0))]);
        correct_entry_price(&mut half, items, 0);
        assert_eq!(cost(&half), sell / 2);

        // `chance="0"` takes Java's *else* branch and counts in full.
        let mut zero = entry(1, &[(1, 1, Some(0.0))]);
        correct_entry_price(&mut zero, items, 0);
        assert_eq!(cost(&zero), sell);
    }

    /// Java's two guards: exactly one ingredient, and it must be adena.
    #[test]
    fn anything_but_a_lone_adena_ingredient_is_left_alone() {
        let items = dist::items();
        let sell = items.get(1).expect("item 1").sell_price();

        let mut two_ingredients = entry(1, &[(1, 1, None)]);
        two_ingredients.ingredients.push(Ingredient {
            id: 1,
            count: 1,
            enchant_level: 0,
            maintain: false,
        });
        correct_entry_price(&mut two_ingredients, items, 0);
        assert_eq!(cost(&two_ingredients), 1, "two ingredients: not priced");

        let mut barter = entry(1, &[(1, 1, None)]);
        barter.ingredients[0].id = 1; // paid in swords, not adena
        correct_entry_price(&mut barter, items, 0);
        assert_eq!(cost(&barter), 1, "not adena: not priced");
        assert!(sell > 1, "…and the price would otherwise have moved");
    }

    /// On the shipped datapack the key changes nothing — 4971 single-ingredient
    /// entries and not one underpriced — so this is the counterpart of the
    /// buy-list assertion: the mechanism is pinned above, the data here.
    #[test]
    fn the_shipped_lists_are_the_same_either_way() {
        let items = dist::items();
        let root = crate::data::DIST_GAME;
        let on = MultisellData::load_from(root, items, true, true);
        let off = MultisellData::load_from(root, items, true, false);
        let differing = on
            .by_id
            .iter()
            .flat_map(|(id, list)| {
                let other = off.by_id.get(id).expect("same lists");
                list.entries
                    .iter()
                    .zip(other.entries.iter())
                    .filter(|(a, b)| {
                        a.ingredients
                            .iter()
                            .map(|i| (i.id, i.count))
                            .ne(b.ingredients.iter().map(|i| (i.id, i.count)))
                    })
            })
            .count();
        assert_eq!(differing, 0);
    }
}
