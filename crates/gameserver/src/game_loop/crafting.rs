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
//! so they stay `+0`. `Stat.RECIPE_DWARVEN/COMMON` (Expand Dwarven/Common
//! Craft, G19 `EnlargeSlot`) *is* wired — see `learn_recipe`'s limit lookup.

use super::helpers::{adena, player_of, send_inventory_item_list};
use crate::data::item_data::ADENA_ID;
use crate::data::recipe_data::RecipeList;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::{send_to_client, send_to_player};
use crate::model::components::{ManufactureStore, RecipeBook, SkillBook, StatModifiers, Vitals};
use crate::model::inventory::Inventory;
use crate::model::stats::Stat;
use crate::network::client_packets as cp;
use crate::network::server_packets::{self as sp, SmParam, sm_ids, status_update_type};
use crate::world::World;

/// `CommonSkill.CREATE_DWARVEN` (172) / `CREATE_COMMON` (1320) — the craft
/// ability skills; the player's level in them is their `getDwarvenCraft` /
/// `getCommonCraft`.
const SKILL_CREATE_DWARVEN: i32 = 172;
const SKILL_CREATE_COMMON: i32 = 1320;
/// `PrivateStoreType.MANUFACTURE` (the CharInfo/UserInfo store byte).
const STORE_TYPE_MANUFACTURE: u8 = 5;

// --- small accessors -------------------------------------------------------

/// The player's level in the relevant create-item skill (0 if not known) — Java
/// `getDwarvenCraft` / `getCommonCraft`.
fn craft_skill_level(world: &World, oid: i32, is_dwarven: bool) -> i32 {
    let skill_id = if is_dwarven {
        SKILL_CREATE_DWARVEN
    } else {
        SKILL_CREATE_COMMON
    };
    world
        .objects
        .get_component::<SkillBook>(&oid)
        .and_then(|b| b.0.get(&skill_id).copied())
        .unwrap_or(0)
}

fn store_type(world: &World, oid: i32) -> u8 {
    world
        .objects
        .get_component::<crate::model::Player>(&oid)
        .map(|p| p.store_type)
        .unwrap_or(0)
}

fn set_store_type(world: &mut World, oid: i32, ty: u8) {
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&oid)
    {
        p.store_type = ty;
    }
}

fn vitals(world: &World, oid: i32) -> Option<(f64, i32, f64)> {
    world
        .objects
        .get_component::<Vitals>(&oid)
        .map(|v| (v.cur_mp, v.max_mp, v.cur_hp))
}

// --- recipe book -----------------------------------------------------------

/// `RecipeManager.requestBookOpen` — send the recipe window for one craft type.
/// (The Java "engaged in manufacturing" guard is moot: crafts finish inline, so
/// no maker is ever active when this arrives.)
pub(crate) fn request_book_open(world: &mut World, client_id: u32, is_dwarven: bool) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
    send_recipe_book(world, client_id, oid, is_dwarven);
}

/// `RecipeBookItemList` for one craft type. The max-MP field is the *current*
/// max, not the value the window was opened with, so a resend after a stat
/// change shows the right craft budget.
fn send_recipe_book(world: &mut World, client_id: u32, oid: i32, is_dwarven: bool) {
    let max_mp = world
        .objects
        .get_component::<Vitals>(&oid)
        .map(|v| v.max_mp)
        .unwrap_or(0);
    let recipes = book_ids(world, oid, is_dwarven);
    send_to_client(
        world,
        client_id,
        sp::recipe_book_item_list(is_dwarven, max_mp, &recipes),
    );
}

/// `RequestRecipeBookDestroy` — drop a recipe from whichever book holds it, then
/// resend that book.
pub(crate) fn handle_book_destroy(world: &mut World, client_id: u32, recipe_id: i32) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
    // Java looks the recipe up in RecipeData for its dwarven flag; do the same so
    // we resend the correct book even after removing the id.
    let Some(is_dwarven) = world.data.recipes.get(recipe_id).map(|r| r.is_dwarven) else {
        return;
    };
    if let Some(book) = world.objects.get_component_mut::<RecipeBook>(&oid) {
        book.dwarven.retain(|&id| id != recipe_id);
        book.common.retain(|&id| id != recipe_id);
    }
    send_recipe_book(world, client_id, oid, is_dwarven);
}

