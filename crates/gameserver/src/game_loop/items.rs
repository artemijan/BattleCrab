//! Gear equip/unequip handlers (`UseItem`, `RequestUnEquipItem`) and the
//! `EtcItem` "use" dispatch (`ExtractableItems` for pack/box items).

use tracing::warn;

use crate::data::item_data::ItemHandler;
use crate::model::inventory::Inventory;
use crate::network::client_packets as cp;
use crate::network::enter_world as ew;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

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
/// already has. See [`crate::network::enter_world::inventory_update_added`].
pub(crate) fn add_inventory_item_tracked(
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
            .get_component::<crate::model::inventory::Inventory>(&player_oid)
            .and_then(|inv| {
                inv.items()
                    .iter()
                    .find(|i| i.item_id == item_id)
                    .map(|i| i.object_id)
            });

        if let Some(stack_oid) = existing_stack {
            // Memory-first: the stack grows in memory; the new count persists on
            // the next flush, not here.
            let inv = world
                .objects
                .get_component_mut::<crate::model::inventory::Inventory>(&player_oid)
                .expect("checked");
            inv.add_item(&world.data.item_data, stack_oid, item_id, count);
            return Some(vec![(stack_oid, false)]);
        }
        let new_oid = world.alloc_object_id()?;
        let inv = world
            .objects
            .get_component_mut::<crate::model::inventory::Inventory>(&player_oid)?;
        inv.add_item(&world.data.item_data, new_oid, item_id, count);
        return Some(vec![(new_oid, true)]);
    }

    let mut created = Vec::with_capacity(count.max(1) as usize);
    for _ in 0..count.max(1) {
        let new_oid = world.alloc_object_id()?;
        let inv = world
            .objects
            .get_component_mut::<crate::model::inventory::Inventory>(&player_oid)?;
        inv.add_item(&world.data.item_data, new_oid, item_id, 1);
        created.push((new_oid, true));
    }
    Some(created)
}

/// Port of `clientpackets/RequestItemList.runImpl`: the client opened its
/// inventory window and wants the current contents. Java calls
/// `player.sendItemList(true)`, which (after a 300 ms debounce we don't
/// replicate — there's no per-client timer here) sends `ItemList` with the
/// show-window flag set, then `ExQuestItemList`, `ExAdenaInvenCount` and
/// `ExUserInfoInvenWeight`. The `isInventoryDisabled` guard is a no-op: nothing
/// in this port blocks the inventory yet (set only by trades/some skills, both
/// unported).
pub(crate) fn handle_request_item_list(world: &mut World, client_id: u32) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    let max_load = crate::game_loop::weight::max_load(world, object_id);
    let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
        return;
    };
    let Some(cs) = world.clients.get(&client_id) else {
        return;
    };
    cs.send(ew::item_list(inventory, &world.data, true));
    cs.send(ew::ex_quest_item_list(inventory, &world.data));
    cs.send(ew::ex_adena_inven_count(inventory));
    cs.send(ew::ex_user_info_inven_weight(
        object_id,
        inventory,
        &world.data,
        max_load,
    ));
}

/// Port of `clientpackets/UseItem.runImpl`: right-clicking a `Weapon`/`Armor`
/// toggles equip/unequip; anything else routes through the `EtcItem` handler
/// dispatch (Java: `ItemHandler.getInstance().getHandler(etcItem)`).
pub(crate) fn handle_use_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::UseItem::read(body) else {
        return;
    };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    // `UseItem.runImpl`: `hasBlockActions() || isControlBlocked() ||
    // isAlikeDead()` refuses the use outright. (Death is gated further in.)
    if crate::game_loop::abnormal::is_blocked_from_actions(world, object_id)
        || crate::game_loop::abnormal::is_control_blocked(world, object_id)
    {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    if cursed_weapon_blocks_equip(world, object_id, pkt.object_id) {
        return; // Java returns with no packet at all.
    }
    use_equipable_item(world, client_id, object_id, pkt.object_id);
}

/// The cursed-weapon half of `UseItem.runImpl`'s equipable branch: a wielder of
/// Zariche/Akamanah may neither put on formal wear (6408) nor touch a hand slot
/// — the curse locks the weapon in place, so the "just swap to another sword"
/// escape hatch does not exist.
///
/// Deliberately sits in the *packet* handler rather than in
/// [`use_equipable_item`], mirroring Java: `CursedWeapon.activate` equips the
/// weapon through `getInventory().equipItem(…)`, well below this check, and the
/// queued-while-busy replay re-enters at `useEquippableItem`, past it too.
/// Moving the gate down would make the curse unable to equip itself.
fn cursed_weapon_blocks_equip(world: &World, object_id: i32, item_object_id: i32) -> bool {
    use crate::data::item_data::{SLOT_L_HAND, SLOT_LR_HAND, SLOT_R_HAND};

    if world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .is_none_or(|p| p.cursed_weapon_equipped_id == 0)
    {
        return false;
    }
    let Some((item_id, body_part)) = world
        .objects
        .get_component::<Inventory>(&object_id)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|i| i.object_id == item_object_id)
                .map(|i| i.item_id)
        })
        .map(|id| (id, world.data.item_data.get(id).map_or(0, |t| t.body_part)))
    else {
        return false;
    };
    // "Don't allow to put formal wear while a cursed weapon is equipped."
    item_id == FORMAL_WEAR_ITEM_ID || matches!(body_part, SLOT_LR_HAND | SLOT_L_HAND | SLOT_R_HAND)
}

/// Formal Wear — Java `UseItem` names the id inline in the cursed-weapon guard.
const FORMAL_WEAR_ITEM_ID: i32 = 6408;

