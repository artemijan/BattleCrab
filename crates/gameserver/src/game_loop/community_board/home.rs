//! The home page and the shell wrapper the custom boards render into.

use super::*;
/// A retail board that is just an html shell in Java (`MailBoard`,
/// `MemoBoard`, `FriendsBoard` — their writes are Java TODOs).
pub(super) fn show_shell(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    file: &str,
    command: &str,
) {
    world
        .cb_last_bypass
        .insert(object_id, ("Board".to_string(), command.to_string()));
    let root = world.data.root.clone();
    if let Some(html) = read_html(&root, &format!("data/html/CommunityBoard/{file}")) {
        send_cb_html(world, client_id, &html);
    }
}

/// `//bbs` (AdminBBS) — the GM shortcut onto the board's home page.
pub(crate) fn open_home_for_admin(world: &mut World, client_id: u32, object_id: i32) {
    show_home(world, client_id, object_id, "_bbshome");
}

/// `HomeBoard`'s `_bbshome`/`_bbstop` branch: load the home page (custom or
/// retail) and inject the navigation panel.
pub(super) fn show_home(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    let custom = world.cfg.community_board.custom_enabled;

    // `_bbstop;<page>.html` serves a Custom sub-page (the nav buttons post
    // back through this); bare `_bbshome`/`_bbstop` is the landing page.
    let page = command
        .strip_prefix("_bbstop;")
        .filter(|p| p.ends_with(".html"));
    // Java only `addBypass(player, "Home", command)`s on the bare landing page,
    // so `bbs_add_fav` (client toolbar) bookmarks the board home.
    if page.is_none() {
        world
            .cb_last_bypass
            .insert(object_id, ("Home".to_string(), command.to_string()));
    }
    let root = &world.data.root;
    let rel = match page {
        Some(p) if custom => format!("data/html/CommunityBoard/Custom/{p}"),
        Some(p) => format!("data/html/CommunityBoard/{p}"),
        None if custom => "data/html/CommunityBoard/Custom/home.html".to_string(),
        None => "data/html/CommunityBoard/home.html".to_string(),
    };

    let Some(mut html) = read_html(root, &rel) else {
        warn!("CommunityBoard: missing html [{rel}].");
        return;
    };

    if custom {
        html = finalize_custom(world, object_id, html, "");
    } else {
        // Retail home counters. The favorite count reads the same store the
        // `FavoriteBoard` maintains; region/clan stay 0 with the retail forum
        // boards (see the module header's deferral).
        let favs = world.bbs_favorites.get(&object_id).map_or(0, |f| f.len());
        html = html.replace("%fav_count%", &favs.to_string());
        // `getRegionCount` returns 0 in Java itself (left unimplemented there).
        html = html.replace("%region_count%", "0");
        html = html.replace("%clan_count%", &world.clans.len().to_string());
    }

    send_cb_html(world, client_id, &html);
}
