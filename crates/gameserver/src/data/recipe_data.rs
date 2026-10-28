//! Port of `data/xml/RecipeData` + `data/holders/RecipeHolder`: the crafting
//! recipes from `dist/game/data/Recipes.xml`. The runtime flow (recipe book,
//! self-craft, manufacture stores) lives in `game_loop/commerce/crafting.rs`.
//!
//! Salvation's shape: `<recipe id name level itemId successRate isCommonRecipe
//! [maxOfferingBonus] [maxOffering]>` holding `<materials>`, a `<product>`
//! *group* (one item is picked by chance), the `<npcFee>` an NPC crafter
//! takes, and the `<statUse><stat name val/>` costs (`StatusUpdateType`).

use std::collections::HashMap;

use crate::data::xml;
use crate::data::xml::{attr_f64, attr_i32, attr_i64, attr_str};
use quick_xml::events::Event;
use tracing::{info, warn};

const RECIPES_FILE: &str = "data/Recipes.xml";

/// One `<product><item id count chance/>` (Java `ItemChanceHolder`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RecipeProduct {
    pub item_id: i32,
    pub count: i64,
    pub chance: f64,
}

/// Port of `RecipeHolder` — one crafting recipe. `id` is the recipe *list* id
/// used in the book/packets; `recipe_item_id` (Java `_itemId`) is the etc-item
/// that teaches it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecipeHolder {
    pub id: i32,
    /// `level` — the create-item skill level needed to use it.
    pub level: i32,
    pub recipe_item_id: i32,
    pub name: String,
    /// `successRate` (%).
    pub success_rate: f64,
    /// `isCommonRecipe`; [`Self::is_dwarven`] is its negation.
    pub is_common_recipe: bool,
    /// `maxOfferingBonus`, defaulting to `max(0, 100 - successRate)`.
    pub max_offering_bonus: f64,
    /// `maxOffering` (0 = the recipe takes no offering).
    pub max_offering: i64,
    /// `<materials>` — `(item_id, count)` consumed from the customer.
    pub materials: Vec<(i32, i64)>,
    /// `<product>` — the group one result is drawn from.
    pub product_group: Vec<RecipeProduct>,
    /// `<npcFee>` — what an NPC crafter takes.
    pub npc_fee: Vec<(i32, i64)>,
    /// `<statUse>` — `(StatusUpdateType name, amount)` taken from the crafter.
    pub stat_use: Vec<(String, f64)>,
}

impl RecipeHolder {
    /// Java `isDwarvenRecipe()`.
    pub fn is_dwarven(&self) -> bool {
        !self.is_common_recipe
    }

    /// The `<statUse>` amount for `stat` (`CUR_MP`, `CUR_HP`, …), 0 if absent.
    pub fn stat_use(&self, stat: &str) -> f64 {
        self.stat_use
            .iter()
            .find(|(name, _)| name == stat)
            .map_or(0.0, |&(_, v)| v)
    }

