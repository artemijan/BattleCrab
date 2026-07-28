//! Item commands — `AdminCreateItem`'s `//create_item`, `//give_item_target`,
//! `//give_item_to_all`, `//create_coin`, the `//itemcreate`/`//enchant` HTML
//! menus, and `AdminDestroyItems`' inventory-wipe commands.

use crate::model::Player;
use crate::model::inventory::{Inventory, ItemChange};
use crate::session::ClientSession;
use crate::world::World;

use super::{current_target, send_message};

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
    let target_name = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.name.clone())
        .unwrap_or_default();
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
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
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
    let count = args
        .get(1)
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(1)
        .max(1);
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
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
    // Equipment/appearance may have changed (equipped gear destroyed).
    super::party::broadcast_user_info(world, object_id);
    send_message(
        world,
        client_id,
        &format!("Destroyed {} item(s).", changes.len()),
    );
}

/// Parse `<id> [count]` — item id (required) and count (default 1, min 1).
fn parse_item_args(args: &[&str]) -> (Option<i32>, i64) {
    let item_id = args.first().and_then(|s| s.parse::<i32>().ok());
    let count = args
        .get(1)
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(1)
        .max(1);
    (item_id, count)
}
