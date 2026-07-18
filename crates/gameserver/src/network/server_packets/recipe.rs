//! Recipe / crafting packets (G15.7). Ports of `serverpackets/RecipeBookItemList`,
//! `RecipeItemMakeInfo`, `RecipeShopManageList`, `RecipeShopSellList`,
//! `RecipeShopItemInfo`, `RecipeShopMsg`. The runtime flow lives in
//! `game_loop/crafting.rs`.

use commons::network::PacketWriter;

use super::opcodes;

/// Port of `serverpackets/RecipeBookItemList` — the recipe window contents for
/// one craft type. `is_dwarven` selects the book (Java `_isDwarvenCraft`); the
/// wire flag is its negation (`0` = Dwarven, `1` = Common). `recipes` are
/// recipe-*list* ids in book order.
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

/// Port of `serverpackets/RecipeItemMakeInfo` — the self-craft ("make") window
/// state after opening it or after a craft attempt. `id` is the recipe-list id,
/// `is_dwarven` its book, `success` whether the last attempt succeeded (Java
/// defaults it to true when opening).
pub fn recipe_item_make_info(id: i32, is_dwarven: bool, cur_mp: i32, max_mp: i32, success: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::RECIPE_ITEM_MAKE_INFO);
    w.write_i32(id);
    w.write_i32((!is_dwarven) as i32); // 0 = Dwarven, 1 = Common
    w.write_i32(cur_mp);
    w.write_i32(max_mp);
    w.write_i32(success as i32);
    w.write_u8(0);
    w.write_i64(0);
    w.into_bytes()
}

/// Port of `serverpackets/RecipeShopManageList` — the manufacture-store setup
/// window: the seller's book recipes plus the recipes already in their store.
/// `recipes` are recipe-list ids (book order); `store` is `(recipe_list_id,
/// cost)` for the active manufacture list. Java always opens with `isDwarven`
/// true; the caller passes the book it resolved.
pub fn recipe_shop_manage_list(
    seller_oid: i32,
    adena: i32,
    is_dwarven: bool,
    recipes: &[i32],
    store: &[(i32, i64)],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::RECIPE_SHOP_MANAGE_LIST);
    w.write_i32(seller_oid);
    w.write_i32(adena);
    w.write_i32((!is_dwarven) as i32);
    w.write_i32(recipes.len() as i32);
    for (i, &recipe_id) in recipes.iter().enumerate() {
        w.write_i32(recipe_id);
        w.write_i32(i as i32 + 1);
    }
    w.write_i32(store.len() as i32);
    for &(recipe_id, cost) in store {
        w.write_i32(recipe_id);
        w.write_i32(0);
        w.write_i64(cost);
    }
    w.into_bytes()
}

/// Port of `serverpackets/RecipeShopSellList` — the manufacture list a buyer
/// sees on clicking the manufacturer. `store` is `(recipe_list_id, cost)`.
pub fn recipe_shop_sell_list(
    manufacturer_oid: i32,
    cur_mp: i32,
    max_mp: i32,
    buyer_adena: i64,
    store: &[(i32, i64)],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::RECIPE_SHOP_SELL_LIST);
    w.write_i32(manufacturer_oid);
    w.write_i32(cur_mp);
    w.write_i32(max_mp);
    w.write_i64(buyer_adena);
    w.write_i32(store.len() as i32);
    for &(recipe_id, cost) in store {
        w.write_i32(recipe_id);
        w.write_i32(0);
        w.write_i64(cost);
    }
    w.into_bytes()
}

/// Port of `serverpackets/RecipeShopItemInfo` — the per-recipe make state shown
/// to a buyer in a manufacturer's shop (their MP + the selected recipe).
pub fn recipe_shop_item_info(manufacturer_oid: i32, recipe_id: i32, cur_mp: i32, max_mp: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::RECIPE_SHOP_ITEM_INFO);
    w.write_i32(manufacturer_oid);
    w.write_i32(recipe_id);
    w.write_i32(cur_mp);
    w.write_i32(max_mp);
    w.write_i32(-1); // 0xffffffff — craft time (unused here)
    w.write_i64(0);
    w.write_u8(0); // offering window trigger
    w.write_i64(0);
    w.into_bytes()
}

/// Port of `serverpackets/RecipeShopMsg` — the manufacture store's title,
/// broadcast to nearby players (empty string clears it).
pub fn recipe_shop_msg(player_oid: i32, title: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::RECIPE_SHOP_MSG);
    w.write_i32(player_oid);
    w.write_string(title);
    w.into_bytes()
}