/// The equipable branch of `UseItem.runImpl`, entered from the packet handler
/// and from the queued replay (`run_queued_action`): while busy, Java defers
/// the equip instead of dropping it — to cast end via
/// `setNextAction(NextAction(EVT_FINISH_CASTING, …))`, to swing end via a
/// schedule at `attackEndTime` — sending no packet either way. Non-equipable
/// items never get queued this way (dispatched to `use_etc_item` immediately,
/// same as Java's else-branch which has no busy check).
pub(crate) fn use_equipable_item(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    item_object_id: i32,
) {
    use crate::model::components::{AttackState, Casting, QueuedAction};

    let is_equipable = {
        let catalog = &world.data.item_data;
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        let Some(item) = inventory
            .items()
            .iter()
            .find(|i| i.object_id == item_object_id)
        else {
            return;
        };
        let Some(template) = catalog.get(item.item_id) else {
            return;
        };
        template.is_equipable()
    };
    if !is_equipable {
        use_etc_item(world, client_id, object_id, item_object_id);
        return;
    }

    let mid_swing = world
        .objects
        .get_component::<AttackState>(&object_id)
        .is_some_and(|st| st.attack_end_tick > world.tick);
    if mid_swing || world.objects.has_component::<Casting>(&object_id) {
        world
            .objects
            .add_components(&object_id, QueuedAction::UseItem { item_object_id });
        return;
    }

    let catalog = &world.data.item_data;
    let Some(inventory) = world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&object_id)
    else {
        return;
    };

    // Java resolves the item's *currently occupied* single-bit slot
    // (`getSlotFromItem`) before unequipping — not the item's raw template
    // body part, which is a combined bitmask for rings/earrings and would
    // silently no-op. `unequip_item` clears the exact slot we already know
    // the object id is in, sidestepping that resolution entirely.
    let was_equipped = inventory.paperdoll_slot_of(item_object_id).is_some();
    let changed = if was_equipped {
        inventory.unequip_item(item_object_id)
    } else {
        inventory.equip_item(catalog, item_object_id)
    };
    finish_equip_change(world, client_id, object_id, &changed);
    // Java `Player.useEquipableItem`, right after the "you have equipped"
    // message: "Consume mana - will start a task if required; returns if item
    // is not a shadow item". It is the *clicked* item that pays, and only on
    // the equip half of the branch — Java's `if (item.isEquipped())` after
    // `equipItemAndRecord`, which is why a swap's displaced items never pay
    // and taking something off never does either. A shadow weapon therefore
    // burns its first point the moment it goes on, and that call is what arms
    // the 60 s beat. Last, because at mana 1 it destroys the item and
    // re-enters `finish_equip_change` for the unequip.
    if !was_equipped
        && world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&object_id)
            .is_some_and(|inv| inv.paperdoll_slot_of(item_object_id).is_some())
    {
        super::item_mana::on_item_equipped(world, object_id, item_object_id);
    }
}

/// Port of `clientpackets/RequestUnEquipItem.runImpl` (the mid-attack /
/// mid-cast guards are still skipped).
pub(crate) fn handle_request_un_equip_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(body_part) = cp::read_char_slot(body) else {
        return;
    };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    // "Prevent of unequipping a cursed weapon." Java tests the *requested slot*
    // (`_slot == SLOT_LR_HAND`), not the item, so the two-hand slot is frozen
    // outright while cursed. (`isCombatFlagEquipped` shares the branch; the
    // combat flag isn't modelled yet.) Without it a cursed player could simply
    // take the sword off and walk away un-transformed.
    if body_part == crate::data::item_data::SLOT_LR_HAND
        && world
            .objects
            .get_component::<crate::model::Player>(&object_id)
            .is_some_and(|p| p.cursed_weapon_equipped_id != 0)
    {
        return;
    }
    let Some(inventory) = world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&object_id)
    else {
        return;
    };
    let changed = inventory.unequip_slot(body_part);
    finish_equip_change(world, client_id, object_id, &changed);
}

/// Port of `clientpackets/RequestDestroyItem.runImpl`: destroy `count` of an
/// inventory item. Quest items are protected (Java's non-`isDestroyable` +
/// `DESTROY_ALL_ITEMS=false` guard, narrowed to the flag the port models); an
/// equipped item is unequipped first. The cursed-weapon / hero-item / pet /
/// enchant-transaction guards are skipped (those subsystems aren't ported).
pub(crate) fn handle_request_destroy_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestDestroyItem::read(body) else {
        return;
    };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    if pkt.count <= 0 {
        return;
    }
    // Locate the item + its template facts.
    let Some((item_id, held, is_stackable, undestroyable)) = world
        .objects
        .get_component::<Inventory>(&object_id)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|it| it.object_id == pkt.object_id)
                .map(|it| (it.item_id, it.count))
        })
        .map(|(id, cnt)| {
            let t = world.data.item_data.get(id);
            (
                id,
                cnt,
                t.map(|t| t.is_stackable).unwrap_or(false),
                // Java's guard is `!isDestroyable()` — quest items are one case
                // of it, `is_destroyable="false"` in the datapack is the other.
                t.map(|t| t.is_quest_item || !t.is_destroyable())
                    .unwrap_or(false),
            )
        })
    else {
        send_item_message(world, client_id, "This item cannot be destroyed.");
        return;
    };
    if undestroyable {
        send_item_message(world, client_id, "This item cannot be destroyed.");
        return;
    }
    // Java `RequestDestroyItem`: `CursedWeaponsManager.isCursed(itemId)` is
    // OR'd into the non-destroyable test — you cannot delete your way out of
    // the curse, which would otherwise strand the manager's row forever.
    if crate::game_loop::cursed_weapon::is_cursed_item(world, item_id) {
        send_item_message(world, client_id, "This item cannot be destroyed.");
        return;
    }
    // A non-stackable item can only be destroyed one at a time (Java returns).
    if !is_stackable && pkt.count > 1 {
        return;
    }
    let count = pkt.count.min(held);

    // Unequip first if it's worn (Java unequips, sending its own InventoryUpdate).
    if world
        .objects
        .get_component::<Inventory>(&object_id)
        .is_some_and(|inv| inv.paperdoll_slot_of(pkt.object_id).is_some())
    {
        let changed = world
            .objects
            .get_component_mut::<Inventory>(&object_id)
            .map(|inv| inv.unequip_item(pkt.object_id))
            .unwrap_or_default();
        finish_equip_change(world, client_id, object_id, &changed);
    }

    // `if (itemToRemove.getTemplate().isPetItem())` — destroying a collar
    // destroys the pet bound to it: unsummon it if it's out, then drop the
    // saved row. Object ids are recycled, so an orphan row would eventually
    // bind a stale pet to an unrelated item.
    if world.data.pet_data.is_pet_collar(item_id) {
        if let Some(pet_oid) = crate::game_loop::servitor::pet_of(world, object_id) {
            let bound = world
                .objects
                .get_component::<crate::model::components::PetOf>(&pet_oid)
                .map(|p| p.collar_object_id);
            if bound == Some(pkt.object_id) {
                crate::game_loop::servitor::unsummon_servitor(world, object_id);
            }
        }
        world
            .objects
            .get_component_mut::<crate::model::components::PlayerPets>(&object_id)
            .map(|p| p.0.remove(&pkt.object_id));
        let _ = world.db.send(crate::db::DbCommand::DeletePetRow {
            collar_object_id: pkt.object_id,
        });
    }

    let Some(change) = world
        .objects
        .get_component_mut::<Inventory>(&object_id)
        .and_then(|inv| inv.remove_by_object_id(pkt.object_id, count))
    else {
        return;
    };
    let packet = ew::inventory_update_changes(&world.data, &[change]);
    super::helpers::send_inventory_update(world, client_id, object_id, packet);
}

/// The `Crystallize` common skill (`CommonSkill.CRYSTALLIZE`).
const CRYSTALLIZE_SKILL_ID: i32 = 248;

