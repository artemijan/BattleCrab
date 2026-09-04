//! The drop-search board: item search, drop tables and NPC trace.

use super::read_html;
use super::send_cb_html;
use crate::game_loop::helpers::send_message;
use crate::game_loop::helpers::send_to_client;
use crate::network::server_packets as sp;
use crate::world::World;
use tracing::warn;
/// Port of `DropSearchBoard.parseCommunityBoardCommand`. Java splits the raw
/// command on spaces and switches on `params[0]`, so the nav "Search" button
/// (`_bbs_search_item;`, trailing `;`, no space) matches no case and just
/// renders the empty search page.
pub(super) fn do_drop_search(world: &mut World, client_id: u32, command: &str) {
    let params: Vec<&str> = command.split(' ').collect();
    match params[0] {
        "_bbs_search_item" => {
            // `buildItemName` joins params[1..] with spaces (empty = match all).
            let item_name = params[1..].join(" ");
            let result = build_item_search_result(world, &item_name);
            render_drop_search(world, client_id, |html| {
                html.replace("%searchResult%", &result)
            });
        }
        "_bbs_search_drop" => do_search_drop(world, client_id, &params),
        "_bbs_npc_trace" => {
            do_npc_trace(world, client_id, &params);
            // Java re-renders the (bare) main.html afterwards — the search-result
            // tokens are left unreplaced, exactly as in the datapack.
            render_drop_search(world, client_id, |html| html);
        }
        // The nav `_bbs_search_item;` and any unknown tail: the empty page.
        _ => render_drop_search(world, client_id, |html| html),
    }
}

/// Read `dropsearch/main.html`, apply the caller's result-token substitution,
/// inject the navigation panel (Java replaces `%navigation%` last), and send.
pub(super) fn render_drop_search(world: &World, client_id: u32, f: impl FnOnce(String) -> String) {
    let Some(html) = read_html(
        world,
        client_id,
        "data/html/CommunityBoard/Custom/dropsearch/main.html",
    ) else {
        warn!("CommunityBoard: missing dropsearch/main.html.");
        return;
    };
    let navigation = read_html(
        world,
        client_id,
        "data/html/CommunityBoard/Custom/navigation.html",
    )
    .unwrap_or_default();
    let html = f(html).replace("%navigation%", &navigation);
    send_cb_html(world, client_id, &html);
}

/// Port of `DropSearchBoard.buildItemSearchResult`: the ≤14 lowest-id items
/// whose name contains `item_name` and that appear in the drop index, rendered
/// as a 2-per-row icon grid padded to 7 rows.
pub(super) fn build_item_search_result(world: &World, item_name: &str) -> String {
    let index = world.data.npc_data.drop_index();
    let needle = item_name.to_lowercase();
    let mut items: Vec<&crate::data::item_data::template::ItemTemplate> = world
        .data
        .item_data
        .all()
        .filter(|it| index.contains_key(&it.item_id))
        .filter(|it| it.name.to_lowercase().contains(&needle))
        .collect();
    // Java iterates the id-indexed template array (id-ascending) and stops at 14.
    items.sort_by_key(|it| it.item_id);
    items.truncate(14);

    if items.is_empty() {
        return "<tr><td width=100 align=CENTER>No Match</td></tr>".to_string();
    }

    let mut sb = String::new();
    let mut col = 0; // Java's `i`, cycling 1,2 across the two columns.
    let mut rows = 0;
    for it in &items {
        col += 1;
        if col == 1 {
            rows += 1;
            sb.push_str("<tr>");
        }
        let icon = world.data.item_data.icon(it.item_id);
        sb.push_str(&format!(
            "<td><button value=\".\" action=\"bypass _bbs_search_drop {id} 1 $order $level\" \
             width=32 height=32 back=\"{icon}\" fore=\"{icon}\"></td><td width=200>&#{id};</td>",
            id = it.item_id
        ));
        if col == 2 {
            sb.push_str("</tr>");
            col = 0;
        }
    }
    if col % 2 == 1 {
        sb.push_str("</tr>");
    }
    for _ in rows..7 {
        sb.push_str("<tr><td height=36></td></tr>");
    }
    sb
}

