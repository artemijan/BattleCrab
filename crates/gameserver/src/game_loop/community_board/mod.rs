//! Community board (BBS) — the runtime side of `CommunityBoardHandler` plus
//! the `HomeBoard` handler from `dist/game/data/scripts/handlers/communityboard`.
//!
//! This dist runs `CustomCommunityBoard = True`, so the live board is the
//! custom navigation board. Ported so far:
//!   - the board window plumbing (`RequestShowBoard` → `_bbshome`, the
//!     `_bbs*` bypass routing, and the chunked `ShowBoard` sender);
//!   - `HomeBoard` home rendering (`_bbshome`/`_bbstop`) with the navigation
//!     panel injected;
//!   - the cheap, already-portable actions: `_bbsheal`, `_bbsteleport`,
//!     `_bbsbuff` (direct buff list);
//!   - `_bbspremium` — buy account premium (reuses the `PremiumManager` store
//!     also driving `//premium_*`);
//!   - the scheme buffer (`_bbs_buff_scheme_create`/`_delete`/`_execute`),
//!     backed by the `buffer_schemes` table + the `SchemeBufferSkills.xml`
//!     available-buff levels;
//!   - the `FavoriteBoard` (`_bbsgetfav`/`bbs_add_fav`/`_bbsdelfav_`) backed by
//!     the `bbs_favorites` table, and the `HomepageBoard` (`_bbslink`) — the two
//!     client-toolbar buttons that live outside `HomeBoard`;
//!   - the `DropSearchBoard` (`_bbs_search_item`/`_bbs_search_drop`/
//!     `_bbs_npc_trace`): item-name search over the drop index, the per-item
//!     drop/spoil list at server rates, and the `RadarControl` world-map trace.
//!
//!   - the merchant actions (`_bbsmultisell`/`_bbsexcmultisell`/`_bbssell`)
//!     over the multisell/buy-list systems, and `_bbsdelevel` (config-off on
//!     this dist, ported per the config-disabled rule);
//!   - the `_bbsteleport` 3 s skill lock (`SkillsDisabled` + a timed
//!     re-enable) and the retail home's favorite counter.
//!
//! The retail boards are ported to the reference's own depth — which is
//! shallower than their names suggest: `_bbsloc` renders the region list off
//! the castles (its per-region detail is left unimplemented in Java
//! itself), `_maillist`/`_bbsmemo`/`_friendlist` serve their html shells
//! (their writes are Java TODOs too), and the home page's `%region_count%`
//! is 0 because Java's `getRegionCount` returns 0. The one board with real
//! machinery is the clan board: the paginated clan list, the clan home page,
//! and the clan notice (edit / enable / disable + the `Notice Set` write),
//! stored in `clan_notices` and popped up to members at login while enabled.
//! All of it unreachable in production — `CustomCommunityBoard = True` never
//! links here — ported per the config-disabled rule.

use crate::game_loop::helpers::send_message;
use crate::game_loop::helpers::send_to_client;
use crate::network::server_packets as sp;
use crate::world::World;
use tracing::warn;

/// `Util.sendCBHtml`'s single-packet chunk size (client reassembles 101/102/103).
const CHUNK: usize = 16250;

/// `Config.BUFFER_MAX_SCHEMES` from `config/Custom/SchemeBuffer.ini`
/// (`BufferMaxSchemesPerChar = 5`). Inlined like the premium flag — a dedicated
/// SchemeBuffer.ini loader isn't ported and the dist value is authoritative.
const MAX_SCHEMES: usize = 5;

