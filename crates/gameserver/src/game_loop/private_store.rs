//! Private sell and buy stores: a player opens a manage window, sets a list of
//! items+prices, and sits with a titled store; other players click the owner to
//! see the store and trade with it. Items stay in the owner's inventory until
//! sold.
//!
//! Three of Java's `PrivateStoreType`s live here — `SELL` (1), `BUY` (3) and
//! **`PACKAGE_SELL` (8)**, the `/packagesale` store whose whole list is one lot:
//! the client must buy every line at once, the window carries a "packaged" flag,
//! and its title rides `ExPrivateStoreSetWholeMsg` instead of
//! `PrivateStoreMsgSell`. Manufacture (workshop) stores belong to `crafting`.

use super::helpers::{
    adena, player_of, send_inventory_item_list, send_sm_bare_to_client as send_sm,
};
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::send_to_client;
use crate::model::components::{PrivateStore, StoreItem};
use crate::model::inventory::{Inventory, ItemInstance};
use crate::network::client_packets as cp;
use crate::network::server_packets::{self as sp, StoreLine};
use crate::world::World;

const ADENA_ID: i32 = 57;
const STORE_TYPE_SELL: u8 = 1;
/// Java `PrivateStoreType.PACKAGE_SELL` — the whole list sells as one lot.
const STORE_TYPE_PACKAGE_SELL: u8 = 8;

/// Either kind of sell store (the buyer-facing paths accept both, like Java's
/// `RequestPrivateStoreBuy`).
fn is_sell_store(store_type: u8) -> bool {
    store_type == STORE_TYPE_SELL || store_type == STORE_TYPE_PACKAGE_SELL
}

fn instance(object_id: i32, item_id: i32, count: i64, enchant: i32) -> ItemInstance {
    ItemInstance {
        object_id,
        item_id,
        count,
        enchant_level: enchant,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    }
}

/// `RequestPrivateStoreManageSell` (0x30) / the `PrivateStore` player action:
/// open the setup window — the owner's sellable inventory items plus any
/// already in the store. `packaged` opens it in **package** mode
/// (`/packagesale`, action 61), which the client renders differently and
/// echoes back in `SetPrivateStoreListSell`.
pub(crate) fn open_manage(world: &mut World, client_id: u32) {
    open_manage_kind(world, client_id, false);
}

/// The package-sell manage window (Java `PrivateStoreManageListSell(player,
/// true)`).
pub(crate) fn open_manage_package(world: &mut World, client_id: u32) {
    open_manage_kind(world, client_id, true);
}

fn open_manage_kind(world: &mut World, client_id: u32, packaged: bool) {
    let Some(owner) = player_of(world, client_id) else {
        return;
    };
    if !can_open_private_store(world, client_id, owner) {
        return;
    }
    let Some(inv) = world.objects.get_component::<Inventory>(&owner) else {
        return;
    };
    let sellable: Vec<StoreLine> = inv
        .items()
        .iter()
        .filter(|it| inv.paperdoll_slot_of(it.object_id).is_none())
        .filter_map(|it| {
            let t = world.data.item_data.get(it.item_id)?;
            // Java `TradeList.addItem` (behind `PrivateStoreManageListSell`)
            // refuses untradable items — bound items never reach the window.
            (!t.is_quest_item && t.is_tradable() && t.price > 0).then_some(StoreLine {
                item: *it,
                template: t,
                price: 0,
            })
        })
        .collect();
    let in_store = store_lines(world, owner);
    let packet = sp::manage_list_sell(owner, adena(world, owner), &sellable, &in_store, packaged);
    send_to_client(world, client_id, packet);
}

/// `SetPrivateStoreListSell` (0x31): activate the store with the given
/// items+prices (validated against the owner's inventory). An empty list is a
/// no-op; a valid list sits the owner with a titled store visible to others.
/// `Player.getPrivateSellStoreLimit()` / `getPrivateBuyStoreLimit()` — the
/// dwarf/non-dwarf base finalized through `Stat::TradeSell`/`TradeBuy`
/// (Expand Trade), matching `ex_storage_max_count`'s reporting.
fn store_slot_limit(world: &World, owner: i32, sell: bool) -> usize {
    let race = world
        .objects
        .get_component::<crate::model::Player>(&owner)
        .map_or(0, |p| p.race);
    let is_dwarf = race == crate::enums::Race::Dwarf as i32;
    let (stat, base) = if sell {
        (
            crate::model::stats::Stat::TradeSell,
            if is_dwarf { 4 } else { 3 },
        )
    } else {
        (
            crate::model::stats::Stat::TradeBuy,
            if is_dwarf { 5 } else { 4 },
        )
    };
    world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&owner)
        .map_or(base, |m| {
            crate::model::finalize(m, stat, f64::from(base)) as i32
        })
        .max(0) as usize
}

