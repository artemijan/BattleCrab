//! Merchant shop: `Merchant.showBuyWindow` behind the `Buy` bypass,
//! `RequestBuyItem` (0x40) purchasing, and `RequestSellItem` (0x37) selling
//! back at reference-price/2 (G15). Multisell is out of scope.
//!
//! This module also owns the runtime half of `model/buylist/Product` —
//! `getCount`/`decreaseCount`/`restock` and the `BuyListTaskManager` timer
//! behind them. Java keeps the count on the product; the port keeps
//! `world.data` immutable and hangs [`ProductStock`] off the world instead
//! (see [`crate::world::World::buy_list_stock`]). The static half —
//! `count`/`restock_delay` parsing, prices, the grade filter — is in
//! [`crate::data::buy_list_data`].

use crate::game_loop::helpers::npc_template;
use crate::game_loop::helpers::send_action_failed;
use tracing::warn;

use crate::data::buy_list_data::Product;
use crate::data::item_data::ADENA_ID;
use crate::game_loop::helpers;
use crate::game_loop::helpers::send_message;
use crate::model::components::TargetRef;
use crate::model::inventory::Inventory;
use crate::network::client_packets as cp;
use crate::network::server_packets::sm_ids;
use crate::network::trade;
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::castle::{handle_tax_payment, npc_tax_rate as merchant_tax_rate};
use super::helpers::send_sm_and_action_failed;
use super::target::can_interact;
use crate::game_loop::helpers::npc_id_of;

/// `MAX_ADENA` (Config.MAX_ADENA default 99 999 999 999).
const MAX_ADENA: i64 = 99_999_999_999;

/// What Java stores in `Product._count` plus what `BuyListTaskManager` holds
/// for it — kept together because they are written together on every sale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductStock {
    /// `Product.getCount()` — remaining, never below 0 in the getter.
    pub count: i64,
    /// The absolute epoch-ms deadline `BuyListTaskManager` holds, and the
    /// `next_restock_time` column. **0 means no timer is armed**, matching
    /// `getRestockDelay`'s `getOrDefault(product, 0L)`.
    pub next_restock_time: i64,
}

/// `Product.getCount()` for a product that may never have been touched.
/// An absent entry is a full shelf, which is why nothing is pre-filled.
pub(crate) fn stock_left(world: &World, list_id: i32, product: &Product) -> i64 {
    if !product.has_limited_stock() {
        // `_count == null` → 0. Not a bug to route around: the BuyList packet
        // writes `getCount()` straight into the quantity field, and 0 is what
        // the client reads as "no limit shown".
        return 0;
    }
    world
        .buy_list_stock
        .get(&(list_id, product.item_id))
        .map_or(product.max_count, |s| s.count.max(0))
}

/// `Product.decreaseCount(value)`: arm the restock timer if one is not already
/// running, then subtract. Java returns whether the result stayed at or above
/// zero and **skips delivery when it did not** — an oversell hands the buyer
/// nothing, having already taken their adena. Reproduced; the validation pass
/// makes it unreachable.
fn decrease_count(world: &mut World, list_id: i32, product: &Product, value: i64) -> bool {
    let key = (list_id, product.item_id);
    let mut stock = world
        .buy_list_stock
        .get(&key)
        .copied()
        .unwrap_or(ProductStock {
            count: product.max_count,
            next_restock_time: 0,
        });
    // `BuyListTaskManager.add` is a no-op when the product already has a
    // deadline, so the clock starts at the *first* sale after a restock and
    // is not pushed back by later ones.
    if stock.next_restock_time == 0 {
        stock.next_restock_time = world.now_millis() + product.restock_delay_ms;
        world.scheduler.schedule(
            world.tick + restock_ticks(product.restock_delay_ms),
            ScheduledTask::BuyListRestock {
                list_id,
                item_id: product.item_id,
            },
        );
    }
    stock.count -= value;
    let ok = stock.count >= 0;
    world.buy_list_stock.insert(key, stock);
    save_stock(world, list_id, product.item_id, stock);
    ok
}

/// `Product.restock()` — back to `maxCount`, timer cleared. Dropping the entry
/// says exactly that, since an absent key is a full shelf.
pub(crate) fn handle_restock(world: &mut World, list_id: i32, item_id: i32) {
    let Some(product) = world
        .data
        .buy_lists
        .get(list_id)
        .and_then(|l| l.product(item_id))
    else {
        return;
    };
    let max_count = product.max_count;
    world.buy_list_stock.remove(&(list_id, item_id));
    save_stock(
        world,
        list_id,
        item_id,
        ProductStock {
            count: max_count,
            next_restock_time: 0,
        },
    );
}