/// Entry point for `RequestShowBoard` and every `_bbs*` bypass — port of
/// `CommunityBoardHandler.handleParseCommand` + `HomeBoard.parseCommunityBoardCommand`.
pub(crate) fn handle_parse_command(world: &mut World, client_id: u32, command: &str) {
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };

    if !world.cfg.community_board.enabled {
        send_to_client(
            world,
            client_id,
            sp::system_message_with(sp::sm_ids::THE_COMMUNITY_SERVER_IS_CURRENTLY_OFFLINE, &[]),
        );
        return;
    }

    // `FavoriteBoard` / `HomepageBoard` / `DropSearchBoard` are their own
    // handlers in the Java datapack (not `HomeBoard`), so they skip the combat +
    // karma gates below.
    match first_token(command) {
        "_bbslink" => return show_homepage(world, client_id),
        "_bbsgetfav" => return show_favorites(world, client_id, object_id),
        "bbs_add_fav" => return add_favorite(world, client_id, object_id),
        t if t.starts_with("_bbsdelfav_") => return del_favorite(world, client_id, object_id, t),
        "_bbs_search_item" | "_bbs_search_drop" | "_bbs_npc_trace" => {
            return do_drop_search(world, client_id, command);
        }
        _ => {}
    }

    // `HomeBoard.COMBAT_CHECK`: the custom action commands are refused while the
    // player is busy — every clause of Java's predicate, see `is_busy`.
    if is_custom_action(command) && is_busy(world, object_id) {
        send_message(
            world,
            client_id,
            "You can't use the Community Board right now.",
        );
        return;
    }

    // `HomeBoard.KARMA_CHECK`.
    if world.cfg.community_board.karma_disabled && reputation(world, object_id) < 0 {
        send_message(
            world,
            client_id,
            "Players with Karma cannot use the Community Board.",
        );
        return;
    }

    match first_token(command) {
        "_bbshome" | "_bbstop" => show_home(world, client_id, object_id, command),
        "_bbsheal" => do_heal(world, client_id, object_id, command),
        "_bbsteleport" => do_teleport(world, client_id, object_id, command),
        "_bbsbuff" => do_buff(world, client_id, object_id, command),
        "_bbspremium" => do_premium(world, client_id, object_id, command),
        "_bbs_buff_scheme_create" | "_bbs_buff_scheme_delete" | "_bbs_buff_scheme_execute" => {
            do_scheme(world, client_id, object_id, command)
        }
        // `HomeBoard`'s merchant branches: open a multisell (`_bbsmultisell`
        // full / `_bbsexcmultisell` exchange) or the sell window (`_bbssell`),
        // re-rendering the accompanying Custom page when one is named.
        "_bbsmultisell" => do_multisell(world, client_id, object_id, command, false),
        "_bbsexcmultisell" => do_multisell(world, client_id, object_id, command, true),
        "_bbssell" => do_sell(world, client_id, object_id, command),
        "_bbsdelevel" => do_delevel(world, client_id, object_id),
        "_bbsloc" => show_region_board(world, client_id, object_id, command),
        "_bbsclan"
        | "_bbsclan_clanlist"
        | "_bbsclan_clanhome"
        | "_bbsclan_clannotice_edit"
        | "_bbsclan_clannotice_enable"
        | "_bbsclan_clannotice_disable" => show_clan_board(world, client_id, object_id, command),
        "_maillist" => show_shell(world, client_id, object_id, "mail.html", command),
        "_bbsmemo" => show_shell(world, client_id, object_id, "memo.html", command),
        "_friendlist" => show_shell(world, client_id, object_id, "friends_list.html", command),
        other => {
            warn!("CommunityBoard: unhandled/unported command [{other}] (full: [{command}]).");
        }
    }
}

/// Port of `CommunityBoardHandler.handleWriteCommand` — the `RequestBBSwrite`
/// submit. Java maps `url` → a `_bbs*` write command (Topic/Region/Notice),
/// all of which target the retail forum boards that are deferred here, so we
/// answer Java's "not implemented yet" page for every `url`.
pub(crate) fn handle_write_command(
    world: &mut World,
    client_id: u32,
    url: &str,
    args: &[String; 5],
) {
    if !world.cfg.community_board.enabled {
        return;
    }
    // `ClanBoard.writeCommunityBoardCommand` — "the only Write bypass that
    // comes to this handler is `Write Notice Set _ Content Content Content`":
    // arg1 = "Set", arg3 = the notice text. Every other write target
    // (Topic/Post/Region/Mail/Memo) is left unimplemented in Java itself,
    // so the not-implemented answer below IS the reference's behaviour.
    if url == "Notice" && args[0] == "Set" {
        let Some(object_id) = world.player_oid(client_id) else {
            return;
        };
        let Some(clan_id) = clans::clan_of(world, object_id) else {
            return;
        };
        let is_leader = world
            .clans
            .get(&clan_id)
            .is_some_and(|c| c.leader_id == object_id);
        if is_leader {
            let enabled = world
                .clan_notices
                .get(&clan_id)
                .map(|(e, _)| *e)
                .unwrap_or(false);
            world
                .clan_notices
                .insert(clan_id, (enabled, args[2].clone()));
            let _ = world.db.send(crate::db::DbCommand::SaveClanNotice {
                clan_id,
                enabled,
                notice: args[2].clone(),
            });
            show_clan_board(world, client_id, object_id, "_bbsclan_clannotice_edit");
        }
        return;
    }
    let html = format!(
        "<html><body><br><br><center>The command: {url} is not implemented yet.</center><br><br></body></html>"
    );
    send_cb_html(world, client_id, &html);
}

