//! Crafting (G15.7) — port of `instancemanager/RecipeManager` +
//! `handlers/itemhandlers/Recipes` + the `RequestRecipe*` packet family.
//!
//! `AltGameCreation = False` on this dist, so the whole staged multi-pass craft
//! (the `_activeMakers` scheduler, `MagicSkillUse` animation loop, `SetupGauge`,
//! crafting XP/SP, and the HP/MP rest-wait branch) is dead — `maker.run()` runs
//! `finishCrafting()` inline. This module is therefore the *synchronous*
//! `RecipeItemMaker`: check materials + MP/HP + adena, consume them, roll
//! success, reward or send the failure message. `StoreRecipeShopList = False`,
//! so manufacture stores are transient (no persistence).
//!
//! Deferred (identity in the ported set): `Stat.CRAFT_RATE` (success bonus),
//! `Stat.CRAFTING_CRITICAL` (double-output crit) — no item/skill grants either,
//! so they stay `+0`; `Stat.RECIPE_DWARVEN/COMMON` (recipe-limit modifiers).

use crate::data::recipe_data::RecipeList;
use crate::model::components::{ManufactureStore, RecipeBook, SkillBook, Vitals};
use crate::model::inventory::Inventory;
use crate::network::client_packets as cp;
use crate::network::enter_world as ew;
use crate::network::server_packets::{self as sp, sm_ids, status_update_type, SmParam};
use crate::session::ClientSession;
use crate::world::World;

const ADENA_ID: i32 = 57;
/// `CommonSkill.CREATE_DWARVEN` (172) / `CREATE_COMMON` (1320) — the craft
/// ability skills; the player's level in them is their `getDwarvenCraft` /
/// `getCommonCraft`.
const SKILL_CREATE_DWARVEN: i32 = 172;
const SKILL_CREATE_COMMON: i32 = 1320;
/// `PrivateStoreType.MANUFACTURE` (the CharInfo/UserInfo store byte).
const STORE_TYPE_MANUFACTURE: u8 = 5;

// --- small accessors -------------------------------------------------------

fn player_of(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}

fn adena(world: &World, oid: i32) -> i64 {
    world.objects.get_component::<Inventory>(&oid).map(|i| i.adena()).unwrap_or(0)
}

/// The player's level in the relevant create-item skill (0 if not known) — Java
/// `getDwarvenCraft` / `getCommonCraft`.
fn craft_skill_level(world: &World, oid: i32, is_dwarven: bool) -> i32 {
    let skill_id = if is_dwarven { SKILL_CREATE_DWARVEN } else { SKILL_CREATE_COMMON };
    world.objects.get_component::<SkillBook>(&oid).and_then(|b| b.0.get(&skill_id).copied()).unwrap_or(0)
}

fn store_type(world: &World, oid: i32) -> u8 {
    world.objects.get_component::<crate::model::Player>(&oid).map(|p| p.store_type).unwrap_or(0)
}

fn set_store_type(world: &mut World, oid: i32, ty: u8) {
    if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&oid) {
        p.store_type = ty;
    }
}

fn send_to_client(world: &World, client_id: u32, packet: Vec<u8>) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

fn send_to_player(world: &World, oid: i32, packet: Vec<u8>) {
    if let Some(cid) = super::helpers::client_for_player(world, oid) {
        send_to_client(world, cid, packet);
    }
}

fn vitals(world: &World, oid: i32) -> Option<(f64, i32, f64)> {
    world.objects.get_component::<Vitals>(&oid).map(|v| (v.cur_mp, v.max_mp, v.cur_hp))
}

// --- recipe book -----------------------------------------------------------

/// `RecipeManager.requestBookOpen` — send the recipe window for one craft type.
/// (The Java "engaged in manufacturing" guard is moot: crafts finish inline, so
/// no maker is ever active when this arrives.)
pub(crate) fn request_book_open(world: &mut World, client_id: u32, is_dwarven: bool) {
    let Some(oid) = player_of(world, client_id) else { return };
    let max_mp = world.objects.get_component::<Vitals>(&oid).map(|v| v.max_mp).unwrap_or(0);
    let recipes = book_ids(world, oid, is_dwarven);
    send_to_client(world, client_id, sp::recipe_book_item_list(is_dwarven, max_mp, &recipes));
}

