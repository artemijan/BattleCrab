//! Buy-list stock and pricing (row 17) — the parts of `model/buylist/Product`
//! that are not in the XML: the price an undeclared `price` resolves to, and
//! the `count`/`restock_delay` shelf behind `decreaseCount`/`restock`.
//!
//! These run against the **real datapack lists**, not synthetic ones, because
//! both bugs they cover were bugs about what the datapack actually says.

use super::*;
use crate::data::buy_list_data::BuyListData;
use crate::db::DbEvent;
use crate::game_loop::character::inventory;
use crate::game_loop::commerce::shop;
use crate::scheduler::ScheduledTask;

/// Gludin's Cooper — sells raw materials, every line without a `price`.
const COOPER: i32 = 30829;
const COOPER_LIST: i32 = 3082900;
/// `Iron Canine`, reference price 7000, non-stackable.
const IRON_CANINE: i32 = 2505;

/// Black, the Devastated Castle clan-hall manager. Three products, two of
/// them limited stock.
const HALL_MANAGER: i32 = 35384;
const HALL_LIST: i32 = 3538400;
/// `Scroll of Escape: Clan Hall` — `count="5" restock_delay="60"`, stackable,
/// reference price 500 and no declared price.
const HALL_SOE: i32 = 1829;
const HALL_SOE_STOCK: i64 = 5;
/// 60 minutes at ten ticks a second.
const RESTOCK_TICKS: u64 = 36_000;

/// `shop_world`, but with the datapack's own items and buy lists in place of
/// the synthetic pair — the point of these tests is the real files.
fn dist_shop_world(npc_id: i32) -> (World, db::CmdRx, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, db_rx, _link_rx) = quest_test_world();
    let items = dist::items_owned();
    let lists = BuyListData::load_from(
        crate::data::DIST_GAME,
        &items,
        crate::data::item_data::kinds::CrystalType::S,
        true,
        true,
    );
    world.data.item_data = items;
    world.data.buy_lists = lists;
    add_test_npc(&mut world, NPC_OID, npc_id, "Merchant", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    inventory::add_inventory_item(&mut world, 3001, 57, 10_000_000);
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    drain(&mut rx);
    (world, db_rx, rx)
}

fn stock_of(world: &World, list_id: i32, item_id: i32) -> i64 {
    let product = world
        .data
        .buy_lists
        .get(list_id)
        .and_then(|l| l.product(item_id))
        .expect("product");
    shop::stock_left(world, list_id, product)
}

fn restock_tasks(world: &World) -> Vec<ScheduledTask> {
    world
        .scheduler
        .pending_tasks_for_test()
        .into_iter()
        .filter(|t| matches!(t, ScheduledTask::BuyListRestock { .. }))
        .collect()
}

/// **A line with no `price` sells at the item's reference price.**
///
/// Java resolves this in `Product`'s constructor, not in the parser, so it is
/// easy to read `BuyListData.parseDocument` and conclude a bare `<item
/// id="2505" />` has no price. 3079 of the 8198 product lines on this dist's
/// npc-served lists are bare, so treating -1 as "unbuyable" empties 38 % of
/// the shelves — Cooper among them.
#[test]
fn a_line_with_no_declared_price_sells_at_the_reference_price() {
    let (mut world, _db, _rx) = dist_shop_world(COOPER);
    let before = adena_of(&world, 3001);

    shop::handle_request_buy_item(&mut world, 1, &buy_body(COOPER_LIST, &[(IRON_CANINE, 1)]));

    assert_eq!(count_of_item(&world, 3001, IRON_CANINE), 1, "delivered");
    // 7000 reference price + the list's `baseTax="20"`.
    assert_eq!(
        before - adena_of(&world, 3001),
        8400,
        "reference price + 20%"
    );
}

/// **A limited product sells down to nothing, and the empty shelf disappears
/// from the window rather than showing a zero.**
#[test]
fn a_limited_product_sells_out_and_leaves_the_buy_window() {
    let (mut world, _db, mut rx) = dist_shop_world(HALL_MANAGER);
    assert_eq!(stock_of(&world, HALL_LIST, HALL_SOE), HALL_SOE_STOCK);

    shop::handle_request_buy_item(
        &mut world,
        1,
        &buy_body(HALL_LIST, &[(HALL_SOE, HALL_SOE_STOCK)]),
    );
    assert_eq!(count_of_item(&world, 3001, HALL_SOE), HALL_SOE_STOCK);
    assert_eq!(stock_of(&world, HALL_LIST, HALL_SOE), 0, "shelf emptied");

    // One more is refused outright — nothing delivered, nothing charged.
    let adena = adena_of(&world, 3001);
    shop::handle_request_buy_item(&mut world, 1, &buy_body(HALL_LIST, &[(HALL_SOE, 1)]));
    assert_eq!(
        count_of_item(&world, 3001, HALL_SOE),
        HALL_SOE_STOCK,
        "no sale"
    );
    assert_eq!(adena_of(&world, 3001), adena, "and no charge");

    // And the window drops the line entirely (`getCount() > 0 ||
    // !hasLimitedStock()`), rather than offering a 0-quantity entry.
    drain(&mut rx);
    shop::show_buy_window(&mut world, 1, 3001, NPC_OID, HALL_LIST);
    let offered = buy_tab(&drain(&mut rx)).expect("a buy tab went out");
    assert!(
        !offered.iter().any(|&(id, _)| id == HALL_SOE),
        "the sold-out product is not offered, got {offered:?}"
    );
    // The Pledge Shield on the same list is unlimited and still there — and
    // reads as quantity 0, which is what `getCount()` returns for it.
    assert!(
        offered.contains(&(6902, 0)),
        "the unlimited line is still offered, got {offered:?}"
    );
}