/// Port of `clientpackets/RequestCrystallizeItem.runImpl` (narrowed): destroy a
/// crystallizable item and yield its grade's crystals. Gated on the player's
/// `Crystallize` (248) skill level vs the item grade (D→1 … S→5). With no
/// `ItemCrystallizationData`, Java's fallback is `crystalCount` of the grade's
/// crystal at 100% — that's what we award. Hero/augment guards skipped; the
/// shadow-item one is enforced.
pub(crate) fn handle_request_crystallize_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestDestroyItem::read(body) else {
        return;
    }; // same layout (objectId, count)
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player_oid = session.player_object_id();
    if pkt.count <= 0 {
        return;
    }
    // Locate the item + its crystallization facts.
    let Some((item_id, held, is_stackable)) = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|it| it.object_id == pkt.object_id)
                // Java's first guard: `itemToRemove.isShadowItem() ||
                // isTimeLimitedItem()` → plain `ActionFailed`, no message.
                // Without it a coupon-bought shadow weapon could be melted
                // into free D-grade crystals the minute it was handed over.
                .filter(|it| !super::item_mana::is_shadow_item(it.mana_left))
                .map(|it| (it.item_id, it.count))
        })
        .map(|(id, cnt)| {
            (
                id,
                cnt,
                world
                    .data
                    .item_data
                    .get(id)
                    .map(|t| t.is_stackable)
                    .unwrap_or(false),
            )
        })
    else {
        return;
    };
    let Some(t) = world.data.item_data.get(item_id) else {
        return;
    };
    let (Some(crystal_item), crystal_count) = (t.crystal_type.crystal_item_id(), t.crystal_count)
    else {
        send_item_message(world, client_id, "This item cannot be crystallized.");
        return;
    };
    if crystal_count <= 0 {
        send_item_message(world, client_id, "This item cannot be crystallized.");
        return;
    }
    let required = t.crystal_type.required_crystallize_level();
    let skill_level = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&player_oid)
        .and_then(|b| b.0.get(&CRYSTALLIZE_SKILL_ID).copied())
        .unwrap_or(0);
    if skill_level < required {
        send_item_message(
            world,
            client_id,
            "Your crystallization skill level is too low.",
        );
        return;
    }
    if !is_stackable && pkt.count > 1 {
        return;
    }
    let count = pkt.count.min(held);

    // Unequip first if worn, then destroy, then award the crystals.
    if world
        .objects
        .get_component::<Inventory>(&player_oid)
        .is_some_and(|inv| inv.paperdoll_slot_of(pkt.object_id).is_some())
    {
        let changed = world
            .objects
            .get_component_mut::<Inventory>(&player_oid)
            .map(|inv| inv.unequip_item(pkt.object_id))
            .unwrap_or_default();
        finish_equip_change(world, client_id, player_oid, &changed);
    }
    let Some(removed) = world
        .objects
        .get_component_mut::<Inventory>(&player_oid)
        .and_then(|inv| inv.remove_by_object_id(pkt.object_id, count))
    else {
        return;
    };
    let total = crystal_count as i64 * count;
    add_inventory_item(world, player_oid, crystal_item, total);
    // InventoryUpdate: the destroyed item + the crystal stack (as a modify).
    let mut changes = vec![removed];
    if let Some(inv) = world.objects.get_component::<Inventory>(&player_oid)
        && let Some(stack) = inv.items().iter().find(|it| it.item_id == crystal_item)
    {
        changes.push(crate::model::inventory::ItemChange::Modified(*stack));
    }
    let packet = ew::inventory_update_changes(&world.data, &changes);
    super::helpers::send_inventory_update(world, client_id, player_oid, packet);
}

/// Send a bare `$s1` system-message line to one client.
fn send_item_message(world: &World, client_id: u32, text: &str) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(
            sm_ids::S1_TEXT,
            &[SmParam::Text(text.to_string())],
        ));
    }
}

/// Port of `clientpackets/RequestSaveInventoryOrder.runImpl`: persist the
/// client's custom inventory arrangement. For each `(object_id, order)` pair,
/// Java sets `item.setItemLocation(INVENTORY, order)` — but only for items
/// *currently* in `INVENTORY` (equipped/paperdoll items are skipped). We mirror
/// that guard via `paperdoll_slot_of`, then fire-and-forget the new `loc_data`
/// to the DB; `load_items` restores `ORDER BY loc_data`, so the arrangement
/// survives relog. No response packet — Java sends none either.
pub(crate) fn handle_request_save_inventory_order(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestSaveInventoryOrder::read(body) else {
        return;
    };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    let Some(inventory) = world.objects.get_component_mut::<Inventory>(&object_id) else {
        return;
    };
    // Keep only the pairs naming an item actually in the inventory grid — an
    // equipped item occupies a paperdoll slot and keeps its slot index. Applied
    // to the in-memory order (memory-first); it persists to `loc_data` on the
    // next flush, not here.
    let order: Vec<(i32, i32)> = pkt
        .order
        .into_iter()
        .filter(|&(oid, _)| {
            inventory.items().iter().any(|i| i.object_id == oid)
                && inventory.paperdoll_slot_of(oid).is_none()
        })
        .collect();
    inventory.apply_inventory_order(&order);
}

/// Shared tail of the equip/unequip handlers: persist each changed slot
/// (`items.loc`/`loc_data`), then resend `ExUserInfoEquipSlot` + `UserInfo` +
/// `InventoryUpdate` — in that order, mirroring Java's equip flow:
///   1. `Inventory.setPaperdollItem` sends `ExUserInfoEquipSlot` synchronously
///      *during* the equip, once per paperdoll slot it mutates;
///   2. `Player.useEquippableItem` then calls `broadcastUserInfo` (`UserInfo`);
///   3. …and finally `sendInventoryUpdate` (`InventoryUpdate`).
/// `ExUserInfoEquipSlot` — not just `InventoryUpdate` — is what drives the
/// client's own paperdoll rendering; skipping it leaves newly equipped
/// rings/earrings invisible on the paperdoll even though the inventory list is
/// correct. Two deliberate divergences from Java, both verified in-game:
///   * We send one `ExUserInfoEquipSlot` for the whole action instead of one
///     per `setPaperdollItem` call. The packet is a full 33-slot paperdoll
///     snapshot, so a single send after all slot mutations already carries the
///     final state; Java's per-slot sends only differ in transient intermediate
///     snapshots the client immediately overwrites.
///   * We omit Java's *extra* `ThreadPool.schedule(new ExUserInfoEquipSlot, 100)`
///     in `useEquippableItem` — a redundant second copy of that same snapshot.
pub(crate) fn finish_equip_change(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    changed: &[i32],
) {
    if changed.is_empty() {
        return;
    }
    // Java's equip/unequip listeners fire the augment bonuses first
    // (`Inventory.equipItem`: "Apply augmentation bonuses on equip";
    // `unEquipItemInBodySlot`: "Remove augmentation bonuses on unequip"), and
    // *then* recalculate stats — so an option's modifiers are already in the
    // maps when the recompute below runs. `changed` carries the object ids
    // whose paperdoll slot moved either way; which direction it went is read
    // off the inventory here.
    for &item_oid in changed {
        let equipped = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&object_id)
            .is_some_and(|inv| inv.paperdoll_slot_of(item_oid).is_some());
        if equipped {
            super::options::apply_item_options(world, object_id, item_oid);
        } else {
            super::options::remove_item_options(world, object_id, item_oid);
        }
    }
    // Memory-first: the paperdoll change already lives in the `Inventory`
    // component; the new `loc`/`loc_data` of each changed slot persists on the
    // next flush (`Inventory::to_rows`), so equip/unequip spam can't drive DB
    // writes.

    refresh_equip_state(world, client_id, object_id);

    let Some(inventory) = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&object_id)
    else {
        return;
    };
    let iu = crate::network::enter_world::inventory_update(inventory, &world.data, changed);
    // …and finally Java's `sendInventoryUpdate` — the `InventoryUpdate` plus the
    // adena counter and weight bar it always drags along.
    super::helpers::send_inventory_update(world, client_id, object_id, iu);
    // Java `Inventory.equipItem`/`unEquipItemInBodySlot` fire
    // `refreshExpertisePenalty` on the owner: a newly equipped over-grade item
    // (or one just removed) changes the grade penalty. Runs last so the borrow
    // of `inventory` above is released; it sends its own EtcStatusUpdate +
    // UserInfo when the penalty actually changed.
    crate::game_loop::expertise::refresh_expertise_penalty(world, object_id);
    crate::game_loop::weight::refresh_weight_penalty(world, object_id);
    // Java re-pumps passive skill effects on the same equip listeners: an
    // armor-conditioned passive (Spellcraft/Magician's Movement) flips as a
    // robe is worn or removed. Resends its own UserInfo when the set changed.
    crate::game_loop::passive_skills::refresh_conditioned_passives(world, object_id);
    // NB: no shadow-item mana is spent here. Java burns a point in
    // `Player.useEquipableItem` alone — for the one item the player clicked —
    // and this helper stands in for a good deal more than that click: an
    // enchant refreshing a worn item's glow, an augment re-applying its
    // options, `//mount` stripping a weapon. Charging mana from here made a
    // shadow weapon die early for reasons Java never charges for; the call
    // lives at the `use_equipable_item` equip branch instead. See
    // [`super::item_mana`].
}

