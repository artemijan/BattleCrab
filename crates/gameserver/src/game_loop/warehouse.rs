//! Warehouse windows (`PrivateWarehouse`/`ClanWarehouse` bypass +
//! `SendWareHouse*List`): open the deposit/withdraw windows and move items
//! between the player's [`Inventory`] and the active [`Warehouse`].
//!
//! Two warehouses share this code, distinguished by the player's
//! [`ActiveWarehouse`] (set by the bypass, since the client packets carry no
//! type): the personal warehouse is a `Warehouse` component on the player; the
//! clan warehouse lives on [`Clan`](crate::model::clan::Clan) in `world.clans`
//! and is shared by every online member. Both persist — the personal one via
//! `net::build_save_data`, the clan one via [`persist_clan_warehouse`] on every
//! change.

use crate::model::components::ActiveWarehouse;
use crate::model::inventory::{Inventory, ItemInstance, Warehouse};
use crate::model::Player;
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

/// The clan id whose warehouse `player_oid` is currently using, if their
/// active warehouse is the clan one and they're in a clan.
fn active_clan_id(world: &World, player_oid: i32) -> Option<i32> {
    let active = world.objects.get_component::<ActiveWarehouse>(&player_oid).copied().unwrap_or_default();
    if active != ActiveWarehouse::Clan {
        return None;
    }
    world.objects.get_component::<Player>(&player_oid).map(|p| p.clan_id).filter(|&c| c != 0)
}

/// Read-only view of the player's active warehouse container.
fn warehouse_ref(world: &World, player_oid: i32, clan_id: Option<i32>) -> Option<&Warehouse> {
    match clan_id {
        Some(clan_id) => world.clans.get(&clan_id).map(|c| &c.warehouse),
        None => world.objects.get_component::<Warehouse>(&player_oid),
    }
}

fn wh_type(clan_id: Option<i32>) -> i16 {
    if clan_id.is_some() {
        sp::WH_TYPE_CLAN
    } else {
        sp::WH_TYPE_PRIVATE
    }
}

/// Mark the player's active warehouse (the bypass sets this before opening a
/// window; the deposit/withdraw handlers read it).
pub(crate) fn set_active(world: &mut World, player_oid: i32, active: ActiveWarehouse) {
    world.objects.add_components(&player_oid, active);
}

/// `ClanWarehouse` bypass (`depositc`/`withdrawc`): gate on clan membership,
/// clan level ≥ 1, and — for withdraw — the `CL_VIEW_WAREHOUSE` privilege
/// (Java `ClanWarehouse.useBypass`), then set the active warehouse to the clan
/// one and open the window. `player_oid` doubles as the char id (persistent
/// object ids).
pub(crate) fn open_clan(world: &mut World, client_id: u32, player_oid: i32, withdraw: bool) {
    let Some(player) = world.objects.get_component::<Player>(&player_oid) else { return };
    let clan_id = player.clan_id;
    let privs = player.clan_privs;
    if clan_id == 0 {
        return; // not in a clan
    }
    let Some(clan) = world.clans.get(&clan_id) else { return };
    if clan.level == 0 {
        return; // "only clans of level 1+ can use a clan warehouse"
    }
    if withdraw && !clan.has_privilege(player_oid, privs, crate::model::clan::CL_VIEW_WAREHOUSE) {
        return; // no CL_VIEW_WAREHOUSE right
    }
    set_active(world, player_oid, ActiveWarehouse::Clan);
    if withdraw {
        open_withdraw_window(world, client_id);
    } else {
        open_deposit_window(world, client_id);
    }
}