fn book_ids(world: &World, oid: i32, is_dwarven: bool) -> Vec<i32> {
    world
        .objects
        .get_component::<RecipeBook>(&oid)
        .map(|b| {
            if is_dwarven {
                b.dwarven.clone()
            } else {
                b.common.clone()
            }
        })
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
        let Some(inv) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        let Some(item) = inv.by_object_id(item_object_id) else {
            return;
        };
        item.item_id
    };
    let Some(recipe) = world.data.recipes.by_recipe_item_id(item_id).cloned() else {
        return;
    };

    // Already registered?
    if world
        .objects
        .get_component::<RecipeBook>(&object_id)
        .is_some_and(|b| b.contains(recipe.id))
    {
        send(world, sm_ids::THAT_RECIPE_IS_ALREADY_REGISTERED, &[]);
        return;
    }

    let craft_level = craft_skill_level(world, object_id, recipe.is_dwarven);
    let (limit, book_len) = {
        let book = world.objects.get_component::<RecipeBook>(&object_id);
        let (stat, base, len) = if recipe.is_dwarven {
            (
                Stat::RecipeDwarven,
                world.cfg.character.dwarf_recipe_limit,
                book.map(|b| b.dwarven.len()).unwrap_or(0),
            )
        } else {
            (
                Stat::RecipeCommon,
                world.cfg.character.common_recipe_limit,
                book.map(|b| b.common.len()).unwrap_or(0),
            )
        };
        // Expand Dwarven/Common Craft (1368/1369, `EnlargeSlot`): the base
        // config limit plus whatever the learned passive raises it to.
        let limit = match world.objects.get_component::<StatModifiers>(&object_id) {
            Some(mods) => crate::model::finalize(mods, stat, base as f64) as i32,
            None => base,
        };
        (limit, len)
    };

    // `hasDwarvenCraft` / `hasCommonCraft`: the create-item skill (level ≥ 1).
    if craft_level < 1 {
        send(
            world,
            sm_ids::THE_RECIPE_CANNOT_BE_REGISTERED_YOU_DO_NOT_HAVE_THE_ABILITY_TO_CREATE_ITEMS,
            &[],
        );
        return;
    }
    if recipe.level > craft_level {
        send(
            world,
            sm_ids::YOUR_CREATE_ITEM_LEVEL_IS_TOO_LOW_TO_REGISTER_THIS_RECIPE,
            &[],
        );
        return;
    }
    if book_len as i32 >= limit {
        send(
            world,
            sm_ids::UP_TO_S1_RECIPES_CAN_BE_REGISTERED,
            &[SmParam::Int(limit)],
        );
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
    if let Some(destroyed) =
        super::helpers::remove_inventory_item_change(world, object_id, item_object_id, 1)
    {
        super::helpers::send_inventory_update(world, object_id, vec![destroyed]);
    }
    send(
        world,
        sm_ids::S1_HAS_BEEN_ADDED,
        &[SmParam::ItemName(item_id)],
    );
}

// --- self-craft ------------------------------------------------------------

/// `RequestRecipeItemMakeInfo` — (re)open the self-craft "make" window.
pub(crate) fn handle_make_info(world: &mut World, client_id: u32, id: i32) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
    let Some(is_dwarven) = world.data.recipes.get(id).map(|r| r.is_dwarven) else {
        return;
    };
    let (cur_mp, max_mp) = mp_gauge(world, oid);
    send_to_client(
        world,
        client_id,
        sp::recipe_item_make_info(id, is_dwarven, cur_mp, max_mp, true),
    );
}

/// `RequestRecipeItemMakeSelf` — craft one of the player's own recipes.
pub(crate) fn handle_make_self(world: &mut World, client_id: u32, id: i32) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
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
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
    if is_dead(world, oid) {
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
    send_to_client(
        world,
        client_id,
        sp::recipe_shop_manage_list(oid, adena(world, oid) as i32, is_dwarven, &recipes, &store),
    );
}

/// `RequestRecipeShopMessageSet` — set the store title (Java `setStoreName`,
/// stored on the player across store types; here on the `ManufactureStore`).
pub(crate) fn handle_message_set(world: &mut World, client_id: u32, name: String) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
    const MAX_MSG_LENGTH: usize = 29;
    if name.chars().count() > MAX_MSG_LENGTH {
        return;
    }
    if let Some(store) = world.objects.get_component_mut::<ManufactureStore>(&oid) {
        store.title = name;
    } else {
        world.objects.add_components(
            &oid,
            ManufactureStore {
                items: Vec::new(),
                title: name,
            },
        );
    }
}

