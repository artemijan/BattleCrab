//! Everything that reads or changes a player's bag, in one place: the
//! inventory reads, the `InventoryUpdate` change builders that turn a delta
//! into a packet, the `Player.addItem` stack-or-create mutation core, the item
//! audit trail, and the quest-style give/take with its "You have earned"
//! messages.
//!
//! The container itself is [`crate::model::inventory`]; what lives here is the
//! `World`-aware layer over it. Gear equip/unequip and the `EtcItem` "use"
//! dispatch stay in [`crate::game_loop::items`], which is about what an item
//! *does* rather than about the bag holding it.

use crate::data::item_data::ADENA_ID;
use crate::game_loop::helpers::client_for_player;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::helpers::send_to_player;
use crate::game_loop::items;
use crate::model;
use crate::model::inventory::Inventory;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::world::World;
use tracing::warn;

// ---------------------------------------------------------------------------
// Reads and the `InventoryUpdate` change builders.
// ---------------------------------------------------------------------------

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
    let max_load = crate::game_loop::stats::weight::max_load(world, player_id);
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
    let added = add_inventory_item_tracked(world, owner, item_id, count)?;
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

// ---------------------------------------------------------------------------
// The mutation core (`Player.addItem`) and the item audit trail.
// ---------------------------------------------------------------------------

/// The stack-or-create core of `Player.addItem`: merge into an existing
/// stack (persisting the new count) or allocate an object id and insert a
/// fresh instance. Non-stackable items get one instance *per unit*, mirroring
/// `ItemContainer.addItem`'s `for (i = 0; i < count; i++)` split under
/// `MultipleItemDrop` (**True** here, and Java's default). Getting this
/// wrong is exactly the "2 earrings become 1 that vanishes on equip" class of
/// bug: a non-stackable item with count > 1 crammed into a single instance
/// is a state the paperdoll (one object id per slot) can't represent.
///
/// The key used to be hard-coded to this dist's value behind a comment naming
/// it. Reading it exposes Java's **off** branch, which is worth stating
/// plainly because it is not the intuitive one: `break`ing out of that loop on
/// the first pass creates *one instance of 1* and silently discards the other
/// `count - 1` units. It is a lossy setting, not a stacking setting.
/// Returns every object id created/touched; `None` only on id-pool
/// exhaustion (any already-created units stay, matching Java's partial
/// completion when `createItem` fails mid-loop). Shared by the auto-loot
/// path (`death::give_item`), quest rewards (`quests`), the shop (`shop`),
/// and pack/box extraction (`extract_item` below); the caller owns
/// messaging/`InventoryUpdate`.
pub(crate) fn add_inventory_item(
    world: &mut World,
    player_oid: i32,
    item_id: i32,
    count: i64,
) -> Option<Vec<i32>> {
    add_inventory_item_tracked(world, player_oid, item_id, count)
        .map(|added| added.into_iter().map(|(oid, _)| oid).collect())
}

/// [`add_inventory_item`] with the flag every `InventoryUpdate` builder needs:
/// whether each returned object id is a **freshly created** instance or an
/// existing stack that merely grew. Java decides the same thing inline in
/// `PlayerInventory.addItem`:
///
/// ```java
/// if (item.isStackable() && (item.getCount() > count)) playerIU.addModifiedItem(item);
/// else                                                 playerIU.addNewItem(item);
/// ```
///
/// and the distinction is load-bearing — change type 1 (add) tells the client
/// to create the inventory slot, change type 2 (modify) only refreshes one it
/// already has. See [`crate::network::enter_world::inventory_update_changes`]
/// and the [`added_changes`] adapter that feeds it.
pub(crate) fn add_inventory_item_tracked(
    world: &mut World,
    player_oid: i32,
    item_id: i32,
    count: i64,
) -> Option<Vec<(i32, bool)>> {
    let added = add_inventory_item_inner(world, player_oid, item_id, count);
    // Java records ownership changes down in `Item.setOwnerId`/`changeCount`,
    // which every path funnels through. This wrapper is the equivalent choke
    // point: recording here rather than at each caller is what stops a new item
    // source from silently escaping the audit.
    if added.is_some() {
        record_item_change(world, player_oid, item_id, count, "add");
    }
    added
}