/// `RequestRecipeBookDestroy` — drop a recipe from whichever book holds it, then
/// resend that book.
pub(crate) fn handle_book_destroy(world: &mut World, client_id: u32, recipe_id: i32) {
    let Some(oid) = player_of(world, client_id) else { return };
    // Java looks the recipe up in RecipeData for its dwarven flag; do the same so
    // we resend the correct book even after removing the id.
    let Some(is_dwarven) = world.data.recipes.get(recipe_id).map(|r| r.is_dwarven) else { return };
    if let Some(book) = world.objects.get_component_mut::<RecipeBook>(&oid) {
        book.dwarven.retain(|&id| id != recipe_id);
        book.common.retain(|&id| id != recipe_id);
    }
    let max_mp = world.objects.get_component::<Vitals>(&oid).map(|v| v.max_mp).unwrap_or(0);
    let recipes = book_ids(world, oid, is_dwarven);
    send_to_client(world, client_id, sp::recipe_book_item_list(is_dwarven, max_mp, &recipes));
}

fn book_ids(world: &World, oid: i32, is_dwarven: bool) -> Vec<i32> {
    world
        .objects
        .get_component::<RecipeBook>(&oid)
        .map(|b| if is_dwarven { b.dwarven.clone() } else { b.common.clone() })
        .unwrap_or_default()
}

// --- recipe learning (item handler `Recipes`) ------------------------------

/// Port of `handlers/itemhandlers/Recipes.useItem`: register the recipe the
/// used item teaches (craft-ability / level / limit / duplicate gates), consume
/// the item on success.
pub(crate) fn learn_recipe(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    let send = |world: &World, msg: i16, params: &[SmParam]| {
        send_to_client(world, client_id, sp::system_message_with(msg, params));
    };

    if !world.cfg.character.crafting_enabled {
        return;
    }

    let item_id = {
        let Some(inv) = world.objects.get_component::<Inventory>(&object_id) else { return };
        let Some(item) = inv.items().iter().find(|i| i.object_id == item_object_id) else { return };
        item.item_id
    };
    let Some(recipe) = world.data.recipes.by_recipe_item_id(item_id).cloned() else { return };

    // Already registered?
    if world.objects.get_component::<RecipeBook>(&object_id).is_some_and(|b| b.contains(recipe.id)) {
        send(world, sm_ids::THAT_RECIPE_IS_ALREADY_REGISTERED, &[]);
        return;
    }

    let craft_level = craft_skill_level(world, object_id, recipe.is_dwarven);
    let (limit, book_len) = {
        let book = world.objects.get_component::<RecipeBook>(&object_id);
        if recipe.is_dwarven {
            (world.cfg.character.dwarf_recipe_limit, book.map(|b| b.dwarven.len()).unwrap_or(0))
        } else {
            (world.cfg.character.common_recipe_limit, book.map(|b| b.common.len()).unwrap_or(0))
        }
    };

    // `hasDwarvenCraft` / `hasCommonCraft`: the create-item skill (level ≥ 1).
    if craft_level < 1 {
        send(world, sm_ids::THE_RECIPE_CANNOT_BE_REGISTERED_YOU_DO_NOT_HAVE_THE_ABILITY_TO_CREATE_ITEMS, &[]);
        return;
    }
    if recipe.level > craft_level {
        send(world, sm_ids::YOUR_CREATE_ITEM_LEVEL_IS_TOO_LOW_TO_REGISTER_THIS_RECIPE, &[]);
        return;
    }
    if book_len as i32 >= limit {
        send(world, sm_ids::UP_TO_S1_RECIPES_CAN_BE_REGISTERED, &[SmParam::Int(limit)]);
        return;
    }

    // Register (memory-first; persists on the next flush).
    if let Some(book) = world.objects.get_component_mut::<RecipeBook>(&object_id) {
        if recipe.is_dwarven {
            book.dwarven.push(recipe.id);
        } else {
            book.common.push(recipe.id);
        }
    } else {
        let mut book = RecipeBook::default();
        if recipe.is_dwarven {
            book.dwarven.push(recipe.id);
        } else {
            book.common.push(recipe.id);
        }
        world.objects.add_components(&object_id, book);
    }

    // Consume the recipe item + notify.
    if let Some(destroyed) = world.objects.get_component_mut::<Inventory>(&object_id).and_then(|inv| inv.remove_by_object_id(item_object_id, 1)) {
        send_to_client(world, client_id, ew::inventory_update_changes(&world.data, std::slice::from_ref(&destroyed)));
    }
    send(world, sm_ids::S1_HAS_BEEN_ADDED, &[SmParam::ItemName(item_id)]);
}