/// Port of `DropSearchBoard`'s `_bbs_search_drop` branch: the paged drop/spoil
/// list for one item, each row at the server's real-time rate (the herb /
/// premium / stat-bonus factors stay ×1, like the `NpcViewMod` preview — exact
/// for the stock rates).
pub(super) fn do_search_drop(world: &World, client_id: u32, params: &[&str]) {
    let (Some(item_id), Some(page)) = (
        params.get(1).and_then(|s| s.parse::<i32>().ok()),
        params.get(2).and_then(|s| s.parse::<i32>().ok()),
    ) else {
        return;
    };
    let list = world
        .data
        .npc_data
        .drop_index()
        .get(&item_id)
        .cloned()
        .unwrap_or_default();
    let rates = &world.cfg.rates;

    let mut pages = list.len() / 14;
    if pages == 0 {
        pages += 1;
    }
    // Java: `start = (page-1)*14`, `end = min(size-1, start+14)`, loop `<= end`
    // (an off-by-one that shows 15 rows on a full page — ported verbatim).
    let start = (page.max(1) as usize - 1) * 14;
    let mut result = String::new();
    if !list.is_empty() {
        let end = (list.len() - 1).min(start + 14);
        for d in list.iter().take(end + 1).skip(start) {
            let rate_chance = drop_rate(
                rates.spoil_drop_chance_multiplier,
                rates.raid_drop_chance_multiplier,
                rates.death_drop_chance_multiplier,
                d.is_spoil,
                d.is_raid,
            ) * if d.is_spoil {
                1.0
            } else {
                rates
                    .drop_chance_by_id
                    .get(&d.item_id)
                    .copied()
                    .unwrap_or(1.0)
            };
            let rate_amount = drop_rate(
                rates.spoil_drop_amount_multiplier,
                rates.raid_drop_amount_multiplier,
                rates.death_drop_amount_multiplier,
                d.is_spoil,
                d.is_raid,
            ) * if d.is_spoil {
                1.0
            } else {
                rates
                    .drop_amount_by_id
                    .get(&d.item_id)
                    .copied()
                    .unwrap_or(1.0)
            };
            let min = fmt_amount(d.min as f64 * rate_amount);
            let max = fmt_amount(d.max as f64 * rate_amount);
            result.push_str(&format!(
                "<tr><td width=30>{lvl}</td>\
                 <td width=170><a action=\"bypass _bbs_npc_trace {npc}\">&@{npc};</a></td>\
                 <td width=80 align=CENTER>{min}-{max}</td>\
                 <td width=50 align=CENTER>{chance:.2}%</td>\
                 <td width=50 align=CENTER>{kind}</td></tr>",
                lvl = d.npc_level,
                npc = d.npc_id,
                chance = d.chance * rate_chance,
                kind = if d.is_spoil { "Spoil" } else { "Drop" },
            ));
        }
    }

    let mut pages_html = String::from("<tr>");
    for p in 1..=pages {
        pages_html.push_str(&format!(
            "<td><a action=\"bypass -h _bbs_search_drop {item_id} {p} $order $level\">{p}</a></td>"
        ));
    }
    pages_html.push_str("</tr>");

    render_drop_search(world, client_id, |html| {
        html.replace("%searchResult%", &result)
            .replace("%pages%", &pages_html)
    });
}

/// The `is_spoil`/`is_raid`/normal base-rate pick shared by the chance and
/// amount branches (mirrors `NpcViewMod`'s drop-list preview).
pub(super) fn drop_rate(spoil: f64, raid: f64, death: f64, is_spoil: bool, is_raid: bool) -> f64 {
    if is_spoil {
        spoil
    } else if is_raid {
        raid
    } else {
        death
    }
}

/// Java prints `min * rate` / `max * rate` as a raw double; at stock (integer)
/// rates that is a whole number — show it without the trailing `.0`, else keep
/// two decimals.
pub(super) fn fmt_amount(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.2}")
    }
}

/// Port of `DropSearchBoard`'s `_bbs_npc_trace` branch: pick a random live spawn
/// of the NPC and drop a world-map marker on it (Java `Radar.addMarker`), or
/// message the player when none is spawned (bosses / instance mobs).
pub(super) fn do_npc_trace(world: &mut World, client_id: u32, params: &[&str]) {
    let Some(npc_id) = params.get(1).and_then(|s| s.parse::<i32>().ok()) else {
        return;
    };
    let mut locs: Vec<(i32, i32, i32)> = Vec::new();
    world
        .objects
        .for_each_mut::<&crate::model::npc::Npc>(|npc| {
            if npc.npc_id == npc_id {
                locs.push(npc.spawn_loc);
            }
        });
    if locs.is_empty() {
        send_message(
            world,
            client_id,
            "Cannot find any spawn. Maybe dropped by a boss or instance monster.",
        );
        return;
    }
    let (x, y, z) = locs[world.roll(locs.len() as i32) as usize];
    // `Radar.addMarker` sends the pair (showRadar=2, type=2) then (0, 1).
    send_to_client(world, client_id, sp::radar_control(2, 2, x, y, z));
    send_to_client(world, client_id, sp::radar_control(0, 1, x, y, z));
}
