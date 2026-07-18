//! Community board (BBS) — the runtime side of `CommunityBoardHandler` plus
//! the `HomeBoard` handler from `dist/game/data/scripts/handlers/communityboard`.
//!
//! This dist runs `CustomCommunityBoard = True`, so the live board is the
//! custom navigation board. This first slice (G30) ports:
//!   - the board window plumbing (`RequestShowBoard` → `_bbshome`, the
//!     `_bbs*` bypass routing, and the chunked `ShowBoard` sender);
//!   - `HomeBoard` home rendering (`_bbshome`/`_bbstop`) with the navigation
//!     panel injected;
//!   - the cheap, already-portable actions: `_bbsheal`, `_bbsteleport`,
//!     `_bbsbuff` (direct buff list).
//!
//! Deferred with `TODO(G30)` at each site (needs subsystems not ported yet):
//! multisell/sell, the scheme buffer (`SchemeBufferTable` + `bbs_favorites`/
//! `buffer_schemes` DB), premium-buy, delevel, drop-search, and the retail
//! forum boards (`_bbsloc`/`_bbsclan`/`_bbsmail`/…), which the custom
//! navigation never links to anyway.

use tracing::warn;

use crate::model::components::Casting;
use crate::model::inventory::Inventory;
use crate::model::Player;
use crate::network::server_packets as sp;
use crate::session::ClientSession;
use crate::world::World;

/// `Util.sendCBHtml`'s single-packet chunk size (client reassembles 101/102/103).
const CHUNK: usize = 16250;

/// Entry point for `RequestShowBoard` and every `_bbs*` bypass — port of
/// `CommunityBoardHandler.handleParseCommand` + `HomeBoard.parseCommunityBoardCommand`.
pub(crate) fn handle_parse_command(world: &mut World, client_id: u32, command: &str) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();

    if !world.cfg.community_board.enabled {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(sp::system_message_with(
                sp::sm_ids::THE_COMMUNITY_SERVER_IS_CURRENTLY_OFFLINE,
                &[],
            ));
        }
        return;
    }

    // `HomeBoard.COMBAT_CHECK`: the custom action commands are refused while the
    // player is busy. The gate that exists here is a subset — casting / pvp flag
    // / dead. TODO(G30): duel, olympiad, SIEGE/PVP zones and event state once
    // those exist (Java also checks `isInDuel`/`isInOlympiadMode`/`isOnEvent`).
    if is_custom_action(command) && is_busy(world, object_id) {
        send_message(world, client_id, "You can't use the Community Board right now.");
        return;
    }

    // `HomeBoard.KARMA_CHECK`.
    if world.cfg.community_board.karma_disabled && reputation(world, object_id) < 0 {
        send_message(world, client_id, "Players with Karma cannot use the Community Board.");
        return;
    }

    match first_token(command) {
        "_bbshome" | "_bbstop" => show_home(world, client_id, command),
        "_bbsheal" => do_heal(world, client_id, object_id, command),
        "_bbsteleport" => do_teleport(world, client_id, object_id, command),
        "_bbsbuff" => do_buff(world, client_id, object_id, command),
        // TODO(G30): `_bbsmultisell`/`_bbsexcmultisell`/`_bbssell` need the
        // multisell + buy-list systems (not ported); `_bbs_buff_scheme_*` needs
        // `SchemeBufferTable` + the `buffer_schemes` table; `_bbspremium` needs
        // `PremiumManager.addPremiumTime`; `_bbsdelevel` is config-disabled in
        // the dist. Each is a `HomeBoard` branch in the Java source.
        other => {
            warn!("CommunityBoard: unhandled/unported command [{other}] (full: [{command}]).");
        }
    }
}

/// Port of `CommunityBoardHandler.handleWriteCommand` — the `RequestBBSwrite`
/// submit. Java maps `url` → a `_bbs*` write command (Topic/Region/Notice),
/// all of which target the retail forum boards that are deferred here, so we
/// answer Java's "not implemented yet" page for every `url`.
pub(crate) fn handle_write_command(world: &mut World, client_id: u32, url: &str) {
    if !world.cfg.community_board.enabled {
        return;
    }
    // TODO(G30): Topic/Post/Region/Notice map to the forum boards (`_bbstop`/
    // `_bbsloc`/`_bbsclan`) — port when those boards land.
    let html = format!(
        "<html><body><br><br><center>The command: {url} is not implemented yet.</center><br><br></body></html>"
    );
    send_cb_html(world, client_id, &html);
}