// --- self-craft ------------------------------------------------------------

/// `RequestRecipeItemMakeInfo` — (re)open the self-craft "make" window.
pub(crate) fn handle_make_info(world: &mut World, client_id: u32, id: i32) {
    let Some(oid) = player_of(world, client_id) else { return };
    let Some(is_dwarven) = world.data.recipes.get(id).map(|r| r.is_dwarven) else { return };
    let (cur_mp, max_mp) = world.objects.get_component::<Vitals>(&oid).map(|v| (v.cur_mp as i32, v.max_mp)).unwrap_or((0, 0));
    send_to_client(world, client_id, sp::recipe_item_make_info(id, is_dwarven, cur_mp, max_mp, true));
}

/// `RequestRecipeItemMakeSelf` — craft one of the player's own recipes.
pub(crate) fn handle_make_self(world: &mut World, client_id: u32, id: i32) {
    let Some(oid) = player_of(world, client_id) else { return };
    // Java: refuse while running a store or mid-craft. Crafts finish inline, so
    // only the store guard is observable here.
    if store_type(world, oid) != 0 {
        return;
    }
    do_craft(world, oid, oid, client_id, id, 0);
}

// --- manufacture store -----------------------------------------------------

/// `RequestRecipeShopManageList` — open the manufacture-store setup window.
/// Java always passes `isDwarven = true`; the packet builder falls back to the
/// common book when the seller has no dwarven craft.
pub(crate) fn open_manage(world: &mut World, client_id: u32) {
    let Some(oid) = player_of(world, client_id) else { return };
    if world.objects.get_component::<Vitals>(&oid).is_some_and(|v| v.dead) {
        return;
    }
    // Leaving a different store type when opening the manage window.
    if store_type(world, oid) != 0 {
        set_store_type(world, oid, 0);
        super::party::broadcast_user_info(world, oid);
    }
    // `_isDwarven && hasDwarvenCraft()` selects the book.
    let is_dwarven = craft_skill_level(world, oid, true) >= 1;
    let recipes = book_ids(world, oid, is_dwarven);
    let store = active_store_items(world, oid, is_dwarven);
    send_to_client(world, client_id, sp::recipe_shop_manage_list(oid, adena(world, oid) as i32, is_dwarven, &recipes, &store));
}

/// `RequestRecipeShopMessageSet` — set the store title (Java `setStoreName`,
/// stored on the player across store types; here on the `ManufactureStore`).
pub(crate) fn handle_message_set(world: &mut World, client_id: u32, name: String) {
    let Some(oid) = player_of(world, client_id) else { return };
    const MAX_MSG_LENGTH: usize = 29;
    if name.chars().count() > MAX_MSG_LENGTH {
        return;
    }
    if let Some(store) = world.objects.get_component_mut::<ManufactureStore>(&oid) {
        store.title = name;
    } else {
        world.objects.add_components(&oid, ManufactureStore { items: Vec::new(), title: name });
    }
}

