//! Player-to-player trade (Java `TradeList`): request → answer → each side adds
//! items and presses OK → when both confirm, the items swap. One active trade
//! per player; items stay in the owner's inventory until the swap.

use commons::network::PacketReader;

use crate::model::components::{PendingTrade, StoreItem, Trade};
use crate::model::inventory::{Inventory, ItemInstance};
use crate::network::client_packets as cp;
use crate::network::server_packets as sp;
use crate::session::ClientSession;
use crate::world::World;

fn player_of(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}

fn send(world: &World, oid: i32, packet: Vec<u8>) {
    if let Some(cid) = super::helpers::client_for_player(world, oid) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(packet);
        }
    }
}

fn busy(world: &World, oid: i32) -> bool {
    world.objects.has_component::<Trade>(&oid)
        || world
            .objects
            .get_component::<crate::model::Player>(&oid)
            .map(|p| p.store_type)
            .unwrap_or(0)
            != 0
}

/// `TradeRequest` (0x1A): ask the targeted player to trade.
pub(crate) fn handle_request(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(from) = player_of(world, client_id) else {
        return;
    };
    let Some(target) = PacketReader::new(body).read_i32() else {
        return;
    };
    if target == from
        || !world.objects.has_component::<crate::model::Player>(&target)
        || busy(world, target)
        || busy(world, from)
    {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(sp::action_failed());
        }
        return;
    }
    // `//tradeoff`: the partner refuses all trades (Java `getTradeRefusal`).
    if world
        .objects
        .get_component::<crate::model::Player>(&target)
        .is_some_and(|p| p.trade_refusal)
    {
        send(
            world,
            from,
            sp::system_message_with(
                sp::sm_ids::S1_TEXT,
                &[sp::SmParam::Text(
                    "That person is in trade refusal mode.".to_string(),
                )],
            ),
        );
        return;
    }
    world.objects.add_components(&target, PendingTrade { from });
    send(world, target, sp::send_trade_request(from));
}

/// `AnswerTradeRequest` (0x55): accept (1) or decline the pending request.
pub(crate) fn handle_answer(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(me) = player_of(world, client_id) else {
        return;
    };
    let response = PacketReader::new(body).read_i32().unwrap_or(0);
    let requester = world
        .objects
        .get_component::<PendingTrade>(&me)
        .map(|p| p.from);
    world.objects.remove_component::<PendingTrade>(&me);
    let Some(requester) = requester else { return };
    if response != 1
        || busy(world, me)
        || busy(world, requester)
        || !world
            .objects
            .has_component::<crate::model::Player>(&requester)
    {
        return;
    }
    // Open the trade on both sides.
    world.objects.add_components(
        &me,
        Trade {
            partner: requester,
            items: Vec::new(),
            confirmed: false,
        },
    );
    world.objects.add_components(
        &requester,
        Trade {
            partner: me,
            items: Vec::new(),
            confirmed: false,
        },
    );
    open_window(world, me, requester);
    open_window(world, requester, me);
}

/// Send a player their `TradeStart` (their own tradeable items + the partner).
fn open_window(world: &World, viewer: i32, partner: i32) {
    let level = world
        .objects
        .get_component::<crate::model::Player>(&partner)
        .map(|p| p.level as u8)
        .unwrap_or(1);
    let items: Vec<(ItemInstance, &crate::data::item_data::ItemTemplate)> = world
        .objects
        .get_component::<Inventory>(&viewer)
        .map(|inv| {
            inv.items()
                .iter()
                .filter(|it| inv.paperdoll_slot_of(it.object_id).is_none())
                .filter_map(|it| {
                    let t = world.data.item_data.get(it.item_id)?;
                    (!t.is_quest_item).then_some((*it, t))
                })
                .collect()
        })
        .unwrap_or_default();
    send(world, viewer, sp::trade_start(partner, level, &items));
}

