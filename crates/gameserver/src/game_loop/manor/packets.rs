//! Packet handlers: the setup gates, set_seed/set_crop writes and the
//! buy-seed / procure-crop trades.

use super::persist::save_after_action;
use super::reference_price;
use crate::data::item_data::ADENA_ID;
use crate::game_loop::character::inventory;
use crate::game_loop::helpers::send_action_failed;
use crate::game_loop::helpers::send_sm_to_client;
use crate::game_loop::helpers::send_to_client;
/// Java `RequestSetSeed`/`RequestSetCrop`'s shared owner gate. Returns the
/// player object id when: the manor is in its **modifiable** period, the
/// player's clan owns castle `manor_id`, they hold `CS_MANOR_ADMIN`, and they
/// are in range of the chamberlain (last folk NPC). Otherwise sends
/// `ActionFailed` and returns `None`, mirroring Java's early-outs.
use crate::game_loop::npc::npc_template;
use crate::model::Player;
use crate::model::clan::CS_MANOR_ADMIN;
use crate::model::components::player::LastFolkNpc;
use crate::model::manor::CropProcure;
use crate::model::manor::SeedProduction;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::world::World;
use commons::network::PacketReader;

fn manor_setup_gate(world: &mut World, client_id: u32, manor_id: i32) -> Option<i32> {
    let player_oid = match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => s.player_object_id(),
        _ => return None,
    };
    let ok = world.manor.is_modifiable_period() && {
        let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
            return fail(world, client_id);
        };
        let owns = p.clan_id != 0
            && world.clans.get(&p.clan_id).is_some_and(|c| {
                c.castle_id == manor_id && c.has_privilege(player_oid, p.clan_privs, CS_MANOR_ADMIN)
            });
        // The last folk NPC (the chamberlain) must be in interaction range.
        let in_range = world
            .objects
            .get_component::<LastFolkNpc>(&player_oid)
            .is_some_and(|&LastFolkNpc(npc)| {
                crate::game_loop::combat::target::can_interact(world, player_oid, npc)
            });
        owns && in_range
    };
    if ok {
        Some(player_oid)
    } else {
        fail(world, client_id)
    }
}

/// Send `ActionFailed` and yield `None` (the gate's rejection path).
fn fail(world: &World, client_id: u32) -> Option<i32> {
    send_action_failed(world, client_id);
    None
}

/// The `(manorId, count)` header every batched manor packet opens with, with
/// Java's `if ((count < 1) || (count > MAX) || ((count * BATCH) != available))`
/// sanity check folded in: a count that doesn't match the bytes actually on the
/// wire is a hand-built packet, and reading it would either truncate the batch
/// or spin allocating 2³¹ empty lines.
fn read_batch_header(r: &mut PacketReader, batch: usize) -> Option<(i32, i32)> {
    /// Java `Config.ALT_MANOR_MAX_ITEMS` / the client's own list cap.
    const MAX_LINES: i32 = 1000;

    let (manor_id, count) = (r.read_i32()?, r.read_i32()?);
    (count > 0 && count <= MAX_LINES && r.remaining() == count as usize * batch)
        .then_some((manor_id, count))
}

/// Port of `clientpackets/RequestSetSeed` — the owner submits the next-period
/// seed setup. Reads `manorId, count, [seedId, sales, price]*`; keeps only known
/// seeds within their limit/price band; replaces the castle's next-period seed
/// production.
pub(crate) fn handle_request_set_seed(world: &mut World, client_id: u32, body: &[u8]) {
    const BATCH: usize = 4 + 8 + 8; // seedId + sales + price
    let mut r = PacketReader::new(body);
    let Some((manor_id, count)) = read_batch_header(&mut r, BATCH) else {
        return;
    };
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let (Some(item_id), Some(sales), Some(price)) = (r.read_i32(), r.read_i64(), r.read_i64())
        else {
            return;
        };
        if item_id < 1 || sales < 0 || price < 0 {
            return;
        }
        if sales > 0 {
            items.push((item_id, sales, price));
        }
    }
    if items.is_empty() {
        return;
    }
    let Some(_player) = manor_setup_gate(world, client_id, manor_id) else {
        return;
    };
    // Filter to known seeds within the setup limit/price band.
    let rate = world.cfg.rates.rate_drop_manor;
    let list: Vec<SeedProduction> = items
        .into_iter()
        .filter_map(|(seed_id, sales, price)| {
            let seed = world.data.manor.seed_by_id(seed_id)?;
            let ref_price = reference_price(world, seed_id);
            let min = (ref_price as f64 * 0.6) as i64;
            let max = ref_price as i64 * 10;
            (sales <= (seed.limit_seeds * rate) as i64 && price >= min && price <= max).then_some(
                SeedProduction {
                    seed_id,
                    amount: sales,
                    price,
                    start_amount: sales,
                },
            )
        })
        .collect();
    world.manor.set_next_seed_production(manor_id, list);
    save_after_action(world, manor_id);
}