/// `RequestRecipeShopListSet` — activate the manufacture store with the given
/// recipe/price lines (each validated against the seller's book).
pub(crate) fn handle_list_set(world: &mut World, client_id: u32, lines: Vec<cp::ManufactureLine>) {
    let Some(oid) = player_of(world, client_id) else { return };
    // Combat guard (Java `hasAttackStanceTask || isInDuel`). No duels yet; the
    // attack-stance check maps to a live attack window.
    let in_combat = world
        .objects
        .get_component::<crate::model::components::AttackState>(&oid)
        .is_some_and(|st| st.attack_end_tick > world.tick);
    if in_combat {
        send_to_client(world, client_id, sp::system_message_with(sm_ids::WHILE_YOU_ARE_ENGAGED_IN_COMBAT_YOU_CANNOT_OPERATE_A_PRIVATE_STORE_OR_PRIVATE_WORKSHOP, &[]));
        return;
    }

    // Validate every recipe is in the seller's book; a bad id aborts the whole
    // set (Java `handleIllegalPlayerAction` + return).
    let mut items = Vec::with_capacity(lines.len());
    for line in &lines {
        let known = world.objects.get_component::<RecipeBook>(&oid).is_some_and(|b| b.contains(line.recipe_id));
        if !known {
            return;
        }
        items.push((line.recipe_id, line.cost));
    }

    if items.is_empty() {
        // Java `_items == null` path: close the store.
        set_store_type(world, oid, 0);
        super::party::broadcast_user_info(world, oid);
        return;
    }

    let title = {
        if let Some(store) = world.objects.get_component_mut::<ManufactureStore>(&oid) {
            store.items = items;
            store.title.clone()
        } else {
            world.objects.add_components(&oid, ManufactureStore { items, title: String::new() });
            String::new()
        }
    };
    set_store_type(world, oid, STORE_TYPE_MANUFACTURE);
    // TODO(G15.7): Java also `sitDown()`s here — sitting isn't modelled (the
    // private sell store skips it too); the store byte alone renders the shop.
    super::helpers::broadcast_including_self(world, oid, &sp::recipe_shop_msg(oid, &title));
    super::party::broadcast_user_info(world, oid);
}

/// `RequestRecipeShopManageQuit` — close the manufacture store.
pub(crate) fn handle_manage_quit(world: &mut World, client_id: u32) {
    let Some(oid) = player_of(world, client_id) else { return };
    set_store_type(world, oid, 0);
    super::helpers::broadcast_including_self(world, oid, &sp::recipe_shop_msg(oid, ""));
    super::party::broadcast_user_info(world, oid);
}

/// A customer clicked a manufacture-store owner (`Action`): show their list.
pub(crate) fn open_sell_list(world: &mut World, client_id: u32, buyer: i32, manufacturer: i32) {
    if store_type(world, manufacturer) != STORE_TYPE_MANUFACTURE {
        return;
    }
    let (cur_mp, max_mp) = world.objects.get_component::<Vitals>(&manufacturer).map(|v| (v.cur_mp as i32, v.max_mp)).unwrap_or((0, 0));
    let store = current_store_items(world, manufacturer);
    send_to_client(world, client_id, sp::recipe_shop_sell_list(manufacturer, cur_mp, max_mp, adena(world, buyer), &store));
}

/// `RequestRecipeShopMakeInfo` — the per-recipe info line in a shop.
pub(crate) fn handle_shop_make_info(world: &mut World, client_id: u32, shop_oid: i32, recipe_id: i32) {
    if store_type(world, shop_oid) != STORE_TYPE_MANUFACTURE {
        return;
    }
    let (cur_mp, max_mp) = world.objects.get_component::<Vitals>(&shop_oid).map(|v| (v.cur_mp as i32, v.max_mp)).unwrap_or((0, 0));
    send_to_client(world, client_id, sp::recipe_shop_item_info(shop_oid, recipe_id, cur_mp, max_mp));
}

/// `RequestRecipeShopMakeItem` — a customer buys a craft from a manufacturer.
pub(crate) fn handle_shop_make_item(world: &mut World, client_id: u32, manufacturer: i32, recipe_id: i32) {
    let Some(buyer) = player_of(world, client_id) else { return };
    if buyer == manufacturer || store_type(world, manufacturer) != STORE_TYPE_MANUFACTURE {
        return;
    }
    // `Util.checkIfInRange(150, player, manufacturer, true)`.
    if !in_range(world, buyer, manufacturer, 150) {
        return;
    }
    // The recipe must still be in the manufacturer's live store; the price is
    // whatever they set for it.
    let Some(price) = world
        .objects
        .get_component::<ManufactureStore>(&manufacturer)
        .and_then(|s| s.items.iter().find(|(id, _)| *id == recipe_id).map(|(_, cost)| *cost))
    else {
        return;
    };
    do_craft(world, manufacturer, buyer, client_id, recipe_id, price);
}

// --- the synchronous RecipeItemMaker --------------------------------------

