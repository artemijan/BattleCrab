//! Item commands — `AdminCreateItem`'s `//create_item`, `//give_item_target`,
//! `//give_item_to_all`, `//create_coin`, the `//itemcreate`/`//enchant` HTML
//! menus, and `AdminDestroyItems`' inventory-wipe commands.

use crate::game_loop::guard;
use crate::game_loop::helpers::nth_arg;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::send_to_client;
use crate::model::inventory::{Inventory, ItemChange};
use crate::session::ClientSession;
use crate::world::World;

use super::send_message;

/// `AdminCreateItem`'s `//create_item [id] [num]` — create `num` (default 1) of
/// item `id` on the GM, then (always, exactly like Java) reopen
/// `itemcreation.htm`. Java tokenizes `command.substring(17)`: 1 token → count 1,
/// 2 tokens → given count, 0 or 3+ tokens → nothing created and no error (the
/// "Item" main-menu button with an empty QuickBox sends 0 tokens, so it just
/// opens the menu). A non-numeric id/count → "Specify a valid number.".
pub(super) fn admin_create_item(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    if matches!(args.len(), 1 | 2) {
        match parse_id_count(args) {
            Some((id, num)) => create_item(world, client_id, object_id, object_id, id, num),
            None => send_message(world, client_id, "Specify a valid number."),
        }
    }
    super::menu::show_admin_html(world, client_id, "itemcreation.htm");
}

/// Java `AdminCreateItem.createItem` — give `num` of `id` to `target`, message
/// the GM (and the target if it isn't the GM), and refresh the target's item
/// list + adena counter.
fn create_item(world: &mut World, gm_client: u32, gm_oid: i32, target: i32, id: i32, num: i64) {
    let Some(template) = world.data.item_data.get(id) else {
        send_message(world, gm_client, "This item doesn't exist.");
        return;
    };
    let name = template.name.clone();
    if num > 10 && !template.is_stackable {
        send_message(
            world,
            gm_client,
            "This item does not stack - Creation aborted.",
        );
        return;
    }
    if crate::game_loop::items::add_inventory_item(world, target, id, num).is_none() {
        return;
    }
    // target.sendMessage(...) only when the target is another player.
    if target != gm_oid
        && let Some(tcid) = super::helpers::client_for_player(world, target)
    {
        send_message(
            world,
            tcid,
            &format!("Admin spawned {num} {name} in your inventory."),
        );
    }
    // target.sendItemList(false) + ExAdenaInvenCount.
    if let Some(tcid) = super::helpers::client_for_player(world, target)
        && let Some(inv) = world.objects.get_component::<Inventory>(&target)
    {
        let list = crate::network::enter_world::item_list(inv, &world.data, false);
        let adena = crate::network::enter_world::ex_adena_inven_count(inv);
        if let Some(cs) = world.clients.get(&tcid) {
            cs.send(list);
            cs.send(adena);
        }
    }
    let target_name = player_name_or_empty(world, target);
    send_message(
        world,
        gm_client,
        &format!("You have spawned {num} {name}({id}) in {target_name} inventory."),
    );
}

/// Parse Java's `<id> [num]` tokens (1 or 2). `None` on any non-numeric token
/// (Java `NumberFormatException` → "Specify a valid number.").
fn parse_id_count(args: &[&str]) -> Option<(i32, i64)> {
    let id = args.first()?.parse::<i32>().ok()?;
    let num = match args.get(1) {
        Some(s) => s.parse::<i64>().ok()?,
        None => 1,
    };
    Some((id, num))
}

/// `AdminCreateItem`'s `//give_item_target <id> [count]` — give to the targeted
/// player (or the GM if none is selected).
pub(super) fn admin_give_item_target(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let (Some(item_id), count) = parse_item_args(args) else {
        send_message(world, client_id, "Usage: //give_item_target <id> [count]");
        return;
    };
    if world.data.item_data.get(item_id).is_none() {
        send_message(
            world,
            client_id,
            &format!("Item id {item_id} does not exist."),
        );
        return;
    }
    let target = guard::player_target(world, object_id).unwrap_or(object_id);
    let Some(tcid) = super::helpers::client_for_player(world, target) else {
        return;
    };
    super::quests::give_item_with_earned_message(world, tcid, target, item_id, count);
}

/// `AdminCreateItem`'s `//give_item_to_all <id> [count]` — give to every online
/// player.
pub(super) fn admin_give_item_to_all(world: &mut World, client_id: u32, args: &[&str]) {
    let (Some(item_id), count) = parse_item_args(args) else {
        send_message(world, client_id, "Usage: //give_item_to_all <id> [count]");
        return;
    };
    if world.data.item_data.get(item_id).is_none() {
        send_message(
            world,
            client_id,
            &format!("Item id {item_id} does not exist."),
        );
        return;
    }
    let recipients: Vec<(u32, i32)> = world
        .clients
        .iter()
        .filter_map(|(&cid, cs)| match cs {
            ClientSession::InGame(s) => Some((cid, s.player_object_id())),
            _ => None,
        })
        .collect();
    let count_given = recipients.len();
    for (cid, oid) in recipients {
        super::quests::give_item_with_earned_message(world, cid, oid, item_id, count);
    }
    send_message(
        world,
        client_id,
        &format!("Gave item {item_id} to {count_given} player(s)."),
    );
}