/// Port of `clientpackets/RequestSetCrop` — the owner submits the next-period
/// crop setup. Like [`handle_request_set_seed`] plus a per-line reward-type
/// byte; keeps only crops the castle farms, within their limit/price band.
pub(crate) fn handle_request_set_crop(world: &mut World, client_id: u32, body: &[u8]) {
    const BATCH: usize = 4 + 8 + 8 + 1; // cropId + sales + price + type
    let mut r = PacketReader::new(body);
    let Some((manor_id, count)) = read_batch_header(&mut r, BATCH) else {
        return;
    };
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let (Some(item_id), Some(sales), Some(price), Some(reward_type)) =
            (r.read_i32(), r.read_i64(), r.read_i64(), r.read_u8())
        else {
            return;
        };
        if item_id < 1 || sales < 0 || price < 0 {
            return;
        }
        if sales > 0 {
            items.push((item_id, sales, price, reward_type as i32));
        }
    }
    if items.is_empty() {
        return;
    }
    let Some(_player) = manor_setup_gate(world, client_id, manor_id) else {
        return;
    };
    let rate = world.cfg.rates.rate_drop_manor;
    let list: Vec<CropProcure> = items
        .into_iter()
        .filter_map(|(crop_id, sales, price, reward_type)| {
            // Java `getSeedByCrop(cropId, castleId)` — the crop must be one this
            // castle actually farms.
            let seed = world
                .data
                .manor
                .seeds_for_castle(manor_id)
                .iter()
                .find(|s| s.crop_id == crop_id)?;
            let ref_price = reference_price(world, crop_id);
            let min = (ref_price as f64 * 0.6) as i64;
            let max = ref_price as i64 * 10;
            (sales <= (seed.limit_crops * rate) as i64 && price >= min && price <= max).then_some(
                CropProcure {
                    crop_id,
                    amount: sales,
                    price,
                    start_amount: sales,
                    reward_type,
                },
            )
        })
        .collect();
    world.manor.set_next_crop_procure(manor_id, list);
    save_after_action(world, manor_id);
}

const MAX_ADENA: i64 = 99_999_999_999;

/// The Manor Manager's `manor_id` NPC parameter, if the player's last folk NPC
/// is a Merchant in interaction range whose `manor_id` matches (Java's
/// `manager instanceof Merchant && canInteract && getParameters().getInt(...)`).
fn manor_manager_castle(world: &World, player_oid: i32) -> Option<i32> {
    let &LastFolkNpc(npc) = world.objects.get_component::<LastFolkNpc>(&player_oid)?;
    if !crate::game_loop::commerce::shop::is_merchant(world, npc)
        || !crate::game_loop::combat::target::can_interact(world, player_oid, npc)
    {
        return None;
    }
    let castle = npc_template(world, npc).map(|t| t.ai_param_i32("manor_id", -1))?;
    (castle >= 0).then_some(castle)
}

