//! Private sell stores (`PrivateStoreType.SELL`): a player opens a manage
//! window, sets a list of items+prices, and sits with a titled store; other
//! players click the owner to see the store and buy from it. Items stay in the
//! owner's inventory until sold. Buy/manufacture stores and package sell are out
//! of scope.

use crate::model::components::{PrivateStore, StoreItem};
use crate::model::inventory::{Inventory, ItemInstance};
use crate::network::client_packets as cp;
use crate::network::server_packets::{self as sp, StoreLine};
use crate::session::ClientSession;
use crate::world::World;

const ADENA_ID: i32 = 57;
const STORE_TYPE_SELL: u8 = 1;

fn player_of(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}

fn adena(world: &World, oid: i32) -> i64 {
    world.objects.get_component::<Inventory>(&oid).map(|i| i.adena()).unwrap_or(0)
}

fn instance(object_id: i32, item_id: i32, count: i64, enchant: i32) -> ItemInstance {
    ItemInstance { object_id, item_id, count, enchant_level: enchant, custom_type1: 0, custom_type2: 0, mana_left: -1, time: 0, augment_mineral: 0, augment_option1: 0, augment_option2: 0 }
}

/// `RequestPrivateStoreManageSell` (0x30): open the setup window — the owner's
/// sellable inventory items plus any already in the store.
pub(crate) fn open_manage(world: &mut World, client_id: u32) {
    let Some(owner) = player_of(world, client_id) else { return };
    let Some(inv) = world.objects.get_component::<Inventory>(&owner) else { return };
    let sellable: Vec<StoreLine> = inv
        .items()
        .iter()
        .filter(|it| inv.paperdoll_slot_of(it.object_id).is_none())
        .filter_map(|it| {
            let t = world.data.item_data.get(it.item_id)?;
            (!t.is_quest_item && t.price > 0).then(|| StoreLine { item: *it, template: t, price: 0 })
        })
        .collect();
    let in_store = store_lines(world, owner);
    let packet = sp::manage_list_sell(owner, adena(world, owner), &sellable, &in_store);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// `SetPrivateStoreListSell` (0x31): activate the store with the given
/// items+prices (validated against the owner's inventory). An empty list is a
/// no-op; a valid list sits the owner with a titled store visible to others.
pub(crate) fn handle_set_list(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::PrivateStoreItemList::read_set_list(body) else { return };
    let Some(owner) = player_of(world, client_id) else { return };
    let mut items = Vec::new();
    for (obj_id, count, price) in pkt.items {
        let Some(inv) = world.objects.get_component::<Inventory>(&owner) else { return };
        let Some(inst) = inv.items().iter().find(|it| it.object_id == obj_id) else { continue };
        // Can't sell equipped/quest items, or more than held.
        if inv.paperdoll_slot_of(obj_id).is_some() || world.data.item_data.get(inst.item_id).is_some_and(|t| t.is_quest_item) {
            continue;
        }
        let count = count.min(inst.count);
        items.push(StoreItem { object_id: obj_id, item_id: inst.item_id, count, price, enchant: inst.enchant_level });
    }
    if items.is_empty() {
        return;
    }
    let title = world.objects.get_component::<PrivateStore>(&owner).map(|s| s.title.clone()).unwrap_or_default();
    world.objects.add_components(&owner, PrivateStore { items, title: title.clone() });
    if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&owner) {
        p.store_type = STORE_TYPE_SELL;
    }
    broadcast_store(world, owner, &title);
}

/// `RequestPrivateStoreQuitSell` (0x96): close the store.
pub(crate) fn handle_quit(world: &mut World, client_id: u32) {
    let Some(owner) = player_of(world, client_id) else { return };
    close_store(world, owner);
}

