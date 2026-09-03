//! Packet handlers: item list, use-item entry, destroy, crystallize and
//! inventory-order save.

use super::cursed_weapon_blocks_equip;
use super::finish_equip_change;
use super::unequip_if_worn;
use super::use_equipable_item;
use crate::game_loop::character::inventory;
use crate::game_loop::character::inventory::add_inventory_item;
use crate::game_loop::helpers::send_action_failed;
use crate::game_loop::helpers::send_to_client;
use crate::model::inventory::Inventory;
use crate::network::client_packets as cp;
use crate::network::enter_world as ew;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::world::World;
/// Port of `clientpackets/RequestItemList.runImpl`: the client opened its
/// inventory window and wants the current contents. Java calls
/// `player.sendItemList(true)`, which (after a 300 ms debounce we don't
/// replicate — there's no per-client timer here) sends `ItemList` with the
/// show-window flag set, then `ExQuestItemList`, `ExAdenaInvenCount` and
/// `ExUserInfoInvenWeight`. The `isInventoryDisabled` guard is a no-op: nothing
/// in this port blocks the inventory yet (set only by trades/some skills, both
/// unported).
pub(crate) fn handle_request_item_list(world: &mut World, client_id: u32) {
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    // `if (player.isInventoryDisabled()) return;` — the client fires spurious
    // item-list requests while a shop/warehouse/wear window is opening, and
    // answering them redraws the inventory over the window it just asked for.
    if world.inventory_blocked.contains(&object_id) {
        return;
    }
    let max_load = crate::game_loop::stats::weight::max_load(world, object_id);
    let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
        return;
    };
    send_to_client(
        world,
        client_id,
        ew::item_list(inventory, &world.data, true),
    );
    send_to_client(
        world,
        client_id,
        ew::ex_quest_item_list(inventory, &world.data),
    );
    send_to_client(world, client_id, ew::ex_adena_inven_count(inventory));
    send_to_client(
        world,
        client_id,
        ew::ex_user_info_inven_weight(object_id, inventory, &world.data, max_load),
    );
}

/// Port of `clientpackets/UseItem.runImpl`: right-clicking a `Weapon`/`Armor`
/// toggles equip/unequip; anything else routes through the `EtcItem` handler
/// dispatch (Java: `ItemHandler.getInstance().getHandler(etcItem)`).
pub(crate) fn handle_use_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::UseItem::read(body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    // `UseItem.runImpl`: `hasBlockActions() || isControlBlocked() ||
    // isAlikeDead()` refuses the use outright. (Death is gated further in.)
    if crate::game_loop::abnormal::is_blocked_from_actions(world, object_id)
        || crate::game_loop::abnormal::is_control_blocked(world, object_id)
    {
        send_action_failed(world, client_id);
        return;
    }
    // `if (!item.isEquipped() && !item.getTemplate().checkCondition(player,
    // player, true))` — the item's `<cond>` blocks, plus the Olympiad and
    // event gates in the same function. Only an item being *put on* or used is
    // checked: taking one off is always allowed, which is what lets a player
    // out of gear they no longer meet the terms for.
    if !item_is_equipped(world, object_id, pkt.object_id)
        && !condition_allows(world, object_id, pkt.object_id, true)
    {
        return;
    }
    if cursed_weapon_blocks_equip(world, object_id, pkt.object_id) {
        return; // Java returns with no packet at all.
    }
    use_equipable_item(world, client_id, object_id, pkt.object_id);
}

