//! Merchant shop: `Merchant.showBuyWindow` behind the `Buy` bypass,
//! `RequestBuyItem` (0x40) purchasing, and `RequestSellItem` (0x37) selling
//! back at reference-price/2 (G15). Multisell, limited stock, castle tax, and
//! the weight/capacity gates (no `maxLoad`/slot enforcement exists yet — a G5
//! deferral) are out of scope.

use crate::game_loop::helpers::npc_template;
use crate::game_loop::helpers::send_action_failed;
use tracing::warn;

use crate::data::item_data::ADENA_ID;
use crate::model::components::TargetRef;
use crate::model::inventory::Inventory;
use crate::network::client_packets as cp;
use crate::network::server_packets::sm_ids;
use crate::network::trade;
use crate::world::World;

use super::castle::{handle_tax_payment, npc_tax_rate as merchant_tax_rate};
use super::helpers::send_sm_and_action_failed;
use super::target::can_interact;
use crate::game_loop::helpers::npc_id_of;

/// `MAX_ADENA` (Config.MAX_ADENA default 99 999 999 999).
const MAX_ADENA: i64 = 99_999_999_999;

/// Whether the NPC is a merchant (Java `instanceof Merchant` — the
/// `Merchant`/`Fisherman` instance classes; the `type` attribute stands in
/// for the class hierarchy, like the VillageMaster check).
pub(crate) fn is_merchant(world: &World, npc_object_id: i32) -> bool {
    npc_template(world, npc_object_id)
        .is_some_and(|t| t.type_name == "Merchant" || t.type_name == "Fisherman")
}

/// `bypasshandlers/Buy.java` → `Merchant.showBuyWindow`: the buy tab +
/// the accompanying sell tab. The caller (bypass router) already verified
/// existence and interaction distance.
pub(crate) fn show_buy_window(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    list_id: i32,
) {
    show_buy_window_taxed(world, client_id, player, npc_oid, list_id, true);
}

/// `Merchant.showBuyWindow(player, listId, applyCastleTax)` — the mercenary
/// manager opens its ticket lists with `applyCastleTax = false` ("baseTax is 20%
/// (done in merchant buylists)"). Note Java only skips the tax in the *window*:
/// `RequestBuyItem` still charges (and pays out) the castle rate, so an untaxed
/// display does not mean an untaxed purchase. Kept as-is.
pub(crate) fn show_buy_window_taxed(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    list_id: i32,
    apply_castle_tax: bool,
) {
    let npc_id = npc_id_of(world, npc_oid).unwrap_or(0);
    let Some(list) = world.data.buy_lists.get(list_id) else {
        warn!("Shop: buylist {list_id} not found.");
        send_action_failed(world, client_id);
        return;
    };
    if !list.is_npc_allowed(npc_id) {
        warn!("Shop: npc {npc_id} not allowed in buylist {list_id}.");
        send_action_failed(world, client_id);
        return;
    }
    let tax_rate = if apply_castle_tax {
        merchant_tax_rate(world, npc_oid)
    } else {
        0.0
    };
    let refund_items = refund_items_of(world, player);
    let Some(inventory) = world.objects.get_component::<Inventory>(&player) else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(trade::buy_list(list, inventory, &world.data, tax_rate));
        cs.send(trade::ex_buy_sell_list_sell(
            inventory,
            &refund_items,
            &world.data,
            false,
            crate::game_loop::servitor::active_pet_collar(world, player),
        ));
    }
    // Java `Merchant.showBuyWindow` calls `setInventoryBlockingStatus(true)`
    // just before these sends. It runs after them here only because `list`
    // borrows `world.data` — and the ordering is unobservable, since the flag
    // gates the client's *next* packet, which cannot arrive until this handler
    // has returned.
    super::helpers::block_inventory(world, player);
}

