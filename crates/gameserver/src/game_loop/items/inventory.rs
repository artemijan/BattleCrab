//! The inventory mutation core: `Player.addItem` stack-or-create, the item
//! audit trail, and the quest-style give/take with "You have earned" messages
//! shared by many non-quest modules.

use super::*;

/// The stack-or-create core of `Player.addItem`: merge into an existing
/// stack (persisting the new count) or allocate an object id and insert a
/// fresh instance. Non-stackable items get one instance *per unit*, mirroring
/// `ItemContainer.addItem`'s `for (i = 0; i < count; i++)` split under
/// `MultipleItemDrop = True` — the only value ever shipped in this dist's
/// `General.ini`, so it isn't wired up as a runtime toggle. Getting this
/// wrong is exactly the "2 earrings become 1 that vanishes on equip" class of
/// bug: a non-stackable item with count > 1 crammed into a single instance
/// is a state the paperdoll (one object id per slot) can't represent.
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

/// The `NORMAL` item-skill list Java's `PetFood` handler runs.
pub(crate) fn item_skills(world: &World, item_id: i32) -> Vec<(i32, i32)> {
    world
        .data
        .item_data
        .get(item_id)
        .map(|t| t.item_skills.clone())
        .unwrap_or_default()
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
/// and the [`crate::game_loop::helpers::added_changes`] adapter that feeds it.
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

    let mut created = Vec::with_capacity(count.max(1) as usize);
    for _ in 0..count.max(1) {
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
// many non-quest modules (moved out of quests.rs).
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
    let changes = crate::game_loop::helpers::added_changes(world, player, &added);
    let sm = if item_id == crate::game_loop::death::ADENA_ID {
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
    crate::game_loop::helpers::send_inventory_update(world, player, changes);
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
        let unequipped = unequipped_by_removal(&equipped_before, &changes);
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
    finish_equipped_item_destroyed(world, client_id, player, &unequipped);
    // As in `give_item_with_earned_message`, no bare `ExQuestItemList` — Java's
    // `takeItems` → `destroyItemByItemId` sends only the `InventoryUpdate`, and
    // the change-type-3 entries below are what retire the client's rows.
    crate::game_loop::helpers::send_inventory_update(world, player, changes);
    true
}

// Moved from helpers.
/// Java `Player.setInventoryBlockingStatus(true)` — suppress inventory
/// refreshes for this player, and schedule the 1500 ms `InventoryEnableTask`
/// that lifts it.
///
/// Called wherever Java calls it: opening a merchant buy list, a private or
/// clan warehouse, and the "wear" (try-on) shop.
pub(crate) fn block_inventory(world: &mut World, object_id: i32) {
    world.inventory_blocked.insert(object_id);
    world.scheduler.schedule(
        world.tick + crate::game_loop::helpers::ms_to_ticks(1500),
        crate::scheduler::ScheduledTask::InventoryEnable { object_id },
    );
}