/// The core `RecipeItemMaker` (synchronous, `!ALT_GAME_CREATION`): validate
/// everything, then consume + roll. `crafter` provides the recipe + MP/HP;
/// `customer` provides materials + adena and receives the product (they're the
/// same object for a self-craft). `customer_client` is the customer's socket.
fn do_craft(world: &mut World, crafter: i32, customer: i32, customer_client: u32, recipe_id: i32, price: i64) {
    let Some(recipe) = world.data.recipes.get(recipe_id).cloned() else { return };

    let abort = |world: &World| {
        // Java `abort()` → `updateMakeInfo(false)`: for a self-craft that's the
        // make-info failure; for manufacture it's the shop item-info refresh.
        update_make_info(world, crafter, customer, customer_client, recipe_id, recipe.is_dwarven, false);
    };
    let sm_customer = |world: &World, msg: i16, params: &[SmParam]| {
        send_to_client(world, customer_client, sp::system_message_with(msg, params));
    };

    // The crafter must actually hold this recipe (Java's book check).
    if !world.objects.get_component::<RecipeBook>(&crafter).is_some_and(|b| b.contains(recipe.id)) {
        return;
    }
    // Empty recipe / skill-level gate (crafter's create-item level).
    if recipe.ingredients.is_empty() || recipe.level > craft_skill_level(world, crafter, recipe.is_dwarven) {
        abort(world);
        return;
    }
    // Customer can afford the manufacture fee.
    if crafter != customer && price > 0 && adena(world, customer) < price {
        sm_customer(world, sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA, &[]);
        abort(world);
        return;
    }
    // Materials present on the customer (listItems check).
    for &(item_id, need) in &recipe.ingredients {
        let have = world.objects.get_component::<Inventory>(&customer).map(|i| i.count_of(item_id)).unwrap_or(0);
        if have < need {
            sm_customer(world, sm_ids::YOU_NEED_S2_MORE_S1_S, &[SmParam::ItemName(item_id), SmParam::Long(need - have)]);
            abort(world);
            return;
        }
    }
    // MP/HP present on the crafter (statUse check). HP uses `<=` (can't kill),
    // MP uses `<`, matching Java.
    let Some((cur_mp, _, cur_hp)) = vitals(world, crafter) else { return };
    if recipe.mp_use > 0 && cur_mp < recipe.mp_use as f64 {
        sm_customer(world, sm_ids::NOT_ENOUGH_MP, &[]);
        abort(world);
        return;
    }
    if recipe.hp_use > 0 && cur_hp <= recipe.hp_use as f64 {
        sm_customer(world, sm_ids::NOT_ENOUGH_HP, &[]);
        abort(world);
        return;
    }

    // --- all checks passed; now consume (finishCrafting) ---

    // Reduce the crafter's MP/HP + StatusUpdate.
    if recipe.mp_use > 0 || recipe.hp_use > 0 {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&crafter) {
            v.cur_mp = (v.cur_mp - recipe.mp_use as f64).max(0.0);
            v.cur_hp = (v.cur_hp - recipe.hp_use as f64).max(1.0);
        }
        send_crafter_mp(world, crafter);
    }

    // Pay the manufacture fee (customer → crafter).
    if crafter != customer && price > 0 {
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&customer) {
            inv.remove_item(ADENA_ID, price);
        }
        super::items::add_inventory_item(world, crafter, ADENA_ID, price);
    }

    // Consume materials from the customer + disappear messages.
    for &(item_id, need) in &recipe.ingredients {
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&customer) {
            inv.remove_item(item_id, need);
        }
        if need > 1 {
            sm_customer(world, sm_ids::S2_S1_S_DISAPPEARED, &[SmParam::ItemName(item_id), SmParam::Long(need)]);
        } else {
            sm_customer(world, sm_ids::S1_DISAPPEARED, &[SmParam::ItemName(item_id)]);
        }
    }

    // Roll success (`Rnd.get(100) < successRate`, `CRAFT_RATE` = 0 here).
    let success = world.roll(100) < recipe.success_rate;
    if success {
        reward(world, crafter, customer, customer_client, &recipe, price);
    } else if crafter != customer {
        // Cross-player failure messages.
        send_to_player(
            world,
            crafter,
            sp::system_message_with(
                sm_ids::YOU_FAILED_TO_CREATE_S2_FOR_C1_AT_THE_PRICE_OF_S3_ADENA,
                &[SmParam::PlayerName(name_of(world, customer)), SmParam::ItemName(recipe.item_id), SmParam::Long(price)],
            ),
        );
        sm_customer(
            world,
            sm_ids::C1_HAS_FAILED_TO_CREATE_S2_AT_THE_PRICE_OF_S3_ADENA,
            &[SmParam::PlayerName(name_of(world, crafter)), SmParam::ItemName(recipe.item_id), SmParam::Long(price)],
        );
    } else {
        sm_customer(world, sm_ids::YOU_FAILED_AT_MIXING_THE_ITEM, &[]);
    }

    // updateMakeInfo(success) + refresh the customer's item window (Java
    // `_target.sendItemList(false)`), and the crafter's if they were paid.
    update_make_info(world, crafter, customer, customer_client, recipe_id, recipe.is_dwarven, success);
    refresh_inventory(world, customer);
    if crafter != customer && price > 0 {
        refresh_inventory(world, crafter);
    }
}