    /// Java `getRandomProduct()` given its `Rnd.get(100)` draw: walk the group
    /// subtracting each chance until one outnumbers what is left. `None` when
    /// the chances never do.
    pub fn random_product(&self, roll: f64) -> Option<RecipeProduct> {
        let mut random = roll;
        for product in &self.product_group {
            if product.chance > random {
                return Some(*product);
            }
            random -= product.chance;
        }
        None
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RecipeData {
    /// Keyed by recipe-list id (`_id`).
    recipes: HashMap<i32, RecipeHolder>,
    /// `recipe_item_id` → list id, for the learning path (`getRecipeByItemId`).
    by_item: HashMap<i32, i32>,
}

impl RecipeData {
    pub fn load_from(file_path: &str) -> Self {
        let mut data = Self::default();
        let path = format!("{file_path}{RECIPES_FILE}");
        match std::fs::read_to_string(&path) {
            Ok(content) => data.parse(&content),
            Err(e) => warn!("RecipeData: cannot read {path}: {e}"),
        }
        info!("RecipeData: Loaded {} recipes.", data.recipes.len());
        data
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, recipe: RecipeHolder) {
        self.by_item.insert(recipe.recipe_item_id, recipe.id);
        self.recipes.insert(recipe.id, recipe);
    }

    /// `getRecipeList(listId)`.
    pub fn get(&self, list_id: i32) -> Option<&RecipeHolder> {
        self.recipes.get(&list_id)
    }

    /// `getRecipeByItemId(itemId)` — the recipe an etc-item teaches.
    pub fn by_recipe_item_id(&self, item_id: i32) -> Option<&RecipeHolder> {
        self.by_item
            .get(&item_id)
            .and_then(|id| self.recipes.get(id))
    }

    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }

    fn parse(&mut self, content: &str) {
        /// Which `<item>` list the open child element feeds.
        #[derive(Clone, Copy)]
        enum List {
            Materials,
            Product,
            NpcFee,
        }
        let mut cur: Option<RecipeHolder> = None;
        let mut list: Option<List> = None;
        for event in xml::events(content) {
            match event {
                Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                    "recipe" => {
                        let (Some(id), Some(level), Some(recipe_item_id), Some(success_rate)) = (
                            attr_i32(&e, "id"),
                            attr_i32(&e, "level"),
                            attr_i32(&e, "itemId"),
                            attr_f64(&e, "successRate"),
                        ) else {
                            warn!("RecipeData: recipe missing a required attribute, skipping");
                            cur = None;
                            continue;
                        };
                        cur = Some(RecipeHolder {
                            id,
                            level,
                            recipe_item_id,
                            name: attr_str(&e, "name").unwrap_or_default(),
                            success_rate,
                            is_common_recipe: attr_str(&e, "isCommonRecipe").as_deref()
                                == Some("true"),
                            max_offering_bonus: attr_f64(&e, "maxOfferingBonus")
                                .unwrap_or((100.0 - success_rate).max(0.0)),
                            max_offering: attr_i64(&e, "maxOffering").unwrap_or(0),
                            materials: Vec::new(),
                            product_group: Vec::new(),
                            npc_fee: Vec::new(),
                            stat_use: Vec::new(),
                        });
                    }
                    "materials" => list = Some(List::Materials),
                    "product" => list = Some(List::Product),
                    "npcFee" => list = Some(List::NpcFee),
                    "item" => {
                        let (Some(r), Some(which), Some(item_id), Some(count)) = (
                            cur.as_mut(),
                            list,
                            attr_i32(&e, "id"),
                            attr_i64(&e, "count"),
                        ) else {
                            continue;
                        };
                        match which {
                            List::Materials => r.materials.push((item_id, count)),
                            List::NpcFee => r.npc_fee.push((item_id, count)),
                            List::Product => r.product_group.push(RecipeProduct {
                                item_id,
                                count,
                                chance: attr_f64(&e, "chance").unwrap_or(100.0),
                            }),
                        }
                    }
                    "stat" => {
                        if let (Some(r), Some(name), Some(val)) =
                            (cur.as_mut(), attr_str(&e, "name"), attr_f64(&e, "val"))
                        {
                            r.stat_use.retain(|(n, _)| *n != name);
                            r.stat_use.push((name, val));
                        }
                    }
                    _ => {}
                },
                Event::End(e) => match e.name().as_ref() {
                    "materials" | "product" | "npcFee" => list = None,
                    "recipe" => {
                        if let Some(r) = cur.take() {
                            self.by_item.insert(r.recipe_item_id, r.id);
                            self.recipes.insert(r.id, r);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load() -> RecipeData {
        RecipeData::load_from(crate::data::DIST_GAME)
    }

    /// Recipe 1 in Salvation's `Recipes.xml`: `mk_wooden_arrow`, level 1,
    /// taught by 1666, 100%, `isCommonRecipe="false"` (dwarven); materials
    /// 1864 ×4 + 1869 ×2, product 17 ×500 at chance 100, npcFee 1666 ×1 + 57
    /// ×200, `CUR_MP` 30. 1771 `<recipe>` elements carry 1647 distinct ids —
    /// Java `put`s by id, so a repeated id keeps the last one.
    #[test]
    fn loads_real_dist_recipes() {
        let data = load();
        assert_eq!(data.len(), 1647);

        let r = data.get(1).expect("recipe 1");
        assert_eq!(r.name, "mk_wooden_arrow");
        assert_eq!(r.recipe_item_id, 1666);
        assert!(r.is_dwarven());
        assert_eq!(r.level, 1);
        assert_eq!(r.success_rate, 100.0);
        assert_eq!(r.max_offering_bonus, 0.0, "max(0, 100 - 100)");
        assert_eq!(r.max_offering, 0);
        assert_eq!(r.materials, vec![(1864, 4), (1869, 2)]);
        assert_eq!(
            r.product_group,
            vec![RecipeProduct {
                item_id: 17,
                count: 500,
                chance: 100.0
            }]
        );
        assert_eq!(r.npc_fee, vec![(1666, 1), (57, 200)]);
        assert_eq!(r.stat_use("CUR_MP"), 30.0);
        assert_eq!(r.stat_use("CUR_HP"), 0.0);

        assert_eq!(data.by_recipe_item_id(1666).map(|r| r.id), Some(1));
        assert!(data.by_recipe_item_id(999_999).is_none());
    }

    /// `getRandomProduct` subtracts each chance from the draw until a product
    /// outnumbers the remainder.
    #[test]
    fn random_product_walks_the_group_by_chance() {
        let p = |item_id, chance| RecipeProduct {
            item_id,
            count: 1,
            chance,
        };
        let r = RecipeHolder {
            id: 1,
            level: 1,
            recipe_item_id: 1,
            name: String::new(),
            success_rate: 100.0,
            is_common_recipe: false,
            max_offering_bonus: 0.0,
            max_offering: 0,
            materials: Vec::new(),
            product_group: vec![p(10, 60.0), p(11, 30.0)],
            npc_fee: Vec::new(),
            stat_use: Vec::new(),
        };
        assert_eq!(r.random_product(0.0).map(|p| p.item_id), Some(10));
        assert_eq!(r.random_product(59.0).map(|p| p.item_id), Some(10));
        assert_eq!(r.random_product(60.0).map(|p| p.item_id), Some(11));
        assert_eq!(r.random_product(95.0), None, "the group sums to 90");
    }
}