/// Port of `clientpackets/RequestBuySeed` — a player buys seeds from a Manor
/// Manager's current-period production. Reads `manorId, count, [seedId, cnt]*`;
/// validates the seeds (price/stock/adena) against `ManorState`, takes the adena
/// and decrements the manor's stock, and hands over the seeds.
pub(crate) fn handle_request_buy_seed(world: &mut World, client_id: u32, body: &[u8]) {
    const BATCH: usize = 4 + 8; // itemId + count
    let mut r = PacketReader::new(body);
    let Some((manor_id, count)) = read_batch_header(&mut r, BATCH) else {
        return;
    };
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let (Some(item_id), Some(cnt)) = (r.read_i32(), r.read_i64()) else {
            return;
        };
        if cnt < 1 || item_id < 1 {
            return;
        }
        items.push((item_id, cnt));
    }

    let player_oid = match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => s.player_object_id(),
        _ => return,
    };
    // Java gate: not under maintenance, the castle exists, and the last folk NPC
    // is this castle's Manor Manager in range.
    if world.manor.is_under_maintenance()
        || !world.data.manor.manor_castle_ids().contains(&manor_id)
        || manor_manager_castle(world, player_oid) != Some(manor_id)
    {
        send_action_failed(world, client_id);
        return;
    }

    // Validate every line against the live production, summing the price —
    // and, as Java does in the same loop, the weight and slots the purchase
    // would add.
    let mut total_price = 0i64;
    let mut total_weight = 0i64;
    let mut slots = 0i64;
    for &(item_id, cnt) in &items {
        let ok = world
            .manor
            .seed_product(manor_id, item_id, false)
            .is_some_and(|sp| sp.price > 0 && sp.amount >= cnt && MAX_ADENA / cnt >= sp.price);
        if !ok {
            send_action_failed(world, client_id);
            return;
        }
        let price = world
            .manor
            .seed_product(manor_id, item_id, false)
            .map_or(0, |sp| sp.price);
        total_price += price * cnt;
        if total_price > MAX_ADENA {
            crate::game_loop::moderation::punishment::illegal_action(
                world,
                player_oid,
                &format!(
                    "Player {player_oid} tried to purchase over {MAX_ADENA} adena worth of goods."
                ),
            );
            return;
        }
        total_weight += cnt
            * world
                .data
                .item_data
                .get(item_id)
                .map_or(0, |tpl| i64::from(tpl.weight));
        slots += crate::game_loop::stats::weight::slots_needed(world, player_oid, item_id, cnt);
    }

    // Java's order is weight, then slots, then adena — and it matters: an
    // overloaded player with no money is told about the weight, not the money.
    if !crate::game_loop::stats::weight::validate_weight(world, player_oid, total_weight) {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(sm_ids::YOU_HAVE_EXCEEDED_THE_WEIGHT_LIMIT, &[]),
        );
        return;
    }
    if !crate::game_loop::stats::weight::validate_capacity(world, player_oid, slots) {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(sm_ids::YOUR_INVENTORY_IS_FULL, &[]),
        );
        return;
    }

    let adena = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&player_oid)
        .map_or(0, |i| i.adena());
    if adena < total_price {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA, &[]),
        );
        return;
    }
    if total_price > 0
        && !inventory::take_items(world, client_id, player_oid, ADENA_ID, total_price)
    {
        return;
    }

    // Deliver: decrement each seed's stock and add it to the buyer.
    let mut added: Vec<crate::model::inventory::ItemChange> = Vec::new();
    for &(item_id, cnt) in &items {
        // A concurrent overdraw can't happen on the single game thread, but the
        // `decrease_amount` guard mirrors Java's per-line refund-on-failure.
        if world.manor.decrease_seed_amount(manor_id, item_id, cnt)
            && let Some(changes) =
                inventory::add_inventory_item_changes(world, player_oid, item_id, cnt)
        {
            added.extend(changes);
        }
    }
    // Java: the sale price goes to the castle's vault, untaxed. An unowned
    // castle takes nothing (`addToTreasuryNoTax` returns false on `_ownerId <= 0`),
    // so the adena the buyer just paid simply leaves the economy.
    if total_price > 0 {
        crate::game_loop::siege::treasury::add_to_treasury_no_tax(world, manor_id, total_price);
    }
    inventory::send_inventory_update(world, player_oid, added);
    if total_price > 0 {
        send_sm_to_client(
            world,
            client_id,
            sm_ids::S1_ADENA_DISAPPEARED,
            &[SmParam::Long(total_price)],
        );
    }
}