/// `Product.restartRestockTask(nextRestockTime)` at boot: resume a deadline
/// that outlived the shutdown, or restock immediately if it has already passed.
pub(crate) fn restart_restock_task(world: &mut World, list_id: i32, item_id: i32, deadline: i64) {
    let remaining = deadline - world.now_millis();
    if remaining > 0 {
        world.scheduler.schedule(
            world.tick + restock_ticks(remaining),
            ScheduledTask::BuyListRestock { list_id, item_id },
        );
    } else {
        handle_restock(world, list_id, item_id);
    }
}

/// Java's restock task polls every 60 s, so a product restocks up to a minute
/// *late* there; the port's scheduler fires on the deadline itself. The
/// shortest delay this dist declares is an hour, so the difference is under
/// 2 % either way.
fn restock_ticks(delay_ms: i64) -> u64 {
    (delay_ms.max(0) as u64 * super::time::TICKS_PER_SECOND).div_ceil(1000)
}

fn save_stock(world: &mut World, list_id: i32, item_id: i32, stock: ProductStock) {
    // `Product.save()` — an upsert on every decrease and every restock.
    let _ = world.db.send(crate::db::DbCommand::SaveBuyListStock {
        list_id,
        item_id,
        count: stock.count.max(0),
        next_restock_time: stock.next_restock_time,
    });
}

/// Whether the NPC is a merchant (Java `instanceof Merchant` — the
/// `Merchant`/`Fisherman` instance classes; the `type` attribute stands in
/// for the class hierarchy, like the VillageMaster check).
pub(crate) fn is_merchant(world: &World, npc_object_id: i32) -> bool {
    npc_template(world, npc_object_id)
        .is_some_and(|t| t.type_name == "Merchant" || t.type_name == "Fisherman")
}

/// The `Merchant merchant = (target instanceof Merchant) ? … : null` prologue
/// the buy / sell / refund packets all open with: the *currently selected*
/// target, but only if it is a merchant still in interaction range. Reading
/// the target instead of trusting the packet is what stops a hand-built
/// `RequestBuyItem` from shopping at an NPC across the map.
fn targeted_merchant(world: &World, player: i32) -> Option<i32> {
    world
        .objects
        .get_component::<TargetRef>(&player)
        .copied()
        .unwrap_or_default()
        .0
        .filter(|&t| is_merchant(world, t) && can_interact(world, player, t))
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
    let packet = trade::buy_list(
        list,
        inventory,
        &world.data,
        tax_rate,
        world.cfg.rates.rate_siege_guards_price,
        |p| stock_left(world, list_id, p),
    );
    helpers::send_to_client(world, client_id, packet);
    helpers::send_to_client(
        world,
        client_id,
        trade::ex_buy_sell_list_sell(
            inventory,
            &refund_items,
            &world.data,
            false,
            crate::game_loop::servitor::active_pet_collar(world, player),
        ),
    );
    // Java `Merchant.showBuyWindow` calls `setInventoryBlockingStatus(true)`
    // just before these sends. It runs after them here only because `list`
    // borrows `world.data` — and the ordering is unobservable, since the flag
    // gates the client's *next* packet, which cannot arrive until this handler
    // has returned.
    helpers::block_inventory(world, player);
}

