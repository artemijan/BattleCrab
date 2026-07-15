//! Personal warehouse (`PrivateWarehouse` bypass + `SendWareHouse*List`): open
//! the deposit/withdraw windows and move items between the player's
//! [`Inventory`] and [`Warehouse`]. Both containers persist together
//! (`net::build_save_data`), so a deposited item survives relog.

use crate::model::inventory::{Inventory, Warehouse};
use crate::network::client_packets as cp;
use crate::network::server_packets as sp;
use crate::session::ClientSession;
use crate::world::World;

const ADENA_ID: i32 = 57;

fn player_of(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}

fn adena(world: &World, player_oid: i32) -> i64 {
    world.objects.get_component::<Inventory>(&player_oid).map(|inv| inv.count_of(ADENA_ID)).unwrap_or(0)
}

/// `DepositP` — show the deposit window (the inventory items that can go in).
pub(crate) fn open_deposit_window(world: &mut World, client_id: u32) {
    let Some(player_oid) = player_of(world, client_id) else { return };
    let wh_size = world.objects.get_component::<Warehouse>(&player_oid).map(Warehouse::size).unwrap_or(0) as i32;
    let Some(inv) = world.objects.get_component::<Inventory>(&player_oid) else { return };
    // Depositable = not equipped (Java `getAvailableItems`).
    let items: Vec<(&crate::model::inventory::ItemInstance, &crate::data::item_data::ItemTemplate)> = inv
        .items()
        .iter()
        .filter(|it| inv.paperdoll_slot_of(it.object_id).is_none())
        .filter_map(|it| world.data.item_data.get(it.item_id).map(|t| (it, t)))
        .collect();
    let packet = sp::warehouse_deposit_list(adena(world, player_oid), wh_size, &items);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// `WithdrawP` — show the withdraw window (the warehouse contents).
pub(crate) fn open_withdraw_window(world: &mut World, client_id: u32) {
    let Some(player_oid) = player_of(world, client_id) else { return };
    let inv_size = world.objects.get_component::<Inventory>(&player_oid).map(|i| i.items().len()).unwrap_or(0) as i32;
    let Some(wh) = world.objects.get_component::<Warehouse>(&player_oid) else { return };
    let items: Vec<(&crate::model::inventory::ItemInstance, &crate::data::item_data::ItemTemplate)> = wh
        .0
        .items()
        .iter()
        .filter_map(|it| world.data.item_data.get(it.item_id).map(|t| (it, t)))
        .collect();
    let packet = sp::warehouse_withdrawal_list(adena(world, player_oid), inv_size, &items);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// `SendWareHouseDepositList` (0x3B): move the named items inventory → warehouse.
pub(crate) fn handle_deposit(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::WarehouseItemList::read(body) else { return };
    let Some(player_oid) = player_of(world, client_id) else { return };
    for (obj_id, count) in pkt.items {
        transfer(world, player_oid, obj_id, count, true);
    }
    send_inventory(world, client_id, player_oid);
    open_deposit_window(world, client_id);
}

/// `SendWareHouseWithDrawList` (0x3C): move the named items warehouse → inventory.
pub(crate) fn handle_withdraw(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::WarehouseItemList::read(body) else { return };
    let Some(player_oid) = player_of(world, client_id) else { return };
    for (obj_id, count) in pkt.items {
        transfer(world, player_oid, obj_id, count, false);
    }
    send_inventory(world, client_id, player_oid);
    open_withdraw_window(world, client_id);
}

/// Move `count` of the instance `obj_id` between the two containers. `deposit`
/// = inventory → warehouse, else warehouse → inventory. Preserves enchant;
/// quest/equipped items can't be deposited.
fn transfer(world: &mut World, player_oid: i32, obj_id: i32, count: i64, deposit: bool) {
    if count <= 0 {
        return;
    }
    // Read the source instance's facts from whichever container it's in.
    let src_facts = {
        let src: Option<&Inventory> = if deposit {
            world.objects.get_component::<Inventory>(&player_oid)
        } else {
            world.objects.get_component::<Warehouse>(&player_oid).map(|w| &w.0)
        };
        src.and_then(|c| c.items().iter().find(|it| it.object_id == obj_id).map(|it| (it.item_id, it.count, it.enchant_level)))
    };
    let Some((item_id, held, enchant)) = src_facts else { return };
    // Depositing: refuse equipped / quest items (Java `isDepositable`).
    if deposit {
        let equipped = world.objects.get_component::<Inventory>(&player_oid).is_some_and(|inv| inv.paperdoll_slot_of(obj_id).is_some());
        let quest = world.data.item_data.get(item_id).is_some_and(|t| t.is_quest_item);
        if equipped || quest {
            return;
        }
    }
    let move_count = count.min(held);
    // Does the destination already hold a stack to merge into? (stackables only)
    let stackable = world.data.item_data.get(item_id).is_some_and(|t| t.is_stackable);
    let dst_has_stack = {
        let has = |c: &Inventory| c.items().iter().any(|it| it.item_id == item_id);
        if deposit {
            world.objects.get_component::<Warehouse>(&player_oid).is_some_and(|w| has(&w.0))
        } else {
            world.objects.get_component::<Inventory>(&player_oid).is_some_and(has)
        }
    };
    // A new destination stack/instance needs a fresh object id (the source may
    // keep a partial stack, so its id can't be reused).
    let dst_oid = if stackable && dst_has_stack {
        0 // merged — id unused
    } else {
        let Some(id) = world.alloc_object_id() else { return };
        id
    };

    // Apply: remove from source, insert into destination.
    if deposit {
        if let Some((mut inv, mut wh)) = world.objects.get_many_mut::<(&mut Inventory, &mut Warehouse)>(&player_oid) {
            inv.remove_by_object_id(obj_id, move_count);
            wh.0.insert_instance(&world.data.item_data, dst_oid, item_id, move_count, enchant);
        }
    } else if let Some((mut inv, mut wh)) = world.objects.get_many_mut::<(&mut Inventory, &mut Warehouse)>(&player_oid) {
        wh.0.remove_by_object_id(obj_id, move_count);
        inv.insert_instance(&world.data.item_data, dst_oid, item_id, move_count, enchant);
    }
}

/// Refresh the client's inventory window after a transfer (full `ItemList`).
fn send_inventory(world: &World, client_id: u32, player_oid: i32) {
    if let Some(inv) = world.objects.get_component::<Inventory>(&player_oid) {
        let packet = crate::network::enter_world::item_list(inv, &world.data, false);
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(packet);
        }
    }
}