/// The stat-and-paperdoll half of [`finish_equip_change`]: recompute the
/// wearer's stats, then push the client's own paperdoll snapshot
/// (`ExUserInfoEquipSlot`) and `UserInfo`.
///
/// Java emits `ExUserInfoEquipSlot` from inside `Inventory.setPaperdollItem`,
/// the single choke point *every* paperdoll mutation goes through — including
/// the implicit ones, where nobody called "unequip" at all: `ItemContainer`'s
/// `removeItem` is overridden by `Inventory.removeItem` to unequip whatever it
/// is about to take out of the bag, so dropping, destroying or transferring a
/// worn item refreshes the paperdoll for free. Here the paperdoll lives in a
/// plain data component that cannot reach the client, so each of those paths
/// has to call this itself.
///
/// Forgetting it is not a cosmetic inventory-window bug: `UserInfo` carries
/// only the right-hand *enchant level*, never the paperdoll item ids, so the
/// client keeps rendering a weapon the character no longer owns while the
/// inventory window correctly shows nothing equipped.
pub(crate) fn refresh_equip_state(world: &mut World, client_id: u32, object_id: i32) {
    // Recompute combat stats now that the paperdoll changed: a newly equipped
    // weapon's pAtk / armor's pDef must reach the `UserInfo` below (Java
    // `Inventory.equipItem`/`unEquipItemInBodySlot` → `Creature.recalculateStats`
    // before `broadcastUserInfo`). Without it the client shows the item on the
    // paperdoll but the stat panel never moves.
    if let Some((player, base, mods, inventory, mut vitals, mut speeds, mut combat)) =
        world.objects.get_many_mut::<(
            &crate::model::Player,
            &crate::model::components::BaseStats,
            &crate::model::components::StatModifiers,
            &crate::model::inventory::Inventory,
            &mut crate::model::components::Vitals,
            &mut crate::model::components::Speeds,
            &mut crate::model::components::CombatStats,
        )>(&object_id)
    {
        player.recalculate_stats(&world.data, base, mods, inventory, &mut speeds, &mut combat);
        // Max HP/MP can carry item bonuses (e.g. +MP jewelry), which live in
        // `Vitals` on a separate path from `recalculate_stats`. Recompute them
        // and clamp current values down if a bonus was just removed (Java's
        // MaxHp/MaxMp finalizers run inside the same `recalculateStats`).
        let t = world
            .data
            .player_templates
            .get(player.class_id)
            .or_else(|| world.data.player_templates.get(player.base_class_id))
            .cloned()
            .unwrap_or_default();
        vitals.max_hp =
            crate::model::calc_max_hp(&world.data, &t, player.level, Some(inventory), mods) as i32;
        vitals.max_mp =
            crate::model::calc_max_mp(&world.data, &t, player.level, Some(inventory), mods) as i32;
        vitals.cur_hp = vitals.cur_hp.min(vitals.max_hp as f64);
        vitals.cur_mp = vitals.cur_mp.min(vitals.max_mp as f64);
    }

    let Some(inventory) = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&object_id)
    else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::ex_user_info_equip_slot(
            object_id, inventory,
        ));
        if let Some(v) = crate::model::PlayerView::of_world(world, object_id) {
            cs.send(crate::network::user_info::user_info(
                &v,
                &world.data,
                &world.cfg.character,
                super::party::calculate_relation(world, v.p),
            ));
        }
    }
}

/// The `EtcItem` branch of `UseItem.runImpl` (Java:
/// `ItemHandler.getInstance().getHandler(etcItem)`). Dispatches on
/// `ItemTemplate.handler`; only `ExtractableItems` (pack/box items) is
/// implemented so far. Anything else is consumed as a no-op, matching Java's
/// "Unmanaged Item handler" branch (logged, no visible effect to the player).
fn use_etc_item(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    let handler = {
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        let Some(item) = inventory
            .items()
            .iter()
            .find(|i| i.object_id == item_object_id)
        else {
            return;
        };
        world
            .data
            .item_data
            .get(item.item_id)
            .map(|t| t.handler)
            .unwrap_or_default()
    };
    match handler {
        ItemHandler::ExtractableItems => extract_item(world, client_id, object_id, item_object_id),
        ItemHandler::ItemSkills => use_item_skills(world, client_id, object_id, item_object_id),
        ItemHandler::Seed => use_seed_item(world, client_id, object_id, item_object_id),
        ItemHandler::SoulShots | ItemHandler::SpiritShot | ItemHandler::BlessedSpiritShot => {
            let item_id = world
                .objects
                .get_component::<Inventory>(&object_id)
                .and_then(|inv| {
                    inv.items()
                        .iter()
                        .find(|i| i.object_id == item_object_id)
                        .map(|i| i.item_id)
                });
            if let Some(item_id) = item_id {
                charge_shot(world, object_id, item_id, handler, false);
            }
        }
        // A Beast shot used by hand does nothing: it is spent by the summon's
        // swing (`Summon.rechargeShots`), not by the owner clicking it. Java's
        // `BeastSoulShot` handler likewise only ever runs *from* that path.
        ItemHandler::BeastSoulShot | ItemHandler::BeastSpiritShot => {}
        ItemHandler::EnchantScrolls => {
            super::enchant::open(world, client_id, object_id, item_object_id)
        }
        ItemHandler::Recipes => {
            super::crafting::learn_recipe(world, client_id, object_id, item_object_id)
        }
        // A fishing shot used by hand charges immediately (the fishing engine
        // otherwise charges it on cast via `rechargeShots(fish=true)`).
        ItemHandler::FishShots => {
            let item_id = world
                .objects
                .get_component::<Inventory>(&object_id)
                .and_then(|inv| {
                    inv.items()
                        .iter()
                        .find(|i| i.object_id == item_object_id)
                        .map(|i| i.item_id)
                });
            if let Some(item_id) = item_id {
                charge_fish_shot(world, object_id, item_id);
            }
        }
        ItemHandler::None => {}
    }
}

