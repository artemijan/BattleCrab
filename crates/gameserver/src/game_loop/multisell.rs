//! Multisell exchange: `MultisellData.separateAndSend` (open the window) and
//! `clientpackets/MultiSellChoose` (the purchase/exchange transaction).
//!
//! Reached two ways: the community board (`separateAndSend(id, player, null,
//! …)`) and, since G22's `ai/others` sweep, the `multisell` / `exc_multisell`
//! NPC bypass (`handlers/bypasshandlers/Multisell`), which passes the NPC so
//! its `<npcs>` allow-list matches — every merchant exchange window in the
//! game comes through there.
//!
//! A window is built as **prepared rows** (Java `PreparedMultisellListHolder`):
//! a normal list is one row per entry, an **inventory-only** (`exc_multisell`)
//! list one row per unequipped weapon/armor the player holds that matches an
//! entry's ingredient — the Mammon and town-blacksmith exchange windows. The row
//! carries the paired item instance, so the exchange consumes *that* item and
//! `maintainEnchantment` can carry its enchant onto the product.
//!
//! Still **not** ported (a `TODO` marks each at its site):
//!
//! - chance multisells (one random product), enchanted (`enchantmentLevel`)
//!   ingredients, and `SpecialItemType` (clan reputation / fame / raid / PC
//!   café) ingredients & products.
//! - the weight/slot capacity gates (no encumbrance enforcement exists — the
//!   same G5 deferral as `shop.rs`).

use tracing::warn;

