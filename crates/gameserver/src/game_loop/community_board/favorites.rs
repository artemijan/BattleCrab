//! Favorites and the homepage board.

use super::read_html;
use super::send_cb_html;
use crate::world::World;
use tracing::warn;
/// Port of `HomepageBoard.parseCommunityBoardCommand` (`_bbslink`): serve the
/// static `homepage.html` verbatim (a plain retail page, no navigation inject).
pub(super) fn show_homepage(world: &World, client_id: u32) {
    let Some(html) = read_html(world, client_id, "data/html/CommunityBoard/homepage.html") else {
        warn!("CommunityBoard: missing html [homepage.html].");
        return;
    };
    send_cb_html(world, client_id, &html);
}

/// Port of `FavoriteBoard`'s `_bbsgetfav` branch: render `favorite.html` with
/// one `favorite_list.html` row per stored favorite (newest first, the boot /
/// insert order the mirror keeps). Java re-queries the DB; we render the mirror.
pub(super) fn show_favorites(world: &World, client_id: u32, object_id: i32) {
    let (Some(page), Some(row_tpl)) = (
        read_html(world, client_id, "data/html/CommunityBoard/favorite.html"),
        read_html(
            world,
            client_id,
            "data/html/CommunityBoard/favorite_list.html",
        ),
    ) else {
        warn!("CommunityBoard: missing favorite html.");
        return;
    };
    let mut list = String::new();
    if let Some(favs) = world.bbs_favorites.get(&object_id) {
        for fav in favs {
            let row = row_tpl
                .replace("%fav_bypass%", &fav.bypass)
                .replace("%fav_title%", &fav.title)
                .replace("%fav_add_date%", &fav.add_date)
                .replace("%fav_id%", &fav.fav_id.to_string());
            list.push_str(&row);
        }
    }
    send_cb_html(world, client_id, &page.replace("%fav_list%", &list));
}

/// Port of `FavoriteBoard`'s `bbs_add_fav` branch: pop the last-navigated bypass
/// (`title&bypass`, stored by `_bbshome`), insert it, then re-render the list
/// (Java's `parseCommunityBoardCommand("_bbsgetfav")` callback).
pub(super) fn add_favorite(world: &mut World, client_id: u32, object_id: i32) {
    let Some((title, bypass)) = world.cb_last_bypass.remove(&object_id) else {
        // Java logs "not a valid bypass" when nothing was queued; nothing to add.
        return;
    };
    let fav_id = world.next_fav_id;
    world.next_fav_id += 1;
    let add_date = format_fav_date(commons::util::now_millis());
    world.bbs_favorites.entry(object_id).or_default().insert(
        0,
        crate::world::Favorite {
            fav_id,
            title: title.clone(),
            bypass: bypass.clone(),
            add_date: add_date.clone(),
        },
    );
    let _ = world.db.send(crate::db::DbCommand::StoreFavorite {
        fav_id,
        player_id: object_id,
        title,
        bypass,
        add_date,
    });
    show_favorites(world, client_id, object_id);
}

/// Port of `FavoriteBoard`'s `_bbsdelfav_<id>` branch: drop the favorite by id
/// (Java validates `Util.isDigit`), then re-render the list.
pub(super) fn del_favorite(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    let Some(fav_id) = command
        .strip_prefix("_bbsdelfav_")
        .and_then(|s| s.parse::<i32>().ok())
    else {
        warn!("CommunityBoard: [{command}] is not a valid favorite id.");
        return;
    };
    if let Some(favs) = world.bbs_favorites.get_mut(&object_id) {
        favs.retain(|f| f.fav_id != fav_id);
    }
    let _ = world.db.send(crate::db::DbCommand::DeleteFavorite {
        player_id: object_id,
        fav_id,
    });
    show_favorites(world, client_id, object_id);
}

/// Java `SimpleDateFormat("yyyy-MM-dd HH:mm:ss")` on `favAddDate` — the display
/// string stored verbatim (matches SQL `CURRENT_TIMESTAMP` too).
pub(super) fn format_fav_date(millis: i64) -> String {
    let (year, month, day, hour, minute, second) = commons::util::civil_from_millis(millis);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}