/// `HomeBoard`'s `_bbshome`/`_bbstop` branch: load the home page (custom or
/// retail) and inject the navigation panel.
fn show_home(world: &mut World, client_id: u32, command: &str) {
    let custom = world.cfg.community_board.custom_enabled;
    let root = &world.data.root;

    // `_bbstop;<page>.html` serves a Custom sub-page (the nav buttons post
    // back through this); bare `_bbshome`/`_bbstop` is the landing page.
    let page = command.strip_prefix("_bbstop;").filter(|p| p.ends_with(".html"));
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
        let navigation = read_html(root, "data/html/CommunityBoard/Custom/navigation.html")
            .unwrap_or_default();
        html = html.replace("%navigation%", &navigation);
        html = html.replace("%errorMessage%", "");
        // The scheme buffer isn't ported (TODO(G30)); render the empty-state
        // string Java shows when a player has no schemes.
        if html.contains("%schemenames%") {
            html = html.replace(
                "%schemenames%",
                "No buffer schemes yet, please make sure you have buffs and then click Create Scheme.",
            );
        }
    } else {
        // Retail home counters. TODO(G30): real favorite/region counts (need the
        // `bbs_favorites` table + region registration) and clan count.
        html = html.replace("%fav_count%", "0");
        html = html.replace("%region_count%", "0");
        html = html.replace("%clan_count%", "0");
    }

    send_cb_html(world, client_id, &html);
}

/// `HomeBoard`'s `_bbsheal;<page>` branch: full HP/MP/CP restore, then re-render
/// the page. Reuses the `//heal` primitive.
fn do_heal(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    if !world.cfg.community_board.enable_heal {
        return;
    }
    let price = world.cfg.community_board.heal_price;
    if !charge(world, client_id, object_id, price) {
        return;
    }
    super::admin::vitals::heal_creature(world, object_id);
    // TODO(G30): Java also restores the player's pet/servitors (not summonable
    // until G29).
    send_message(world, client_id, "You used heal!");
    serve_page(world, client_id, command.strip_prefix("_bbsheal;"));
}

/// `HomeBoard`'s `_bbsteleport;<x> <y> <z>` branch: charge, hide the board and
/// teleport to the whitelisted destination. Reuses the gatekeeper teleport
/// primitive.
fn do_teleport(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    if !world.cfg.community_board.enable_teleports {
        return;
    }
    let Some(key) = command.strip_prefix("_bbsteleport;") else { return };
    let Some(&(x, y, z)) = world.cfg.community_board.available_teleports.get(key.trim()) else {
        warn!("CommunityBoard: teleport [{key}] not in the gatekeeper whitelist.");
        return;
    };
    let price = world.cfg.community_board.teleport_price;
    if !charge(world, client_id, object_id, price) {
        return;
    }
    // Java hides the board (`new ShowBoard()`) and `disableAllSkills()` for 3 s
    // around the teleport. TODO(G30): the temporary skill lock (needs a timed
    // re-enable task) — the teleport itself is the observable effect.
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::show_board_hide());
    }
    let dead = world
        .objects
        .get_component::<crate::model::components::Vitals>(&object_id)
        .is_none_or(|v| v.dead);
    if !dead {
        super::death::teleport_player(world, object_id, x, y, z);
    }
}

/// `HomeBoard`'s `_bbsbuff;<id,lvl>;…;<page>` branch: apply each whitelisted
/// buff to the player, then re-render the page. Reuses the effect engine.
fn do_buff(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    if !world.cfg.community_board.enable_buffs {
        return;
    }
    let body = command.strip_prefix("_bbsbuff;").unwrap_or("");
    let parts: Vec<&str> = body.split(';').collect();
    // Last token is the return page; the rest are `id,level` pairs.
    let (page, buffs) = parts.split_last().map(|(p, b)| (Some(*p), b)).unwrap_or((None, &[]));

    let price = world.cfg.community_board.buff_price * buffs.len() as i64;
    if !charge(world, client_id, object_id, price) {
        return;
    }

    for spec in buffs {
        let mut it = spec.split(',');
        let (Some(id), Some(lvl)) = (
            it.next().and_then(|s| s.trim().parse::<i32>().ok()),
            it.next().and_then(|s| s.trim().parse::<i32>().ok()),
        ) else {
            continue;
        };
        if !world.cfg.community_board.available_buffs.contains(&id) {
            continue; // anti-exploit whitelist (Java `COMMUNITY_AVAILABLE_BUFFS`)
        }
        let Some(skill) = world.data.skill_data.get(id, lvl).cloned() else {
            warn!("CommunityBoard: buff skill {id}/{lvl} missing from skill data.");
            continue;
        };
        // Self-cast, no MP/cast bar (Java `skill.applyEffects(player, player)`).
        // TODO(G30): pet/servitor targets + the `CommunityCastAnimations`
        // `MagicSkillUse` broadcast (summons land in G29).
        crate::game_loop::skills::effects::apply_skill_effects(world, object_id, object_id, &skill);
    }
    serve_page(world, client_id, page);
}