/// The body of [`add_inventory_item_tracked`], split out so the audit record in
/// its wrapper covers every one of the three return paths below.
fn add_inventory_item_inner(
    world: &mut World,
    player_oid: i32,
    item_id: i32,
    count: i64,
) -> Option<Vec<(i32, bool)>> {
    let stackable = world
        .data
        .item_data
        .get(item_id)
        .map(|t| t.is_stackable)
        .unwrap_or(false);
    if stackable {
        let existing_stack = world
            .objects
            .get_component::<Inventory>(&player_oid)
            .and_then(|inv| inv.first_of_item(item_id).map(|i| i.object_id));

        if let Some(stack_oid) = existing_stack {
            // Memory-first: the stack grows in memory; the new count persists on
            // the next flush, not here.
            let inv = world
                .objects
                .get_component_mut::<Inventory>(&player_oid)
                .expect("checked");
            inv.add_item(&world.data.item_data, stack_oid, item_id, count);
            return Some(vec![(stack_oid, false)]);
        }
        let new_oid = world.alloc_object_id()?;
        let inv = world.objects.get_component_mut::<Inventory>(&player_oid)?;
        inv.add_item(&world.data.item_data, new_oid, item_id, count);
        return Some(vec![(new_oid, true)]);
    }

    // `MultipleItemDrop` off → Java breaks after the first pass, so exactly one
    // unit is created and the remainder is lost.
    let units = if world.cfg.general.multiple_item_drop {
        count.max(1)
    } else {
        1
    };
    let mut created = Vec::with_capacity(units as usize);
    for _ in 0..units {
        let new_oid = world.alloc_object_id()?;
        let inv = world.objects.get_component_mut::<Inventory>(&player_oid)?;
        inv.add_item(&world.data.item_data, new_oid, item_id, 1);
        created.push((new_oid, true));
    }
    Some(created)
}

/// One item-audit record, honouring Java's three-way gate: `LogItems` for
/// everything, plus the small-log and id-list *overrides*, which admit their
/// own items even when the broad switch is off (see
/// [`GeneralConfig::should_log_item`](crate::config::general::GeneralConfig::should_log_item)).
///
/// `count` is signed by convention: positive for a gain, negative for a loss,
/// so one query over the file reconstructs a balance.
pub(crate) fn record_item_change(
    world: &World,
    player_oid: i32,
    item_id: i32,
    count: i64,
    process: &str,
) {
    let equipable = world
        .data
        .item_data
        .get(item_id)
        .map(|t| t.is_equipable())
        .unwrap_or(false);
    if !world.cfg.general.should_log_item(item_id, equipable) {
        return;
    }
    let (char_name, account) = match world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
    {
        Some(p) => (Some(p.name.clone()), None::<String>),
        None => (None, None),
    };
    commons::audit::record(
        commons::audit::Category::Item,
        serde_json::json!({
            "process": process,
            "char_name": char_name,
            "account": account,
            "oid": player_oid,
            "item_id": item_id,
            "item_name": world.data.item_data.get(item_id).map(|t| t.name.clone()),
            "count": count,
        }),
    );
}

/// Turns the losses noted on every player's [`Inventory`] into audit records.
///
/// This is the other half of the item audit. Gains have one choke point
/// ([`add_inventory_item_tracked`]); losses have ~43, all of them holding a
/// `&mut Inventory` borrow that a `World`-aware call could not coexist with. So
/// the removal methods note what left, and this runs once per tick where the
/// config gate, the item names and the owning player are all reachable.
///
/// Draining per tick rather than per removal also means a burst of consumption
/// in one tick costs one pass, not one lookup per item.
pub(crate) fn drain_item_audit(world: &mut World) {
    // Cheap common case: nothing was removed this tick from anyone.
    let owners: Vec<i32> = world
        .clients
        .values()
        .filter_map(|cs| match cs {
            crate::session::ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        })
        .filter(|oid| {
            world
                .objects
                .get_component::<Inventory>(oid)
                .is_some_and(|inv| inv.has_pending_audit())
        })
        .collect();

    for oid in owners {
        let Some(pending) = world
            .objects
            .get_component_mut::<Inventory>(&oid)
            .map(|inv| inv.take_pending_audit())
        else {
            continue;
        };
        for (item_id, count) in pending {
            // Negative by the convention `record_item_change` documents: one
            // query over the file reconstructs a balance.
            record_item_change(world, oid, item_id, -count, "consume");
        }
    }
}

// ---------------------------------------------------------------------------
// Quest-style item give/take with the "You have earned" message — shared by
// many non-quest modules.
// ---------------------------------------------------------------------------

/// `Player.addItem("Quest", …)` + `sendItemGetMessage`: SM 52/53/54 ("You
/// have earned …") + `InventoryUpdate`.
///
/// Deliberately **no** `ExQuestItemList` here, matching Java: that packet is
/// only ever sent by `EnterWorld` and by `Player.sendItemList`, which always
/// puts a full `ItemList` in front of it. The client treats it as a list to
/// append to the inventory it was just handed, not as a standalone refresh, so
/// firing it bare on every quest item gain appends the whole quest tab again —
/// one visible duplicate row per gain, surviving until the next relog rebuilds
/// the inventory from `ItemList`. The `InventoryUpdate` below is the entire
/// client-side refresh Java performs (`PlayerInventory.addItem`).
pub(crate) fn give_item_with_earned_message(
    world: &mut World,
    client_id: u32,
    player: i32,
    item_id: i32,
    count: i64,
) {
    give_item_with_earned_message_enchanted(world, client_id, player, item_id, count, 0);
}