/// Port of `handlers/itemhandlers/{SoulShots,SpiritShot,BlessedSpiritShot}.useItem`:
/// charge the matching shot on the equipped weapon. `auto` = true is the
/// `rechargeShots` re-entry (an item toggled for auto-use): it suppresses the
/// enable/error chat and the not-enough message, exactly like Java gating those
/// on `!getAutoSoulShot().contains(itemId)`. Returns whether a shot was charged.
///
/// Narrowing vs. Java: the `reducedSoulshot`/`reducedSoulshotChance` weapon
/// perk (a chance to spend fewer shots) isn't modelled — no Interlude weapon in
/// the dist declares it — and the ruby/sapphire brooch visual swap doesn't
/// exist (no jewels), so the shot's own `<skills>` visual always plays.
pub(crate) fn charge_shot(
    world: &mut World,
    object_id: i32,
    shot_item_id: i32,
    handler: ItemHandler,
    auto: bool,
) -> bool {
    use crate::model::{Player, ShotType};

    let physical = handler.is_soulshot();
    let shot_type = match handler {
        ItemHandler::SoulShots => ShotType::Soulshots,
        ItemHandler::SpiritShot => ShotType::Spiritshots,
        ItemHandler::BlessedSpiritShot => ShotType::BlessedSpiritshots,
        _ => return false,
    };
    let client_id = crate::game_loop::helpers::client_for_player(world, object_id);
    let send = |world: &World, msg: i16| {
        if !auto
            && let Some(cid) = client_id
            && let Some(cs) = world.clients.get(&cid)
        {
            cs.send(server_packets::system_message_with(msg, &[]));
        }
    };

    // Equipped weapon + its per-charge shot count / grade.
    let (weapon_item_id, shot_visual) = {
        let Some(inv) = world.objects.get_component::<Inventory>(&object_id) else {
            return false;
        };
        let weapon = inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand);
        let visual = world
            .data
            .item_data
            .get(shot_item_id)
            .map(|t| t.item_skills.clone())
            .unwrap_or_default();
        (weapon, visual)
    };
    let shot_count = if physical {
        world.data.item_data.soulshot_count(weapon_item_id)
    } else {
        world.data.item_data.spiritshot_count(weapon_item_id)
    };

    // No weapon, or a weapon that can't take this shot kind.
    if weapon_item_id == 0 || shot_count == 0 {
        send(
            world,
            if physical {
                sm_ids::CANNOT_USE_SOULSHOTS
            } else {
                sm_ids::YOU_MAY_NOT_USE_SPIRITSHOTS
            },
        );
        return false;
    }

    // Grade must match (`getCrystalTypePlus`).
    let weapon_grade = world
        .data
        .item_data
        .get(weapon_item_id)
        .map(|t| t.crystal_type.plus());
    let shot_grade = world
        .data
        .item_data
        .get(shot_item_id)
        .map(|t| t.crystal_type.plus());
    if weapon_grade != shot_grade {
        send(
            world,
            if physical {
                sm_ids::THE_SOULSHOT_YOU_ARE_ATTEMPTING_TO_USE_DOES_NOT_MATCH_THE_GRADE_OF_YOUR_EQUIPPED_WEAPON
            } else {
                sm_ids::YOUR_SPIRITSHOT_DOES_NOT_MATCH_THE_WEAPON_S_GRADE
            },
        );
        return false;
    }

    // Already charged → no-op (also how the auto path avoids re-spending).
    if world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.is_charged_shot(shot_type))
    {
        return false;
    }

    // Consume the shots; not enough → drop auto-use for this item.
    let have = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(|inv| inv.count_of(shot_item_id))
        .unwrap_or(0);
    if have < shot_count as i64 {
        if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
            p.auto_shots.retain(|&id| id != shot_item_id);
        }
        send(
            world,
            if physical {
                sm_ids::YOU_DO_NOT_HAVE_ENOUGH_SOULSHOTS_FOR_THAT
            } else {
                sm_ids::YOU_DO_NOT_HAVE_ENOUGH_SPIRITSHOT_FOR_THAT
            },
        );
        return false;
    }
    let changes = world
        .objects
        .get_component_mut::<Inventory>(&object_id)
        .map(|inv| inv.remove_item(shot_item_id, shot_count as i64))
        .unwrap_or_default();

    // Charge, notify, replay the count change, play the visual.
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.charge_shot(shot_type);
    }
    if !changes.is_empty()
        && let Some(cid) = client_id
    {
        let iu = ew::inventory_update_changes(&world.data, &changes);
        super::helpers::send_inventory_update(world, cid, object_id, iu);
    }
    send(
        world,
        if physical {
            sm_ids::YOUR_SOULSHOTS_ARE_ENABLED
        } else {
            sm_ids::YOUR_SPIRITSHOT_HAS_BEEN_ENABLED
        },
    );
    broadcast_shot_visual(world, object_id, &shot_visual);
    true
}