/// `RequestRecipeShopListSet` — activate the manufacture store with the given
/// recipe/price lines (each validated against the seller's book).
pub(crate) fn handle_list_set(world: &mut World, client_id: u32, lines: Vec<cp::ManufactureLine>) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
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

    // Validate every recipe is in the seller's book; a bad id punishes and
    // aborts the whole set (Java `handleIllegalPlayerAction` + return).
    let mut items = Vec::with_capacity(lines.len());
    for line in &lines {
        let known = world
            .objects
            .get_component::<RecipeBook>(&oid)
            .is_some_and(|b| b.contains(line.recipe_id));
        if !known {
            super::punishment::illegal_action(
                world,
                oid,
                &format!("Player {oid} tried to set recipe which he dont have."),
            );
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
            world.objects.add_components(
                &oid,
                ManufactureStore {
                    items,
                    title: String::new(),
                },
            );
            String::new()
        }
    };
    set_store_type(world, oid, STORE_TYPE_MANUFACTURE);
    // Java `sitDown()`s the manufacturer — a shop owner sits behind their
    // wares, and `standUp` refuses while the store is open, so they stay put
    // until they close it.
    super::sit_stand::sit_down(world, oid);
    super::helpers::broadcast_including_self(world, oid, &sp::recipe_shop_msg(oid, &title));
    super::party::broadcast_user_info(world, oid);
}

/// `RequestRecipeShopManageQuit` — close the manufacture store.
pub(crate) fn handle_manage_quit(world: &mut World, client_id: u32) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
    set_store_type(world, oid, 0);
    super::helpers::broadcast_including_self(world, oid, &sp::recipe_shop_msg(oid, ""));
    super::party::broadcast_user_info(world, oid);
    super::offline_trade::on_store_type_cleared(world, oid);
}

/// A customer clicked a manufacture-store owner (`Action`): show their list.
pub(crate) fn open_sell_list(world: &mut World, client_id: u32, buyer: i32, manufacturer: i32) {
    if store_type(world, manufacturer) != STORE_TYPE_MANUFACTURE {
        return;
    }
    let (cur_mp, max_mp) = mp_gauge(world, manufacturer);
    let store = current_store_items(world, manufacturer);
    send_to_client(
        world,
        client_id,
        sp::recipe_shop_sell_list(manufacturer, cur_mp, max_mp, adena(world, buyer), &store),
    );
}

/// `(curMp as int, maxMp)` — the crafter's MP gauge, as every recipe packet in
/// this file carries it.
///
/// Java reads `getCurrentMp()` / `getMaxMp()` off the *manufacturer*, who is
/// not always the packet's recipient: a customer browsing a private workshop
/// sees the shop owner's MP, because that is what limits what can be crafted.
///
/// `(0, 0)` when the object has left the world — the packet still goes out, and
/// an empty gauge is what Java's null-safe path draws.
fn mp_gauge(world: &World, oid: i32) -> (i32, i32) {
    world
        .objects
        .get_component::<Vitals>(&oid)
        .map(|v| (v.cur_mp as i32, v.max_mp))
        .unwrap_or((0, 0))
}

/// `RequestRecipeShopMakeInfo` — the per-recipe info line in a shop.
pub(crate) fn handle_shop_make_info(
    world: &mut World,
    client_id: u32,
    shop_oid: i32,
    recipe_id: i32,
) {
    if store_type(world, shop_oid) != STORE_TYPE_MANUFACTURE {
        return;
    }
    let (cur_mp, max_mp) = mp_gauge(world, shop_oid);
    send_to_client(
        world,
        client_id,
        sp::recipe_shop_item_info(shop_oid, recipe_id, cur_mp, max_mp),
    );
}