/// Port of `clientpackets/RequestBuyItem.runImpl`, minus the karma gate and
/// the GM bypasses. Castle tax is applied and paid into the merchant's castle
/// treasury; limited stock is gated here and consumed in the delivery loop,
/// and the weight and slot checks are Java's, quirk included (see below).
pub(crate) fn handle_request_buy_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestBuyItem::read(body) else {
        return;
    };
    let Some(player) = world.player_oid(client_id) else {
        return;
    };

    // The target must be a merchant within interaction distance.
    let Some(merchant_oid) = targeted_merchant(world, player) else {
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
        let template = world.data.item_data.get(line.item_id);
        let stackable = template.is_some_and(|t| t.is_stackable);
        // `product.getPrice()` — read once here, as Java does, so the
        // CASTLE_GUARD rate lands on the validation *and* the charge.
        let unit_price = template.map_or(product.price, |t| {
            product.price_at(t, world.cfg.rates.rate_siege_guards_price)
        });
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
        if unit_price < 0 {
            warn!(
                "Shop: no price for item {} on buylist {}.",
                line.item_id, pkt.list_id
            );
            send_action_failed(world, client_id);
            return;
        }
        // `Config.ONLY_GM_ITEMS_FREE` (True here). A 0 price survives the
        // loader only on a list with no `<npcs>` block — the GM shop — and no
        // such list can be opened at a merchant, so this is the second lock on
        // a door that is already shut. Java punishes rather than refusing.
        //
        // The key was hard-coded to this dist's `True` behind that comment;
        // reading it is what lets an operator who ships free rows actually get
        // them.
        if world.cfg.general.only_gm_items_free && unit_price == 0 && !helpers::is_gm(world, player)
        {
            send_message(
                world,
                client_id,
                "Ohh Cheat dont work? You have a problem now!",
            );
            super::punishment::illegal_action(
                world,
                player,
                &format!("Player {player} tried buy item for 0 adena."),
            );
            return;
        }
        // "trying to buy more then available" — refused silently, before any
        // adena moves. `stock_left` is the whole shelf for a product nobody
        // has bought from yet.
        if product.has_limited_stock() && line.count > stock_left(world, pkt.list_id, product) {
            send_action_failed(world, client_id);
            return;
        }
        if MAX_ADENA / line.count < unit_price {
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
        let price = (unit_price as f64
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
    if !helpers::is_gm(world, player) {
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

    // Deliver. A limited-stock line is decremented first and handed over only
    // if the decrement stayed at or above zero, which is Java's order.
    let mut added: Vec<crate::model::inventory::ItemChange> = Vec::new();
    for line in &pkt.items {
        let Some(product) = world
            .data
            .buy_lists
            .get(pkt.list_id)
            .and_then(|l| l.product(line.item_id))
            .cloned()
        else {
            continue;
        };
        if product.has_limited_stock() && !decrease_count(world, pkt.list_id, &product, line.count)
        {
            continue;
        }
        if let Some(changes) =
            helpers::add_inventory_item_changes(world, player, line.item_id, line.count)
        {
            added.extend(changes);
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
    helpers::send_inventory_update(world, player, added);
    if let Some(inventory) = world.objects.get_component::<Inventory>(&player) {
        helpers::send_to_client(
            world,
            client_id,
            trade::ex_buy_sell_list_sell(
                inventory,
                &refund_items,
                &world.data,
                true,
                crate::game_loop::servitor::active_pet_collar(world, player),
            ),
        );
        helpers::send_to_client(
            world,
            client_id,
            crate::network::enter_world::system_message(sm_ids::EXCHANGE_IS_SUCCESSFUL),
        );
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

    if targeted_merchant(world, player).is_none() {
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
        let unit_price = t.map(|t| t.sell_price()).unwrap_or(0);
        let quest = t.map(|t| t.is_quest_item).unwrap_or(false);
        let sellable = t.map(|t| t.is_sellable).unwrap_or(false);
        if equipped || quest || !sellable || inst.is_augmented() {
            continue;
        }
        let sell = count.min(inst.count);
        total_price = total_price.saturating_add(unit_price * sell).min(MAX_ADENA);
        if let Some(change) = helpers::remove_inventory_item_change(world, player, obj_id, sell) {
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
                // `remove_by_object_id` never produces an `Added`.
                crate::model::inventory::ItemChange::Added(_) => {}
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
    send_sell_list_refresh(world, client_id, player, changes);
}

/// The tail every sell-tab mutation ends on: push the inventory delta, then
/// redraw the sell window (`ExBuySellList` in sell mode) so the refund tab and
/// the pet-collar exclusion match the bag the client now holds.
fn send_sell_list_refresh(
    world: &mut World,
    client_id: u32,
    player: i32,
    changes: Vec<crate::model::inventory::ItemChange>,
) {
    let refund_items = refund_items_of(world, player);
    let collar = crate::game_loop::servitor::active_pet_collar(world, player);
    helpers::send_inventory_update(world, player, changes);
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

    if targeted_merchant(world, player).is_none() {
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
        let unit_price = world
            .data
            .item_data
            .get(inst.item_id)
            .map(|t| t.sell_price())
            .unwrap_or(0);
        adena_cost = adena_cost.saturating_add(unit_price * inst.count);
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
    send_sell_list_refresh(world, client_id, player, changes);
}

/// `RequestPreviewItem` (0xC7) — the merchant window's "try on" button.
///
/// The outfit is **not** equipped: nothing enters the inventory and no stat
/// changes, the client is simply told to draw those items for `WearDelay`
/// seconds. What it costs is real, though — `WearPrice` **per slot**.
///
/// SKIP(census): Java's Kamael weapon/armour filters (rapier, crossbow,
/// ancient sword, heavy/magic armour) are skipped — Kamael is a Gracia race
/// and no character on this dist can be one, so every branch is unreachable.
pub(crate) fn handle_request_preview_item(world: &mut World, client_id: u32, body: &[u8]) {
    use crate::model::inventory::PaperdollSlot;
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = commons::network::PacketReader::new(body);
    let Some([_unk, list_id, count]) = r.read_i32_array::<3>() else {
        return;
    };
    // "prevent too long lists" — Java abandons the *read*, so nothing runs.
    if count > 100 {
        return;
    }
    let count = count.max(0);
    let mut wanted = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let Some(id) = r.read_i32() else { return };
        wanted.push(id);
    }

    // `AltGameKarmaPlayerCanShop` — a criminal may not even window-shop.
    let reputation = world
        .objects
        .get_component::<crate::model::Player>(&player)
        .map_or(0, |p| p.reputation);
    if !world.cfg.character.alt_karma_player_can_shop && reputation < 0 {
        return;
    }
    // The merchant + range gate, skipped outright for a GM (who may preview
    // from the `//gmshop` window with nothing targeted).
    let is_merchant_target = targeted_merchant(world, player).is_some();
    if !helpers::is_gm(world, player) && !is_merchant_target {
        return;
    }
    if count < 1 || list_id >= 4_000_000 {
        send_action_failed(world, client_id);
        return;
    }
    let Some(list) = world.data.buy_lists.get(list_id) else {
        super::punishment::illegal_action(
            world,
            player,
            &format!("Player {player} sent a false BuyList list_id {list_id}"),
        );
        return;
    };

    let mut outfit: std::collections::HashMap<PaperdollSlot, i32> =
        std::collections::HashMap::new();
    let mut total_price: i64 = 0;
    let wear_price = i64::from(world.cfg.general.wear_price);
    for &item_id in &wanted {
        if list.product(item_id).is_none() {
            super::punishment::illegal_action(
                world,
                player,
                &format!(
                    "Player {player} sent a false BuyList list_id {list_id} and item_id {item_id}"
                ),
            );
            return;
        }
        let Some(template) = world.data.item_data.get(item_id) else {
            continue;
        };
        // Anything that does not go in a paperdoll slot is silently skipped,
        // not refused — Java `continue`s, so a potion in the list costs
        // nothing and shows nothing.
        let Some(slot) = Inventory::primary_slot(template.body_part) else {
            continue;
        };
        if outfit.contains_key(&slot) {
            send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::YOU_CAN_NOT_TRY_THOSE_ITEMS_ON_AT_THE_SAME_TIME,
                &[],
            );
            return;
        }
        outfit.insert(slot, item_id);
        total_price += wear_price;
        if total_price > MAX_ADENA {
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

    // "a Try On is not Free".
    if total_price > 0
        && !super::quests::take_items(world, client_id, player, ADENA_ID, total_price)
    {
        send_sm_and_action_failed(world, client_id, sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA, &[]);
        return;
    }
    if outfit.is_empty() {
        return;
    }
    helpers::send_to_client(
        world,
        client_id,
        crate::network::server_packets::shop_preview_info(&outfit),
    );
    // `ThreadPool.schedule(new RemoveWearItemsTask(player), WEAR_DELAY * 1000)`.
    let delay = (world.cfg.general.wear_delay.max(0) as u64) * super::time::TICKS_PER_SECOND;
    world.scheduler.schedule(
        world.tick + delay,
        ScheduledTask::RemoveWornPreview { player_oid: player },
    );
}

/// `RemoveWearItemsTask` — the try-on wearing off.
///
/// Java sends `ExUserInfoEquipSlot`, which is an **extended packet at
/// sub-opcode 0x156** and so unreadable by any Interlude client. The port
/// sends the full `UserInfo` it already has, which is the Interlude-era way of
/// saying the same thing: redraw this character from its real paperdoll.
pub(crate) fn handle_remove_worn_preview(world: &mut World, player_oid: i32) {
    if !world
        .objects
        .has_component::<crate::model::Player>(&player_oid)
    {
        return;
    }
    helpers::send_sm_to_player(
        world,
        player_oid,
        sm_ids::YOU_ARE_NO_LONGER_TRYING_ON_EQUIPMENT,
        &[],
    );
    crate::game_loop::player_info::broadcast_user_info(world, player_oid);
}
