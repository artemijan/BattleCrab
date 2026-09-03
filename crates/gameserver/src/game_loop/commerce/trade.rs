//! Player-to-player trade (Java `TradeList`): request → answer → each side adds
//! items and presses OK → when both confirm, the items swap. One active trade
//! per player; items stay in the owner's inventory until the swap.

use crate::game_loop::character::inventory;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::helpers::{player_of, send_to_player as send};
use crate::model::components::{PendingTrade, StoreItem, Trade};
use crate::model::inventory::{Inventory, ItemInstance};
use crate::network::client_packets as cp;
use crate::network::server_packets as sp;
use crate::world::World;
use commons::network::PacketReader;

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
        send_to_client(world, client_id, sp::action_failed());
        return;
    }
    // Java `TradeRequest`: a `BOT_PENALTY` buff whose `BlockAction` list holds
    // `TRADE_ACTION_BLOCK_ID` refuses the trade outright — the bot-report
    // punishment at 100/125 reports.
    if crate::game_loop::moderation::bot_report::is_action_blocked(
        world,
        from,
        crate::game_loop::moderation::bot_report::TRADE_ACTION_BLOCK_ID,
    ) {
        crate::game_loop::helpers::send_sm_to_player(
            world,
            from,
            sp::sm_ids::REPORTED_SO_YOUR_ACTIONS_ARE_RESTRICTED,
            &[],
        );
        send_to_client(world, client_id, sp::action_failed());
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
    // Java `TradeRequest`: `BlockList.isBlocked(partner, player)`, right after
    // the trade-refusal check and before the 150-unit range test.
    if crate::game_loop::social::chat::block_list::is_blocked(world, target, from) {
        let partner_name = player_name_or_empty(world, target);
        send(
            world,
            from,
            sp::system_message_with(
                sp::sm_ids::C1_HAS_PLACED_YOU_ON_HIS_HER_IGNORE_LIST,
                &[sp::SmParam::Text(partner_name)],
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
    // `TradeStart` reads `getAvailableItems(true, canOverrideCond(ITEM_CONDITIONS)
    // && GM_TRADE_RESTRICTED_ITEMS, false)`. Named for the override it reads,
    // because `handle_add_item` below has its own differently-shaped exemption
    // on the same config key and one shared name would hide that.
    let lists_bound_items = world.cfg.general.gm_trade_restricted_items
        && world
            .objects
            .get_component::<crate::model::Player>(&viewer)
            .is_some_and(|p| p.can_override_cond(crate::game_loop::admin::ITEM_CONDITIONS_ORDINAL));
    let items: Vec<(ItemInstance, &crate::data::item_data::ItemTemplate)> = world
        .objects
        .get_component::<Inventory>(&viewer)
        .map(|inv| {
            inv.unequipped_with_templates(&world.data.item_data)
                // Java `TradeList.addItem` refuses untradable items, so the
                // window never lists them either — unless the viewer is exempt,
                // in which case Java lists them and `addItem` still decides.
                .filter(|(_, t)| lists_bound_items || (!t.is_quest_item && t.is_tradable()))
                .map(|(it, t)| (*it, t))
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
                        inv.by_object_id(pkt.object_id)
                            .map(|it| (it.item_id, it.count, it.enchant_level))
                    })
                    .flatten()
            })
    else {
        return;
    };
    // Java `TradeList.addItem`: `!(item.isTradeable() || (isGM() &&
    // GM_TRADE_RESTRICTED_ITEMS)) || item.isQuestItem()`.
    //
    // Two things this site does **not** share with the `TradeStart` list that
    // fills the window: it reads plain `isGM()` rather than an override, and
    // the exemption covers only the tradeable half — `isQuestItem()` sits
    // outside the parenthesis, so a quest item is refused even to an exempt
    // GM. Hence a separate name: the window may list an item this refuses.
    let may_offer_untradeable =
        world.cfg.general.gm_trade_restricted_items && crate::game_loop::helpers::is_gm(world, me);
    if world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.is_quest_item || (!t.is_tradable() && !may_offer_untradeable))
    {
        crate::game_loop::helpers::send_sm_and_action_failed(
            world,
            client_id,
            sp::sm_ids::THIS_ITEM_CANNOT_BE_TRADED_OR_SOLD,
            &[],
        );
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
        let inst = ItemInstance::detached(pkt.object_id, item_id, add, enchant);
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
        inventory::send_inventory_item_list(world, oid);
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
            .and_then(|inv| inv.by_object_id(line.object_id).map(|it| it.count))
            .unwrap_or(0);
        let n = line.count.min(held);
        if n <= 0 {
            continue;
        }
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&from) {
            inv.remove_by_object_id(line.object_id, n);
        }
        inventory::give_transferred_item(world, to, line.item_id, n, line.enchant);
    }
}