/// Re-render a Custom sub-page after an action (the `page` tail the action
/// bypasses carry, e.g. `buffer/buffs.html`). No-op if the tail is missing.
fn serve_page(world: &mut World, client_id: u32, page: Option<&str>) {
    let Some(page) = page.filter(|p| !p.is_empty()) else { return };
    let rel = format!("data/html/CommunityBoard/Custom/{page}");
    let Some(mut html) = read_html(&world.data.root, &rel) else {
        warn!("CommunityBoard: missing sub-page [{rel}].");
        return;
    };
    let navigation =
        read_html(&world.data.root, "data/html/CommunityBoard/Custom/navigation.html").unwrap_or_default();
    html = html.replace("%navigation%", &navigation).replace("%errorMessage%", "");
    if html.contains("%schemenames%") {
        html = html.replace(
            "%schemenames%",
            "No buffer schemes yet, please make sure you have buffs and then click Create Scheme.",
        );
    }
    send_cb_html(world, client_id, &html);
}

// --- helpers ---------------------------------------------------------------

/// The `_bbs*` commands `HomeBoard.CUSTOM_COMMANDS` guards behind the combat
/// check (everything but `_bbshome`/`_bbstop`).
fn is_custom_action(command: &str) -> bool {
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
fn first_token(command: &str) -> &str {
    command.split([';', ' ']).next().unwrap_or(command)
}

/// The subset of `isInCombat()`-style busy states currently modeled.
fn is_busy(world: &World, object_id: i32) -> bool {
    let casting = world.objects.has_component::<Casting>(&object_id);
    let pvp = world
        .objects
        .get_component::<crate::model::components::PvpState>(&object_id)
        .is_some_and(|s| s.flag > 0);
    let dead = world
        .objects
        .get_component::<crate::model::components::Vitals>(&object_id)
        .is_some_and(|v| v.dead);
    casting || pvp || dead
}

fn reputation(world: &World, object_id: i32) -> i32 {
    world.objects.get_component::<Player>(&object_id).map(|p| p.reputation).unwrap_or(0)
}

/// `player.destroyItemByItemId(currency, price)` with the "not enough" guard.
/// A zero/negative price is free (no inventory touch). Returns whether the
/// action may proceed.
fn charge(world: &mut World, client_id: u32, object_id: i32, price: i64) -> bool {
    if price <= 0 {
        return true;
    }
    let currency = world.cfg.community_board.currency_id;
    let have = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(|inv| inv.count_of(currency))
        .unwrap_or(0);
    if have < price || !super::quests::take_items(world, client_id, object_id, currency, price) {
        send_message(world, client_id, "Not enough currency!");
        return false;
    }
    true
}

fn read_html(root: &str, rel: &str) -> Option<String> {
    std::fs::read_to_string(format!("{root}{rel}")).ok()
}

fn send_message(world: &World, client_id: u32, text: &str) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::system_message_with(
            sp::sm_ids::S1_TEXT,
            &[sp::SmParam::Text(text.to_string())],
        ));
    }
}

/// Port of `Util.sendCBHtml`: split the html into ≤3 chunks tagged 101/102/103
/// and send each as a `ShowBoard`. Split by char boundaries (htmls are ASCII,
/// so this matches Java's UTF-16-length branches for the content we serve).
fn send_cb_html(world: &World, client_id: u32, html: &str) {
    let Some(cs) = world.clients.get(&client_id) else { return };
    for packet in build_cb_packets(html) {
        cs.send(packet);
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
        assert!(cb_content(&pkts[0]).starts_with("101\u{0008}"), "first chunk tagged 101");
        assert!(cb_content(&pkts[0]).contains("hi"), "html lands in the first chunk");
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
        assert_eq!(c0.chars().count(), "101\u{0008}".chars().count() + CHUNK, "first chunk is full");
        assert_eq!(c1.chars().count(), "102\u{0008}".chars().count() + 500, "remainder in the second");
        assert_eq!(cb_content(&pkts[2]), "103\u{0008}null", "third chunk empty");
    }
}