/// Port of `clientpackets/RequestProcureCropList` — a player sells crops to a
/// Manor Manager for the crop's reward item. Reads
/// `count, [objId, cropId, manorId, cnt]*`; validates every line against the
/// inventory + `CropProcure` state, then per line pays out
/// `price / rewardReferencePrice` of the reward item, charging a 5 % adena fee
/// when selling to a manor other than where the crop's procurement is set.
pub(crate) fn handle_request_procure_crop_list(world: &mut World, client_id: u32, body: &[u8]) {
    use crate::model::inventory::Inventory;
    const BATCH: usize = 4 + 4 + 4 + 8; // objId + cropId + manorId + cnt
    let mut r = PacketReader::new(body);
    let Some(count) = r.read_i32() else {
        return;
    };
    if count <= 0 || count > 1000 || r.remaining() != count as usize * BATCH {
        return;
    }
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let (Some(obj_id), Some(crop_id), Some(item_manor), Some(cnt)) =
            (r.read_i32(), r.read_i32(), r.read_i32(), r.read_i64())
        else {
            return;
        };
        if obj_id < 1 || crop_id < 1 || item_manor < 0 || cnt < 0 {
            return;
        }
        items.push((obj_id, crop_id, item_manor, cnt));
    }

    let player_oid = match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => s.player_object_id(),
        _ => return,
    };
    // Gate: not under maintenance, and the last folk NPC is a Manor Manager in
    // range (its `manor_id` param is the manager's castle).
    let Some(castle_id) = (if world.manor.is_under_maintenance() {
        None
    } else {
        manor_manager_castle(world, player_oid)
    }) else {
        send_action_failed(world, client_id);
        return;
    };

    // Loop 1: validate every line (any failure rejects the whole packet).
    for &(obj_id, crop_id, item_manor, cnt) in &items {
        let item_ok = world
            .objects
            .get_component::<Inventory>(&player_oid)
            .and_then(|i| i.item_by_object_id(obj_id))
            .is_some_and(|(id, held)| id == crop_id && held >= cnt);
        let cp_ok = world
            .manor
            .crop_procure_for(item_manor, crop_id, false)
            .is_some_and(|cp| cp.amount >= cnt);
        if !item_ok || !cp_ok {
            send_action_failed(world, client_id);
            return;
        }
    }

    // Loop 2: execute, skipping (with a message) lines that can't pay out.
    let mut crop_changes = Vec::new();
    let mut reward_changes: Vec<crate::model::inventory::ItemChange> = Vec::new();
    for &(obj_id, crop_id, item_manor, cnt) in &items {
        let (price, reward_type) = world
            .manor
            .crop_procure_for(item_manor, crop_id, false)
            .map(|cp| (cnt * cp.price, cp.reward_type))
            .expect("validated in loop 1");
        let Some(reward_id) = world
            .data
            .manor
            .seed_by_crop(crop_id)
            .map(|s| s.reward(reward_type))
        else {
            continue;
        };
        let reward_price = reference_price(world, reward_id) as i64;
        if reward_price == 0 {
            continue;
        }
        let reward_count = price / reward_price;
        if reward_count < 1 {
            // Java reports the line and skips it.
            send_to_client(
                world,
                client_id,
                server_packets::system_message_with(
                    sm_ids::FAILED_IN_TRADING_S2_OF_S1_CROPS,
                    &[SmParam::ItemName(crop_id), SmParam::Long(cnt)],
                ),
            );
            continue;
        }
        // A 5 % adena fee when selling at a manor other than the crop's own.
        let fee = if castle_id == item_manor {
            0
        } else {
            (price as f64 * 0.05) as i64
        };
        if fee > 0 {
            let adena = world
                .objects
                .get_component::<Inventory>(&player_oid)
                .map_or(0, |i| i.adena());
            if adena < fee {
                send_to_client(
                    world,
                    client_id,
                    server_packets::system_message_with(sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA, &[]),
                );
                continue;
            }
        }

        // Everything validated → decrement the procurement, take the fee, take
        // the crops, hand over the reward.
        if !world.manor.decrease_crop_amount(item_manor, crop_id, cnt) {
            continue;
        }
        if fee > 0 {
            inventory::take_items(world, client_id, player_oid, ADENA_ID, fee);
        }
        if let Some(change) =
            inventory::remove_inventory_item_change(world, player_oid, obj_id, cnt)
        {
            crop_changes.push(change);
        }
        if let Some(changes) =
            inventory::add_inventory_item_changes(world, player_oid, reward_id, reward_count)
        {
            reward_changes.extend(changes);
        }
    }

    // Reflect the sold crops and the received rewards.
    if !crop_changes.is_empty() {
        inventory::send_inventory_update(world, player_oid, crop_changes);
    }
    if !reward_changes.is_empty() {
        inventory::send_inventory_update(world, player_oid, reward_changes);
    }
}
