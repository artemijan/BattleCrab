//! Shared board plumbing: busy/zone/reputation gates, adena and item
//! charges, html loading and the custom-page renderer.

use super::render_scheme_names;
use super::send_cb_html;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::send_message;
use crate::game_loop::user_commands::in_combat;
use crate::model::Player;
use crate::model::components::Casting;
use crate::model::inventory::Inventory;
use crate::session::ClientSession;
use crate::world::World;
use tracing::warn;
/// Re-render a Custom sub-page after an action (the `page` tail the action
/// bypasses carry, e.g. `buffer/main` or `buffer/schemes.html`), with an
/// optional error banner. No-op if the tail is missing.
pub(super) fn serve_page(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    page: Option<&str>,
    error_message: &str,
) {
    let Some(page) = page.filter(|p| !p.is_empty()) else {
        return;
    };
    // Java's `_bbsbuff`/`_bbsheal` pass a bare tail (`buffer/main`) and append
    // ".html" unconditionally; the scheme/premium paths already carry it.
    let file = if page.ends_with(".html") {
        page.to_string()
    } else {
        format!("{page}.html")
    };
    let rel = format!("data/html/CommunityBoard/Custom/{file}");
    let Some(html) = read_html(world, client_id, &rel) else {
        warn!("CommunityBoard: missing sub-page [{rel}].");
        return;
    };
    let html = finalize_custom(world, object_id, html, error_message);
    send_cb_html(world, client_id, &html);
}

/// Port of `HomeBoard`'s post-build custom substitution: inject the navigation
/// panel, the `%errorMessage%` banner, and the `%schemenames%` scheme buttons.
pub(super) fn finalize_custom(
    world: &World,
    object_id: i32,
    html: String,
    error_message: &str,
) -> String {
    let navigation = read_html_for(
        world,
        object_id,
        "data/html/CommunityBoard/Custom/navigation.html",
    )
    .unwrap_or_default();
    let mut html = html
        .replace("%navigation%", &navigation)
        .replace("%errorMessage%", error_message);
    if html.contains("%schemenames%") {
        html = html.replace("%schemenames%", &render_scheme_names(world, object_id));
    }
    html
}

/// The `_bbs*` commands `HomeBoard.CUSTOM_COMMANDS` guards behind the combat
/// check (everything but `_bbshome`/`_bbstop`).
pub(super) fn is_custom_action(command: &str) -> bool {
    let c = first_token(command);
    matches!(
        c,
        "_bbspremium"
            | "_bbsexcmultisell"
            | "_bbsmultisell"
            | "_bbssell"
            | "_bbsteleport"
            | "_bbsbuff"
            | "_bbsheal"
            | "_bbsdelevel"
    ) || c.starts_with("_bbs_buff_scheme")
}

/// The command word up to the first `;` or space (`_bbsteleport;1 2 3` →
/// `_bbsteleport`).
pub(super) fn first_token(command: &str) -> &str {
    command.split([';', ' ']).next().unwrap_or(command)
}

/// The subset of `isInCombat()`-style busy states currently modeled.
pub(super) fn is_busy(world: &World, object_id: i32) -> bool {
    let casting = world.objects.has_component::<Casting>(&object_id);
    let pvp = world
        .objects
        .get_component::<crate::model::components::PvpState>(&object_id)
        .is_some_and(|s| s.flag > 0);
    let dead = is_dead(world, object_id);
    // `isInCombat()` — the 15 s attack stance, not merely mid-swing.
    let is_in_combat = in_combat(world, object_id);
    let in_duel = crate::game_loop::duel::is_in_duel(world, object_id);
    // `isInOlympiadMode()` — the set the match runner maintains.
    let in_olympiad = world.olympiad.in_competition.contains(&object_id);
    // `isInsideZone(SIEGE) || isInsideZone(PVP)` — the zones, which is wider
    // than `is_in_siege` (that one asks whether a *siege* is running).
    let in_pvp_zone = crate::game_loop::pvp::is_in_siege(world, object_id)
        || in_zone(world, object_id, crate::data::zone_data::ZoneKind::Siege)
        || in_zone(world, object_id, crate::data::zone_data::ZoneKind::Pvp);
    let on_event = crate::game_loop::events::tvt::is_on_event(world, object_id);
    casting || pvp || dead || is_in_combat || in_duel || in_olympiad || in_pvp_zone || on_event
}

/// `player.isInsideZone(kind)` — read off the cached `ZoneFlags` the movement
/// path maintains, not recomputed from the position.
pub(super) fn in_zone(
    world: &World,
    object_id: i32,
    kind: crate::data::zone_data::ZoneKind,
) -> bool {
    world
        .objects
        .get_component::<crate::model::components::ZoneFlags>(&object_id)
        .is_some_and(|f| f.contains(kind))
}

pub(super) fn reputation(world: &World, object_id: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.reputation)
        .unwrap_or(0)
}

/// The account name behind a client (Java `player.getAccountName()`), for the
/// account-scoped premium store.
pub(super) fn account_of(world: &World, client_id: u32) -> Option<String> {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => Some(s.account().to_string()),
        _ => None,
    }
}

/// `player.destroyItemByItemId(currency, price)` on the board currency with the
/// "not enough" guard.
pub(super) fn charge(world: &mut World, client_id: u32, object_id: i32, price: i64) -> bool {
    let currency = world.cfg.community_board.currency_id;
    charge_item(world, client_id, object_id, currency, price)
}

/// `player.destroyItemByItemId(item_id, price)` with the "not enough" guard.
/// A zero/negative price is free (no inventory touch). Returns whether the
/// action may proceed.
pub(super) fn charge_item(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    item_id: i32,
    price: i64,
) -> bool {
    if price <= 0 {
        return true;
    }
    let have = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(|inv| inv.count_of(item_id))
        .unwrap_or(0);
    if have < price
        || !crate::game_loop::quests::take_items(world, client_id, object_id, item_id, price)
    {
        send_message(world, client_id, "Not enough currency!");
        return false;
    }
    true
}

/// A community-board page, served **to a client** — Java's board handlers all
/// pass the player to `HtmCache.getHtm`, which is what carries
/// `GMDebugHtmlPaths`. The datapack root comes off `world` rather than being
/// threaded separately; the old `(root, rel)` signature had no recipient to
/// give the debug line to.
pub(super) fn read_html(world: &World, client_id: u32, rel: &str) -> Option<String> {
    crate::data::htm_cache::read_htm_for_client(
        world,
        client_id,
        format!("{}{rel}", world.data.root),
    )
}

/// [`read_html`] for the handlers that hold the viewer's object id instead.
pub(super) fn read_html_for(world: &World, object_id: i32, rel: &str) -> Option<String> {
    crate::data::htm_cache::read_htm_for(world, object_id, format!("{}{rel}", world.data.root))
}