/// `AdminCreateItem`'s `//create_coin <name> [amount]` — create a named coin
/// (adena and the alt currencies) on the GM.
pub(super) fn admin_create_coin(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(name) = args.first() else {
        send_message(world, client_id, "Usage: //create_coin <name> [amount]");
        return;
    };
    let Some(item_id) = coin_id(name) else {
        send_message(world, client_id, "Unknown coin name.");
        return;
    };
    let count = nth_arg::<i64>(args, 1).unwrap_or(1).max(1);
    super::quests::give_item_with_earned_message(world, client_id, object_id, item_id, count);
}

/// Java `AdminCreateItem.getCoinId` — the fixed name→item-id table.
fn coin_id(name: &str) -> Option<i32> {
    Some(match name.to_lowercase().as_str() {
        "adena" => 57,
        "ancientadena" => 5575,
        "festivaladena" => 6673,
        "blueeva" => 4355,
        "goldeinhasad" => 4356,
        "silvershilen" => 4357,
        "bloodypaagrio" => 4358,
        "fantasyislecoin" => 13067,
        _ => return None,
    })
}

/// `AdminCreateItem`'s `//itemcreate` / `AdminEnchant`'s `//enchant` — open the
/// corresponding admin HTML menu.
pub(super) fn admin_item_menu(world: &mut World, client_id: u32, page: &str) {
    super::menu::show_admin_html(world, client_id, page);
}

/// `AdminDestroyItems` — wipe the GM's own inventory. The `all` variants
/// (`//destroy_all_items`, `//destroyallitems`) also destroy equipped items;
/// the plain variants skip equipped gear (Java `command.contains("all")`).
pub(super) fn admin_destroy_items(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    include_equipped: bool,
) {
    let changes: Vec<ItemChange> = {
        let Some(inv) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        // Snapshot the targets first (object_id + count), so we don't mutate the
        // list while borrowing it.
        let targets: Vec<(i32, i64)> = inv
            .items()
            .iter()
            .filter(|it| include_equipped || inv.paperdoll_slot_of(it.object_id).is_none())
            .map(|it| (it.object_id, it.count))
            .collect();
        let Some(inv) = world.objects.get_component_mut::<Inventory>(&object_id) else {
            return;
        };
        targets
            .into_iter()
            .filter_map(|(oid, count)| inv.remove_by_object_id(oid, count))
            .collect()
    };
    if changes.is_empty() {
        send_message(world, client_id, "No items to destroy.");
        return;
    }
    let packet = crate::network::enter_world::inventory_update_changes(&world.data, &changes);
    super::helpers::send_inventory_update(world, client_id, object_id, packet);
    // Equipment/appearance may have changed (equipped gear destroyed). The
    // GM's own paperdoll comes from `ExUserInfoEquipSlot`, which neither the
    // `InventoryUpdate` above nor `broadcastUserInfo` carries.
    if include_equipped {
        crate::game_loop::items::refresh_equip_state(world, client_id, object_id);
    }
    super::party::broadcast_user_info(world, object_id);
    send_message(
        world,
        client_id,
        &format!("Destroyed {} item(s).", changes.len()),
    );
}

/// `AdminCreateItem`'s `//delete_item <objectId> [count]` — destroy part or all
/// of a single stack from its owner's inventory.
///
/// The argument is the *item's object id*, not its template id: Java resolves it
/// through `World.findObject(idval)` and then destroys by that same id. Object
/// ids are what the `GMViewItemList` windows (`//show_pet_inv`) and the `items`
/// table's `object_id` column carry. Java's token parse is 1 token → count 1,
/// 2 tokens → the given count, and a count of 0 means the whole stack.
///
/// Deviation: Java looks the item up in the world object table and reports
/// "Player is not online." when the owner is offline. The Rust world only holds
/// items that live in a loaded inventory, so the owner scan *is* the lookup, and
/// an unknown id and an offline owner collapse into one message.
pub(super) fn admin_delete_item(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(item_oid) = nth_arg::<i32>(args, 0) else {
        send_message(world, client_id, "Usage: //delete_item <objectId> [count]");
        return;
    };
    let requested = nth_arg::<i64>(args, 1).unwrap_or(1);
    let players: Vec<i32> = world.in_game_player_oids().collect();
    let owned = players.into_iter().find_map(|oid| {
        let inv = world.objects.get_component::<Inventory>(&oid)?;
        let stack = inv.by_object_id(item_oid)?;
        Some((oid, stack.count))
    });
    let Some((owner, stack_count)) = owned else {
        send_message(
            world,
            client_id,
            &format!("No online player owns item object {item_oid}."),
        );
        return;
    };
    // Java: `numval == 0` destroys the whole stack.
    let count = if requested <= 0 {
        stack_count
    } else {
        requested.min(stack_count)
    };
    let Some(change) = world
        .objects
        .get_component_mut::<Inventory>(&owner)
        .and_then(|inv| inv.remove_by_object_id(item_oid, count))
    else {
        send_message(world, client_id, "Item could not be destroyed.");
        return;
    };
    // The owner's own client needs the InventoryUpdate; the GM gets the same
    // refreshed item list Java answers with.
    if let Some(owner_cid) = super::helpers::client_for_player(world, owner) {
        let packet = crate::network::enter_world::inventory_update_changes(
            &world.data,
            std::slice::from_ref(&change),
        );
        super::helpers::send_inventory_update(world, owner_cid, owner, packet);
    }
    if let Some(inv) = world.objects.get_component::<Inventory>(&owner) {
        let name = player_name_or_empty(world, owner);
        let pkt = crate::network::enter_world::gm_view_item_list(&name, inv, &world.data);
        send_to_client(world, client_id, pkt);
    }
    super::party::broadcast_user_info(world, owner);
    send_message(world, client_id, "Item deleted.");
}