mod actions;
mod clan;
mod drop_search;
mod favorites;
mod home;
mod merchant;
mod scheme;
mod util;

use crate::game_loop::clans;
use actions::{do_buff, do_delevel, do_heal, do_premium, do_teleport};
use clan::{show_clan_board, show_region_board};
use drop_search::do_drop_search;
use favorites::{add_favorite, del_favorite, show_favorites, show_homepage};
pub(crate) use home::open_home_for_admin;
use home::{show_home, show_shell};
use merchant::{do_multisell, do_sell};
#[cfg(test)]
pub(crate) use scheme::apply_scheme;
use scheme::{do_scheme, render_scheme_names};
use util::{
    account_of, charge, charge_item, finalize_custom, first_token, is_busy, is_custom_action,
    read_html, reputation, serve_page,
};

/// Port of `Util.sendCBHtml`: split the html into ≤3 chunks tagged 101/102/103
/// and send each as a `ShowBoard`. Split by char boundaries (htmls are ASCII,
/// so this matches Java's UTF-16-length branches for the content we serve).
pub(crate) fn send_cb_html(world: &World, client_id: u32, html: &str) {
    for packet in build_cb_packets(html) {
        send_to_client(world, client_id, packet);
    }
}

/// The chunk packets for one board html (pure, so it's unit-testable).
fn build_cb_packets(html: &str) -> Vec<Vec<u8>> {
    let chars: Vec<char> = html.chars().collect();
    let n = chars.len();
    let take = |from: usize, to: usize| -> String { chars[from..to.min(n)].iter().collect() };
    if n < CHUNK {
        vec![
            sp::show_board("101", Some(html)),
            sp::show_board("102", None),
            sp::show_board("103", None),
        ]
    } else if n < CHUNK * 2 {
        vec![
            sp::show_board("101", Some(&take(0, CHUNK))),
            sp::show_board("102", Some(&take(CHUNK, n))),
            sp::show_board("103", None),
        ]
    } else if n < CHUNK * 3 {
        vec![
            sp::show_board("101", Some(&take(0, CHUNK))),
            sp::show_board("102", Some(&take(CHUNK, CHUNK * 2))),
            sp::show_board("103", Some(&take(CHUNK * 2, n))),
        ]
    } else {
        vec![
            sp::show_board(
                "101",
                Some("<html><body><br><center>Error: HTML was too long!</center></body></html>"),
            ),
            sp::show_board("102", None),
            sp::show_board("103", None),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decode a `ShowBoard` packet's content string (skip the opcode, the
    /// show/hide byte and the 8 fixed nav strings).
    fn cb_content(pkt: &[u8]) -> String {
        assert_eq!(pkt[0], sp::opcodes::SHOW_BOARD);
        let mut r = commons::network::PacketReader::new(&pkt[2..]);
        for _ in 0..8 {
            r.read_string().unwrap();
        }
        r.read_string().unwrap()
    }

    #[test]
    fn short_html_is_one_content_packet_plus_two_empty() {
        let pkts = build_cb_packets("<html><body>hi</body></html>");
        assert_eq!(pkts.len(), 3);
        assert!(
            cb_content(&pkts[0]).starts_with("101\u{0008}"),
            "first chunk tagged 101"
        );
        assert!(
            cb_content(&pkts[0]).contains("hi"),
            "html lands in the first chunk"
        );
        // Empty continuation chunks reproduce Java's literal `null`.
        assert_eq!(cb_content(&pkts[1]), "102\u{0008}null");
        assert_eq!(cb_content(&pkts[2]), "103\u{0008}null");
    }

    #[test]
    fn long_html_splits_across_two_chunks() {
        let html = "x".repeat(CHUNK + 500); // between 1 and 2 chunks
        let pkts = build_cb_packets(&html);
        assert_eq!(pkts.len(), 3);
        let c0 = cb_content(&pkts[0]);
        let c1 = cb_content(&pkts[1]);
        assert_eq!(
            c0.chars().count(),
            "101\u{0008}".chars().count() + CHUNK,
            "first chunk is full"
        );
        assert_eq!(
            c1.chars().count(),
            "102\u{0008}".chars().count() + 500,
            "remainder in the second"
        );
        assert_eq!(cb_content(&pkts[2]), "103\u{0008}null", "third chunk empty");
    }
}