/// `RequestRecipeShopMakeItem` — a customer buys a craft from a manufacturer.
pub(crate) fn handle_shop_make_item(
    world: &mut World,
    client_id: u32,
    manufacturer: i32,
    recipe_id: i32,
) {
    let Some(buyer) = player_of(world, client_id) else {
        return;
    };
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
        .and_then(|s| {
            s.items
                .iter()
                .find(|(id, _)| *id == recipe_id)
                .map(|(_, cost)| *cost)
        })
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
fn do_craft(
    world: &mut World,
    crafter: i32,
    customer: i32,
    customer_client: u32,
    recipe_id: i32,
    price: i64,
) {
    let Some(recipe) = world.data.recipes.get(recipe_id).cloned() else {
        return;
    };

    let abort = |world: &World| {
        // Java `abort()` → `updateMakeInfo(false)`: for a self-craft that's the
        // make-info failure; for manufacture it's the shop item-info refresh.
        update_make_info(
            world,
            crafter,
            customer,
            customer_client,
            recipe_id,
            recipe.is_dwarven,
            false,
        );
    };
    let sm_customer = |world: &World, msg: i16, params: &[SmParam]| {
        send_to_client(world, customer_client, sp::system_message_with(msg, params));
    };

    // The crafter must actually hold this recipe; a request for a recipe not
    // in the book punishes the requester (Java `RecipeManager`).
    if !world
        .objects
        .get_component::<RecipeBook>(&crafter)
        .is_some_and(|b| b.contains(recipe.id))
    {
        super::punishment::illegal_action(
            world,
            customer,
            &format!("Player {customer} sent a false recipe id."),
        );
        return;
    }
    // Empty recipe / skill-level gate (crafter's create-item level).
    if recipe.ingredients.is_empty()
        || recipe.level > craft_skill_level(world, crafter, recipe.is_dwarven)
    {
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
        let have = world
            .objects
            .get_component::<Inventory>(&customer)
            .map(|i| i.count_of(item_id))
            .unwrap_or(0);
        if have < need {
            sm_customer(
                world,
                sm_ids::YOU_NEED_S2_MORE_S1_S,
                &[SmParam::ItemName(item_id), SmParam::Long(need - have)],
            );
            abort(world);
            return;
        }
    }
    // MP/HP present on the crafter (statUse check). HP uses `<=` (can't kill),
    // MP uses `<`, matching Java.
    let Some((cur_mp, _, cur_hp)) = vitals(world, crafter) else {
        return;
    };
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

    // --- all checks passed ---

    // `Config.ALT_GAME_CREATION`: the staged multi-pass craft takes over —
    // materials are "equipped" in per-pass grabs with the create-skill
    // animation and gauge, stat use is paid per pass, and the settle runs
    // when the last grab lands (Java `RecipeItemMaker.run`).
    if world.cfg.character.alt_game_creation {
        staged_start(
            world,
            crafter,
            customer,
            customer_client,
            recipe_id,
            price,
            &recipe,
        );
        return;
    }

    // Reduce the crafter's MP/HP + StatusUpdate.
    if recipe.mp_use > 0 || recipe.hp_use > 0 {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&crafter) {
            v.cur_mp = (v.cur_mp - recipe.mp_use as f64).max(0.0);
            v.cur_hp = (v.cur_hp - recipe.hp_use as f64).max(1.0);
        }
        send_crafter_mp(world, crafter);
    }

    settle_and_reward(
        world,
        crafter,
        customer,
        customer_client,
        recipe_id,
        price,
        &recipe,
    );
}

/// One player's staged craft in flight (Java `RecipeItemMaker` under
/// `_activeMakers`). Component on the **crafter**; its presence IS Java's
/// `isCrafting()` — which is what the logout gate observes.
#[derive(bevy_ecs::component::Component, Debug, Clone)]
pub struct ActiveCraft {
    pub customer: i32,
    pub customer_client: u32,
    pub recipe_id: i32,
    pub price: i64,
    /// The material units still to "equip", as `(item_id, remaining)` — Java's
    /// `TempItem` list.
    pub remaining: Vec<(i32, i64)>,
    /// `_creationPasses` — per-pass stat use divides by this.
    pub passes: i32,
    /// `_itemGrab` — units consumed per pass (skill level × GIM).
    pub item_grab: i64,
    /// The last pass delay in ms (`_delay`), reused by the finish gauge.
    pub delay_ms: i32,
}

/// Java `RecipeItemMaker`'s ALT branch of the constructor + `_activeMakers`
/// registration: compute the grab size and pass count, park the session on
/// the crafter, and schedule the first pass (Java `ThreadPool.schedule(maker,
/// 100)`).
fn staged_start(
    world: &mut World,
    crafter: i32,
    customer: i32,
    customer_client: u32,
    recipe_id: i32,
    price: i64,
    recipe: &RecipeList,
) {
    // Java refuses a second craft while one runs (`_activeMakers.containsKey`
    // → "You are busy creating").
    if world.objects.has_component::<ActiveCraft>(&crafter) {
        send_to_client(
            world,
            customer_client,
            sp::system_message_with(
                sm_ids::YOU_MAY_NOT_ALTER_YOUR_RECIPE_BOOK_WHILE_ENGAGED_IN_MANUFACTURING,
                &[],
            ),
        );
        return;
    }
    let skill_level = craft_skill_level(world, crafter, recipe.is_dwarven);
    // `calculateAltStatChange`: grab = skill level × GIM; passes = ceil.
    let item_grab = i64::from(skill_level.max(1)) * i64::from(recipe.alt_gim.max(1));
    let total: i64 = recipe.ingredients.iter().map(|&(_, n)| n).sum();
    let passes = ((total + item_grab - 1) / item_grab).max(1) as i32;
    world.objects.add_components(
        &crafter,
        ActiveCraft {
            customer,
            customer_client,
            recipe_id,
            price,
            remaining: recipe.ingredients.clone(),
            passes,
            item_grab,
            delay_ms: 0,
        },
    );
    update_make_info(
        world,
        crafter,
        customer,
        customer_client,
        recipe_id,
        recipe.is_dwarven,
        true,
    );
    world.scheduler.schedule(
        world.tick + 1,
        crate::scheduler::ScheduledTask::CraftPass {
            crafter_oid: crafter,
        },
    );
}