use crate::data::multisell_data::{MultisellEntry, MultisellList, PAGE_SIZE};
use crate::model::components::{ActiveMultisell, PreparedRow};
use crate::model::inventory::{Inventory, ItemChange};
use crate::network::client_packets as cp;
use crate::network::enter_world as ew;
use crate::network::server_packets::{self as sp, SmParam, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

/// The client's own cap (`_amount > 999999`), enforced before any per-product
/// count math.
const CLIENT_MAX_AMOUNT: i64 = 999_999;

/// Port of `MultisellData.separateAndSend(listId, player, npc, inventoryOnly)`:
/// send one `MultiSellList` per page and record the open list on the player.
///
/// `npc_oid` is the **object** id of the NPC the list was opened from (Java's
/// `Npc npc`), `None` for the npc-less community-board path. Its template id
/// checks the `<npcs>` allow-list and its position resolves the castle tax.
pub(crate) fn separate_and_send(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: Option<i32>,
    list_id: i32,
    inventory_only: bool,
) {
    let npc_id = npc_oid.map(|oid| {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .map_or(0, |n| n.npc_id)
    });
    // Java `PreparedMultisellListHolder`: the rate is captured when the window
    // opens and reused for the exchange, so a castle changing hands mid-window
    // can't change the price the player was quoted.
    let tax_rate = npc_oid.map_or(0.0, |oid| super::castle::npc_tax_rate(world, oid));
    let Some(list) = world.data.multisells.get(list_id) else {
        warn!("Multisell: list {list_id} not found (player {player}).");
        return;
    };

    // `!isNpcAllowed(-1) && (((npc != null) && !isNpcAllowed(npc.getId())) ||
    // ((npc == null) && isNpcOnly()))` — a list without the `-1` sentinel is
    // restricted to its `<npcs>` allow-list, and with no npc at all an npc-only
    // list is out of reach. (The GM bypass Java grants here is omitted.)
    let restricted = match npc_id {
        Some(id) => !list.is_npc_allowed(id),
        None => list.is_npc_only(),
    };
    if !list.is_npc_allowed(-1) && restricted {
        warn!("Multisell: list {list_id} is not allowed from npc {npc_id:?} (player {player}).",);
        return;
    }

    let rows = if inventory_only {
        inventory_rows(world, player, list)
    } else {
        (0..list.entries.len())
            .map(|entry_index| PreparedRow {
                entry_index,
                item_object_id: 0,
                enchant_level: 0,
            })
            .collect()
    };

    let pages = build_pages(list, &rows, &world.data.item_data, tax_rate);
    if let Some(cs) = world.clients.get(&client_id) {
        for page in pages {
            cs.send(page);
        }
    }
    world.objects.add_components(
        &player,
        ActiveMultisell {
            list_id,
            npc_oid: npc_oid.unwrap_or(0),
            tax_rate,
            rows,
        },
    );
}

/// Java `PreparedMultisellListHolder`'s inventory-only match-up: walk the
/// player's items and, for every **unequipped weapon or armor**, emit one row
/// per entry that names it as an ingredient. One item held twice gives two rows;
/// an item matching two entries gives two rows — Java's nested loops verbatim,
/// which is what lets the window show each instance with its own enchant.
fn inventory_rows(world: &World, player: i32, list: &MultisellList) -> Vec<PreparedRow> {
    let Some(inv) = world.objects.get_component::<Inventory>(&player) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for item in inv.items() {
        let equippable = world.data.item_data.get(item.item_id).is_some_and(|t| {
            matches!(
                t.kind,
                crate::data::item_data::ItemKind::Weapon | crate::data::item_data::ItemKind::Armor
            )
        });
        if !equippable || inv.paperdoll_slot_of(item.object_id).is_some() {
            continue;
        }
        for (entry_index, entry) in list.entries.iter().enumerate() {
            if entry.ingredients.iter().any(|ing| ing.id == item.item_id) {
                rows.push(PreparedRow {
                    entry_index,
                    item_object_id: item.object_id,
                    enchant_level: item.enchant_level,
                });
            }
        }
    }
    rows
}

/// Build every `MultiSellList` page (Java's `do … while index < size` loop —
/// at least one page, even for an empty list).
fn build_pages(
    list: &MultisellList,
    rows: &[PreparedRow],
    items: &crate::data::item_data::ItemData,
    tax_rate: f64,
) -> Vec<Vec<u8>> {
    let mut pages = Vec::new();
    let mut index = 0;
    loop {
        pages.push(sp::multi_sell_list(list, rows, index, items, tax_rate));
        index += PAGE_SIZE;
        if index >= rows.len() {
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
        .cloned();
    let Some(active) = active.filter(|a| a.list_id == pkt.list_id) else {
        world.objects.remove_component::<ActiveMultisell>(&player);
        return;
    };

    // `entryId` is 1-based and indexes the **rows the window displayed**, which
    // for an inventory-only list are item-paired duplicates of the entries.
    let Some(&row) = active.rows.get((pkt.entry_id - 1) as usize) else {
        warn!(
            "Multisell: player {player} chose out-of-range entry {} in list {}.",
            pkt.entry_id, pkt.list_id
        );
        world.objects.remove_component::<ActiveMultisell>(&player);
        return;
    };
    // Snapshot the entry (clone so we can drop the `world.data` borrow before
    // mutating the inventory).
    let Some(entry) = world
        .data
        .multisells
        .get(active.list_id)
        .and_then(|l| l.entries.get(row.entry_index))
    else {
        world.objects.remove_component::<ActiveMultisell>(&player);
        return;
    };
    let entry: MultisellEntry = entry.clone();
    let (ing_mult, prod_mult, apply_taxes, maintain_enchantment) = world
        .data
        .multisells
        .get(active.list_id)
        .map(|l| {
            (
                l.ingredient_multiplier,
                l.product_multiplier,
                l.apply_taxes,
                l.maintain_enchantment,
            )
        })
        .unwrap_or((1.0, 1.0, false, false));
    // Java `PreparedMultisellListHolder.getTaxRate()`: 0 unless the list applies
    // taxes. The rate itself was latched when the window opened.
    let tax_rate = if apply_taxes { active.tax_rate } else { 0.0 };

    // Java's `itemEnchantment` guard: a row bound to one item instance can only
    // be exchanged once at a time, and the client must echo that item's stats
    // back (the port compares the enchant level; it tracks no attributes).
    if row.item_object_id != 0 && (pkt.amount > 1 || pkt.enchant_level != row.enchant_level) {
        warn!(
            "Multisell: player {player} sent mismatched item stats for list {} entry {}.",
            pkt.list_id, pkt.entry_id
        );
        world.objects.remove_component::<ActiveMultisell>(&player);
        return;
    }

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
        let Some(total) = mul(
            ingredient_count(ing.id, ing.count, ing_mult, tax_rate),
            pkt.amount,
        ) else {
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
    // The paired instance must still be in the inventory (Java's
    // `destroyItem(objectId, …)` returning null is the same refusal).
    if row.item_object_id != 0
        && world
            .objects
            .get_component::<Inventory>(&player)
            .and_then(|inv| inv.item_by_object_id(row.item_object_id))
            .is_none()
    {
        world.objects.remove_component::<ActiveMultisell>(&player);
        return;
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
    // The enchant/augment the produced item inherits under `maintainEnchantment`
    // comes from the paired equippable ingredient (Java's `itemEnchantment`).
    let mut carried = if row.item_object_id != 0 {
        world
            .objects
            .get_component::<Inventory>(&player)
            .and_then(|inv| {
                inv.items()
                    .iter()
                    .find(|it| it.object_id == row.item_object_id)
                    .copied()
            })
    } else {
        None
    };
    let paired_item_id = carried.map(|it| it.item_id).unwrap_or(0);

    let mut changes: Vec<ItemChange> = Vec::new();
    let mut paired_taken = row.item_object_id == 0;
    for &(id, total) in &needed {
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
            // The line naming the paired item destroys *that* instance; every
            // other line is taken by item id as usual.
            if !paired_taken && id == paired_item_id {
                paired_taken = true;
                changes.extend(inv.remove_by_object_id(row.item_object_id, total));
            } else {
                changes.extend(inv.remove_item(id, total));
            }
        }
    }

    for product in &entry.products {
        let total = product_count(product.count, prod_mult) * pkt.amount;
        let added =
            super::items::add_inventory_item(world, player, product.id, total).unwrap_or_default();
        // Java: `maintainEnchantment` copies the consumed equippable's enchant
        // (and augmentation) onto the new item, once, when both are equippable.
        let equippable_product = world.data.item_data.get(product.id).is_some_and(|t| {
            matches!(
                t.kind,
                crate::data::item_data::ItemKind::Weapon | crate::data::item_data::ItemKind::Armor
            )
        });
        if maintain_enchantment
            && equippable_product
            && let Some(src) = carried.take()
        {
            for oid in &added {
                if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
                    inv.set_item_enchant(*oid, src.enchant_level);
                    if src.augment_option1 != 0 || src.augment_option2 != 0 {
                        inv.set_augmentation(
                            *oid,
                            src.augment_mineral,
                            src.augment_option1,
                            src.augment_option2,
                        );
                    }
                }
            }
        }
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

    // "Finally, give the tax to the castle": the tax slice of every adena
    // ingredient, times the amount exchanged. Only the *tax* part is paid —
    // the rest of the price is the exchange's own cost and simply vanishes.
    if active.npc_oid != 0 && tax_rate > 0.0 {
        let tax_paid: i64 = entry
            .ingredients
            .iter()
            .filter(|ing| ing.id == crate::data::item_data::ADENA_ID)
            .map(|ing| {
                ((ing.count as f64 * ing_mult * tax_rate).round() as i64).saturating_mul(pkt.amount)
            })
            .sum();
        super::castle::handle_tax_payment(world, active.npc_oid, tax_paid);
    }
}

/// `PreparedMultisellListHolder.getIngredientCount` — the castle tax rides on
/// the adena ingredient only. `tax_rate` has already been zeroed for a list
/// that doesn't apply taxes.
fn ingredient_count(item_id: i32, count: i64, multiplier: f64, tax_rate: f64) -> i64 {
    if item_id == crate::data::item_data::ADENA_ID {
        (count as f64 * multiplier * (1.0 + tax_rate)).round() as i64
    } else {
        (count as f64 * multiplier).round() as i64
    }
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