/// `rewardPlayer` (`!ALT_GAME_CREATION` slice — no XP/SP): produce the item,
/// rolling the masterwork rare when the recipe has one.
fn reward(world: &mut World, crafter: i32, customer: i32, customer_client: u32, recipe: &RecipeList, price: i64) {
    let mut item_id = recipe.item_id;
    let mut count = recipe.count;
    // Masterwork: `(rareProdId != -1) && (rareProdId == itemId || CRAFT_MASTERWORK)`
    // then `Rnd.get(100) <= rarity`.
    if let Some(rare) = recipe.rare {
        if (rare.item_id == item_id || world.cfg.character.craft_masterwork) && world.roll(100) <= rare.rarity {
            item_id = rare.item_id;
            count = rare.count;
        }
    }

    super::items::add_inventory_item(world, customer, item_id, count as i64);
    // TODO(G15.7): `Stat.CRAFTING_CRITICAL` double-output crit — no source
    // grants the stat in the ported set, so it never fires.

    // Cross-player profit/receipt messages (manufacture only).
    if crafter != customer {
        let crafter_name = name_of(world, crafter);
        let customer_name = name_of(world, customer);
        if count == 1 {
            send_to_player(
                world,
                crafter,
                sp::system_message_with(
                    sm_ids::S2_HAS_BEEN_CREATED_FOR_C1_AFTER_THE_PAYMENT_OF_S3_ADENA_WAS_RECEIVED,
                    &[SmParam::PlayerName(customer_name.clone()), SmParam::ItemName(item_id), SmParam::Long(price)],
                ),
            );
            send_to_client(
                world,
                customer_client,
                sp::system_message_with(
                    sm_ids::C1_CREATED_S2_AFTER_RECEIVING_S3_ADENA,
                    &[SmParam::PlayerName(crafter_name), SmParam::ItemName(item_id), SmParam::Long(price)],
                ),
            );
        } else {
            send_to_player(
                world,
                crafter,
                sp::system_message_with(
                    sm_ids::S3_S2_S_HAVE_BEEN_CREATED_FOR_C1_AT_THE_PRICE_OF_S4_ADENA,
                    &[SmParam::PlayerName(customer_name.clone()), SmParam::Int(count), SmParam::ItemName(item_id), SmParam::Long(price)],
                ),
            );
            send_to_client(
                world,
                customer_client,
                sp::system_message_with(
                    sm_ids::C1_CREATED_S3_S2_S_AT_THE_PRICE_OF_S4_ADENA,
                    &[SmParam::PlayerName(crafter_name), SmParam::Int(count), SmParam::ItemName(item_id), SmParam::Long(price)],
                ),
            );
        }
    }

    // "You have earned …" to the customer.
    if count > 1 {
        send_to_client(world, customer_client, sp::system_message_with(sm_ids::YOU_HAVE_EARNED_S2_S1_S, &[SmParam::ItemName(item_id), SmParam::Long(count as i64)]));
    } else {
        send_to_client(world, customer_client, sp::system_message_with(sm_ids::YOU_HAVE_EARNED_S1, &[SmParam::ItemName(item_id)]));
    }
}