/// `DepositP`/`DepositC` — show the deposit window (the inventory items that
/// can go in).
pub(crate) fn open_deposit_window(world: &mut World, client_id: u32) {
    let Some(player_oid) = player_of(world, client_id) else { return };
    let clan_id = active_clan_id(world, player_oid);
    let wh_size = warehouse_ref(world, player_oid, clan_id).map(Warehouse::size).unwrap_or(0) as i32;
    let Some(inv) = world.objects.get_component::<Inventory>(&player_oid) else { return };
    // Depositable = not equipped (Java `getAvailableItems`).
    let items: Vec<(&ItemInstance, &crate::data::item_data::ItemTemplate)> = inv
        .items()
        .iter()
        .filter(|it| inv.paperdoll_slot_of(it.object_id).is_none())
        .filter_map(|it| world.data.item_data.get(it.item_id).map(|t| (it, t)))
        .collect();
    let packet = sp::warehouse_deposit_list(wh_type(clan_id), adena(world, player_oid), wh_size, &items);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// `WithdrawP`/`WithdrawC` — show the withdraw window (the warehouse contents).
pub(crate) fn open_withdraw_window(world: &mut World, client_id: u32) {
    let Some(player_oid) = player_of(world, client_id) else { return };
    let clan_id = active_clan_id(world, player_oid);
    let inv_size = world.objects.get_component::<Inventory>(&player_oid).map(|i| i.items().len()).unwrap_or(0) as i32;
    let Some(wh) = warehouse_ref(world, player_oid, clan_id) else { return };
    let items: Vec<(&ItemInstance, &crate::data::item_data::ItemTemplate)> = wh
        .0
        .items()
        .iter()
        .filter_map(|it| world.data.item_data.get(it.item_id).map(|t| (it, t)))
        .collect();
    let packet = sp::warehouse_withdrawal_list(wh_type(clan_id), adena(world, player_oid), inv_size, &items);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// `SendWareHouseDepositList` (0x3B): move the named items inventory → warehouse.
pub(crate) fn handle_deposit(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::WarehouseItemList::read(body) else { return };
    let Some(player_oid) = player_of(world, client_id) else { return };
    let mut moved = false;
    for (obj_id, count) in pkt.items {
        moved |= transfer(world, player_oid, obj_id, count, true);
    }
    if moved {
        maybe_persist_clan(world, player_oid);
    }
    send_inventory(world, client_id, player_oid);
    open_deposit_window(world, client_id);
}

/// `SendWareHouseWithDrawList` (0x3C): move the named items warehouse → inventory.
pub(crate) fn handle_withdraw(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::WarehouseItemList::read(body) else { return };
    let Some(player_oid) = player_of(world, client_id) else { return };
    let mut moved = false;
    for (obj_id, count) in pkt.items {
        moved |= transfer(world, player_oid, obj_id, count, false);
    }
    if moved {
        maybe_persist_clan(world, player_oid);
    }
    send_inventory(world, client_id, player_oid);
    open_withdraw_window(world, client_id);
}

/// Move `count` of the instance `obj_id` between the inventory and the active
/// warehouse. `deposit` = inventory → warehouse, else warehouse → inventory.
/// Preserves enchant; quest/equipped items can't be deposited. Returns whether
/// anything moved.
fn transfer(world: &mut World, player_oid: i32, obj_id: i32, count: i64, deposit: bool) -> bool {
    if count <= 0 {
        return false;
    }
    let clan_id = active_clan_id(world, player_oid);

    // Read the source instance's facts from whichever container it's in.
    let src_facts = {
        let src: Option<&Inventory> = if deposit {
            world.objects.get_component::<Inventory>(&player_oid)
        } else {
            warehouse_ref(world, player_oid, clan_id).map(|w| &w.0)
        };
        src.and_then(|c| c.items().iter().find(|it| it.object_id == obj_id).map(|it| (it.item_id, it.count, it.enchant_level)))
    };
    let Some((item_id, held, enchant)) = src_facts else { return false };
    // Depositing: refuse equipped / quest items (Java `isDepositable`).
    if deposit {
        let equipped = world.objects.get_component::<Inventory>(&player_oid).is_some_and(|inv| inv.paperdoll_slot_of(obj_id).is_some());
        let quest = world.data.item_data.get(item_id).is_some_and(|t| t.is_quest_item);
        if equipped || quest {
            return false;
        }
    }
    let move_count = count.min(held);
    // Does the destination already hold a stack to merge into? (stackables only)
    let stackable = world.data.item_data.get(item_id).is_some_and(|t| t.is_stackable);
    let dst_has_stack = {
        let has = |c: &Inventory| c.items().iter().any(|it| it.item_id == item_id);
        if deposit {
            warehouse_ref(world, player_oid, clan_id).is_some_and(|w| has(&w.0))
        } else {
            world.objects.get_component::<Inventory>(&player_oid).is_some_and(has)
        }
    };
    // A new destination stack/instance needs a fresh object id (the source may
    // keep a partial stack, so its id can't be reused).
    let dst_oid = if stackable && dst_has_stack {
        0 // merged — id unused
    } else {
        let Some(id) = world.alloc_object_id() else { return false };
        id
    };

    // Apply: remove from source, insert into destination. The clan warehouse
    // lives in `world.clans` (a different field from `world.objects`), so its
    // borrow splits cleanly from the inventory's; the personal warehouse is a
    // component on the same object, needing `get_many_mut`.
    match clan_id {
        Some(clan_id) => {
            let World { objects, clans, data, .. } = world;
            let (Some(inv), Some(clan)) = (objects.get_component_mut::<Inventory>(&player_oid), clans.get_mut(&clan_id)) else {
                return false;
            };
            apply_move(inv, &mut clan.warehouse, &data.item_data, obj_id, item_id, move_count, enchant, dst_oid, deposit);
        }
        None => {
            let catalog = &world.data.item_data;
            let Some((mut inv, mut wh)) = world.objects.get_many_mut::<(&mut Inventory, &mut Warehouse)>(&player_oid) else {
                return false;
            };
            apply_move(&mut inv, &mut wh, catalog, obj_id, item_id, move_count, enchant, dst_oid, deposit);
        }
    }
    true
}

/// Remove `move_count` of `obj_id` from the source container and insert it into
/// the destination (enchant preserved), directed by `deposit`.
#[allow(clippy::too_many_arguments)]
fn apply_move(inv: &mut Inventory, wh: &mut Warehouse, catalog: &crate::data::item_data::ItemData, obj_id: i32, item_id: i32, move_count: i64, enchant: i32, dst_oid: i32, deposit: bool) {
    if deposit {
        inv.remove_by_object_id(obj_id, move_count);
        wh.0.insert_instance(catalog, dst_oid, item_id, move_count, enchant);
    } else {
        wh.0.remove_by_object_id(obj_id, move_count);
        inv.insert_instance(catalog, dst_oid, item_id, move_count, enchant);
    }
}

/// After a clan-warehouse change, flush its contents to the DB.
fn maybe_persist_clan(world: &World, player_oid: i32) {
    if let Some(clan_id) = active_clan_id(world, player_oid) {
        persist_clan_warehouse(world, clan_id);
    }
}

/// Emit a `StoreClanWarehouse` DB command for `clan_id`'s current contents
/// (fire-and-forget delete-then-reinsert, mirroring the player item save).
pub(crate) fn persist_clan_warehouse(world: &World, clan_id: i32) {
    if let Some(clan) = world.clans.get(&clan_id) {
        let items = clan.warehouse.to_rows_clan();
        let _ = world.db.send(crate::db::DbCommand::StoreClanWarehouse { clan_id, items });
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