pub(crate) fn handle_set_list(world: &mut World, client_id: u32, body: &[u8]) {
    let Some((packaged, pkt)) = cp::PrivateStoreItemList::read_set_list(body) else {
        return;
    };
    let Some(owner) = player_of(world, client_id) else {
        return;
    };
    // "Check maximum number of allowed slots for pvt shops" — the raw list
    // length against `getPrivateSellStoreLimit()`, refused with SM 1036
    // before any validation, as in Java.
    if pkt.items.len() > store_slot_limit(world, owner, true) {
        send_to_client(
            world,
            client_id,
            sp::system_message_with(
                sp::sm_ids::YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED,
                &[],
            ),
        );
        return;
    }
    // Java `Item.addToTradeList`'s `(MAX_ADENA / count) < price` per-line
    // guard, then the running total seeded with the seller's held adena
    // (`totalCost = player.getAdena()` — Java counts the coins already in the
    // pocket toward the cap). Either overflow punishes.
    let mut total: i64 = world
        .objects
        .get_component::<Inventory>(&owner)
        .map(|inv| inv.count_of(ADENA_ID))
        .unwrap_or(0);
    for (_, count, price) in &pkt.items {
        if (*count > 0 && (MAX_ADENA / count) < *price) || {
            total = total.saturating_add(count.saturating_mul(*price));
            total > MAX_ADENA
        } {
            super::punishment::illegal_action(
                world,
                owner,
                &format!(
                    "Player {owner} tried to set price more than {MAX_ADENA} adena in Private Store - Sell."
                ),
            );
            return;
        }
    }
    let mut items = Vec::new();
    for (obj_id, count, price) in pkt.items {
        let Some(inv) = world.objects.get_component::<Inventory>(&owner) else {
            return;
        };
        let Some(inst) = inv.by_object_id(obj_id) else {
            continue;
        };
        // Can't sell equipped/quest/untradable items, or more than held.
        if inv.paperdoll_slot_of(obj_id).is_some()
            || world
                .data
                .item_data
                .get(inst.item_id)
                .is_some_and(|t| t.is_quest_item || !t.is_tradable())
        {
            continue;
        }
        let count = count.min(inst.count);
        items.push(StoreItem {
            object_id: obj_id,
            item_id: inst.item_id,
            count,
            price,
            enchant: inst.enchant_level,
        });
    }
    if items.is_empty() {
        return;
    }
    let title = world
        .objects
        .get_component::<PrivateStore>(&owner)
        .map(|s| s.title.clone())
        .unwrap_or_default();
    world.objects.add_components(
        &owner,
        PrivateStore {
            items,
            title: title.clone(),
            packaged,
        },
    );
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&owner)
    {
        p.store_type = if packaged {
            STORE_TYPE_PACKAGE_SELL
        } else {
            STORE_TYPE_SELL
        };
    }
    // Java broadcasts `ExPrivateStoreSetWholeMsg` for a package store and
    // `PrivateStoreMsgSell` for a normal one — the two title packets the client
    // renders above the seller's head.
    broadcast_store(world, owner, &title, packaged);
}

/// `RequestPrivateStoreQuitSell` (0x96): close the store.
pub(crate) fn handle_quit(world: &mut World, client_id: u32) {
    let Some(owner) = player_of(world, client_id) else {
        return;
    };
    close_store(world, owner);
}

/// A customer clicked the store owner (`Action`): show them the store.
pub(crate) fn open_buyer_view(world: &mut World, client_id: u32, buyer: i32, seller: i32) {
    if !world
        .objects
        .get_component::<crate::model::Player>(&seller)
        .is_some_and(|p| is_sell_store(p.store_type))
    {
        return;
    }
    let packaged = world
        .objects
        .get_component::<PrivateStore>(&seller)
        .is_some_and(|s| s.packaged);
    let lines = store_lines(world, seller);
    let packet = sp::list_sell(seller, adena(world, buyer), &lines, packaged);
    send_to_client(world, client_id, packet);
}