// --- helpers ---------------------------------------------------------------

/// `updateMakeInfo`: self-craft → `RecipeItemMakeInfo`; manufacture → the
/// buyer's `RecipeShopItemInfo`.
fn update_make_info(world: &World, crafter: i32, customer: i32, customer_client: u32, recipe_id: i32, is_dwarven: bool, success: bool) {
    if crafter == customer {
        let (cur_mp, max_mp) = world.objects.get_component::<Vitals>(&customer).map(|v| (v.cur_mp as i32, v.max_mp)).unwrap_or((0, 0));
        send_to_client(world, customer_client, sp::recipe_item_make_info(recipe_id, is_dwarven, cur_mp, max_mp, success));
    } else {
        let (cur_mp, max_mp) = world.objects.get_component::<Vitals>(&crafter).map(|v| (v.cur_mp as i32, v.max_mp)).unwrap_or((0, 0));
        send_to_client(world, customer_client, sp::recipe_shop_item_info(crafter, recipe_id, cur_mp, max_mp));
    }
}

/// The crafter's `StatusUpdate(CUR_MP/CUR_HP)` after a craft consumed vitals.
fn send_crafter_mp(world: &World, crafter: i32) {
    let Some(v) = world.objects.get_component::<Vitals>(&crafter) else { return };
    send_to_player(
        world,
        crafter,
        sp::status_update(crafter, &[(status_update_type::CUR_MP, v.cur_mp as i32), (status_update_type::CUR_HP, v.cur_hp as i32)]),
    );
}

fn refresh_inventory(world: &World, oid: i32) {
    if let Some(inv) = world.objects.get_component::<Inventory>(&oid) {
        send_to_player(world, oid, ew::item_list(inv, &world.data, false));
    }
}

fn name_of(world: &World, oid: i32) -> String {
    world.objects.get_component::<crate::model::Player>(&oid).map(|p| p.name.clone()).unwrap_or_default()
}

/// The seller's active manufacture list `(recipe_id, cost)`, filtered to the
/// book side being shown and recipes still in the book (Java `RecipeShopManageList`).
fn active_store_items(world: &World, oid: i32, is_dwarven: bool) -> Vec<(i32, i64)> {
    let Some(store) = world.objects.get_component::<ManufactureStore>(&oid) else { return Vec::new() };
    if store_type(world, oid) != STORE_TYPE_MANUFACTURE {
        return Vec::new();
    }
    store
        .items
        .iter()
        .filter(|(id, _)| {
            world.data.recipes.get(*id).map(|r| r.is_dwarven) == Some(is_dwarven)
                && world.objects.get_component::<RecipeBook>(&oid).is_some_and(|b| b.contains(*id))
        })
        .copied()
        .collect()
}

/// The seller's full active manufacture list (buyer view — no book-side filter).
fn current_store_items(world: &World, oid: i32) -> Vec<(i32, i64)> {
    world.objects.get_component::<ManufactureStore>(&oid).map(|s| s.items.clone()).unwrap_or_default()
}

/// Whether the object is running a manufacture store (for `Action` routing).
pub(crate) fn is_manufacture_owner(world: &World, oid: i32) -> bool {
    store_type(world, oid) == STORE_TYPE_MANUFACTURE
}

/// `Util.checkIfInRange(range, a, b, includeZBAxis=true)`: 3D distance vs
/// `range + a.collisionRadius + b.collisionRadius`.
fn in_range(world: &World, a: i32, b: i32, range: i32) -> bool {
    use crate::model::components::{Collision, Position};
    let (Some(pa), Some(pb)) = (
        world.objects.get_component::<Position>(&a),
        world.objects.get_component::<Position>(&b),
    ) else {
        return false;
    };
    let ra = world.objects.get_component::<Collision>(&a).map(|c| c.radius).unwrap_or(0.0);
    let rb = world.objects.get_component::<Collision>(&b).map(|c| c.radius).unwrap_or(0.0);
    let reach = range as f64 + ra + rb;
    let (dx, dy, dz) = ((pa.x - pb.x) as f64, (pa.y - pb.y) as f64, (pa.z - pb.z) as f64);
    dx * dx + dy * dy + dz * dz <= reach * reach
}