/// `AddTradeItem` (0x1B): offer an item into the trade (resets both confirms).
pub(crate) fn handle_add_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::AddTradeItem::read(body) else {
        return;
    };
    let Some(me) = player_of(world, client_id) else {
        return;
    };
    let Some(partner) = world.objects.get_component::<Trade>(&me).map(|t| t.partner) else {
        return;
    };
    // Validate the instance: in inventory, not equipped/quest, count in range,
    // and not the full amount already offered.
    let Some((item_id, held, enchant)) =
        world
            .objects
            .get_component::<Inventory>(&me)
            .and_then(|inv| {
                (inv.paperdoll_slot_of(pkt.object_id).is_none())
                    .then(|| {
                        inv.items()
                            .iter()
                            .find(|it| it.object_id == pkt.object_id)
                            .map(|it| (it.item_id, it.count, it.enchant_level))
                    })
                    .flatten()
            })
    else {
        return;
    };
    if world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.is_quest_item)
    {
        return;
    }
    let already = world
        .objects
        .get_component::<Trade>(&me)
        .map(|t| {
            t.items
                .iter()
                .filter(|s| s.object_id == pkt.object_id)
                .map(|s| s.count)
                .sum::<i64>()
        })
        .unwrap_or(0);
    let add = pkt.count.min(held - already);
    if add <= 0 {
        return;
    }
    // Adding invalidates any prior confirmation on both sides (Java).
    reset_confirms(world, me, partner);
    if let Some(t) = world.objects.get_component_mut::<Trade>(&me) {
        match t.items.iter_mut().find(|s| s.object_id == pkt.object_id) {
            Some(s) => s.count += add,
            None => t.items.push(StoreItem {
                object_id: pkt.object_id,
                item_id,
                count: add,
                price: 0,
                enchant,
            }),
        }
    }
    if let Some(t) = world.data.item_data.get(item_id) {
        let inst = ItemInstance {
            object_id: pkt.object_id,
            item_id,
            count: add,
            enchant_level: enchant,
            custom_type1: 0,
            custom_type2: 0,
            mana_left: -1,
            time: 0,
            augment_mineral: 0,
            augment_option1: 0,
            augment_option2: 0,
        };
        send(world, me, sp::trade_add(true, &inst, t));
        send(world, partner, sp::trade_add(false, &inst, t));
    }
}

/// `TradeDone` (0x1C): confirm (1) or cancel (0). Both confirms → the swap.
pub(crate) fn handle_done(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(me) = player_of(world, client_id) else {
        return;
    };
    let response = PacketReader::new(body).read_i32().unwrap_or(0);
    let Some(partner) = world.objects.get_component::<Trade>(&me).map(|t| t.partner) else {
        return;
    };
    if response != 1 {
        cancel(world, me, partner);
        return;
    }
    if let Some(t) = world.objects.get_component_mut::<Trade>(&me) {
        t.confirmed = true;
    }
    send(world, me, sp::trade_press_ok(true));
    send(world, partner, sp::trade_press_ok(false));
    let both = world
        .objects
        .get_component::<Trade>(&me)
        .is_some_and(|t| t.confirmed)
        && world
            .objects
            .get_component::<Trade>(&partner)
            .is_some_and(|t| t.confirmed);
    if both {
        execute(world, me, partner);
    }
}

fn reset_confirms(world: &mut World, a: i32, b: i32) {
    for oid in [a, b] {
        if let Some(t) = world.objects.get_component_mut::<Trade>(&oid) {
            t.confirmed = false;
        }
    }
}

fn cancel(world: &mut World, me: i32, partner: i32) {
    for oid in [me, partner] {
        world.objects.remove_component::<Trade>(&oid);
        send(world, oid, sp::trade_done(false));
    }
}

/// Swap both sides' offered items, then close the trade.
fn execute(world: &mut World, a: i32, b: i32) {
    transfer_side(world, a, b);
    transfer_side(world, b, a);
    for oid in [a, b] {
        world.objects.remove_component::<Trade>(&oid);
        send(world, oid, sp::trade_done(true));
        refresh(world, oid);
    }
}

/// Move `from`'s offered items to `to`.
fn transfer_side(world: &mut World, from: i32, to: i32) {
    let offered = world
        .objects
        .get_component::<Trade>(&from)
        .map(|t| t.items.clone())
        .unwrap_or_default();
    for line in offered {
        // Clamp to what's actually still held.
        let held = world
            .objects
            .get_component::<Inventory>(&from)
            .and_then(|inv| {
                inv.items()
                    .iter()
                    .find(|it| it.object_id == line.object_id)
                    .map(|it| it.count)
            })
            .unwrap_or(0);
        let n = line.count.min(held);
        if n <= 0 {
            continue;
        }
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&from) {
            inv.remove_by_object_id(line.object_id, n);
        }
        if let Some(new_oid) = world.alloc_object_id() {
            if let Some(inv) = world.objects.get_component_mut::<Inventory>(&to) {
                inv.insert_instance(
                    &world.data.item_data,
                    new_oid,
                    line.item_id,
                    n,
                    line.enchant,
                );
            }
        }
    }
}

fn refresh(world: &World, oid: i32) {
    if let (Some(cid), Some(inv)) = (
        super::helpers::client_for_player(world, oid),
        world.objects.get_component::<Inventory>(&oid),
    ) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(crate::network::enter_world::item_list(
                inv,
                &world.data,
                false,
            ));
        }
    }
}