/// `RequestPrivateStoreBuy` (0x83): a customer buys items from `seller`'s store —
/// items move seller→buyer, adena buyer→seller. The store closes when emptied.
pub(crate) fn handle_buy(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::PrivateStoreItemList::read_buy(body) else {
        return;
    };
    let Some(buyer) = player_of(world, client_id) else {
        return;
    };
    // Java `RequestPrivateStoreBuy`: a cursed wielder can't buy from a store.
    if super::cursed_weapon::is_cursed(world, buyer) {
        return;
    }
    let seller = pkt.target_object_id;
    if seller == buyer
        || !world
            .objects
            .get_component::<crate::model::Player>(&seller)
            .is_some_and(|p| is_sell_store(p.store_type))
    {
        return;
    }
    // Java: a package store is all-or-nothing — asking for fewer lines than it
    // holds is treated as a bot signature and punished.
    let package_short = world
        .objects
        .get_component::<PrivateStore>(&seller)
        .is_some_and(|s| s.packaged && s.items.len() > pkt.items.len());
    if package_short {
        super::punishment::illegal_action(
            world,
            buyer,
            &format!(
                "[RequestPrivateStoreBuy] player {buyer} tried to buy less items than sold by package-sell, ban this player for bot usage!"
            ),
        );
        send_to_client(world, client_id, sp::action_failed());
        return;
    }
    // Match each requested line against the live store + verify the seller still
    // holds the item, and total the price.
    let mut buys: Vec<(i32, i32, i64, i32)> = Vec::new(); // (obj_id, item_id, count, enchant)
    let mut total: i64 = 0;
    for (obj_id, count, price) in &pkt.items {
        let Some(store) = world.objects.get_component::<PrivateStore>(&seller) else {
            return;
        };
        let Some(line) = store
            .items
            .iter()
            .find(|s| s.object_id == *obj_id && s.price == *price)
        else {
            continue;
        };
        let held = world
            .objects
            .get_component::<Inventory>(&seller)
            .and_then(|inv| {
                inv.items()
                    .iter()
                    .find(|it| it.object_id == *obj_id)
                    .map(|it| it.count)
            })
            .unwrap_or(0);
        let n = (*count).min(line.count).min(held);
        if n <= 0 {
            continue;
        }
        total = total.saturating_add(line.price * n);
        buys.push((*obj_id, line.item_id, n, line.enchant));
    }
    if buys.is_empty() || adena(world, buyer) < total {
        send_to_client(
            world,
            client_id,
            sp::system_message_with(sp::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA, &[]),
        );
        return;
    }
    // Move the items seller → buyer.
    let mut seller_changes = Vec::new();
    for &(obj_id, item_id, n, enchant) in &buys {
        if let Some(change) = world
            .objects
            .get_component_mut::<Inventory>(&seller)
            .and_then(|inv| inv.remove_by_object_id(obj_id, n))
        {
            seller_changes.push(change);
        }
        // Buyer gets a fresh instance preserving enchant.
        if let Some(new_oid) = world.alloc_object_id()
            && let Some(inv) = world.objects.get_component_mut::<Inventory>(&buyer)
        {
            // `mana` -1: a private store only moves tradable items, and every
            // shadow item is `is_tradable="false"`, so none can reach here.
            inv.insert_instance(&world.data.item_data, new_oid, item_id, n, enchant, -1);
        }
        // Reduce the store line.
        if let Some(store) = world.objects.get_component_mut::<PrivateStore>(&seller) {
            if let Some(line) = store.items.iter_mut().find(|s| s.object_id == obj_id) {
                line.count -= n;
            }
            store.items.retain(|s| s.count > 0);
        }
    }
    // Adena buyer → seller.
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&buyer) {
        inv.remove_item(ADENA_ID, total);
    }
    super::items::add_inventory_item(world, seller, ADENA_ID, total);

    // Refresh both inventories.
    send_inventory_item_list(world, buyer);
    send_inventory_item_list(world, seller);
    let _ = seller_changes;

    // Close the store if empty, else re-show the buyer view.
    let empty = world
        .objects
        .get_component::<PrivateStore>(&seller)
        .is_none_or(|s| s.items.is_empty());
    if empty {
        close_store(world, seller);
    } else {
        open_buyer_view(world, client_id, buyer, seller);
    }
    // Java `RequestPrivateStoreBuy`: `onTransaction(storePlayer, itemCount == 0,
    // false)` — an unattended shop's rows follow every sale.
    super::offline_trade::on_transaction(world, seller);
}