/// Port of `clientpackets/RequestBuyItem.runImpl`, minus the systems that
/// don't exist: karma gate, GM bypasses, limited stock, and the weight/slot
/// capacity checks (no encumbrance enforcement yet). Castle tax is applied and
/// paid into the merchant's castle treasury.
pub(crate) fn handle_request_buy_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestBuyItem::read(body) else {
        return;
    };
    let Some(player) = world.player_oid(client_id) else {
        return;
    };

    // The target must be a merchant within interaction distance.
    let target = world
        .objects
        .get_component::<TargetRef>(&player)
        .copied()
        .unwrap_or_default()
        .0;
    let Some(merchant_oid) =
        target.filter(|&t| is_merchant(world, t) && can_interact(world, player, t))
    else {
        send_action_failed(world, client_id);
        return;
    };
    let merchant_id = npc_id_of(world, merchant_oid).unwrap_or(0);

    let Some(list) = world.data.buy_lists.get(pkt.list_id) else {
        super::punishment::illegal_action(
            world,
            player,
            &format!(
                "Player {player} sent a false BuyList list_id {}",
                pkt.list_id
            ),
        );
        return;
    };
    if !list.is_npc_allowed(merchant_id) {
        send_action_failed(world, client_id);
        return;
    }

    // Java `merchant.getCastleTaxRate(TaxType.BUY)`: 0 outside a tax zone.
    let castle_tax_rate = merchant_tax_rate(world, merchant_oid);

    // Validate every line and total the price (Java's first pass).
    let mut sub_total: i64 = 0;
    for line in &pkt.items {
        let Some(product) = list.product(line.item_id) else {
            super::punishment::illegal_action(
                world,
                player,
                &format!(
                    "Player {player} sent a false BuyList list_id {} and item_id {}",
                    pkt.list_id, line.item_id
                ),
            );
            return;
        };
        let stackable = world
            .data
            .item_data
            .get(line.item_id)
            .is_some_and(|t| t.is_stackable);
        if !stackable && line.count > 1 {
            super::punishment::illegal_action(
                world,
                player,
                &format!(
                    "Player {player} tried to purchase invalid quantity of items at the same time."
                ),
            );
            send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED,
                &[],
            );
            return;
        }
        if product.price < 0 {
            warn!(
                "Shop: no price for item {} on buylist {}.",
                line.item_id, pkt.list_id
            );
            send_action_failed(world, client_id);
            return;
        }
        if MAX_ADENA / line.count < product.price {
            super::punishment::illegal_action(
                world,
                player,
                &format!(
                    "Player {player} tried to purchase over {MAX_ADENA} adena worth of goods."
                ),
            );
            return;
        }
        // Java: per-item price with tax first, then multiply by the count.
        let price = (product.price as f64
            * (1.0 + castle_tax_rate + f64::from(product.base_tax) / 100.0))
            as i64;
        sub_total += line.count * price;
        if sub_total > MAX_ADENA {
            super::punishment::illegal_action(
                world,
                player,
                &format!(
                    "Player {player} tried to purchase over {MAX_ADENA} adena worth of goods."
                ),
            );
            return;
        }
    }

    // Java `RequestBuyItem`'s weight and slot gates, in its position: after the
    // price loop, before anything is charged.
    //
    // **The slot rule is Java's, quirk included** — one slot per *product line*
    // the player holds none of (`getItemByItemId(id) == null`), with no
    // stackability test and no multiplication by count. Buying ten
    // non-stackable swords is therefore charged one slot, not ten. Reusing
    // `weight::slots_needed` is the reasonable-looking mistake: it returns
    // `count` for a non-stackable, a different rule serving different callers.
    //
    // Both checks are skipped outright for a GM (`!player.isGM() && …`), which
    // is broader than `validate_weight`'s own diet-mode exemption.
    let is_gm = world
        .objects
        .get_component::<crate::model::Player>(&player)
        .is_some_and(|p| p.is_gm(&world.data));
    if !is_gm {
        let (mut weight, mut slots): (i64, i64) = (0, 0);
        for line in &pkt.items {
            let unit_weight = world
                .data
                .item_data
                .get(line.item_id)
                .map_or(0, |t| i64::from(t.weight));
            weight = weight.saturating_add(line.count.saturating_mul(unit_weight));
            let holds_none = world
                .objects
                .get_component::<Inventory>(&player)
                .is_none_or(|i| i.count_of(line.item_id) == 0);
            if holds_none {
                slots += 1;
            }
        }
        if weight < 0 || !super::weight::validate_weight(world, player, weight) {
            send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::YOU_HAVE_EXCEEDED_THE_WEIGHT_LIMIT,
                &[],
            );
            return;
        }
        if !super::weight::validate_capacity(world, player, slots) {
            send_sm_and_action_failed(world, client_id, sm_ids::YOUR_INVENTORY_IS_FULL, &[]);
            return;
        }
    }

    // Charge (Java `reduceAdena`) — refuse without touching anything on a
    // shortfall.
    let adena = world
        .objects
        .get_component::<Inventory>(&player)
        .map(|i| i.adena())
        .unwrap_or(0);
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
        if let Some(oids) =
            super::items::add_inventory_item(world, player, line.item_id, line.count)
        {
            added.extend(oids);
        }
    }
    // Java `merchant.handleTaxPayment(subTotal * castleTaxRate)` — the castle
    // whose tax zone the merchant stands in takes its cut, after the buyer has
    // already paid the taxed price.
    handle_tax_payment(
        world,
        merchant_oid,
        (sub_total as f64 * castle_tax_rate) as i64,
    );

    let refund_items = refund_items_of(world, player);
    let iu = world
        .objects
        .get_component::<Inventory>(&player)
        .map(|inventory| {
            crate::network::enter_world::inventory_update(inventory, &world.data, &added)
        });
    if let Some(iu) = iu {
        super::helpers::send_inventory_update(world, client_id, player, iu);
    }
    if let (Some(inventory), Some(cs)) = (
        world.objects.get_component::<Inventory>(&player),
        world.clients.get(&client_id),
    ) {
        cs.send(trade::ex_buy_sell_list_sell(
            inventory,
            &refund_items,
            &world.data,
            true,
            crate::game_loop::servitor::active_pet_collar(world, player),
        ));
        cs.send(crate::network::enter_world::system_message(
            sm_ids::EXCHANGE_IS_SUCCESSFUL,
        ));
    }
}