/// A customer clicked the store owner (`Action`): show them the store.
pub(crate) fn open_buyer_view(world: &mut World, client_id: u32, buyer: i32, seller: i32) {
    if world.objects.get_component::<crate::model::Player>(&seller).map(|p| p.store_type) != Some(STORE_TYPE_SELL) {
        return;
    }
    let lines = store_lines(world, seller);
    let packet = sp::list_sell(seller, adena(world, buyer), &lines);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// `RequestPrivateStoreBuy` (0x83): a customer buys items from `seller`'s store —
/// items move seller→buyer, adena buyer→seller. The store closes when emptied.
pub(crate) fn handle_buy(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::PrivateStoreItemList::read_buy(body) else { return };
    let Some(buyer) = player_of(world, client_id) else { return };
    let seller = pkt.target_object_id;
    if seller == buyer || world.objects.get_component::<crate::model::Player>(&seller).map(|p| p.store_type) != Some(STORE_TYPE_SELL) {
        return;
    }
    // Match each requested line against the live store + verify the seller still
    // holds the item, and total the price.
    let mut buys: Vec<(i32, i32, i64, i32)> = Vec::new(); // (obj_id, item_id, count, enchant)
    let mut total: i64 = 0;
    for (obj_id, count, price) in &pkt.items {
        let Some(store) = world.objects.get_component::<PrivateStore>(&seller) else { return };
        let Some(line) = store.items.iter().find(|s| s.object_id == *obj_id && s.price == *price) else { continue };
        let held = world.objects.get_component::<Inventory>(&seller).and_then(|inv| inv.items().iter().find(|it| it.object_id == *obj_id).map(|it| it.count)).unwrap_or(0);
        let n = (*count).min(line.count).min(held);
        if n <= 0 {
            continue;
        }
        total = total.saturating_add(line.price * n);
        buys.push((*obj_id, line.item_id, n, line.enchant));
    }
    if buys.is_empty() || adena(world, buyer) < total {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(sp::system_message_with(sp::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA, &[]));
        }
        return;
    }
    // Move the items seller → buyer.
    let mut seller_changes = Vec::new();
    for &(obj_id, item_id, n, enchant) in &buys {
        if let Some(change) = world.objects.get_component_mut::<Inventory>(&seller).and_then(|inv| inv.remove_by_object_id(obj_id, n)) {
            seller_changes.push(change);
        }
        // Buyer gets a fresh instance preserving enchant.
        if let Some(new_oid) = world.alloc_object_id() {
            if let Some(inv) = world.objects.get_component_mut::<Inventory>(&buyer) {
                inv.insert_instance(&world.data.item_data, new_oid, item_id, n, enchant);
            }
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
    refresh_inventory(world, buyer);
    refresh_inventory(world, seller);
    let _ = seller_changes;

    // Close the store if empty, else re-show the buyer view.
    let empty = world.objects.get_component::<PrivateStore>(&seller).is_none_or(|s| s.items.is_empty());
    if empty {
        close_store(world, seller);
    } else {
        open_buyer_view(world, client_id, buyer, seller);
    }
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
                    Some(StoreLine { item: instance(s.object_id, s.item_id, s.count, s.enchant), template: t, price: s.price })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Activate the store visually: the store title to self + nearby, and a CharInfo
/// re-send so the store byte / sitting shows on other clients.
fn broadcast_store(world: &World, owner: i32, title: &str) {
    super::helpers::broadcast_including_self(world, owner, &sp::msg_sell(owner, title));
    super::party::broadcast_user_info(world, owner);
}

/// Clear the store, drop the store byte, and re-broadcast.
fn close_store(world: &mut World, owner: i32) {
    world.objects.remove_component::<PrivateStore>(&owner);
    if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&owner) {
        p.store_type = 0;
    }
    super::helpers::broadcast_including_self(world, owner, &sp::msg_sell(owner, ""));
    super::party::broadcast_user_info(world, owner);
}

/// Resend a player's inventory window after a store transaction.
fn refresh_inventory(world: &World, oid: i32) {
    if let (Some(cid), Some(inv)) = (super::helpers::client_for_player(world, oid), world.objects.get_component::<Inventory>(&oid)) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(crate::network::enter_world::item_list(inv, &world.data, false));
        }
    }
}

/// Whether the object is a store owner (for `Action` routing).
pub(crate) fn is_store_owner(world: &World, oid: i32) -> bool {
    world.objects.get_component::<crate::model::Player>(&oid).map(|p| p.store_type) == Some(STORE_TYPE_SELL)
}
