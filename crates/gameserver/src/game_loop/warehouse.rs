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

use crate::model::Player;
use crate::model::components::ActiveWarehouse;
use crate::model::inventory::{Freight, Inventory, ItemInstance, Warehouse};
use crate::network::client_packets as cp;
use crate::network::server_packets as sp;
use crate::session::ClientSession;
use crate::world::World;

const ADENA_ID: i32 = 57;

/// The container the player's [`ActiveWarehouse`] currently points at — the
/// personal warehouse (a player component), the shared clan warehouse (in
/// `world.clans`), or the freight (another player component).
#[derive(Clone, Copy, PartialEq, Eq)]
enum WhTarget {
    Private,
    Clan(i32),
    Freight,
}

fn player_of(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}

fn adena(world: &World, player_oid: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&player_oid)
        .map(|inv| inv.count_of(ADENA_ID))
        .unwrap_or(0)
}

/// Resolve the active-warehouse target. A `Clan` selection with no (valid) clan
/// falls back to the personal warehouse.
fn target(world: &World, player_oid: i32) -> WhTarget {
    match world
        .objects
        .get_component::<ActiveWarehouse>(&player_oid)
        .copied()
        .unwrap_or_default()
    {
        ActiveWarehouse::Private => WhTarget::Private,
        ActiveWarehouse::Freight => WhTarget::Freight,
        ActiveWarehouse::Clan => world
            .objects
            .get_component::<Player>(&player_oid)
            .map(|p| p.clan_id)
            .filter(|&c| c != 0 && world.clans.contains_key(&c))
            .map(WhTarget::Clan)
            .unwrap_or(WhTarget::Private),
    }
}

/// Read-only view of the target container's inner item list.
fn container_ref(world: &World, player_oid: i32, target: WhTarget) -> Option<&Inventory> {
    match target {
        WhTarget::Private => world
            .objects
            .get_component::<Warehouse>(&player_oid)
            .map(|w| &w.0),
        WhTarget::Clan(clan_id) => world.clans.get(&clan_id).map(|c| &c.warehouse.0),
        WhTarget::Freight => world
            .objects
            .get_component::<Freight>(&player_oid)
            .map(|f| &f.0),
    }
}