/// The `(item id, quantity)` pairs of the buy tab (`ExBuySellList` type 0),
/// parsed rather than scanned for — the sell tab that follows it lists what
/// the player is holding, so a byte scan finds the item either way.
fn buy_tab(packets: &[Vec<u8>]) -> Option<Vec<(i32, i64)>> {
    /// mask, oid, id, T1, count, type2, ct1, equipped, bodypart, enchant,
    /// ct2, mana, time, available, price.
    const PRODUCT_LEN: usize = 1 + 4 + 4 + 1 + 8 + 1 + 1 + 2 + 8 + 1 + 1 + 4 + 4 + 1 + 8;
    /// 0xFE, 0x00B8, type, money, list id, slots, entry count.
    const HEADER_LEN: usize = 1 + 2 + 4 + 8 + 4 + 4 + 2;
    let i32_at = |p: &[u8], o: usize| i32::from_le_bytes(p[o..o + 4].try_into().unwrap());
    let p = packets.iter().find(|p| {
        p.len() >= HEADER_LEN
            && p[0] == 0xFE
            && i16::from_le_bytes([p[1], p[2]]) == crate::network::trade::EX_BUY_SELL_LIST
            && i32_at(p, 3) == 0
    })?;
    let n = i16::from_le_bytes([p[HEADER_LEN - 2], p[HEADER_LEN - 1]]) as usize;
    assert_eq!(
        p.len(),
        HEADER_LEN + n * PRODUCT_LEN,
        "the declared entry count must match the bytes actually written — \
         Java's own BuyList packet fails this when a product is sold out"
    );
    Some(
        (0..n)
            .map(|i| {
                let at = HEADER_LEN + i * PRODUCT_LEN;
                (
                    i32_at(p, at + 5),
                    i64::from_le_bytes(p[at + 10..at + 18].try_into().unwrap()),
                )
            })
            .collect(),
    )
}

/// **The shelf refills after `restock_delay`.**
#[test]
fn stock_comes_back_after_the_restock_delay() {
    let (mut world, _db, _rx) = dist_shop_world(HALL_MANAGER);
    shop::handle_request_buy_item(&mut world, 1, &buy_body(HALL_LIST, &[(HALL_SOE, 2)]));
    assert_eq!(stock_of(&world, HALL_LIST, HALL_SOE), 3);

    advance_ticks(&mut world, RESTOCK_TICKS - 1);
    assert_eq!(stock_of(&world, HALL_LIST, HALL_SOE), 3, "not yet");

    advance_ticks(&mut world, 1);
    assert_eq!(
        stock_of(&world, HALL_LIST, HALL_SOE),
        HALL_SOE_STOCK,
        "back to a full shelf"
    );
    assert!(restock_tasks(&world).is_empty(), "and the timer is spent");
}

/// **The restock clock starts at the first sale since the last restock, not
/// at the most recent one.**
///
/// `BuyListTaskManager.add` is a `containsKey` no-op, so a shop that is bought
/// from steadily still restocks on schedule. Pushing the deadline forward on
/// each sale — the reasonable-looking alternative — would let a busy merchant
/// stay empty forever.
#[test]
fn the_restock_clock_starts_at_the_first_sale_not_the_last() {
    let (mut world, _db, _rx) = dist_shop_world(HALL_MANAGER);
    shop::handle_request_buy_item(&mut world, 1, &buy_body(HALL_LIST, &[(HALL_SOE, 1)]));

    // Half the delay later, buy again.
    advance_ticks(&mut world, RESTOCK_TICKS / 2);
    shop::handle_request_buy_item(&mut world, 1, &buy_body(HALL_LIST, &[(HALL_SOE, 1)]));
    assert_eq!(stock_of(&world, HALL_LIST, HALL_SOE), 3);
    assert_eq!(
        restock_tasks(&world).len(),
        1,
        "the second sale did not arm a second timer"
    );

    // The remaining half of the *original* delay is enough.
    advance_ticks(&mut world, RESTOCK_TICKS / 2);
    assert_eq!(
        stock_of(&world, HALL_LIST, HALL_SOE),
        HALL_SOE_STOCK,
        "restocked on the first sale's clock"
    );
}