/// Port of `clientpackets/RequestAutoSoulShot.runImpl` (player-shot branch —
/// summon shots aren't in scope): toggle a shot item into the auto-use set.
/// Body: `itemId:i32, enable:i32(1/0), type:i32`.
pub(crate) fn handle_request_auto_soul_shot(world: &mut World, client_id: u32, ex_body: &[u8]) {
    use crate::model::Player;

    if ex_body.len() < 12 {
        return;
    }
    let item_id = i32::from_le_bytes(ex_body[0..4].try_into().unwrap());
    let enable = i32::from_le_bytes(ex_body[4..8].try_into().unwrap()) == 1;
    let shot_type = i32::from_le_bytes(ex_body[8..12].try_into().unwrap());

    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    // `!player.isDead()` — a dead player can't toggle shots.
    if world
        .objects
        .get_component::<crate::model::components::Vitals>(&object_id)
        .is_none_or(|v| v.dead)
    {
        return;
    }
    // The item must be in the inventory, and be a player shot we handle.
    let handler = {
        let Some(inv) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        if inv.count_of(item_id) == 0 {
            return;
        }
        world
            .data
            .item_data
            .get(item_id)
            .map(|t| t.handler)
            .unwrap_or_default()
    };
    if !handler.is_soulshot() && !handler.is_spiritshot() && !handler.is_fishshot() {
        return;
    }

    let send = |world: &World, msg: i16, params: &[SmParam]| {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(msg, params));
        }
    };

    // A **summon** shot takes Java's `isSummonShot` branch, which checks that
    // the player *has* a summon and never looks at their weapon — the shots
    // are for the pet's swing, not the owner's.
    let is_summon_shot = matches!(
        handler,
        crate::data::item_data::ItemHandler::BeastSoulShot
            | crate::data::item_data::ItemHandler::BeastSpiritShot
    );
    if enable && is_summon_shot {
        if crate::game_loop::servitor::pet_of(world, object_id).is_none()
            && crate::game_loop::servitor::servitor_of(world, object_id).is_none()
        {
            send(world, sm_ids::YOU_DO_NOT_HAVE_A_SERVITOR_FOR_AUTO_USE, &[]);
            return;
        }
        if let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&object_id)
            && !p.auto_shots.contains(&item_id)
        {
            p.auto_shots.push(item_id);
        }
        send(
            world,
            sm_ids::THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_ACTIVATED,
            &[SmParam::ItemName(item_id)],
        );
        // Java charges the summon immediately on activation.
        if let Some(summon) = crate::game_loop::servitor::pet_of(world, object_id)
            .or_else(|| crate::game_loop::servitor::servitor_of(world, object_id))
        {
            crate::game_loop::servitor::recharge_shots(world, summon, true);
        }
        return;
    }

    if enable {
        // Grade check (`item.getCrystalType() != weapon.getCrystalTypePlus()`,
        // or no weapon at all — fists).
        let weapon_item_id = world
            .objects
            .get_component::<Inventory>(&object_id)
            .map(|inv| inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand))
            .unwrap_or(0);
        let weapon_grade = world
            .data
            .item_data
            .get(weapon_item_id)
            .map(|t| t.crystal_type.plus());
        let shot_grade = world.data.item_data.get(item_id).map(|t| t.crystal_type);
        if weapon_item_id == 0 || weapon_grade != shot_grade {
            send(
                world,
                if handler.is_soulshot() {
                    sm_ids::THE_SOULSHOT_YOU_ARE_ATTEMPTING_TO_USE_DOES_NOT_MATCH_THE_GRADE_OF_YOUR_EQUIPPED_WEAPON
                } else {
                    sm_ids::YOUR_SPIRITSHOT_DOES_NOT_MATCH_THE_WEAPON_S_GRADE
                },
                &[],
            );
            return;
        }
        // Activate.
        if let Some(p) = world.objects.get_component_mut::<Player>(&object_id)
            && !p.auto_shots.contains(&item_id)
        {
            p.auto_shots.push(item_id);
        }
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::ex_auto_soul_shot(item_id, true, shot_type));
        }
        send(
            world,
            sm_ids::THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_ACTIVATED,
            &[SmParam::ItemName(item_id)],
        );
        // Charge immediately (Java `player.rechargeShots(...)`).
        recharge_shots(
            world,
            object_id,
            handler.is_soulshot(),
            handler.is_spiritshot(),
            handler.is_fishshot(),
        );
    } else {
        // Deactivate.
        if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
            p.auto_shots.retain(|&id| id != item_id);
        }
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::ex_auto_soul_shot(item_id, false, shot_type));
        }
        send(
            world,
            sm_ids::THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_DEACTIVATED,
            &[SmParam::ItemName(item_id)],
        );
    }
}

/// Port of `Player.rechargeShots(physical, magic, fish)`: for each shot item
/// the player toggled for auto-use, if its category matches the requested one,
/// (re)charge it. Java runs this at the start of every attack (`physical`) and
/// cast (`magic`). A toggled item that's no longer in the inventory is dropped
/// from the auto set (Java's `removeAutoSoulShot` on `getItemByItemId == null`).
pub(crate) fn recharge_shots(
    world: &mut World,
    object_id: i32,
    physical: bool,
    magic: bool,
    fish: bool,
) {
    let auto = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map(|p| p.auto_shots.clone())
        .unwrap_or_default();
    for item_id in auto {
        if world
            .objects
            .get_component::<Inventory>(&object_id)
            .map(|inv| inv.count_of(item_id))
            .unwrap_or(0)
            == 0
        {
            if let Some(p) = world
                .objects
                .get_component_mut::<crate::model::Player>(&object_id)
            {
                p.auto_shots.retain(|&id| id != item_id);
            }
            continue;
        }
        let handler = world
            .data
            .item_data
            .get(item_id)
            .map(|t| t.handler)
            .unwrap_or_default();
        if (magic && handler.is_spiritshot()) || (physical && handler.is_soulshot()) {
            charge_shot(world, object_id, item_id, handler, true);
        } else if fish && handler.is_fishshot() {
            charge_fish_shot(world, object_id, item_id);
        }
    }
}

/// Java `FishShots` item handler: charge `FISH_SOULSHOTS` and spend one fishing
/// shot. Unlike weapon shots it has no grade/weapon check and always consumes
/// exactly one. Returns whether the flag flipped on.
pub(crate) fn charge_fish_shot(world: &mut World, object_id: i32, shot_item_id: i32) -> bool {
    use crate::model::{Player, ShotType};
    let already = world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.is_charged_shot(ShotType::FishSoulshots));
    if already {
        return false;
    }
    let have = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(|inv| inv.count_of(shot_item_id))
        .unwrap_or(0);
    if have < 1 {
        if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
            p.auto_shots.retain(|&id| id != shot_item_id);
        }
        return false;
    }
    let changes = world
        .objects
        .get_component_mut::<Inventory>(&object_id)
        .map(|inv| inv.remove_item(shot_item_id, 1))
        .unwrap_or_default();
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.charge_shot(ShotType::FishSoulshots);
    }
    if !changes.is_empty()
        && let Some(cid) = crate::game_loop::helpers::client_for_player(world, object_id)
    {
        let iu = ew::inventory_update_changes(&world.data, &changes);
        super::helpers::send_inventory_update(world, cid, object_id, iu);
    }
    true
}

/// `Broadcast.toSelfAndKnownPlayersInRadius(player, new MagicSkillUse(...))`:
/// the shot's `<skills>` (NORMAL) entries as a self-targeted, zero-time
/// `MagicSkillUse` — the client renders the charge glow off it.
fn broadcast_shot_visual(world: &mut World, object_id: i32, skills: &[(i32, i32)]) {
    let Some((player, pos)) = ({
        let p = world
            .objects
            .get_component::<crate::model::Player>(&object_id)
            .cloned();
        let pos = world
            .objects
            .get_component::<crate::model::components::Position>(&object_id)
            .copied();
        p.zip(pos)
    }) else {
        return;
    };
    for &(skill_id, skill_level) in skills {
        let pkt = server_packets::magic_skill_use(
            &player,
            &pos,
            (object_id, pos.x, pos.y, pos.z),
            skill_id,
            skill_level,
            0,
            0,
            0,
        );
        crate::game_loop::helpers::broadcast_including_self(world, object_id, &pkt);
    }
}

