//! Inventory reads and the InventoryUpdate change builders.

use super::client_for_player;
use super::send_to_player;
use crate::model;
use crate::model::inventory::Inventory;
use crate::world::World;
pub(crate) fn send_inventory_item_list(world: &World, player: i32) {
    if let Some(inv) = world.objects.get_component::<Inventory>(&player) {
        send_to_player(
            world,
            player,
            crate::network::enter_world::item_list(inv, &world.data, false),
        );
    }
}

pub(crate) fn get_inventory_items_oids(world: &World, player_oid: i32) -> Vec<i32> {
    world
        .objects
        .get_component::<Inventory>(&player_oid)
        .map(|inv| inv.items().iter().map(|it| it.object_id).collect())
        .unwrap_or_default()
}

pub(crate) fn count_of(world: &World, player_oid: i32, item_id: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&player_oid)
        .map_or(0, |inv| inv.count_of(item_id))
}
/// How much adena `object_id` is carrying — Java `Inventory.getAdena`. Zero for
/// anything with no [`Inventory`] at all, which is what every caller wants.
pub(crate) fn adena(world: &World, object_id: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&object_id)
        .map_or(0, |inv| inv.adena())
}

/// The object id of a **usable** instance of `item_id` the player is carrying,
/// or `None` when they have none.
///
/// "Usable" is the `count > 0` filter: a stack that has been spent down to
/// zero is still in the bag until the next inventory flush, and the auto-use
/// scans must not keep firing at it.
pub(crate) fn carried_item(world: &World, player_oid: i32, item_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|i| i.item_id == item_id && i.count > 0)
                .map(|i| i.object_id)
        })
}

/// The item id of one inventory instance, found by its object id. `None` if the
/// owner has no [`Inventory`] or is not holding that instance — the two cases
/// callers treat alike, since both mean "not theirs to act on".
pub(crate) fn item_id_of(world: &World, owner_object_id: i32, item_object_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<Inventory>(&owner_object_id)
        .and_then(|inv| inv.by_object_id(item_object_id).map(|it| it.item_id))
}

/// Java `Player.sendInventoryUpdate`: an `InventoryUpdate` never travels alone —
/// it's always followed by the adena counter (`ExAdenaInvenCount`) and the
/// weight bar (`ExUserInfoInvenWeight`), so any inventory change refreshes both.
/// Ported paths that only sent the bare `InventoryUpdate` left the adena display
/// stale (e.g. `//create_coin Adena`). `iu` is the already-built InventoryUpdate.
pub(crate) fn send_inventory_update(
    world: &World,
    player_id: i32,
    changes: Vec<model::inventory::ItemChange>,
) {
    let Some(client_id) = client_for_player(world, player_id) else {
        return;
    };
    let max_load = crate::game_loop::weight::max_load(world, player_id);
    let inventory = world.objects.get_component::<Inventory>(&player_id);
    let iu =
        crate::network::enter_world::inventory_update_changes(&world.data, inventory, &changes);
    let extras = inventory.map(|inv| {
        (
            crate::network::enter_world::ex_adena_inven_count(inv),
            crate::network::enter_world::ex_user_info_inven_weight(
                player_id,
                inv,
                &world.data,
                max_load,
            ),
        )
    });
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(iu);
        if let Some((adena, weight)) = extras {
            cs.send(adena);
            cs.send(weight);
        }
    }
}

/// Snapshot still-carried instances as [`ItemChange::Modified`] — the adapter
/// for the paths that know their delta only as object ids of items that stayed
/// in the bag (equip/unequip, an enchant landing, a mana tick). Ids no longer
/// in the inventory are skipped: nothing coherent can be told to the client
/// about an instance this path believes still exists.
pub(crate) fn modified_changes(
    world: &World,
    owner: i32,
    object_ids: &[i32],
) -> Vec<model::inventory::ItemChange> {
    let Some(inv) = world.objects.get_component::<Inventory>(&owner) else {
        return Vec::new();
    };
    object_ids
        .iter()
        .filter_map(|oid| inv.by_object_id(*oid))
        .map(|item| model::inventory::ItemChange::Modified(*item))
        .collect()
}

/// Snapshot the result of `add_inventory_item_tracked` as [`ItemChange`]s:
/// a freshly minted instance becomes `Added` (the client must create the
/// slot), a grown stack `Modified`. Taken *after* any post-add stamping
/// (quest/skill grants set the enchant level between the add and the send),
/// so the packet carries the final state.
pub(crate) fn added_changes(
    world: &World,
    owner: i32,
    added: &[(i32, bool)],
) -> Vec<model::inventory::ItemChange> {
    let Some(inv) = world.objects.get_component::<Inventory>(&owner) else {
        return Vec::new();
    };
    added
        .iter()
        .filter_map(|&(oid, is_new)| {
            inv.by_object_id(oid).map(|item| {
                if is_new {
                    model::inventory::ItemChange::Added(*item)
                } else {
                    model::inventory::ItemChange::Modified(*item)
                }
            })
        })
        .collect()
}

/// `add_inventory_item_tracked` + [`added_changes`] in one step, for the
/// gain paths with nothing to stamp between the add and the
/// `InventoryUpdate`. `None` means the add itself failed (object-id pool
/// exhausted), exactly as `add_inventory_item` reports it.
pub(crate) fn add_inventory_item_changes(
    world: &mut World,
    owner: i32,
    item_id: i32,
    count: i64,
) -> Option<Vec<model::inventory::ItemChange>> {
    let added = crate::game_loop::items::add_inventory_item_tracked(world, owner, item_id, count)?;
    Some(added_changes(world, owner, &added))
}

/// The loss counterpart of [`add_inventory_item_changes`]: take `count` off the
/// instance `item_object_id` in `owner`'s bag and report it as the
/// [`ItemChange`](model::inventory::ItemChange) the `InventoryUpdate` needs
/// (`Removed` when the stack ran out, `Modified` when it shrank). `None` when
/// the object holds no inventory or the instance can't cover the count —
/// nothing was taken in that case, so callers can treat it as a failed removal.
pub(crate) fn remove_inventory_item_change(
    world: &mut World,
    owner: i32,
    item_object_id: i32,
    count: i64,
) -> Option<model::inventory::ItemChange> {
    world
        .objects
        .get_component_mut::<Inventory>(&owner)
        .and_then(|inv| inv.remove_by_object_id(item_object_id, count))
}

/// Hand `receiver` a fresh instance of an item that changed hands, preserving
/// its enchant. The receiving half of every player-to-player transfer (trade,
/// private sell store, private buy store): the sender's instance is removed and
/// the receiver gets a newly-allocated object id, never the sender's.
///
/// A no-op when object ids are exhausted or `receiver` holds no inventory.
///
/// `mana` -1: these paths only move tradable items, and every shadow item is
/// `is_tradable="false"`, so none can reach here.
pub(crate) fn give_transferred_item(
    world: &mut World,
    receiver: i32,
    item_id: i32,
    count: i64,
    enchant: i32,
) {
    if let Some(new_oid) = world.alloc_object_id()
        && let Some(inv) = world.objects.get_component_mut::<Inventory>(&receiver)
    {
        inv.insert_instance(&world.data.item_data, new_oid, item_id, count, enchant, -1);
    }
}