/// `//delete_quest_item <itemId> [count] [charName]` — destroy items by their
/// *template* id, the counterpart to [`admin_delete_item`]'s object-id form.
///
/// `count` defaults to **everything** the holder has of that id, which is what
/// clearing a quest reagent means; pass a count to trim a stack instead (`…
/// 2716 1` takes one Ghoul's Skin off the pile). `charName` defaults to the
/// GM's target, and to the GM when nothing playable is targeted. The count and
/// name are told apart by parsing, so `… 2716 Ameno` and `… 2716 3 Ameno` both
/// work.
///
/// Custom command — Java has no item-id destroyer other than
/// `//destroy_all_items`, which wipes the whole inventory. Deliberately *not*
/// gated on `is_quest_item`: most quest reagents (2716 Ghoul's Skin among them)
/// carry no `is_questitem` flag in the item XML at all, so that gate would
/// refuse exactly the items this exists for.
///
/// [`admin_delete_item`]: admin_delete_item
pub(super) fn admin_delete_quest_item(
    world: &mut World,
    client_id: u32,
    gm_oid: i32,
    args: &[&str],
) {
    let Some(item_id) = nth_arg::<i32>(args, 0) else {
        send_message(
            world,
            client_id,
            "Usage: //delete_quest_item <itemId> [count] [charName] (no count = all)",
        );
        return;
    };
    // `[count] [name]`, either omitted: a numeric second token is the count.
    let (count_arg, name_arg) = match (args.get(1), args.get(2)) {
        (Some(a), Some(b)) => (a.parse::<i64>().ok(), Some(*b)),
        (Some(a), None) => match a.parse::<i64>() {
            Ok(n) => (Some(n), None),
            Err(_) => (None, Some(*a)),
        },
        _ => (None, None),
    };
    let target = match name_arg {
        Some(name) => match super::find_online_player(world, name) {
            Some(oid) => oid,
            None => {
                send_message(world, client_id, &format!("Player '{name}' is not online."));
                return;
            }
        },
        None => guard::player_target(world, gm_oid).unwrap_or(gm_oid),
    };
    let held = world
        .objects
        .get_component::<Inventory>(&target)
        .map(|inv| inv.count_of(item_id))
        .unwrap_or(0);
    let name = player_name_or_empty(world, target);
    if held <= 0 {
        send_message(
            world,
            client_id,
            &format!("{name} holds no item {item_id}."),
        );
        return;
    }
    // No count, or a non-positive one, clears the lot.
    let count = match count_arg {
        Some(n) if n > 0 => n.min(held),
        _ => held,
    };
    // A GM can destroy gear the target is wearing, so this takes the destroy
    // protocol rather than a bare removal.
    let changes = crate::game_loop::items::destroy_item_by_id(world, target, item_id, count);
    if changes.is_empty() {
        send_message(world, client_id, "Item could not be destroyed.");
        return;
    }
    if let Some(target_cid) = super::helpers::client_for_player(world, target) {
        let packet = crate::network::enter_world::inventory_update_changes(&world.data, &changes);
        super::helpers::send_inventory_update(world, target_cid, target, packet);
    }
    if let Some(inv) = world.objects.get_component::<Inventory>(&target) {
        let pkt = crate::network::enter_world::gm_view_item_list(&name, inv, &world.data);
        send_to_client(world, client_id, pkt);
    }
    super::party::broadcast_user_info(world, target);
    send_message(
        world,
        client_id,
        &format!(
            "Destroyed {count} of item {item_id} on {name} ({} left).",
            held - count
        ),
    );
}

/// Parse `<id> [count]` — item id (required) and count (default 1, min 1).
fn parse_item_args(args: &[&str]) -> (Option<i32>, i64) {
    let item_id = nth_arg::<i32>(args, 0);
    let count = nth_arg::<i64>(args, 1).unwrap_or(1).max(1);
    (item_id, count)
}
