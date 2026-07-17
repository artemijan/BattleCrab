//! Recipe / crafting packets. Ported from `serverpackets/RecipeBookItemList`.
//!
//! Recipe books aren't implemented yet (a fresh char owns no recipes), so the
//! builder always reports an empty book — enough for the client's "Common
//! Craft" / "Dwarven Craft" action to open the (empty) recipe window instead of
//! doing nothing. TODO(crafting): feed the player's real recipe lists.

use commons::network::PacketWriter;

use super::opcodes;

/// Port of `serverpackets/RecipeBookItemList` — the recipe window contents for
/// one craft type. `is_dwarven` selects the book (Java `_isDwarvenCraft`); the
/// wire flag is its negation (`0` = Dwarven, `1` = Common). `recipes` are
/// `(recipe_id)` entries in book order; empty until crafting is ported.
pub fn recipe_book_item_list(is_dwarven: bool, max_mp: i32, recipes: &[i32]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::RECIPE_BOOK_ITEM_LIST);
    w.write_i32((!is_dwarven) as i32); // 0 = Dwarven, 1 = Common
    w.write_i32(max_mp);
    w.write_i32(recipes.len() as i32);
    for (i, &recipe_id) in recipes.iter().enumerate() {
        w.write_i32(recipe_id);
        w.write_i32(i as i32 + 1); // 1-based slot index (Java `count++`)
    }
    w.into_bytes()
}