/// As [`give_item_with_earned_message`], but stamping `enchant` on what it
/// creates.
///
/// **Java never needs this.** An enchanted item keeps its `+N` across a drop
/// and pickup there because both move the *same* `Item` instance between
/// containers; this port mints a fresh instance on the give path, so the level
/// has to be carried across explicitly. It must be stamped *before* the
/// `InventoryUpdate` below is built, or the client is told about a `+0` item
/// the server considers enchanted.
pub(crate) fn give_item_with_earned_message_enchanted(
    world: &mut World,
    client_id: u32,
    player: i32,
    item_id: i32,
    count: i64,
    enchant: i32,
) {
    let Some(added) = add_inventory_item_tracked(world, player, item_id, count) else {
        warn!("quest give_items: object-id pool exhausted, dropping {item_id}×{count}");
        return;
    };
    if enchant != 0
        && let Some(inv) = world.objects.get_component_mut::<Inventory>(&player)
    {
        // Enchantable items are never stackable, so this is exactly one
        // freshly-created instance.
        for &(oid, _) in &added {
            inv.set_enchant_level(oid, enchant);
        }
    }
    // Snapshot after the enchant stamp, so the packet carries the `+N`.
    let changes = added_changes(world, player, &added);
    let sm = if item_id == ADENA_ID {
        server_packets::system_message_with(
            sm_ids::YOU_HAVE_EARNED_S1_ADENA,
            &[SmParam::Long(count)],
        )
    } else if count > 1 {
        server_packets::system_message_with(
            sm_ids::YOU_HAVE_EARNED_S2_S1_S,
            &[SmParam::ItemName(item_id), SmParam::Long(count)],
        )
    } else {
        server_packets::system_message_with(
            sm_ids::YOU_HAVE_EARNED_S1,
            &[SmParam::ItemName(item_id)],
        )
    };
    send_to_client(world, client_id, sm);
    // `InventoryUpdate` + adena counter + weight bar (Java `sendInventoryUpdate`),
    // so the status-bar adena count refreshes on adena gains (`//create_coin`).
    send_inventory_update(world, player, changes);
}

/// The game-loop half of `takeItems`: `Inventory::remove_item` + DB deletes/
/// count updates + `InventoryUpdate` (with removed entries) + quest-tab
/// refresh.
pub(crate) fn take_items(
    world: &mut World,
    client_id: u32,
    player: i32,
    item_id: i32,
    count: i64,
) -> bool {
    let (changes, unequipped) = {
        let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) else {
            return false;
        };
        // Java's `Inventory.removeItem` unequips whatever it takes out of the
        // bag; here the paperdoll clearing is silent, so note which worn
        // instances the removal took. A quest item can be equipment — Q229
        // `Test of Witchcraft` registers the Sword of Seal (a weapon), and its
        // `exitQuest` sweep destroys it while it is still in the player's hand.
        let equipped_before = inv.equipped_object_ids();
        let changes = inv.remove_item(item_id, count);
        let unequipped = items::unequipped_by_removal(&equipped_before, &changes);
        (changes, unequipped)
    };
    if changes.is_empty() {
        return false;
    }
    // Memory-first: the count decrements / removals already applied to the
    // `Inventory` component; they persist on the next flush.
    //
    // Java unequips *before* the destroy's `InventoryUpdate` goes out (the
    // `ExUserInfoEquipSlot` comes from inside `setPaperdollItem`), so this
    // runs first — without it the client keeps rendering a destroyed weapon.
    items::finish_equipped_item_destroyed(world, client_id, player, &unequipped);
    // As in `give_item_with_earned_message`, no bare `ExQuestItemList` — Java's
    // `takeItems` → `destroyItemByItemId` sends only the `InventoryUpdate`, and
    // the change-type-3 entries below are what retire the client's rows.
    send_inventory_update(world, player, changes);
    true
}

// ---------------------------------------------------------------------------
// The inventory-refresh block.
// ---------------------------------------------------------------------------

/// Java `Player.setInventoryBlockingStatus(true)` — suppress inventory
/// refreshes for this player, and schedule the 1500 ms `InventoryEnableTask`
/// that lifts it.
///
/// Called wherever Java calls it: opening a merchant buy list, a private or
/// clan warehouse, and the "wear" (try-on) shop.
pub(crate) fn block_inventory(world: &mut World, object_id: i32) {
    world.inventory_blocked.insert(object_id);
    world.scheduler.schedule(
        world.tick + crate::scheduler::ms_to_ticks(1500),
        crate::scheduler::ScheduledTask::InventoryEnable { object_id },
    );
}
