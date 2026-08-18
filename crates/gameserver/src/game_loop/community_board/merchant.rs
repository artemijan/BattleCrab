//! The merchant board: multisell, sell and the merchant page.

use super::*;
/// `HomeBoard`'s `_bbsmultisell;<id>,<page>` / `_bbsexcmultisell;<id>,<page>`
/// branch: open the multisell window, then re-render the named Custom page (the
/// nav pages pass `_bbstop`, whose file is absent → no re-render, exactly like
/// Java's null `returnHtml`).
pub(super) fn do_multisell(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    command: &str,
    exchange: bool,
) {
    let prefix = if exchange {
        "_bbsexcmultisell;"
    } else {
        "_bbsmultisell;"
    };
    let Some(rest) = command.strip_prefix(prefix) else {
        return;
    };
    let mut opts = rest.split(',');
    let Some(list_id) = opts.next().and_then(|s| s.trim().parse::<i32>().ok()) else {
        warn!("CommunityBoard: bad multisell command [{command}].");
        return;
    };
    let page = opts.next().map(str::trim);
    render_merchant_page(world, client_id, object_id, page);
    crate::game_loop::multisell::separate_and_send(
        world, client_id, object_id, None, list_id, exchange,
    );
}

/// `HomeBoard`'s `_bbssell;<page>` branch: open the sell window (BuyList 423 +
/// the sell tab), then re-render the named page. Buylist 423 is absent on this
/// dist (as it is in the Java datapack — the command is unreachable from the
/// shipped htmls), so the window is skipped with a warn rather than NPE'ing.
pub(super) fn do_sell(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    let page = command.strip_prefix("_bbssell;").map(str::trim);
    render_merchant_page(world, client_id, object_id, page);

    const CB_SELL_BUYLIST: i32 = 423;
    let Some(list) = world.data.buy_lists.get(CB_SELL_BUYLIST) else {
        warn!("CommunityBoard: sell buylist {CB_SELL_BUYLIST} not found.");
        return;
    };
    let list = list.clone();
    let refund_items = crate::game_loop::shop::refund_items_of(world, object_id);
    if let Some(inv) = world.objects.get_component::<Inventory>(&object_id) {
        // Java `HomeBoard`: `new BuyList(…, player, 0)` — the board shop is
        // npc-less, so no castle takes a cut.
        send_to_client(
            world,
            client_id,
            crate::network::trade::buy_list(
                &list,
                inv,
                &world.data,
                0.0,
                world.cfg.rates.rate_siege_guards_price,
                |p| crate::game_loop::shop::stock_left(world, CB_SELL_BUYLIST, p),
            ),
        );
        send_to_client(
            world,
            client_id,
            crate::network::trade::ex_buy_sell_list_sell(
                inv,
                &refund_items,
                &world.data,
                false,
                crate::game_loop::servitor::active_pet_collar(world, object_id),
            ),
        );
    }
}

/// The accompanying-page render for the merchant branches: like [`serve_page`]
/// but silent when the file is missing (the `_bbstop` sentinel names no file
/// and is hit on every purchase — Java just leaves the board unchanged).
pub(super) fn render_merchant_page(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    page: Option<&str>,
) {
    let Some(page) = page.filter(|p| !p.is_empty()) else {
        return;
    };
    let file = if page.ends_with(".html") {
        page.to_string()
    } else {
        format!("{page}.html")
    };
    let rel = format!("data/html/CommunityBoard/Custom/{file}");
    let Some(html) = read_html(world, client_id, &rel) else {
        return;
    };
    let html = finalize_custom(world, object_id, html, "");
    send_cb_html(world, client_id, &html);
}
