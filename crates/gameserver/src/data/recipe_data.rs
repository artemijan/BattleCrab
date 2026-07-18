//! Port of `data/xml/RecipeData` + `model/RecipeList` (G15.7): the crafting
//! recipes from `dist/game/data/Recipes.xml`. The runtime flow (recipe book,
//! self-craft, manufacture stores) lives in `game_loop/crafting.rs`.
//!
//! `AltGameCreation = False` on this dist, so `altStatChange` (XP/SP/GIM) is
//! dead — only `statUse` HP/MP is kept. The Java `production`-block
//! max-equipable-grade filter (`MAX_EQUIPABLE_ITEM_GRADE`) is a no-op here
//! (`MaxEquipableItemGrade = S`, the top grade), so every recipe loads.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::{info, warn};

const RECIPES_FILE: &str = "data/Recipes.xml";

/// The rare ("masterwork") production of a recipe (`productionRare`).
#[derive(Debug, Clone, Copy)]
pub struct RareProduction {
    pub item_id: i32,
    pub count: i32,
    /// Chance (%) the rare item is produced instead of the normal one.
    pub rarity: i32,
}

/// Port of `model/RecipeList` — one crafting recipe. `id` is the recipe *list*
/// id used in the book/packets; `recipe_item_id` (Java `recipeId`) is the id of
/// the etc-item that teaches it.
#[derive(Debug, Clone)]
pub struct RecipeList {
    pub id: i32,
    /// `craftLevel` — the create-item skill level needed to use it.
    pub level: i32,
    pub recipe_item_id: i32,
    pub name: String,
    /// `successRate` (%), before `Stat.CRAFT_RATE` (identity in the ported set).
    pub success_rate: i32,
    pub item_id: i32,
    pub count: i32,
    pub rare: Option<RareProduction>,
    pub is_dwarven: bool,
    /// `(item_id, count)` materials consumed from the customer's inventory.
    pub ingredients: Vec<(i32, i64)>,
    /// `statUse` MP cost on the crafter (0 if none).
    pub mp_use: i32,
    /// `statUse` HP cost on the crafter (0 if none).
    pub hp_use: i32,
}

#[derive(Debug, Clone, Default)]
pub struct RecipeData {
    /// Keyed by recipe-list id (`_id`).
    recipes: HashMap<i32, RecipeList>,
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
    pub fn insert_for_test(&mut self, recipe: RecipeList) {
        self.by_item.insert(recipe.recipe_item_id, recipe.id);
        self.recipes.insert(recipe.id, recipe);
    }

    /// `getRecipeList(listId)`.
    pub fn get(&self, list_id: i32) -> Option<&RecipeList> {
        self.recipes.get(&list_id)
    }

    /// `getRecipeByItemId(itemId)` — the recipe an etc-item teaches.
    pub fn by_recipe_item_id(&self, item_id: i32) -> Option<&RecipeList> {
        self.by_item.get(&item_id).and_then(|id| self.recipes.get(id))
    }

    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }

    fn parse(&mut self, content: &str) {
        let mut reader = Reader::from_str(content);
        // The item currently being built plus its child lists.
        let mut cur: Option<RecipeList> = None;
        loop {
            let event = match reader.read_event() {
                Ok(e) => e,
                Err(_) => break,
            };
            match event {
                Event::Start(e) | Event::Empty(e) => {
                    let attr = |key: &[u8]| -> Option<String> {
                        e.attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == key)
                            .map(|a| String::from_utf8_lossy(&a.value).into_owned())
                    };
                    let num = |key: &[u8]| attr(key).and_then(|v| v.parse::<i64>().ok());
                    match e.name().as_ref() {
                        b"item" => {
                            let (Some(id), Some(recipe_item_id), Some(level), Some(success_rate)) = (
                                num(b"id"),
                                num(b"recipeId"),
                                num(b"craftLevel"),
                                num(b"successRate"),
                            ) else {
                                warn!("RecipeData: recipe item missing required attribute, skipping");
                                cur = None;
                                continue;
                            };
                            cur = Some(RecipeList {
                                id: id as i32,
                                level: level as i32,
                                recipe_item_id: recipe_item_id as i32,
                                name: attr(b"name").unwrap_or_default(),
                                success_rate: success_rate as i32,
                                item_id: 0,
                                count: 0,
                                rare: None,
                                is_dwarven: attr(b"type").as_deref() == Some("dwarven"),
                                ingredients: Vec::new(),
                                mp_use: 0,
                                hp_use: 0,
                            });
                        }
                        b"ingredient" => {
                            if let (Some(r), Some(id), Some(count)) =
                                (cur.as_mut(), num(b"id"), num(b"count"))
                            {
                                if count > 0 {
                                    r.ingredients.push((id as i32, count));
                                }
                            }
                        }
                        b"production" => {
                            if let (Some(r), Some(id), Some(count)) =
                                (cur.as_mut(), num(b"id"), num(b"count"))
                            {
                                r.item_id = id as i32;
                                r.count = count as i32;
                            }
                        }
                        b"productionRare" => {
                            if let (Some(r), Some(id), Some(count), Some(rarity)) =
                                (cur.as_mut(), num(b"id"), num(b"count"), num(b"rarity"))
                            {
                                r.rare = Some(RareProduction {
                                    item_id: id as i32,
                                    count: count as i32,
                                    rarity: rarity as i32,
                                });
                            }
                        }
                        b"statUse" => {
                            if let (Some(r), Some(name), Some(value)) =
                                (cur.as_mut(), attr(b"name"), num(b"value"))
                            {
                                match name.as_str() {
                                    "MP" => r.mp_use = value as i32,
                                    "HP" => r.hp_use = value as i32,
                                    _ => {}
                                }
                            }
                        }
                        // altStatChange (XP/SP/GIM) is AltGameCreation-only (False here) — ignored.
                        _ => {}
                    }
                }
                Event::End(e) if e.name().as_ref() == b"item" => {
                    if let Some(r) = cur.take() {
                        self.by_item.insert(r.recipe_item_id, r.id);
                        self.recipes.insert(r.id, r);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load() -> RecipeData {
        RecipeData::load_from(&format!("{}/../../dist/game/", env!("CARGO_MANIFEST_DIR")))
    }

    #[test]
    fn loads_real_dist_recipes() {
        let data = load();
        // 631 `<item>` entries in Recipes.xml.
        assert_eq!(data.len(), 631);

        // Recipe list 1 = Wooden Arrow, taught by item 1666, dwarven, 100%.
        let r = data.get(1).expect("recipe 1");
        assert_eq!(r.recipe_item_id, 1666);
        assert!(r.is_dwarven);
        assert_eq!(r.level, 1);
        assert_eq!(r.success_rate, 100);
        assert_eq!(r.item_id, 17); // Wooden Arrow
        assert_eq!(r.count, 500);
        assert_eq!(r.mp_use, 30);
        assert_eq!(r.hp_use, 0);
        // Stem x4, Iron Ore x2.
        assert_eq!(r.ingredients, vec![(1864, 4), (1869, 2)]);

        // Learning lookup resolves the teaching item to the list.
        assert_eq!(data.by_recipe_item_id(1666).map(|r| r.id), Some(1));
        assert!(data.by_recipe_item_id(999_999).is_none());
    }
}