/// `whType` in the list packets (Java: `PRIVATE=1`, `CLAN=2`, `FREIGHT=1`).
fn wh_type(target: WhTarget) -> i16 {
    match target {
        WhTarget::Clan(_) => sp::WH_TYPE_CLAN,
        WhTarget::Private | WhTarget::Freight => sp::WH_TYPE_PRIVATE,
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
    let Some(player) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = player.clan_id;
    let privs = player.clan_privs;
    if clan_id == 0 {
        return; // not in a clan
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
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
    let Some(player_oid) = player_of(world, client_id) else {
        return;
    };
    let tgt = target(world, player_oid);
    let wh_size = container_ref(world, player_oid, tgt)
        .map(|c| c.items().len())
        .unwrap_or(0) as i32;
    let Some(inv) = world.objects.get_component::<Inventory>(&player_oid) else {
        return;
    };
    // Depositable = not equipped (Java `getAvailableItems`).
    let items: Vec<(&ItemInstance, &crate::data::item_data::ItemTemplate)> = inv
        .items()
        .iter()
        .filter(|it| inv.paperdoll_slot_of(it.object_id).is_none())
        .filter_map(|it| world.data.item_data.get(it.item_id).map(|t| (it, t)))
        .collect();
    let packet =
        sp::warehouse_deposit_list(wh_type(tgt), adena(world, player_oid), wh_size, &items);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// `WithdrawP`/`WithdrawC`/`package_withdraw` — show the withdraw window (the
/// active container's contents).
pub(crate) fn open_withdraw_window(world: &mut World, client_id: u32) {
    let Some(player_oid) = player_of(world, client_id) else {
        return;
    };
    let tgt = target(world, player_oid);
    let inv_size = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .map(|i| i.items().len())
        .unwrap_or(0) as i32;
    let Some(container) = container_ref(world, player_oid, tgt) else {
        return;
    };
    let items: Vec<(&ItemInstance, &crate::data::item_data::ItemTemplate)> = container
        .items()
        .iter()
        .filter_map(|it| world.data.item_data.get(it.item_id).map(|t| (it, t)))
        .collect();
    let packet =
        sp::warehouse_withdrawal_list(wh_type(tgt), adena(world, player_oid), inv_size, &items);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// `Freight` bypass (`package_withdraw`): set the active warehouse to the
/// player's freight and open the withdraw window (Java `Freight.useBypass`,
/// the withdraw half). The send half — `package_deposit` → `PackageToList` →
/// `RequestPackageSend`, which needs the account's character list and writes to
/// a possibly-offline recipient's freight — is not yet wired.
pub(crate) fn open_freight_withdraw(world: &mut World, client_id: u32) {
    let Some(player_oid) = player_of(world, client_id) else {
        return;
    };
    set_active(world, player_oid, ActiveWarehouse::Freight);
    open_withdraw_window(world, client_id);
}

/// `SendWareHouseDepositList` (0x3B): move the named items inventory → warehouse.
pub(crate) fn handle_deposit(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::WarehouseItemList::read(body) else {
        return;
    };
    let Some(player_oid) = player_of(world, client_id) else {
        return;
    };
    let mut moved = false;
    for (obj_id, count) in pkt.items {
        moved |= transfer(world, player_oid, obj_id, count, true);
    }
    if moved {
        persist_target(world, player_oid);
    }
    send_inventory(world, client_id, player_oid);
    open_deposit_window(world, client_id);
}

/// `SendWareHouseWithDrawList` (0x3C): move the named items warehouse → inventory.
pub(crate) fn handle_withdraw(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::WarehouseItemList::read(body) else {
        return;
    };
    let Some(player_oid) = player_of(world, client_id) else {
        return;
    };
    let mut moved = false;
    for (obj_id, count) in pkt.items {
        moved |= transfer(world, player_oid, obj_id, count, false);
    }
    if moved {
        persist_target(world, player_oid);
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
    let tgt = target(world, player_oid);

    // Read the source instance's facts from whichever container it's in.
    let src_facts = {
        let src: Option<&Inventory> = if deposit {
            world.objects.get_component::<Inventory>(&player_oid)
        } else {
            container_ref(world, player_oid, tgt)
        };
        src.and_then(|c| {
            c.items()
                .iter()
                .find(|it| it.object_id == obj_id)
                .map(|it| (it.item_id, it.count, it.enchant_level))
        })
    };
    let Some((item_id, held, enchant)) = src_facts else {
        return false;
    };
    // Depositing: refuse equipped / quest items (Java `isDepositable`).
    if deposit {
        let equipped = world
            .objects
            .get_component::<Inventory>(&player_oid)
            .is_some_and(|inv| inv.paperdoll_slot_of(obj_id).is_some());
        let quest = world
            .data
            .item_data
            .get(item_id)
            .is_some_and(|t| t.is_quest_item);
        if equipped || quest {
            return false;
        }
    }
    let move_count = count.min(held);
    // Does the destination already hold a stack to merge into? (stackables only)
    let stackable = world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.is_stackable);
    let dst_has_stack = {
        let has = |c: &Inventory| c.items().iter().any(|it| it.item_id == item_id);
        if deposit {
            container_ref(world, player_oid, tgt).is_some_and(has)
        } else {
            world
                .objects
                .get_component::<Inventory>(&player_oid)
                .is_some_and(has)
        }
    };
    // A new destination stack/instance needs a fresh object id (the source may
    // keep a partial stack, so its id can't be reused).
    let dst_oid = if stackable && dst_has_stack {
        0 // merged — id unused
    } else {
        let Some(id) = world.alloc_object_id() else {
            return false;
        };
        id
    };

    // Apply: remove from source, insert into destination. The clan warehouse
    // lives in `world.clans` (a different field from `world.objects`), so its
    // borrow splits cleanly from the inventory's; the personal warehouse and
    // freight are components on the same object, needing `get_many_mut`.
    match tgt {
        WhTarget::Clan(clan_id) => {
            let World {
                objects,
                clans,
                data,
                ..
            } = world;
            let (Some(inv), Some(clan)) = (
                objects.get_component_mut::<Inventory>(&player_oid),
                clans.get_mut(&clan_id),
            ) else {
                return false;
            };
            apply_move(
                inv,
                &mut clan.warehouse.0,
                &data.item_data,
                obj_id,
                item_id,
                move_count,
                enchant,
                dst_oid,
                deposit,
            );
        }
        WhTarget::Private => {
            let catalog = &world.data.item_data;
            let Some((mut inv, mut wh)) = world
                .objects
                .get_many_mut::<(&mut Inventory, &mut Warehouse)>(&player_oid)
            else {
                return false;
            };
            apply_move(
                &mut inv, &mut wh.0, catalog, obj_id, item_id, move_count, enchant, dst_oid,
                deposit,
            );
        }
        WhTarget::Freight => {
            let catalog = &world.data.item_data;
            let Some((mut inv, mut fr)) = world
                .objects
                .get_many_mut::<(&mut Inventory, &mut Freight)>(&player_oid)
            else {
                return false;
            };
            apply_move(
                &mut inv, &mut fr.0, catalog, obj_id, item_id, move_count, enchant, dst_oid,
                deposit,
            );
        }
    }
    true
}

/// Remove `move_count` of `obj_id` from the source container and insert it into
/// the destination (enchant preserved), directed by `deposit`. `container` is
/// the warehouse/freight side; the inventory is always the other side.
#[allow(clippy::too_many_arguments)]
fn apply_move(
    inv: &mut Inventory,
    container: &mut Inventory,
    catalog: &crate::data::item_data::ItemData,
    obj_id: i32,
    item_id: i32,
    move_count: i64,
    enchant: i32,
    dst_oid: i32,
    deposit: bool,
) {
    if deposit {
        inv.remove_by_object_id(obj_id, move_count);
        container.insert_instance(catalog, dst_oid, item_id, move_count, enchant);
    } else {
        container.remove_by_object_id(obj_id, move_count);
        inv.insert_instance(catalog, dst_oid, item_id, move_count, enchant);
    }
}

/// Flush the active container after a change. The clan warehouse persists on
/// its own DB path (a shared, possibly-offline owner); the personal warehouse
/// and freight ride the player's memory-first autosave, so they need no
/// immediate flush here.
fn persist_target(world: &World, player_oid: i32) {
    if let WhTarget::Clan(clan_id) = target(world, player_oid) {
        persist_clan_warehouse(world, clan_id);
    }
}

/// Emit a `StoreClanWarehouse` DB command for `clan_id`'s current contents
/// (fire-and-forget delete-then-reinsert, mirroring the player item save).
pub(crate) fn persist_clan_warehouse(world: &World, clan_id: i32) {
    if let Some(clan) = world.clans.get(&clan_id) {
        let items = clan.warehouse.to_rows_clan();
        let _ = world
            .db
            .send(crate::db::DbCommand::StoreClanWarehouse { clan_id, items });
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

// ---------------------------------------------------------------------------
// Freight send — `package_deposit` → `PackageToList` → `RequestPackageSendable
// ItemList` → `RequestPackageSend` (Java `bypasshandlers/Freight` + the two
// `RequestPackage*` packets).
// ---------------------------------------------------------------------------

/// `bypasshandlers/Freight`'s `package_deposit`: offer the account's other
/// characters as freight recipients. Java refuses when the account has none.
pub(crate) fn open_freight_send(world: &mut World, client_id: u32) {
    let chars = account_chars(world, client_id);
    let packet = if chars.is_empty() {
        sp::system_message_with(sp::sm_ids::THAT_CHARACTER_DOES_NOT_EXIST, &[])
    } else {
        sp::package_to_list(&chars)
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// `RequestPackageSendableItemList` (0xA7): the sender's freightable items, for
/// the recipient they picked. Java sends the window for any object id; the
/// recipient is validated when the send actually arrives.
pub(crate) fn handle_package_sendable_list(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(recipient) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };
    let Some(player_oid) = player_of(world, client_id) else {
        return;
    };
    let Some(inv) = world.objects.get_component::<Inventory>(&player_oid) else {
        return;
    };
    let items: Vec<(&ItemInstance, &crate::data::item_data::ItemTemplate)> = inv
        .items()
        .iter()
        .filter(|it| inv.paperdoll_slot_of(it.object_id).is_none())
        .filter_map(|it| {
            let t = world.data.item_data.get(it.item_id)?;
            t.is_freightable.then_some((it, t))
        })
        .collect();
    let packet = sp::package_sendable_list(recipient, inv.adena(), &items);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// `RequestPackageSend` (0xA8): freight the listed items to another character
/// on the account. Java's gate ladder, in order — the recipient must be one of
/// the account's characters, the freight NPC must be in range, a negative
/// reputation is refused (`AltKarmaPlayerCanUseWarehouse`), the destination
/// freight must have room, and the `FreightPrice`-per-slot fee must be paid
/// *before* anything moves.
///
/// The recipient is usually **offline**, so the items are written straight to
/// their `items` rows (`loc = FREIGHT`) through the DB thread. When they happen
/// to be online, their live `Freight` component is updated instead — writing
/// both would double the delivery, since a component flushes on its own.
pub(crate) fn handle_package_send(world: &mut World, client_id: u32, body: &[u8]) {
    let Some((recipient, lines)) = read_package_send(body) else {
        return;
    };
    let Some(player_oid) = player_of(world, client_id) else {
        return;
    };
    if !account_chars(world, client_id)
        .iter()
        .any(|(id, _)| *id == recipient)
    {
        return;
    }
    // Java: the freight manager must be the last folk NPC and in talk range.
    let manager_in_range = world
        .objects
        .get_component::<crate::model::components::LastFolkNpc>(&player_oid)
        .is_some_and(|&crate::model::components::LastFolkNpc(npc)| {
            super::target::can_interact(world, player_oid, npc)
        });
    if !manager_in_range {
        return;
    }
    if !world.cfg.character.alt_karma_player_can_use_warehouse
        && world
            .objects
            .get_component::<Player>(&player_oid)
            .is_some_and(|p| p.reputation < 0)
    {
        return;
    }

    // Resolve each line against the inventory: freightable, not equipped, held.
    let mut moving: Vec<(i32, i32, i64, i32)> = Vec::new(); // (obj, item, count, enchant)
    {
        let Some(inv) = world.objects.get_component::<Inventory>(&player_oid) else {
            return;
        };
        for (object_id, count) in &lines {
            let Some(item) = inv.items().iter().find(|it| it.object_id == *object_id) else {
                return; // Java aborts the whole send on an invalid line
            };
            let freightable = world
                .data
                .item_data
                .get(item.item_id)
                .is_some_and(|t| t.is_freightable);
            if !freightable || inv.paperdoll_slot_of(*object_id).is_some() || *count > item.count {
                return;
            }
            moving.push((*object_id, item.item_id, *count, item.enchant_level));
        }
    }
    if moving.is_empty() {
        return;
    }

    // Slot check against the destination freight, then the fee.
    let slots = destination_slots(world, recipient, &moving);
    if slots > world.cfg.character.freight_slots {
        send_sm(
            world,
            client_id,
            sp::sm_ids::YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED,
        );
        return;
    }
    let fee = i64::from(world.cfg.character.freight_price) * moving.len() as i64;
    let adena_after_send: i64 = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .map_or(0, |inv| inv.adena())
        - moving
            .iter()
            .filter(|(_, item_id, _, _)| *item_id == ADENA_ID)
            .map(|(_, _, count, _)| *count)
            .sum::<i64>();
    if adena_after_send < fee {
        send_sm(world, client_id, sp::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA);
        return;
    }
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player_oid) {
        inv.remove_item(ADENA_ID, fee);
    }

    // Move the items out of the sender…
    let mut rows = Vec::new();
    for &(object_id, item_id, count, enchant) in &moving {
        if world
            .objects
            .get_component_mut::<Inventory>(&player_oid)
            .and_then(|inv| inv.remove_by_object_id(object_id, count))
            .is_none()
        {
            continue;
        }
        rows.push((item_id, count, enchant));
    }
    // …and into the recipient's freight, live if they're online, else on disk.
    let online = super::helpers::client_for_player(world, recipient).is_some();
    if online {
        for &(item_id, count, enchant) in &rows {
            let Some(new_oid) = world.alloc_object_id() else {
                break;
            };
            let World { objects, data, .. } = world;
            if let Some(freight) = objects.get_component_mut::<Freight>(&recipient) {
                freight
                    .0
                    .insert_instance(&data.item_data, new_oid, item_id, count, enchant);
            }
        }
    } else {
        let mut items = Vec::new();
        for &(item_id, count, enchant) in &rows {
            let Some(object_id) = world.alloc_object_id() else {
                break;
            };
            items.push(crate::db::FreightItemRow {
                object_id,
                item_id,
                count,
                enchant_level: enchant,
            });
        }
        let _ = world.db.send(crate::db::DbCommand::AddFreightItems {
            owner_id: recipient,
            items,
        });
    }
    send_inventory(world, client_id, player_oid);
}

/// `RequestPackageSend`'s body: the recipient's object id, then `(objectId,
/// count)` pairs.
fn read_package_send(body: &[u8]) -> Option<(i32, Vec<(i32, i64)>)> {
    let mut r = commons::network::PacketReader::new(body);
    let recipient = r.read_i32()?;
    let count = r.read_i32()?;
    if !(1..=500).contains(&count) {
        return None;
    }
    let mut lines = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let object_id = r.read_i32()?;
        let cnt = r.read_i64()?;
        if object_id < 1 || cnt < 0 {
            return None;
        }
        lines.push((object_id, cnt));
    }
    Some((recipient, lines))
}

/// The account's other characters (Java `Player.getAccountChars()`), snapshotted
/// on the session at character select.
fn account_chars(world: &World, client_id: u32) -> Vec<(i32, String)> {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => s.account_chars().to_vec(),
        _ => Vec::new(),
    }
}

/// Java's slot math against the destination container: a non-stackable line
/// costs one slot per unit, a stackable one costs a slot only when the freight
/// doesn't already hold that item, and adena costs none.
fn destination_slots(world: &World, recipient: i32, moving: &[(i32, i32, i64, i32)]) -> i32 {
    let existing = world.objects.get_component::<Freight>(&recipient);
    let mut slots = 0;
    for &(_, item_id, count, _) in moving {
        if item_id == ADENA_ID {
            continue;
        }
        let stackable = world
            .data
            .item_data
            .get(item_id)
            .is_some_and(|t| t.is_stackable);
        if !stackable {
            slots += count as i32;
        } else if existing.is_none_or(|f| f.0.count_of(item_id) == 0) {
            slots += 1;
        }
    }
    slots
}

fn send_sm(world: &World, client_id: u32, message_id: i16) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::system_message_with(message_id, &[]));
    }
}