/// One staged pass (Java `RecipeItemMaker.run`'s ALT arm): pay the per-pass
/// stat use (waiting on the gauge when HP/MP are short), grab up to
/// `item_grab` material units with the "equipped" message, and either animate
/// into the next pass or glide the gauge into the finish.
pub(crate) fn handle_craft_pass(world: &mut World, crafter: i32) {
    let Some(session) = world
        .objects
        .get_component::<ActiveCraft>(&crafter)
        .cloned()
    else {
        return; // aborted (Java's "Item creation aborted" fires on the abort itself)
    };
    let Some(recipe) = world.data.recipes.get(session.recipe_id).cloned() else {
        world.objects.remove_component::<ActiveCraft>(&crafter);
        return;
    };
    // `calculateStatUse(isWait = true, isReduce = true)` — per-pass share.
    let (mp_share, hp_share) = (
        f64::from(recipe.mp_use) / f64::from(session.passes),
        f64::from(recipe.hp_use) / f64::from(session.passes),
    );
    let Some((cur_mp, _, cur_hp)) = vitals(world, crafter) else {
        world.objects.remove_component::<ActiveCraft>(&crafter);
        return;
    };
    if (recipe.mp_use > 0 && cur_mp < mp_share) || (recipe.hp_use > 0 && cur_hp <= hp_share) {
        // Short on HP/MP: rest — gauge + retry after the same delay, nothing
        // consumed (Java's isWait branch).
        send_to_player(
            world,
            crafter,
            sp::setup_gauge(crafter, 0, session.delay_ms),
        );
        world.scheduler.schedule(
            world.tick + 1 + super::helpers::ms_to_ticks(session.delay_ms),
            crate::scheduler::ScheduledTask::CraftPass {
                crafter_oid: crafter,
            },
        );
        return;
    }
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&crafter) {
        v.cur_mp = (v.cur_mp - mp_share).max(0.0);
        v.cur_hp = (v.cur_hp - hp_share).max(1.0);
    }
    send_crafter_mp(world, crafter);

    // `grabSomeItems`: consume up to item_grab units off the temp list.
    let mut grab = session.item_grab;
    let mut remaining = session.remaining.clone();
    while grab > 0 && !remaining.is_empty() {
        let (item_id, qty) = remaining[0];
        let take = qty.min(grab);
        if qty - take <= 0 {
            remaining.remove(0);
        } else {
            remaining[0] = (item_id, qty - take);
        }
        grab -= take;
        if session.customer == crafter {
            send_to_player(
                world,
                crafter,
                sp::system_message_with(
                    sm_ids::EQUIPPED_S1_S2,
                    &[SmParam::Long(take), SmParam::ItemName(item_id)],
                ),
            );
        }
    }

    // Delay = speed × the create skill's reuse (Java `getReuseTime(_skill)`).
    let skill_id = if recipe.is_dwarven {
        SKILL_CREATE_DWARVEN
    } else {
        SKILL_CREATE_COMMON
    };
    let skill_level = craft_skill_level(world, crafter, recipe.is_dwarven);
    let reuse_ms = world
        .data
        .skill_data
        .get(skill_id, skill_level.max(1))
        .map(|sk| sk.reuse_delay)
        .unwrap_or(0);
    let delay_ms = (world.cfg.character.alt_game_creation_speed * f64::from(reuse_ms)) as i32;

    let done = remaining.is_empty();
    if let Some(sess) = world.objects.get_component_mut::<ActiveCraft>(&crafter) {
        sess.remaining = remaining;
        sess.delay_ms = delay_ms;
    }
    if !done {
        // The crafting animation + gauge, then the next pass.
        let pos = world
            .objects
            .get_component::<crate::model::components::Position>(&crafter)
            .map(|p| (crafter, p.x, p.y, p.z))
            .unwrap_or((crafter, 0, 0, 0));
        let msu = sp::magic_skill_use_raw(pos, pos, skill_id, skill_level, delay_ms);
        super::helpers::broadcast_including_self(world, crafter, &msu);
        send_to_player(world, crafter, sp::setup_gauge(crafter, 0, delay_ms));
        world.scheduler.schedule(
            world.tick + 1 + super::helpers::ms_to_ticks(delay_ms),
            crate::scheduler::ScheduledTask::CraftPass {
                crafter_oid: crafter,
            },
        );
    } else {
        // Last grab: gauge out, then settle (Java sleeps `_delay` and calls
        // `finishCrafting`).
        send_to_player(world, crafter, sp::setup_gauge(crafter, 0, delay_ms));
        world.scheduler.schedule(
            world.tick + 1 + super::helpers::ms_to_ticks(delay_ms),
            crate::scheduler::ScheduledTask::CraftFinish {
                crafter_oid: crafter,
            },
        );
    }
}