/// Port of `clientpackets/RequestUnEquipItem.runImpl` (the mid-attack /
/// mid-cast guards are still skipped).
pub(crate) fn handle_request_un_equip_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(body_part) = cp::read_char_slot(body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
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
    let Some(inventory) = world.objects.get_component_mut::<Inventory>(&object_id) else {
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
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    // Java: `_count < 0` punishes; `_count == 0` is a plain refusal.
    if pkt.count < 0 {
        crate::game_loop::moderation::punishment::illegal_action(
            world,
            object_id,
            &format!(
                "[RequestDestroyItem] Player {object_id} tried to destroy item with oid {} but has count < 0!",
                pkt.object_id
            ),
        );
        return;
    }
    if pkt.count == 0 {
        return;
    }
    // Locate the item + its template facts.
    let Some((item_id, held, is_stackable, undestroyable)) = world
        .objects
        .get_component::<Inventory>(&object_id)
        .and_then(|inv| {
            inv.by_object_id(pkt.object_id)
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
    // Java's whole refusal is one expression:
    //
    // ```java
    // if (!Config.DESTROY_ALL_ITEMS
    //     && ((!canOverrideCond(DESTROY_ALL_ITEMS) && !itemToRemove.isDestroyable())
    //         || CursedWeaponsManager.getInstance().isCursed(itemId)))
    // ```
    //
    // Two things fall out of that shape. `DestroyAllItems` (**False** here)
    // switches off the gate *entirely*, cursed weapons included. And the
    // `DESTROY_ALL_ITEMS` override exempts a holder from the undestroyable
    // half only — `isCursed` sits outside its parenthesis, so a GM still
    // cannot delete their way out of a curse and strand the manager's row.
    if !world.cfg.general.destroy_all_items {
        let overrides = world
            .objects
            .get_component::<crate::model::Player>(&object_id)
            .is_some_and(|p| {
                p.can_override_cond(crate::game_loop::admin::DESTROY_ALL_ITEMS_ORDINAL)
            });
        if (undestroyable && !overrides) || super::cursed_weapon::is_cursed_item(world, item_id) {
            send_item_message(world, client_id, "This item cannot be destroyed.");
            return;
        }
    }
    // A non-stackable item can only be destroyed one at a time; asking for
    // more punishes (Java `handleIllegalPlayerAction`).
    if !is_stackable && pkt.count > 1 {
        crate::game_loop::moderation::punishment::illegal_action(
            world,
            object_id,
            &format!(
                "[RequestDestroyItem] Player {object_id} tried to destroy a non-stackable item with oid {} but has count > 1!",
                pkt.object_id
            ),
        );
        return;
    }
    let count = pkt.count.min(held);

    // Unequip first if it's worn (Java unequips, sending its own InventoryUpdate).
    unequip_if_worn(world, client_id, object_id, pkt.object_id);

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
                // Capture the pet's state first (Java `unSummon` → `storeMe`);
                // without this a voluntary recall dropped every delta since
                // the summon.
                crate::game_loop::servitor::sync_pet_row(world, object_id);
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

    let Some(change) =
        inventory::remove_inventory_item_change(world, object_id, pkt.object_id, count)
    else {
        return;
    };
    // No explicit audit call here: `remove_by_object_id` noted the loss, and
    // `drain_item_audit` turns it into a record on the next tick. Recording it
    // here as well would double-count exactly the destroys people look at most.
    inventory::send_inventory_update(world, object_id, vec![change]);
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
    let Some(player_oid) = world.player_oid(client_id) else {
        return;
    };
    // Java: `_count <= 0` is a punish ("[RequestCrystallizeItem] count <= 0!").
    if pkt.count <= 0 {
        crate::game_loop::moderation::punishment::illegal_action(
            world,
            player_oid,
            &format!(
                "[RequestCrystallizeItem] count <= 0! ban! oid: {} owner: {player_oid}",
                pkt.object_id
            ),
        );
        return;
    }
    // Locate the item + its crystallization facts.
    let Some((item_id, held, is_stackable)) = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| {
            inv.by_object_id(pkt.object_id)
                // Java's first guard: `itemToRemove.isShadowItem() ||
                // isTimeLimitedItem()` → plain `ActionFailed`, no message.
                // Without it a coupon-bought shadow weapon could be melted
                // into free D-grade crystals the minute it was handed over.
                .filter(|it| !crate::game_loop::items::item_mana::is_shadow_item(it.mana_left))
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
    unequip_if_worn(world, client_id, player_oid, pkt.object_id);
    let Some(removed) =
        inventory::remove_inventory_item_change(world, player_oid, pkt.object_id, count)
    else {
        return;
    };
    let total = crystal_count as i64 * count;
    add_inventory_item(world, player_oid, crystal_item, total);
    // InventoryUpdate: the destroyed item + the crystal stack (as a modify).
    let mut changes = vec![removed];
    if let Some(inv) = world.objects.get_component::<Inventory>(&player_oid)
        && let Some(stack) = inv.first_of_item(crystal_item)
    {
        changes.push(crate::model::inventory::ItemChange::Modified(*stack));
    }
    inventory::send_inventory_update(world, player_oid, changes);
}

/// Send a bare `$s1` system-message line to one client.
pub(crate) fn send_item_message(world: &World, client_id: u32, text: &str) {
    send_to_client(
        world,
        client_id,
        server_packets::system_message_with(sm_ids::S1_TEXT, &[SmParam::Text(text.to_string())]),
    );
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
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
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

/// `Item.isEquipped()` for an inventory item of `object_id`.
fn item_is_equipped(world: &World, object_id: i32, item_object_id: i32) -> bool {
    world
        .objects
        .get_component::<Inventory>(&object_id)
        .is_some_and(|inv| inv.paperdoll_slot_of(item_object_id).is_some())
}

/// `ItemTemplate.checkCondition` for an item held by `object_id`, looked up by
/// object id. An item whose template is missing is not gated — the catalogue
/// is the same one the inventory was built from, so this cannot happen for a
/// real item, and refusing would be a worse failure than allowing.
fn condition_allows(
    world: &World,
    object_id: i32,
    item_object_id: i32,
    send_message: bool,
) -> bool {
    let Some(item_id) = world
        .objects
        .get_component::<Inventory>(&object_id)
        .and_then(|inv| inv.by_object_id(item_object_id).map(|it| it.item_id))
    else {
        return true;
    };
    let Some(template) = world.data.item_data.get(item_id) else {
        return true;
    };
    super::check_condition(world, object_id, template, send_message)
}
