//! Multisell exchange: `MultisellData.separateAndSend` (open the window) and
//! `clientpackets/MultiSellChoose` (the purchase/exchange transaction).
//!
//! Scoped to the community-board shop path — `separateAndSend(id, player,
//! null, false)`: no npc, no castle tax, no inventory-only filtering, no
//! `maintainEnchantment`, and adena/regular ingredients → regular products.
//! The following Java branches are **not** ported (a `TODO(G30)` marks each at
//! its site; none is reached by the `-1`/CB lists on this dist):
//!
//! - inventory-only lists (`_bbsexcmultisell`) — the equippable-item match-up +
//!   per-enchant entry duplication. `exc` still opens the full list here.
//! - chance multisells (one random product), `maintainEnchantment`, enchanted
//!   ingredients, and `SpecialItemType` (clan reputation / fame / raid / PC
//!   café) ingredients & products.
//! - castle tax (`handleTaxPayment`) and the weight/slot capacity gates (no
//!   encumbrance enforcement exists — the same G5 deferral as `shop.rs`).

use tracing::warn;

use crate::data::multisell_data::{MultisellEntry, MultisellList, PAGE_SIZE};
use crate::model::components::ActiveMultisell;
use crate::model::inventory::{Inventory, ItemChange};
use crate::network::client_packets as cp;
use crate::network::enter_world as ew;
use crate::network::server_packets::{self as sp, SmParam, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

/// The client's own cap (`_amount > 999999`), enforced before any per-product
/// count math.
const CLIENT_MAX_AMOUNT: i64 = 999_999;

/// Port of `MultisellData.separateAndSend(listId, player, null, inventoryOnly)`
/// for the npc-less community-board path: send one `MultiSellList` per page and
/// record the open list on the player.
pub(crate) fn separate_and_send(
    world: &mut World,
    client_id: u32,
    player: i32,
    list_id: i32,
    inventory_only: bool,
) {
    let Some(list) = world.data.multisells.get(list_id) else {
        warn!("Multisell: list {list_id} not found (player {player}).");
        return;
    };

    // `!isNpcAllowed(-1) && ((npc == null) && isNpcOnly())` — with no npc, an
    // npc-only list that doesn't carry the `-1` sentinel is restricted. (The GM
    // bypass Java grants here is omitted — CB lists carry `-1`, so this only
    // trips a genuinely misconfigured list.)
    if !list.is_npc_allowed(-1) && list.is_npc_only() {
        warn!("Multisell: list {list_id} is npc-only and not CB-allowed (player {player}).");
        return;
    }

    // TODO(G30): `inventoryOnly` (the `_bbsexcmultisell` exchange mode) should
    // filter to entries whose ingredients are unequipped weapons/armor the
    // player holds and duplicate them per enchant level
    // (`PreparedMultisellListHolder`). Unported: `exc` opens the full list.
    if inventory_only {
        warn!("Multisell: inventory-only list {list_id} opened as full list (unported filter).");
    }

    let pages = build_pages(list, &world.data.item_data);
    if let Some(cs) = world.clients.get(&client_id) {
        for page in pages {
            cs.send(page);
        }
    }
    world
        .objects
        .add_components(&player, ActiveMultisell { list_id });
}

/// Build every `MultiSellList` page (Java's `do … while index < size` loop —
/// at least one page, even for an empty list).
fn build_pages(list: &MultisellList, items: &crate::data::item_data::ItemData) -> Vec<Vec<u8>> {
    let mut pages = Vec::new();
    let mut index = 0;
    loop {
        pages.push(sp::multi_sell_list(list, index, items));
        index += PAGE_SIZE;
        if index >= list.entries.len() {
            break;
        }
    }
    pages
}

/// Port of `clientpackets/MultiSellChoose.runImpl` for the community-board path.
pub(crate) fn handle_multi_sell_choose(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::MultiSellChoose::read(body) else {
        return;
    };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();

    // `(_amount < 1) || (_amount > 999999)`.
    if pkt.amount < 1 || pkt.amount > CLIENT_MAX_AMOUNT {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED,
            &[],
        );
        return;
    }

    // The open list must match the one the client claims.
    let active = world
        .objects
        .get_component::<ActiveMultisell>(&player)
        .copied();
    let Some(active) = active.filter(|a| a.list_id == pkt.list_id) else {
        world.objects.remove_component::<ActiveMultisell>(&player);
        return;
    };

    // Snapshot the entry (clone so we can drop the `world.data` borrow before
    // mutating the inventory).
    let Some(entry) = world.data.multisells.get(active.list_id).and_then(|l| {
        // `entryId` is 1-based and indexes the list directly on this path.
        l.entries.get((pkt.entry_id - 1) as usize)
    }) else {
        warn!(
            "Multisell: player {player} chose out-of-range entry {} in list {}.",
            pkt.entry_id, pkt.list_id
        );
        world.objects.remove_component::<ActiveMultisell>(&player);
        return;
    };
    let entry: MultisellEntry = entry.clone();
    let (ing_mult, prod_mult) = world
        .data
        .multisells
        .get(active.list_id)
        .map(|l| (l.ingredient_multiplier, l.product_multiplier))
        .unwrap_or((1.0, 1.0));

    // `!entry.isStackable() && (_amount > 1)`.
    if !entry.stackable && pkt.amount > 1 {
        warn!(
            "Multisell: player {player} set amount > 1 on non-stackable entry (list {}).",
            pkt.list_id
        );
        world.objects.remove_component::<ActiveMultisell>(&player);
        return;
    }

    // --- Validate products (templates exist, counts in range). No weight/slot
    // gate (no encumbrance enforcement — see module docs). ---
    for product in &entry.products {
        if product.id < 0 {
            // TODO(G30): SpecialItemType products (clan reputation / fame / raid
            // points). Refuse rather than silently grant nothing.
            warn!(
                "Multisell: list {} has an unported special product {}.",
                pkt.list_id, product.id
            );
            return;
        }
        if world.data.item_data.get(product.id).is_none() {
            world.objects.remove_component::<ActiveMultisell>(&player);
            return;
        }
        let count = mul(product_count(product.count, prod_mult), pkt.amount);
        let Some(count) = count.filter(|&c| (1..=i32::MAX as i64).contains(&c)) else {
            send_sm(
                world,
                client_id,
                sm_ids::YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED,
                &[],
            );
            return;
        };
        let _ = count;
        // TODO(G30): chance multisell gives one *random* product — none of the
        // CB lists are chance lists, so every declared product is granted.
    }

    // --- Validate ingredients present (sum by id; Java `summedIngredients`).
    // Enchanted / special ingredients are unported (never in CB lists). ---
    let mut needed: Vec<(i32, i64)> = Vec::new();
    for ing in &entry.ingredients {
        if ing.enchant_level > 0 || ing.id < 0 {
            // TODO(G30): enchanted-item and SpecialItemType ingredients.
            warn!(
                "Multisell: list {} has an unported enchant/special ingredient {}.",
                pkt.list_id, ing.id
            );
            return;
        }
        if ing.maintain {
            continue; // not consumed, so no presence requirement
        }
        let Some(total) = mul(ingredient_count(ing.count, ing_mult), pkt.amount) else {
            send_sm(
                world,
                client_id,
                sm_ids::YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED,
                &[],
            );
            return;
        };
        if let Some(slot) = needed.iter_mut().find(|(id, _)| *id == ing.id) {
            slot.1 = slot.1.saturating_add(total);
        } else {
            needed.push((ing.id, total));
        }
    }
    for &(id, total) in &needed {
        let have = world
            .objects
            .get_component::<Inventory>(&player)
            .map(|i| i.count_of(id))
            .unwrap_or(0);
        if have < total {
            send_sm(
                world,
                client_id,
                sm_ids::YOU_NEED_S2_S1_S,
                &[SmParam::ItemName(id), SmParam::Long(total)],
            );
            return;
        }
    }

    // --- Commit: take ingredients, then give products (all validated). ---
    let mut changes: Vec<ItemChange> = Vec::new();
    for &(id, total) in &needed {
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
            changes.extend(inv.remove_item(id, total));
        }
    }

    for product in &entry.products {
        let total = product_count(product.count, prod_mult) * pkt.amount;
        let added =
            super::items::add_inventory_item(world, player, product.id, total).unwrap_or_default();
        for oid in &added {
            if let Some(item) = world
                .objects
                .get_component::<Inventory>(&player)
                .and_then(|inv| inv.items().iter().find(|i| i.object_id == *oid).copied())
            {
                changes.push(ItemChange::Modified(item));
            }
        }
        // Acquisition message (Java's count > 1 / enchant > 0 / else split).
        if total > 1 {
            send_sm(
                world,
                client_id,
                sm_ids::YOU_HAVE_EARNED_S2_S1_S,
                &[SmParam::ItemName(product.id), SmParam::Long(total)],
            );
        } else if product.enchant_level > 0 {
            send_sm(
                world,
                client_id,
                sm_ids::ACQUIRED_S1_S2,
                &[
                    SmParam::Long(product.enchant_level as i64),
                    SmParam::ItemName(product.id),
                ],
            );
        } else {
            send_sm(
                world,
                client_id,
                sm_ids::YOU_HAVE_EARNED_S1,
                &[SmParam::ItemName(product.id)],
            );
        }
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(sp::ex_multi_sell_result(
                true,
                0,
                total.min(i32::MAX as i64) as i32,
            ));
        }
    }

    // One InventoryUpdate + weight refresh for the whole exchange.
    if let (Some(inv), Some(cs)) = (
        world.objects.get_component::<Inventory>(&player),
        world.clients.get(&client_id),
    ) {
        cs.send(ew::inventory_update_changes(&world.data, &changes));
        cs.send(ew::ex_user_info_inven_weight(player, inv, &world.data));
    }
}

/// `PreparedMultisellListHolder.getIngredientCount` (no tax on this path).
fn ingredient_count(count: i64, multiplier: f64) -> i64 {
    (count as f64 * multiplier).round() as i64
}

/// `PreparedMultisellListHolder.getProductCount`.
fn product_count(count: i64, multiplier: f64) -> i64 {
    (count as f64 * multiplier).round() as i64
}

/// `Math.multiplyExact` — `None` on overflow (Java throws → "quantity exceeded").
fn mul(a: i64, b: i64) -> Option<i64> {
    a.checked_mul(b)
}

fn send_sm(world: &World, client_id: u32, message_id: i16, params: &[SmParam]) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::system_message_with(message_id, params));
    }
}