/// The staged finish: drop the session and run the shared settle. Stat use
/// was already paid per pass, so this is fee → materials → roll → reward —
/// and on success the ALT XP/SP award (Java `rewardPlayer`'s alt tail).
pub(crate) fn handle_craft_finish(world: &mut World, crafter: i32) {
    let Some(session) = world
        .objects
        .get_component::<ActiveCraft>(&crafter)
        .cloned()
    else {
        return;
    };
    world.objects.remove_component::<ActiveCraft>(&crafter);
    let Some(recipe) = world.data.recipes.get(session.recipe_id).cloned() else {
        return;
    };
    // Re-verify the materials are still there (Java's `listItems(true)` after
    // the passes — "handle possible cheaters here").
    for &(item_id, need) in &recipe.ingredients {
        let have = world
            .objects
            .get_component::<Inventory>(&session.customer)
            .map(|i| i.count_of(item_id))
            .unwrap_or(0);
        if have < need {
            return; // Java falls through silently on the cheater branch
        }
    }
    settle_and_reward(
        world,
        crafter,
        session.customer,
        session.customer_client,
        session.recipe_id,
        session.price,
        &recipe,
    );
}

/// Java `Player.isCrafting()` — a staged craft is in flight. The logout gate
/// reads this; the inline mode never exposes it (the craft finishes within
/// its own packet).
pub(crate) fn is_crafting(world: &World, player_oid: i32) -> bool {
    world.objects.has_component::<ActiveCraft>(&player_oid)
}

/// The shared settle (Java `finishCrafting` minus the stat use, which each
/// mode pays its own way): fee, materials, the success roll, reward or
/// failure messages, make-info + inventory refresh.
fn settle_and_reward(
    world: &mut World,
    crafter: i32,
    customer: i32,
    customer_client: u32,
    recipe_id: i32,
    price: i64,
    recipe: &RecipeList,
) {
    let sm_customer = |world: &World, msg: i16, params: &[SmParam]| {
        send_to_client(world, customer_client, sp::system_message_with(msg, params));
    };
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
            sm_customer(
                world,
                sm_ids::S2_S1_S_DISAPPEARED,
                &[SmParam::ItemName(item_id), SmParam::Long(need)],
            );
        } else {
            sm_customer(world, sm_ids::S1_DISAPPEARED, &[SmParam::ItemName(item_id)]);
        }
    }

    // Roll success (`Rnd.get(100) < successRate`, `CRAFT_RATE` = 0 here).
    let success = world.roll(100) < recipe.success_rate;
    if success {
        reward(world, crafter, customer, customer_client, recipe, price);
    } else if crafter != customer {
        // Cross-player failure messages.
        send_to_player(
            world,
            crafter,
            sp::system_message_with(
                sm_ids::YOU_FAILED_TO_CREATE_S2_FOR_C1_AT_THE_PRICE_OF_S3_ADENA,
                &[
                    SmParam::PlayerName(player_name_or_empty(world, customer)),
                    SmParam::ItemName(recipe.item_id),
                    SmParam::Long(price),
                ],
            ),
        );
        sm_customer(
            world,
            sm_ids::C1_HAS_FAILED_TO_CREATE_S2_AT_THE_PRICE_OF_S3_ADENA,
            &[
                SmParam::PlayerName(player_name_or_empty(world, crafter)),
                SmParam::ItemName(recipe.item_id),
                SmParam::Long(price),
            ],
        );
    } else {
        sm_customer(world, sm_ids::YOU_FAILED_AT_MIXING_THE_ITEM, &[]);
    }

    // updateMakeInfo(success) + refresh the customer's item window (Java
    // `_target.sendItemList(false)`), and the crafter's if they were paid.
    update_make_info(
        world,
        crafter,
        customer,
        customer_client,
        recipe_id,
        recipe.is_dwarven,
        success,
    );
    send_inventory_item_list(world, customer);
    if crafter != customer && price > 0 {
        send_inventory_item_list(world, crafter);
    }
}

