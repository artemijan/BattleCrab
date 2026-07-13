//! The Buy shop slice (G12): `Merchant.showBuyWindow` behind the `Buy`
//! bypass, and `RequestBuyItem` (0x40) actually purchasing. Sell
//! (`RequestSellItem`), multisell, limited stock, castle tax, and the
//! weight/capacity gates (no `maxLoad`/slot enforcement exists yet — a G5
//! deferral) are out of scope.

use tracing::warn;

use crate::data::item_data::ADENA_ID;
use crate::model::components::TargetRef;
use crate::model::inventory::Inventory;
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, sm_ids};
use crate::network::trade;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::send_sm_and_action_failed;
use super::target::can_interact;

/// `MAX_ADENA` (Config.MAX_ADENA default 99 999 999 999).
const MAX_ADENA: i64 = 99_999_999_999;

/// Whether the NPC is a merchant (Java `instanceof Merchant` — the
/// `Merchant`/`Fisherman` instance classes; the `type` attribute stands in
/// for the class hierarchy, like the VillageMaster check).
pub(crate) fn is_merchant(world: &World, npc_object_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_object_id)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.type_name == "Merchant" || t.type_name == "Fisherman")
}

/// `bypasshandlers/Buy.java` → `Merchant.showBuyWindow`: the buy tab +
/// the accompanying sell tab. The caller (bypass router) already verified
/// existence and interaction distance.
pub(crate) fn show_buy_window(world: &mut World, client_id: u32, player: i32, npc_oid: i32, list_id: i32) {
    let npc_id = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .map(|n| n.npc_id)
        .unwrap_or(0);
    let Some(list) = world.data.buy_lists.get(list_id) else {
        warn!("Shop: buylist {list_id} not found.");
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    };
    if !list.is_npc_allowed(npc_id) {
        warn!("Shop: npc {npc_id} not allowed in buylist {list_id}.");
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    let Some(inventory) = world.objects.get_component::<Inventory>(&player) else { return };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(trade::buy_list(list, inventory, &world.data));
        cs.send(trade::ex_buy_sell_list_sell(inventory, &world.data, false));
    }
}

/// Port of `clientpackets/RequestBuyItem.runImpl`, minus the systems that
/// don't exist: karma gate, GM bypasses, castle tax, limited stock, and the
/// weight/slot capacity checks (no encumbrance enforcement yet).
pub(crate) fn handle_request_buy_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestBuyItem::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();

    // The target must be a merchant within interaction distance.
    let target = world.objects.get_component::<TargetRef>(&player).copied().unwrap_or_default().0;
    let Some(merchant_oid) = target.filter(|&t| is_merchant(world, t) && can_interact(world, player, t)) else {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    };
    let merchant_id =
        world.objects.get_component::<crate::model::npc::Npc>(&merchant_oid).map(|n| n.npc_id).unwrap_or(0);

    let Some(list) = world.data.buy_lists.get(pkt.list_id) else {
        warn!("Shop: player {player} sent a false buylist id {}.", pkt.list_id);
        return;
    };
    if !list.is_npc_allowed(merchant_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    // Validate every line and total the price (Java's first pass).
    let mut sub_total: i64 = 0;
    for line in &pkt.items {
        let Some(product) = list.product(line.item_id) else {
            warn!("Shop: player {player} sent an item {} not on buylist {}.", line.item_id, pkt.list_id);
            return;
        };
        let stackable = world.data.item_data.get(line.item_id).is_some_and(|t| t.is_stackable);
        if !stackable && line.count > 1 {
            send_sm_and_action_failed(world, client_id, sm_ids::YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED, &[]);
            return;
        }
        if product.price < 0 {
            warn!("Shop: no price for item {} on buylist {}.", line.item_id, pkt.list_id);
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::action_failed());
            }
            return;
        }
        if MAX_ADENA / line.count < product.price {
            return;
        }
        let price = (product.price as f64 * (1.0 + product.base_tax as f64 / 100.0)) as i64;
        sub_total += line.count * price;
        if sub_total > MAX_ADENA {
            return;
        }
    }

    // Charge (Java `reduceAdena`) — refuse without touching anything on a
    // shortfall.
    let adena = world.objects.get_component::<Inventory>(&player).map(|i| i.adena()).unwrap_or(0);
    if adena < sub_total {
        send_sm_and_action_failed(world, client_id, sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA, &[]);
        return;
    }
    if sub_total > 0 && !super::quests::take_items(world, client_id, player, ADENA_ID, sub_total) {
        send_sm_and_action_failed(world, client_id, sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA, &[]);
        return;
    }

    // Deliver.
    let mut added: Vec<i32> = Vec::new();
    for line in &pkt.items {
        if let Some(oid) = super::items::add_inventory_item(world, player, line.item_id, line.count) {
            added.push(oid);
        }
    }
    if let (Some(inventory), Some(cs)) =
        (world.objects.get_component::<Inventory>(&player), world.clients.get(&client_id))
    {
        cs.send(crate::network::enter_world::inventory_update(inventory, &world.data, &added));
        cs.send(crate::network::enter_world::ex_user_info_inven_weight(player, inventory, &world.data));
        cs.send(trade::ex_buy_sell_list_sell(inventory, &world.data, true));
        cs.send(crate::network::enter_world::system_message(sm_ids::EXCHANGE_IS_SUCCESSFUL));
    }
}
