//! Item-granting commands — `AdminCreateItem`'s `//create_item`,
//! `//give_item_target`, and `//give_item_to_all`.

use crate::model::Player;
use crate::session::ClientSession;
use crate::world::World;

use super::{current_target, send_message};

/// `AdminCreateItem`'s `//create_item <id> [count]` — create an item on the GM.
pub(super) fn admin_create_item(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(item_id), count) = parse_item_args(args) else {
        send_message(world, client_id, "Usage: //create_item <id> [count]");
        return;
    };
    if world.data.item_data.get(item_id).is_none() {
        send_message(world, client_id, &format!("Item id {item_id} does not exist."));
        return;
    }
    super::quests::give_item_with_earned_message(world, client_id, object_id, item_id, count);
}

/// `AdminCreateItem`'s `//give_item_target <id> [count]` — give to the targeted
/// player (or the GM if none is selected).
pub(super) fn admin_give_item_target(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(item_id), count) = parse_item_args(args) else {
        send_message(world, client_id, "Usage: //give_item_target <id> [count]");
        return;
    };
    if world.data.item_data.get(item_id).is_none() {
        send_message(world, client_id, &format!("Item id {item_id} does not exist."));
        return;
    }
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    let Some(tcid) = super::helpers::client_for_player(world, target) else { return };
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
        send_message(world, client_id, &format!("Item id {item_id} does not exist."));
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
    send_message(world, client_id, &format!("Gave item {item_id} to {count_given} player(s)."));
}

/// Parse `<id> [count]` — item id (required) and count (default 1, min 1).
fn parse_item_args(args: &[&str]) -> (Option<i32>, i64) {
    let item_id = args.first().and_then(|s| s.parse::<i32>().ok());
    let count = args.get(1).and_then(|s| s.parse::<i64>().ok()).unwrap_or(1).max(1);
    (item_id, count)
}