/// `rewardPlayer` (`!ALT_GAME_CREATION` slice — no XP/SP): produce the item,
/// rolling the masterwork rare when the recipe has one.
fn reward(
    world: &mut World,
    crafter: i32,
    customer: i32,
    customer_client: u32,
    recipe: &RecipeList,
    price: i64,
) {
    let mut item_id = recipe.item_id;
    let mut count = recipe.count;
    // Masterwork: `(rareProdId != -1) && (rareProdId == itemId || CRAFT_MASTERWORK)`
    // then `Rnd.get(100) <= rarity`.
    if let Some(rare) = recipe.rare
        && (rare.item_id == item_id || world.cfg.character.craft_masterwork)
        && world.roll(100) <= rare.rarity
    {
        item_id = rare.item_id;
        count = rare.count;
    }

    super::items::add_inventory_item(world, customer, item_id, count as i64);
    // `rewardPlayer`'s ALT_GAME_CREATION tail: the crafter earns XP/SP scaled
    // by the recipe level, the rare production, and the creation-speed knobs.
    if world.cfg.character.alt_game_creation {
        let cfgc = &world.cfg.character;
        let mut exp = recipe.alt_exp.unwrap_or_else(|| {
            let reference = world
                .data
                .item_data
                .get(item_id)
                .map(|t| t.price)
                .unwrap_or(0);
            reference * i64::from(count) / i64::from(recipe.level.max(1))
        });
        let mut sp = recipe.alt_sp.unwrap_or(exp / 10);
        if recipe.rare.is_some_and(|r| r.item_id == item_id) {
            exp = (exp as f64 * cfgc.alt_game_creation_rare_xpsp_rate) as i64;
            sp = (sp as f64 * cfgc.alt_game_creation_rare_xpsp_rate) as i64;
        }
        let (mut exp, mut sp) = (exp.max(0), sp.max(0));
        // Crafting under-level recipes decays the reward 4× per level over.
        let skill_level = craft_skill_level(world, crafter, recipe.is_dwarven);
        for _ in recipe.level..skill_level {
            exp /= 4;
            sp /= 4;
        }
        let exp = exp as f64 * cfgc.alt_game_creation_xp_rate * cfgc.alt_game_creation_speed;
        let sp = sp as f64 * cfgc.alt_game_creation_sp_rate * cfgc.alt_game_creation_speed;
        if exp > 0.0 || sp > 0.0 {
            crate::game_loop::death::add_exp_and_sp(world, crafter, exp, sp, false);
        }
    }
    // `Stat.CRAFTING_CRITICAL` (double output) is not modelled. Not a
    // deferral: nothing in `dist/game/data/stats` grants the stat, so the roll
    // would always be against zero.

    // Cross-player profit/receipt messages (manufacture only).
    if crafter != customer {
        let crafter_name = player_name_or_empty(world, crafter);
        let customer_name = player_name_or_empty(world, customer);
        if count == 1 {
            send_to_player(
                world,
                crafter,
                sp::system_message_with(
                    sm_ids::S2_HAS_BEEN_CREATED_FOR_C1_AFTER_THE_PAYMENT_OF_S3_ADENA_WAS_RECEIVED,
                    &[
                        SmParam::PlayerName(customer_name.clone()),
                        SmParam::ItemName(item_id),
                        SmParam::Long(price),
                    ],
                ),
            );
            send_to_client(
                world,
                customer_client,
                sp::system_message_with(
                    sm_ids::C1_CREATED_S2_AFTER_RECEIVING_S3_ADENA,
                    &[
                        SmParam::PlayerName(crafter_name),
                        SmParam::ItemName(item_id),
                        SmParam::Long(price),
                    ],
                ),
            );
        } else {
            send_to_player(
                world,
                crafter,
                sp::system_message_with(
                    sm_ids::S3_S2_S_HAVE_BEEN_CREATED_FOR_C1_AT_THE_PRICE_OF_S4_ADENA,
                    &[
                        SmParam::PlayerName(customer_name.clone()),
                        SmParam::Int(count),
                        SmParam::ItemName(item_id),
                        SmParam::Long(price),
                    ],
                ),
            );
            send_to_client(
                world,
                customer_client,
                sp::system_message_with(
                    sm_ids::C1_CREATED_S3_S2_S_AT_THE_PRICE_OF_S4_ADENA,
                    &[
                        SmParam::PlayerName(crafter_name),
                        SmParam::Int(count),
                        SmParam::ItemName(item_id),
                        SmParam::Long(price),
                    ],
                ),
            );
        }
    }

    // "You have earned …" to the customer.
    if count > 1 {
        send_to_client(
            world,
            customer_client,
            sp::system_message_with(
                sm_ids::YOU_HAVE_EARNED_S2_S1_S,
                &[SmParam::ItemName(item_id), SmParam::Long(count as i64)],
            ),
        );
    } else {
        send_to_client(
            world,
            customer_client,
            sp::system_message_with(sm_ids::YOU_HAVE_EARNED_S1, &[SmParam::ItemName(item_id)]),
        );
    }
}