/// Port of `clientpackets/RequestSellItem.runImpl`: sell inventory items to the
/// targeted merchant for adena (reference price / 2 each). The buy-list gate is
/// skipped (a merchant buys anything sellable); the sellable gate is Java's
/// `Item.isSellable` — the `is_sellable` template flag (which already covers
/// adena and this dist's quest items) plus unaugmented — and equipped items
/// are refused. Sold items move to the `Refund` buy-back container (Java
/// `Config.ALLOW_REFUND`, on for this dist).
pub(crate) fn handle_request_sell_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestSellItem::read(body) else {
        return;
    };
    let Some(player) = world.player_oid(client_id) else {
        return;
    };

    let target = world
        .objects
        .get_component::<TargetRef>(&player)
        .copied()
        .unwrap_or_default()
        .0;
    if target
        .filter(|&t| is_merchant(world, t) && can_interact(world, player, t))
        .is_none()
    {
        send_action_failed(world, client_id);
        return;
    }

    let mut total_price: i64 = 0;
    let mut changes: Vec<crate::model::inventory::ItemChange> = Vec::new();
    let mut sold: Vec<crate::model::inventory::ItemInstance> = Vec::new();
    for (obj_id, item_id, count) in pkt.items {
        // The instance must exist, match the claimed item id, and be unequipped.
        let Some((inst, equipped)) =
            world
                .objects
                .get_component::<Inventory>(&player)
                .and_then(|inv| {
                    inv.items()
                        .iter()
                        .find(|it| it.object_id == obj_id && it.item_id == item_id)
                        .map(|it| (*it, inv.paperdoll_slot_of(obj_id).is_some()))
                })
        else {
            continue;
        };
        let t = world.data.item_data.get(item_id);
        let price = t.map(|t| t.price).unwrap_or(0);
        let quest = t.map(|t| t.is_quest_item).unwrap_or(false);
        let sellable = t.map(|t| t.is_sellable).unwrap_or(false);
        if equipped || quest || !sellable || inst.is_augmented() {
            continue;
        }
        let sell = count.min(inst.count);
        total_price = total_price
            .saturating_add((price / 2) * sell)
            .min(MAX_ADENA);
        if let Some(change) = world
            .objects
            .get_component_mut::<Inventory>(&player)
            .and_then(|inv| inv.remove_by_object_id(obj_id, sell))
        {
            // The refund entry is the sold chunk. A full removal keeps the
            // instance's identity; a partial one (stackables) leaves the
            // original in the inventory, so the split gets a fresh object id
            // (Java `transferItem` splits the same way).
            let mut refund_inst = inst;
            refund_inst.count = sell;
            match change {
                crate::model::inventory::ItemChange::Removed(_) => sold.push(refund_inst),
                crate::model::inventory::ItemChange::Modified(_) => {
                    if let Some(new_oid) = world.alloc_object_id() {
                        refund_inst.object_id = new_oid;
                        sold.push(refund_inst);
                    }
                }
            }
            changes.push(change);
        }
    }

    if !sold.is_empty() {
        if world
            .objects
            .get_component::<crate::model::inventory::Refund>(&player)
            .is_none()
        {
            world
                .objects
                .add_components(&player, crate::model::inventory::Refund::default());
        }
        if let Some(refund) = world
            .objects
            .get_component_mut::<crate::model::inventory::Refund>(&player)
        {
            for inst in sold {
                refund.push(inst);
            }
        }
    }

    if total_price > 0 {
        super::items::add_inventory_item(world, player, ADENA_ID, total_price);
        // Fold the (grown) adena stack into the same InventoryUpdate.
        if let Some(adena) = world
            .objects
            .get_component::<Inventory>(&player)
            .and_then(|inv| inv.first_of_item(ADENA_ID).copied())
        {
            changes.push(crate::model::inventory::ItemChange::Modified(adena));
        }
    }
    if changes.is_empty() {
        return;
    }
    let iu = crate::network::enter_world::inventory_update_changes(&world.data, &changes);
    let refund_items = refund_items_of(world, player);
    let collar = crate::game_loop::servitor::active_pet_collar(world, player);
    super::helpers::send_inventory_update(world, client_id, player, iu);
    if let Some((cs, inv)) = world
        .clients
        .get(&client_id)
        .zip(world.objects.get_component::<Inventory>(&player))
    {
        cs.send(trade::ex_buy_sell_list_sell(
            inv,
            &refund_items,
            &world.data,
            true,
            collar,
        ));
    }
}