// --- helpers ---

/// Build the store's item lines (instance + template + price) for a packet.
fn store_lines(world: &World, owner: i32) -> Vec<StoreLine<'_>> {
    world
        .objects
        .get_component::<PrivateStore>(&owner)
        .map(|store| {
            store
                .items
                .iter()
                .filter_map(|s| {
                    let t = world.data.item_data.get(s.item_id)?;
                    Some(StoreLine {
                        item: instance(s.object_id, s.item_id, s.count, s.enchant),
                        template: t,
                        price: s.price,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Activate the store visually: the store title to self + nearby, and a CharInfo
/// re-send so the store byte / sitting shows on other clients.
fn broadcast_store(world: &mut World, owner: i32, title: &str, packaged: bool) {
    let packet = if packaged {
        sp::ex_private_store_whole_msg(owner, title)
    } else {
        sp::msg_sell(owner, title)
    };
    super::helpers::broadcast_including_self(world, owner, &packet);
    super::party::broadcast_user_info(world, owner);
}

/// Clear the store, drop the store byte, and re-broadcast.
/// `setPrivateStoreType(NONE)` from outside the store handlers — the seated
/// shopkeeper who just took a hit (Java `PlayerStatus.reduceHp`). Routes to
/// whichever store kind is actually open so the right close packet goes out.
pub(crate) fn close_any_store(world: &mut World, owner: i32) {
    if is_buy_store_owner(world, owner)
        || world
            .objects
            .get_component::<crate::model::Player>(&owner)
            .is_some_and(|p| p.store_type == STORE_TYPE_BUY_MANAGE)
    {
        close_buy_store(world, owner);
    } else {
        close_store(world, owner);
    }
}

fn close_store(world: &mut World, owner: i32) {
    world.objects.remove_component::<PrivateStore>(&owner);
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&owner)
    {
        p.store_type = 0;
    }
    super::helpers::broadcast_including_self(world, owner, &sp::msg_sell(owner, ""));
    super::party::broadcast_user_info(world, owner);
    // Java `Player.setPrivateStoreType(NONE)` → `OFFLINE_DISCONNECT_FINISHED`:
    // an unattended shop that sold out leaves the world.
    super::offline_trade::on_store_type_cleared(world, owner);
}

/// Whether the object is a store owner (for `Action` routing).
pub(crate) fn is_store_owner(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::Player>(&oid)
        .map(|p| p.store_type)
        == Some(STORE_TYPE_SELL)
}

// ---------------------------------------------------------------------------
// Private BUY stores (`PrivateStoreType.BUY`)
// ---------------------------------------------------------------------------
//
// The mirror image of the sell store: the owner posts what they *want* and
// sits on the adena; a customer walks up and sells into it. The wanted lines
// are keyed by item id (the owner holds nothing yet), and the owner's adena is
// checked when the store opens and again per sale — Java re-checks because the
// owner can spend elsewhere while the store stands.

use crate::game_loop::helpers::region_cell_of;
use crate::model::components::{PrivateBuyStore, WantedItem};

const STORE_TYPE_BUY: u8 = 3;
const STORE_TYPE_BUY_MANAGE: u8 = 4;
/// `Inventory.MAX_ADENA`.
const MAX_ADENA: i64 = 99_900_000_000;

/// `RequestPrivateStoreManageBuy` (0x99) → `Player.tryOpenPrivateBuyStore`:
/// open the setup window (the owner's inventory as a price reference, plus
/// whatever is already on the wanted list) and flag them BUY_MANAGE.
pub(crate) fn open_manage_buy(world: &mut World, client_id: u32) {
    let Some(owner) = player_of(world, client_id) else {
        return;
    };
    // Java: an already-open buy store is torn down first (`setPrivateStoreType
    // (NONE)`), so re-opening the manage window closes the live store.
    if world
        .objects
        .get_component::<crate::model::Player>(&owner)
        .map(|p| p.store_type)
        == Some(STORE_TYPE_BUY)
    {
        close_buy_store(world, owner);
    }
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&owner)
    {
        p.store_type = STORE_TYPE_BUY_MANAGE;
    }
    send_manage_buy_window(world, client_id);
}

/// Re-send `PrivateStoreManageListBuy` **without** touching the store type —
/// what Java does on every `SetPrivateStoreListBuy` refusal (the player is
/// already in BUY_MANAGE from opening the window).
fn send_manage_buy_window(world: &mut World, client_id: u32) {
    let Some(owner) = player_of(world, client_id) else {
        return;
    };
    let packet = {
        let Some(inv) = world.objects.get_component::<Inventory>(&owner) else {
            return;
        };
        let inventory: Vec<StoreLine> = inv
            .items()
            .iter()
            .filter_map(|it| {
                let t = world.data.item_data.get(it.item_id)?;
                // Java `getUniqueItems(false, true)` → `isSellable() &&
                // isAvailable(..., allowNonTradeable = false)`, i.e. untradable
                // items are left out of the reference list too.
                (!t.is_quest_item && t.is_tradable()).then_some(StoreLine {
                    item: *it,
                    template: t,
                    price: 0,
                })
            })
            .collect();
        let wanted = wanted_lines(world, owner);
        sp::manage_list_buy(owner, adena(world, owner), &inventory, &wanted)
    };
    send_to_client(world, client_id, packet);
}

/// `SetPrivateStoreListBuy` (0x9A): open the store for business. Java's gates,
/// in order — combat/duel, the store limit, per-line and total price overflow,
/// and "can you actually afford everything you just asked for".
pub(crate) fn handle_set_list_buy(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(owner) = player_of(world, client_id) else {
        return;
    };
    let Some(lines) = cp::PrivateStoreItemList::read_set_list_buy(body) else {
        // Java: a malformed list drops the store type back to NONE.
        close_buy_store(world, owner);
        return;
    };
    // `AttackStanceTaskManager.hasAttackStanceTask(player) || player.isInDuel()`.
    if super::combat::has_attack_stance(world, owner) {
        send_sm(
            world,
            client_id,
            sp::sm_ids::WHILE_YOU_ARE_ENGAGED_IN_COMBAT_YOU_CANNOT_OPERATE_A_PRIVATE_STORE_OR_PRIVATE_WORKSHOP,
        );
        send_manage_buy_window(world, client_id);
        return;
    }
    let limit = private_store_limit(world, owner);
    if lines.len() as i32 > limit {
        send_sm(
            world,
            client_id,
            sp::sm_ids::YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED,
        );
        send_manage_buy_window(world, client_id);
        return;
    }

    let mut items = Vec::with_capacity(lines.len());
    let mut total: i64 = 0;
    for line in &lines {
        if world.data.item_data.get(line.item_id).is_none() {
            continue;
        }
        // `(MAX_ADENA / count) < price` — the per-line overflow guard; either
        // overflow punishes (Java `handleIllegalPlayerAction`).
        if line.count > 0 && (MAX_ADENA / line.count) < line.price {
            super::punishment::illegal_action(
                world,
                owner,
                &format!(
                    "Player {owner} tried to set price more than {MAX_ADENA} adena in Private Store - Buy."
                ),
            );
            return;
        }
        total = total.saturating_add(line.count.saturating_mul(line.price));
        if total > MAX_ADENA {
            super::punishment::illegal_action(
                world,
                owner,
                &format!(
                    "Player {owner} tried to set total price more than {MAX_ADENA} adena in Private Store - Buy."
                ),
            );
            return;
        }
        items.push(WantedItem {
            item_id: line.item_id,
            count: line.count,
            price: line.price,
            enchant: line.enchant,
        });
    }
    if items.is_empty() {
        close_buy_store(world, owner);
        return;
    }
    // "The purchase price is higher than the amount of money that you have."
    if total > adena(world, owner) {
        send_sm(
            world,
            client_id,
            sp::sm_ids::THE_PURCHASE_PRICE_IS_HIGHER_THAN_YOUR_MONEY,
        );
        send_manage_buy_window(world, client_id);
        return;
    }

    let title = world
        .objects
        .get_component::<PrivateBuyStore>(&owner)
        .map(|s| s.title.clone())
        .unwrap_or_default();
    world.objects.add_components(
        &owner,
        PrivateBuyStore {
            items,
            title: title.clone(),
        },
    );
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&owner)
    {
        p.store_type = STORE_TYPE_BUY;
    }
    // Java `sitDown()` then broadcasts the type + the title.
    super::helpers::broadcast_including_self(world, owner, &sp::msg_buy(owner, &title));
    super::party::broadcast_user_info(world, owner);
}

/// `RequestPrivateStoreQuitBuy` (0x9C).
pub(crate) fn handle_quit_buy(world: &mut World, client_id: u32) {
    let Some(owner) = player_of(world, client_id) else {
        return;
    };
    close_buy_store(world, owner);
}

/// `SetPrivateStoreMsgBuy` (0x9D) / `SetPrivateStoreMsgSell` (0x97): name the
/// store. The title is kept on the component and re-broadcast.
pub(crate) fn handle_set_msg(world: &mut World, client_id: u32, body: &[u8], buy: bool) {
    let Some(owner) = player_of(world, client_id) else {
        return;
    };
    let title = commons::network::PacketReader::new(body)
        .read_string()
        .unwrap_or_default();
    // Java `MAX_MSG_LENGTH = 29` — an over-long title punishes.
    if title.chars().count() > 29 {
        let store = if buy { "buy" } else { "sell" };
        super::punishment::illegal_action(
            world,
            owner,
            &format!("Player {owner} tried to overflow private store {store} message"),
        );
        return;
    }
    if buy {
        if let Some(store) = world.objects.get_component_mut::<PrivateBuyStore>(&owner) {
            store.title = title.clone();
        }
        super::helpers::broadcast_including_self(world, owner, &sp::msg_buy(owner, &title));
    } else {
        if let Some(store) = world.objects.get_component_mut::<PrivateStore>(&owner) {
            store.title = title.clone();
        }
        super::helpers::broadcast_including_self(world, owner, &sp::msg_sell(owner, &title));
    }
}

/// `SetPrivateStoreWholeMsg` (ex 0x47): the **package** store's title. Java
/// stores it on the sell list and echoes `ExPrivateStoreSetWholeMsg` to the
/// owner only (the broadcast to bystanders happens when the store opens).
pub(crate) fn handle_set_whole_msg(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(owner) = player_of(world, client_id) else {
        return;
    };
    let title = commons::network::PacketReader::new(body)
        .read_string()
        .unwrap_or_default();
    // Java's `MAX_MSG_LENGTH` overflow check punishes.
    if title.chars().count() > 29 {
        super::punishment::illegal_action(
            world,
            owner,
            &format!("Player {owner} tried to overflow private store whole message"),
        );
        return;
    }
    if let Some(store) = world.objects.get_component_mut::<PrivateStore>(&owner) {
        store.title = title.clone();
    } else {
        world.objects.add_components(
            &owner,
            PrivateStore {
                title: title.clone(),
                ..Default::default()
            },
        );
    }
    send_to_client(
        world,
        client_id,
        sp::ex_private_store_whole_msg(owner, &title),
    );
}

/// A customer clicked a buy-store owner: show them what is wanted. Java sends
/// only the lines the viewer can fill (`getAvailableItems(inventory)`).
pub(crate) fn open_seller_view(world: &mut World, client_id: u32, viewer: i32, owner: i32) {
    if world
        .objects
        .get_component::<crate::model::Player>(&owner)
        .map(|p| p.store_type)
        != Some(STORE_TYPE_BUY)
    {
        return;
    }
    let lines = wanted_lines(world, owner)
        .into_iter()
        .filter(|line| {
            world
                .objects
                .get_component::<Inventory>(&viewer)
                .is_some_and(|inv| {
                    inv.items().iter().any(|it| {
                        it.item_id == line.item.item_id
                            && inv.paperdoll_slot_of(it.object_id).is_none()
                    })
                })
        })
        .collect::<Vec<_>>();
    let packet = sp::list_buy(owner, adena(world, viewer), &lines);
    send_to_client(world, client_id, packet);
}

/// `RequestPrivateStoreSell` (0x9F): the customer hands items over and takes
/// the owner's adena. Items customer → owner, adena owner → customer; the
/// store closes once every line is filled.
pub(crate) fn handle_store_sell(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::PrivateStoreItemList::read_store_sell(body) else {
        return;
    };
    let Some(seller) = player_of(world, client_id) else {
        return;
    };
    // Java `RequestPrivateStoreSell`: nor sell into a buy-store.
    if super::cursed_weapon::is_cursed(world, seller) {
        return;
    }
    let owner = pkt.store_player;
    if owner == seller
        || world
            .objects
            .get_component::<crate::model::Player>(&owner)
            .map(|p| p.store_type)
            != Some(STORE_TYPE_BUY)
    {
        return;
    }
    // Java `isInsideRadius3D(storePlayer, INTERACTION_DISTANCE)`.
    if !super::target::can_interact(world, seller, owner) {
        return;
    }

    // Match each offered line against a live wanted line + the seller's actual
    // holdings, and total what the owner owes.
    let mut sales: Vec<(i32, i32, i64, i64)> = Vec::new(); // (obj, item, count, price)
    let mut total: i64 = 0;
    for line in &pkt.items {
        let Some(store) = world.objects.get_component::<PrivateBuyStore>(&owner) else {
            return;
        };
        let Some(wanted) = store
            .items
            .iter()
            .find(|w| w.item_id == line.item_id && w.price == line.price)
        else {
            continue;
        };
        // Java `TradeList.privateStoreSell`: `if (!oldItem.isTradeable())
        // return null` — an untradable item can't be sold into a buy store
        // either, whatever the store advertises.
        if world
            .data
            .item_data
            .get(line.item_id)
            .is_some_and(|t| !t.is_tradable())
        {
            continue;
        }
        let held = world
            .objects
            .get_component::<Inventory>(&seller)
            .and_then(|inv| {
                inv.items()
                    .iter()
                    .find(|it| it.object_id == line.object_id && it.item_id == line.item_id)
                    .filter(|it| inv.paperdoll_slot_of(it.object_id).is_none())
                    .map(|it| it.count)
            })
            .unwrap_or(0);
        let n = line.count.min(wanted.count).min(held);
        if n <= 0 {
            continue;
        }
        total = total.saturating_add(wanted.price * n);
        sales.push((line.object_id, line.item_id, n, wanted.price));
    }
    if sales.is_empty() {
        return;
    }
    // The owner may have spent their adena elsewhere since opening the store.
    if adena(world, owner) < total {
        send_sm(world, client_id, sp::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA);
        return;
    }

    for &(obj_id, item_id, n, _price) in &sales {
        if world
            .objects
            .get_component_mut::<Inventory>(&seller)
            .and_then(|inv| inv.remove_by_object_id(obj_id, n))
            .is_none()
        {
            continue;
        }
        let enchant = world
            .objects
            .get_component::<PrivateBuyStore>(&owner)
            .and_then(|s| s.items.iter().find(|w| w.item_id == item_id))
            .map(|w| w.enchant)
            .unwrap_or(0);
        if let Some(new_oid) = world.alloc_object_id()
            && let Some(inv) = world.objects.get_component_mut::<Inventory>(&owner)
        {
            // `mana` -1: a private store only moves tradable items, and every
            // shadow item is `is_tradable="false"`, so none can reach here.
            inv.insert_instance(&world.data.item_data, new_oid, item_id, n, enchant, -1);
        }
        if let Some(store) = world.objects.get_component_mut::<PrivateBuyStore>(&owner) {
            if let Some(w) = store.items.iter_mut().find(|w| w.item_id == item_id) {
                w.count -= n;
            }
            store.items.retain(|w| w.count > 0);
        }
    }
    // Adena owner → seller.
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&owner) {
        inv.remove_item(ADENA_ID, total);
    }
    super::items::add_inventory_item(world, seller, ADENA_ID, total);

    send_inventory_item_list(world, seller);
    send_inventory_item_list(world, owner);

    let empty = world
        .objects
        .get_component::<PrivateBuyStore>(&owner)
        .is_none_or(|s| s.items.is_empty());
    if empty {
        close_buy_store(world, owner);
    } else {
        open_seller_view(world, client_id, seller, owner);
    }
    // `RequestPrivateStoreSell`'s `onTransaction(storePlayer, …)`.
    super::offline_trade::on_transaction(world, owner);
}

/// Whether the object is a buy-store owner (for `Action` routing).
pub(crate) fn is_buy_store_owner(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::Player>(&oid)
        .map(|p| p.store_type)
        == Some(STORE_TYPE_BUY)
}

/// The wanted lines as packet rows. The item block wants an instance, so each
/// line is described by a synthetic one (object id 0 — nothing owns it yet).
fn wanted_lines(world: &World, owner: i32) -> Vec<StoreLine<'_>> {
    world
        .objects
        .get_component::<PrivateBuyStore>(&owner)
        .map(|store| {
            store
                .items
                .iter()
                .filter_map(|w| {
                    let t = world.data.item_data.get(w.item_id)?;
                    Some(StoreLine {
                        item: instance(0, w.item_id, w.count, w.enchant),
                        template: t,
                        price: w.price,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `Player.getPrivateBuyStoreLimit()` — 5 lines for a Dwarf, 4 for everyone
/// else on this dist.
fn private_store_limit(world: &World, owner: i32) -> i32 {
    let dwarf = world
        .objects
        .get_component::<crate::model::Player>(&owner)
        .and_then(|p| crate::enums::Race::from_ordinal(p.race))
        .is_some_and(|r| r == crate::enums::Race::Dwarf);
    if dwarf {
        world.cfg.character.max_pvtstore_buy_slots_dwarf
    } else {
        world.cfg.character.max_pvtstore_buy_slots_other
    }
}

fn close_buy_store(world: &mut World, owner: i32) {
    world.objects.remove_component::<PrivateBuyStore>(&owner);
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&owner)
    {
        p.store_type = 0;
    }
    super::helpers::broadcast_including_self(world, owner, &sp::msg_buy(owner, ""));
    super::party::broadcast_user_info(world, owner);
    super::offline_trade::on_store_type_cleared(world, owner);
}

/// Java `Player.canOpenPrivateStore` — the shared gate on every
/// `tryOpenPrivateXStore`. Its first half is the `Custom/PrivateStoreRange.ini`
/// spacing rule: nothing within 1000 units may have a **minimum shop distance**
/// that the owner is standing inside. Java gets that number from
/// `getMinShopDistance()`, which is `ShopMinRangeFromNpc` for any NPC and
/// `ShopMinRangeFromPlayer` for a player **only while seated** — i.e. one who
/// already has a store up — so the rule spaces shops apart rather than blocking
/// on any passer-by.
///
/// The second half is the state check. The `NO_STORE` zone kind is not loaded,
/// so that one leg is absent; the rest — including `_isSellingBuffs`, which
/// landed with the sell-buffs slice — are here.
pub(crate) fn can_open_private_store(world: &World, client_id: u32, owner: i32) -> bool {
    // Java `!_isSellingBuffs` — a buff shop and an ordinary store are mutually
    // exclusive, and both ride the same `PACKAGE_SELL` store type.
    if super::sell_buffs::is_selling(world, owner) {
        return false;
    }
    let cfg = &world.cfg.custom_misc;
    if cfg.shop_min_range_from_npc > 0 || cfg.shop_min_range_from_player > 0 {
        let Some(pos) = maybe_position(world, owner) else {
            return false;
        };
        let too_close = |other: i32, min_distance: i32| {
            if min_distance <= 0 {
                return false;
            }
            crate::geo::distance::within_3d_xyz(
                world,
                other,
                pos.x,
                pos.y,
                pos.z,
                f64::from(min_distance),
            )
        };
        // Java sweeps `getVisibleObjectsInRange(this, Creature.class, 1000)`;
        // the port's equivalent neighbourhood is the 3×3 region block, the same
        // sweep the NPC AI uses for its own range queries.
        let Some(region) = region_cell_of(world, owner) else {
            return false;
        };
        let nearby_npcs = world.npcs_visible_from(region);
        for npc in nearby_npcs {
            if too_close(npc, cfg.shop_min_range_from_npc) {
                send_cannot_open_here(world, client_id);
                return false;
            }
        }
        for cs in world.clients.values() {
            let crate::session::ClientSession::InGame(s) = cs else {
                continue;
            };
            let other = s.player_object_id();
            // `Player.getMinShopDistance()` is non-zero only while seated.
            let seated = world
                .objects
                .get_component::<crate::model::Player>(&other)
                .is_some_and(|p| p.sitting);
            if other != owner && seated && too_close(other, cfg.shop_min_range_from_player) {
                send_cannot_open_here(world, client_id);
                return false;
            }
        }
    }
    world
        .objects
        .get_component::<crate::model::components::Vitals>(&owner)
        .is_some_and(|v| !v.dead)
        && !world
            .objects
            .get_component::<crate::model::Player>(&owner)
            .is_some_and(crate::model::Player::is_mounted)
        && !world.olympiad.in_competition.contains(&owner)
        && !world
            .objects
            .has_component::<crate::model::components::Casting>(&owner)
}

fn send_cannot_open_here(world: &World, client_id: u32) {
    send_to_client(
        world,
        client_id,
        sp::system_message_with(sp::sm_ids::YOU_CANNOT_OPEN_A_PRIVATE_STORE_HERE, &[]),
    );
}