// --- helpers ---------------------------------------------------------------

/// `updateMakeInfo`: self-craft → `RecipeItemMakeInfo`; manufacture → the
/// buyer's `RecipeShopItemInfo`.
fn update_make_info(
    world: &World,
    crafter: i32,
    customer: i32,
    customer_client: u32,
    recipe_id: i32,
    is_dwarven: bool,
    success: bool,
) {
    if crafter == customer {
        let (cur_mp, max_mp) = mp_gauge(world, customer);
        send_to_client(
            world,
            customer_client,
            sp::recipe_item_make_info(recipe_id, is_dwarven, cur_mp, max_mp, success),
        );
    } else {
        let (cur_mp, max_mp) = mp_gauge(world, crafter);
        send_to_client(
            world,
            customer_client,
            sp::recipe_shop_item_info(crafter, recipe_id, cur_mp, max_mp),
        );
    }
}

/// The crafter's `StatusUpdate(CUR_MP/CUR_HP)` after a craft consumed vitals.
fn send_crafter_mp(world: &World, crafter: i32) {
    let Some(v) = world.objects.get_component::<Vitals>(&crafter) else {
        return;
    };
    send_to_player(
        world,
        crafter,
        sp::status_update(
            crafter,
            &[
                (status_update_type::CUR_MP, v.cur_mp as i32),
                (status_update_type::CUR_HP, v.cur_hp as i32),
            ],
        ),
    );
}

/// The seller's active manufacture list `(recipe_id, cost)`, filtered to the
/// book side being shown and recipes still in the book (Java `RecipeShopManageList`).
fn active_store_items(world: &World, oid: i32, is_dwarven: bool) -> Vec<(i32, i64)> {
    let Some(store) = world.objects.get_component::<ManufactureStore>(&oid) else {
        return Vec::new();
    };
    if store_type(world, oid) != STORE_TYPE_MANUFACTURE {
        return Vec::new();
    }
    store
        .items
        .iter()
        .filter(|(id, _)| {
            world.data.recipes.get(*id).map(|r| r.is_dwarven) == Some(is_dwarven)
                && world
                    .objects
                    .get_component::<RecipeBook>(&oid)
                    .is_some_and(|b| b.contains(*id))
        })
        .copied()
        .collect()
}

/// The seller's full active manufacture list (buyer view — no book-side filter).
fn current_store_items(world: &World, oid: i32) -> Vec<(i32, i64)> {
    world
        .objects
        .get_component::<ManufactureStore>(&oid)
        .map(|s| s.items.clone())
        .unwrap_or_default()
}

/// Whether the object is running a manufacture store (for `Action` routing).
pub(crate) fn is_manufacture_owner(world: &World, oid: i32) -> bool {
    store_type(world, oid) == STORE_TYPE_MANUFACTURE
}

/// `Util.checkIfInRange(range, a, b, includeZBAxis=true)`: 3D distance vs
/// `range + a.collisionRadius + b.collisionRadius`.
fn in_range(world: &World, a: i32, b: i32, range: i32) -> bool {
    use crate::model::components::Collision;
    let ra = world
        .objects
        .get_component::<Collision>(&a)
        .map(|c| c.radius)
        .unwrap_or(0.0);
    let rb = world
        .objects
        .get_component::<Collision>(&b)
        .map(|c| c.radius)
        .unwrap_or(0.0);
    // Java widens the gate by both collision radii before comparing, so this
    // cannot be a plain `within_3d`.
    let reach = range as f64 + ra + rb;
    crate::geo::distance::distance_3d(world, a, b).is_some_and(|d| d <= reach)
}