/// **A shelf that was part-sold before the shutdown comes back part-sold, and
/// resumes the remainder of its delay.**
#[test]
fn boot_restores_saved_stock_and_resumes_its_timer() {
    let (mut world, _db, _rx) = dist_shop_world(HALL_MANAGER);
    let now = world.now_millis();
    handle_db_event(
        &mut world,
        DbEvent::BuyListStockLoaded {
            // Two left, restocking in half an hour.
            rows: vec![(HALL_LIST, HALL_SOE, 2, now + 30 * 60_000)],
        },
    );
    assert_eq!(stock_of(&world, HALL_LIST, HALL_SOE), 2, "restored");

    advance_ticks(&mut world, RESTOCK_TICKS / 2 - 1);
    assert_eq!(stock_of(&world, HALL_LIST, HALL_SOE), 2, "not yet");
    advance_ticks(&mut world, 1);
    assert_eq!(stock_of(&world, HALL_LIST, HALL_SOE), HALL_SOE_STOCK);
}

/// **A deadline that expired while the server was down restocks at boot
/// instead of waiting out a fresh delay** (`restartRestockTask`'s `else`).
#[test]
fn boot_restocks_a_deadline_that_already_passed() {
    let (mut world, _db, _rx) = dist_shop_world(HALL_MANAGER);
    let now = world.now_millis();
    handle_db_event(
        &mut world,
        DbEvent::BuyListStockLoaded {
            rows: vec![(HALL_LIST, HALL_SOE, 1, now - 1)],
        },
    );

    assert_eq!(
        stock_of(&world, HALL_LIST, HALL_SOE),
        HALL_SOE_STOCK,
        "restocked immediately"
    );
    assert!(restock_tasks(&world).is_empty(), "no timer left running");
}

/// **A saved row whose product the datapack no longer declares is dropped,
/// not resurrected as a phantom shelf.** Java warns and `continue`s; the port
/// does the lookup on the game thread, where the lists are.
#[test]
fn boot_drops_stock_rows_the_datapack_no_longer_declares() {
    let (mut world, _db, _rx) = dist_shop_world(HALL_MANAGER);
    let now = world.now_millis();
    handle_db_event(
        &mut world,
        DbEvent::BuyListStockLoaded {
            rows: vec![
                (HALL_LIST, 999_999, 1, now + 60_000), // no such product
                (999_999, HALL_SOE, 1, now + 60_000),  // no such list
                (HALL_LIST, 6902, 1, now + 60_000),    // unlimited product
            ],
        },
    );
    assert!(world.buy_list_stock.is_empty());
    assert!(restock_tasks(&world).is_empty());
}

/// **A saved row already at full stock is skipped**, which is also what drops
/// a stale deadline: a full product has no timer.
#[test]
fn boot_skips_a_row_that_is_already_full() {
    let (mut world, _db, _rx) = dist_shop_world(HALL_MANAGER);
    let now = world.now_millis();
    handle_db_event(
        &mut world,
        DbEvent::BuyListStockLoaded {
            rows: vec![(HALL_LIST, HALL_SOE, HALL_SOE_STOCK, now + 60_000)],
        },
    );
    assert!(world.buy_list_stock.is_empty());
    assert!(restock_tasks(&world).is_empty());
}

/// Gludio's mercenary manager Greenspan and one of his posting tickets — the
/// only items on this dist with `etcitem_type="CASTLE_GUARD"`.
const MERC_MANAGER: i32 = 35102;
const MERC_LIST: i32 = 351021;
const MERC_TICKET: i32 = 3960;

/// **`RateSiegeGuardsPrice` scales a `CASTLE_GUARD` item's price, and only
/// that item's.**
///
/// The rate ships as 1, so nothing on this dist notices; the multiply is
/// carried because it is the one knob between a server that raises it and a
/// garrison that costs nothing. Asserted at a rate of 3 so the arithmetic is
/// actually exercised rather than confirmed to be an identity.
#[test]
fn the_siege_guard_rate_scales_only_castle_guard_items() {
    let (mut world, _db, _rx) = dist_shop_world(MERC_MANAGER);
    world.cfg.rates.rate_siege_guards_price = 3.0;
    let before = adena_of(&world, 3001);

    shop::handle_request_buy_item(&mut world, 1, &buy_body(MERC_LIST, &[(MERC_TICKET, 1)]));
    assert_eq!(count_of_item(&world, 3001, MERC_TICKET), 1);
    assert_eq!(
        before - adena_of(&world, 3001),
        150_000,
        "50 000 × 3, and this list declares no baseTax"
    );

    // A plain item on a plain list is untouched by the same rate.
    let (mut world, _db, _rx) = dist_shop_world(COOPER);
    world.cfg.rates.rate_siege_guards_price = 3.0;
    let before = adena_of(&world, 3001);
    shop::handle_request_buy_item(&mut world, 1, &buy_body(COOPER_LIST, &[(IRON_CANINE, 1)]));
    assert_eq!(
        before - adena_of(&world, 3001),
        8400,
        "still 7000 + 20% — not a CASTLE_GUARD item"
    );
}