/// Snapshot of the player's refund container (empty when none exists yet).
pub(crate) fn refund_items_of(
    world: &World,
    player: i32,
) -> Vec<crate::model::inventory::ItemInstance> {
    world
        .objects
        .get_component::<crate::model::inventory::Refund>(&player)
        .map(|r| r.items().to_vec())
        .unwrap_or_default()
}

/// Port of `clientpackets/RequestRefundItem.runImpl` (ex 0x72): buy back items
/// from the refund tab at the same half-reference-price. The buy-list gate is
/// skipped like the sell path; the weight/slot capacity checks are the same
/// G5 encumbrance deferral as `RequestBuyItem`.
pub(crate) fn handle_request_refund_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestRefundItem::read(body) else {
        return;
    };
    let Some(player) = world.player_oid(client_id) else {
        return;
    };

    let target = world
        .objects
        .get_component::<TargetRef>(&player)
        .copied()
        .unwrap_or_default()
        .0;
    if target
        .filter(|&t| is_merchant(world, t) && can_interact(world, player, t))
        .is_none()
    {
        send_action_failed(world, client_id);
        return;
    }

    // Validate the requested slots against the container (Java refuses the
    // whole request on any bad or duplicate index) and total the price.
    let refund_items = refund_items_of(world, player);
    let mut adena_cost: i64 = 0;
    for (i, &idx) in pkt.indexes.iter().enumerate() {
        if idx < 0 || idx as usize >= refund_items.len() || pkt.indexes[..i].contains(&idx) {
            send_action_failed(world, client_id);
            return;
        }
        let inst = &refund_items[idx as usize];
        let price = world
            .data
            .item_data
            .get(inst.item_id)
            .map(|t| t.price)
            .unwrap_or(0);
        adena_cost = adena_cost.saturating_add((price / 2) * inst.count);
    }

    let held_adena = world
        .objects
        .get_component::<Inventory>(&player)
        .map(|i| i.adena())
        .unwrap_or(0);
    if held_adena < adena_cost
        || (adena_cost > 0
            && !super::quests::take_items(world, client_id, player, ADENA_ID, adena_cost))
    {
        send_sm_and_action_failed(world, client_id, sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA, &[]);
        return;
    }

    // Move the bought-back items home. Indexes are taken highest-first so the
    // remaining container positions stay valid while removing.
    let mut indexes = pkt.indexes;
    indexes.sort_unstable_by(|a, b| b.cmp(a));
    let mut restored: Vec<crate::model::inventory::ItemInstance> = Vec::new();
    for idx in indexes {
        let Some(inst) = world
            .objects
            .get_component_mut::<crate::model::inventory::Refund>(&player)
            .and_then(|r| r.take(idx as usize))
        else {
            continue;
        };
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
            restored.push(inv.restore_instance(&world.data.item_data, inst));
        }
    }

    let changes: Vec<crate::model::inventory::ItemChange> = restored
        .into_iter()
        .map(crate::model::inventory::ItemChange::Modified)
        .collect();
    let iu = crate::network::enter_world::inventory_update_changes(&world.data, &changes);
    let refund_items = refund_items_of(world, player);
    let collar = crate::game_loop::servitor::active_pet_collar(world, player);
    super::helpers::send_inventory_update(world, client_id, player, iu);
    if let Some((cs, inv)) = world
        .clients
        .get(&client_id)
        .zip(world.objects.get_component::<Inventory>(&player))
    {
        cs.send(trade::ex_buy_sell_list_sell(
            inv,
            &refund_items,
            &world.data,
            true,
            collar,
        ));
    }
}