/// Port of `handlers/itemhandlers/ItemSkillsTemplate.useItem` (potions, buff
/// scrolls, escape scrolls, …). Each of the item's `<skills>` entries takes
/// one of Java's two branches:
///
/// * **instant** (`SkillCaster.triggerCast`) when the skill is
///   `withoutAction` or the item carries `immediate_effect`/
///   `ex_immediate_effect` — the effects land at once, no cast bar. This is
///   the potion/herb/capsule path.
/// * **cast** (`playable.useMagic(itemSkill, item, …)`) otherwise — a real
///   cast bar of the skill's own `hitTime`, interruptible by damage. This is
///   the scroll path: a Scroll of Escape (736 → 2013) casts for 20 s, a
///   Scroll: Might (3933 → 2057) for 4 s.
///
/// Consumption follows `checkConsume` (see [`check_consume`]) and, as in
/// Java, happens as soon as the branch *starts* — a scroll is spent even if
/// the cast is interrupted, since `useMagic` returning true is what sets
/// `successfulUse`.
///
/// Narrowing: no pets (none exist yet), no Olympiad guard (no Olympiad), no
/// `<cond>` gating (not parsed for items — see `item_data`'s header comment).
/// TODO(G15): Java's busy check is `!isPotion && !isElixir && !isScroll &&
/// isCastingNow()`, and its `useMagic` would *queue* a skill that loses that
/// race. `QueuedAction::Skill` replays by skill id through `use_magic_on`,
/// which would not find an item skill on the player's skill list, so the cast
/// branch just refuses while another cast is running instead of queueing.
/// Port of `handlers/itemhandlers/Seed.useItem` — sow a manor seed on the
/// player's targeted monster: validate the target, flag the mob with the seed
/// (`Attackable.setSeeded(seed, player)`), then cast the item's Sow skill (which
/// runs [`crate::game_loop::skills::effects`]'s `Sow`). The item is consumed by
/// the skill cast, as with any `<skills>` item.
///
/// The sow-location gate (`seed.getCastleId() == target.getTaxCastle()`) is
/// honored, `THIS_SEED_MAY_NOT_BE_SOWN_HERE` included.
fn use_seed_item(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    use crate::model::components::TargetRef;
    use crate::model::npc::Npc;
    use crate::network::server_packets::sm_ids;

    if !world.cfg.general.allow_manor {
        return;
    }
    let item_id = world
        .objects
        .get_component::<Inventory>(&object_id)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|i| i.object_id == item_object_id)
                .map(|i| i.item_id)
        });
    let Some(item_id) = item_id else {
        return;
    };
    let send = |world: &World, sm: i16| {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(sm, &[]));
        }
    };

    // The seeded target is the player's current target.
    let Some(target_oid) = world
        .objects
        .get_component::<TargetRef>(&object_id)
        .and_then(|t| t.0)
        .filter(|oid| crate::game_loop::combat::is_npc_oid(*oid))
    else {
        send(world, sm_ids::INVALID_TARGET);
        return;
    };
    // Must be a live, `canBeSown` monster that isn't already seeded.
    let can_be_sown = world
        .objects
        .get_component::<Npc>(&target_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.can_be_sown);
    let dead = world
        .objects
        .get_component::<crate::model::components::Vitals>(&target_oid)
        .map(|v| v.dead)
        .unwrap_or(true);
    let already_seeded = world
        .objects
        .get_component::<Npc>(&target_oid)
        .map(|n| n.seeded)
        .unwrap_or(false);
    if !can_be_sown || dead {
        // Java: THE_TARGET_IS_UNAVAILABLE_FOR_SEEDING / INVALID_TARGET.
        send(world, sm_ids::INVALID_TARGET);
        return;
    }
    if already_seeded {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    // The seed must be in the catalogue (Java `getSeed(itemId)`)…
    let Some(seed_castle) = world.data.manor.seed_by_id(item_id).map(|s| s.castle_id) else {
        return;
    };
    // …and it may only be sown inside its own castle's territory (Java
    // `(taxCastle == null) || (seed.getCastleId() != taxCastle.getResidenceId())`).
    if crate::game_loop::castle::npc_tax_castle(world, target_oid) != Some(seed_castle) {
        send(world, sm_ids::THIS_SEED_MAY_NOT_BE_SOWN_HERE);
        return;
    }

    // Flag the mob (Java `setSeeded(seed, player)` — sets seed + seeder, not the
    // seeded state; the Sow effect sets that on success).
    if let Some(npc) = world.objects.get_component_mut::<Npc>(&target_oid) {
        npc.seed_id = item_id;
        npc.seeder_object_id = object_id;
    }
    // Cast the item's Sow skill (consumes the seed, applies the `Sow` effect).
    use_item_skills(world, client_id, object_id, item_object_id);
}

/// Drink/consume one carried item by object id, on the player's own behalf —
/// the auto-potion loop's entry into the ordinary item-skill path, so the cast,
/// the cooldown and the consumption are identical to using it by hand.
pub(crate) fn use_item_by_object_id(world: &mut World, player_oid: i32, item_object_id: i32) {
    let Some(client_id) = super::helpers::client_for_player(world, player_oid) else {
        return;
    };
    use_item_skills(world, client_id, player_oid, item_object_id);
}

fn use_item_skills(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    use crate::game_loop::skills::cast::{
        check_skill_reuse, resolve_cast_target, set_skill_reuse, start_casting,
    };
    use crate::game_loop::skills::effects::apply_skill_effects;
    use crate::model::Player;
    use crate::model::components::{Casting, Position, TargetRef};
    use crate::model::skill::TargetType;

    let (item_skills, immediate_effect, ex_immediate_effect, default_action) = {
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        let Some(item) = inventory
            .items()
            .iter()
            .find(|i| i.object_id == item_object_id)
        else {
            return;
        };
        let Some(template) = world.data.item_data.get(item.item_id) else {
            return;
        };
        (
            template.item_skills.clone(),
            template.immediate_effect,
            template.ex_immediate_effect,
            template.default_action,
        )
    };
    if item_skills.is_empty() {
        return;
    }
    // Java's `SummonItems` handler attaches a `PetItemHolder` to the player
    // before casting, because the `SummonPet` effect never receives the item.
    // Park the collar's object id the same way; the effect *takes* it, so an
    // unused one cannot linger into an unrelated cast.
    {
        let is_collar = world
            .objects
            .get_component::<Inventory>(&object_id)
            .and_then(|inv| {
                inv.items()
                    .iter()
                    .find(|i| i.object_id == item_object_id)
                    .map(|i| i.item_id)
            })
            .is_some_and(|item_id| world.data.pet_data.is_pet_collar(item_id));
        if let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&object_id)
        {
            p.pending_pet_collar = if is_collar {
                Some(item_object_id)
            } else {
                None
            };
        }
    }

    let mut used = false;
    // `hasConsumeSkill` — Java sets it for every listed skill, *before* any of
    // the per-skill `continue`s, so a skill that never fires still counts.
    let mut has_consume_skill = false;
    for (skill_id, skill_level) in item_skills {
        let Some(skill) = world.data.skill_data.get(skill_id, skill_level).cloned() else {
            continue;
        };
        if skill.item_consume_id > 0 {
            has_consume_skill = true;
        }
        if !check_skill_reuse(world, client_id, object_id, &skill) {
            continue;
        }
        let target_oid = match skill.target_type {
            TargetType::Self_ => object_id,
            _ => {
                let Some(player) = world.objects.get_component::<Player>(&object_id) else {
                    continue;
                };
                let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() else {
                    continue;
                };
                let target_ref = world
                    .objects
                    .get_component::<TargetRef>(&object_id)
                    .copied()
                    .unwrap_or_default()
                    .0;
                match resolve_cast_target(world, player, &pos, target_ref, &skill, true, false) {
                    Ok(oid) => oid,
                    Err(_) => continue,
                }
            }
        };
        if skill.without_action || immediate_effect || ex_immediate_effect {
            apply_skill_effects(world, object_id, target_oid, &skill);
            set_skill_reuse(world, object_id, &skill);
        } else {
            if world.objects.has_component::<Casting>(&object_id) {
                continue;
            }
            // `start_casting` registers the reuse itself.
            start_casting(world, client_id, object_id, &skill, target_oid);
            // Java `SkillCaster(caster, target, skill, item, …)`: a
            // `SKILL_REDUCE_ON_SKILL_SUCCESS` item rides the cast and is spent
            // by `finishSkill` only if it lands.
            if default_action == crate::data::item_data::ActionType::SkillReduceOnSkillSuccess {
                crate::game_loop::skills::cast::set_cast_trigger_item(
                    world,
                    object_id,
                    item_object_id,
                );
            }
        }
        used = true;
    }

    if used && check_consume(default_action, has_consume_skill, immediate_effect) {
        destroy_used_item(world, client_id, object_id, item_object_id);
    }
}

/// Port of `ItemSkillsTemplate.checkConsume`: whether the *item handler* is
/// the one that destroys the item.
fn check_consume(
    default_action: crate::data::item_data::ActionType,
    has_consume_skill: bool,
    immediate_effect: bool,
) -> bool {
    use crate::data::item_data::ActionType;
    match default_action {
        // Java: `if (!hasConsumeSkill && hasImmediateEffect()) return true;`
        // then falls out of the switch to `return hasConsumeSkill`.
        ActionType::Capsule | ActionType::SkillReduce => has_consume_skill || immediate_effect,
        // Java returns false: these are destroyed by `SkillCaster.finishSkill`
        // when the cast actually *lands* — the cast carries the item
        // (`CastState.trigger_item_object_id`) and the finish phase spends
        // `itemConsumeCount` of it, so an interrupted cast costs nothing.
        ActionType::SkillReduceOnSkillSuccess => false,
        // Summon shots are never consumed by a direct item-use: they are spent
        // by `servitor::recharge_shots` when the summon swings, in the count
        // the pet's level demands. Using one by hand does nothing.
        ActionType::SummonSoulshot | ActionType::SummonSpiritshot => false,
        ActionType::Other => has_consume_skill,
    }
}

/// Destroys one unit of a used etc item and notifies the client — the
/// consume tail shared by `ExtractableItems` and `ItemSkills`.
fn destroy_used_item(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    let Some(destroyed) = ({
        let Some(inventory) = world.objects.get_component_mut::<Inventory>(&object_id) else {
            return;
        };
        inventory.remove_by_object_id(item_object_id, 1)
    }) else {
        return;
    };
    // Memory-first: the count decrement / removal already applied to the
    // `Inventory` component; it persists on the next flush.
    let iu = ew::inventory_update_changes(&world.data, std::slice::from_ref(&destroyed));
    super::helpers::send_inventory_update(world, client_id, object_id, iu);
}

/// Port of `handlers/itemhandlers/ExtractableItems.useItem`: destroys the
/// used item, then rolls its `<capsuled_items>` list and grants what hits.
/// `extractableCountMin == 0` (every currently-loaded pack/box item) takes a
/// single pass over the list; `> 0` re-rolls the whole list until at least
/// that many entries have been granted, mirroring Java's `while` loop (used
/// by "pick one of N" reward boxes) — capped at a generous iteration count
/// so a misconfigured item (chances that can never sum to the minimum)
/// can't hang the single-threaded game loop the way it could a Java
/// per-client thread. Per-entry enchant rolls are skipped (later milestone;
/// nothing currently loaded needs them).
fn extract_item(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    let (capsules, count_min, count_max) = {
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        let Some(item) = inventory
            .items()
            .iter()
            .find(|i| i.object_id == item_object_id)
        else {
            return;
        };
        let Some(template) = world.data.item_data.get(item.item_id) else {
            return;
        };
        (
            template.capsuled_items.clone(),
            template.extractable_count_min.max(0),
            template.extractable_count_max,
        )
    };
    if capsules.is_empty() {
        return;
    }

    // Port of `Player.isInventoryUnder80(false)`, the gate
    // `ExtractableItems.useItem` checks before touching the item: refuse
    // (leaving the box and inventory untouched) if the bag is already too
    // full for the reward roll to have anywhere to go.
    let race = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map(|p| p.race)
        .unwrap_or(0);
    let normal_limit = world.cfg.character.inventory_limit(race);
    let under_80 = world
        .objects
        .get_component::<Inventory>(&object_id)
        .is_some_and(|inv| inv.is_under_80_percent(&world.data.item_data, normal_limit));
    if !under_80 {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                sm_ids::YOUR_INVENTORY_IS_FULL,
                &[],
            ));
        }
        return;
    }

    destroy_used_item(world, client_id, object_id, item_object_id);

    let mut granted: Vec<(i32, i64)> = Vec::new();
    for _ in 0..1000 {
        for product in &capsules {
            if count_max > 0 && granted.len() as i32 >= count_max {
                break;
            }
            if world.roll(100_000) > product.chance {
                continue;
            }
            let span = (product.max - product.min + 1).max(1) as i32;
            let amount = if product.max == product.min {
                product.min
            } else {
                product.min + world.roll(span) as i64
            };
            if amount != 0 {
                granted.push((product.item_id, amount));
            }
        }
        if granted.len() as i32 >= count_min {
            break;
        }
    }

    if granted.is_empty() {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE,
                &[],
            ));
        }
        return;
    }

    for (item_id, amount) in granted {
        let Some(changed_oids) = add_inventory_item(world, object_id, item_id, amount) else {
            warn!("ExtractableItems: object-id pool exhausted, dropping {item_id}x{amount}");
            continue;
        };
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
            continue;
        };
        let iu = ew::inventory_update(inventory, &world.data, &changed_oids);
        if let Some(cs) = world.clients.get(&client_id) {
            let sm = if amount > 1 {
                server_packets::system_message_with(
                    sm_ids::YOU_HAVE_OBTAINED_S2_S1,
                    &[SmParam::ItemName(item_id), SmParam::Long(amount)],
                )
            } else {
                server_packets::system_message_with(
                    sm_ids::YOU_HAVE_OBTAINED_S1,
                    &[SmParam::ItemName(item_id)],
                )
            };
            cs.send(sm);
        }
        super::helpers::send_inventory_update(world, client_id, object_id, iu);
    }
}
