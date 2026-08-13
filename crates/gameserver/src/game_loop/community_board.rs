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

use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::send_message;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::helpers::skill_by_id;
use crate::game_loop::user_commands::in_combat;
use crate::model::Player;
use crate::model::components::Casting;
use crate::model::inventory::Inventory;
use crate::network::server_packets as sp;
use crate::session::ClientSession;
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
        let Some(clan_id) = super::guard::clan_of(world, object_id) else {
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

/// `RegionBoard`: `_bbsloc` renders the nine regions off the castles — name
/// fstring, owning clan + alliance, buy-tax. The per-region detail
/// (`_bbsloc;id`) is left unimplemented in Java itself, so a valid id gets
/// Java's silent nothing (an invalid one, Java's warn).
fn show_region_board(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    if let Some(id) = command.strip_prefix("_bbsloc;") {
        if id.parse::<u32>().is_err() {
            warn!("CommunityBoard: player {object_id} sent an invalid region bypass [{command}].");
        }
        return;
    }
    world
        .cb_last_bypass
        .insert(object_id, ("Region".to_string(), command.to_string()));
    // The region-name npcstring ids, castle 1..=9 (Java `REGIONS`).
    const REGIONS: [i32; 9] = [1049, 1052, 1053, 1057, 1060, 1059, 1248, 1247, 1056];
    let root = world.data.root.clone();
    let Some(row_tpl) = read_html(&root, "data/html/CommunityBoard/region_list.html") else {
        return;
    };
    let mut rows = String::new();
    for (i, name) in REGIONS.iter().enumerate() {
        let castle_id = i as i32 + 1;
        let owner = crate::game_loop::siege::owner_clan_id_opt(world, castle_id)
            .and_then(|id| world.clans.get(&id));
        let (clan_name, ally_name) = owner
            .map(|c| (c.name.clone(), c.ally_name.clone()))
            .unwrap_or(("NPC".to_string(), String::new()));
        let tax = crate::game_loop::castle::tax_percent(
            world,
            castle_id,
            crate::model::castle::TaxType::Buy,
        );
        rows.push_str(
            &row_tpl
                .replace("%region_id%", &i.to_string())
                .replace("%region_name%", &name.to_string())
                .replace("%region_owning_clan%", &clan_name)
                .replace("%region_owning_clan_alliance%", &ally_name)
                .replace("%region_tax_rate%", &format!("{tax}%")),
        );
    }
    let Some(html) = read_html(&root, "data/html/CommunityBoard/region.html") else {
        return;
    };
    send_cb_html(world, client_id, &html.replace("%region_list%", &rows));
}

/// A retail board that is just an html shell in Java (`MailBoard`,
/// `MemoBoard`, `FriendsBoard` — their writes are Java TODOs).
fn show_shell(world: &mut World, client_id: u32, object_id: i32, file: &str, command: &str) {
    world
        .cb_last_bypass
        .insert(object_id, ("Board".to_string(), command.to_string()));
    let root = world.data.root.clone();
    if let Some(html) = read_html(&root, &format!("data/html/CommunityBoard/{file}")) {
        send_cb_html(world, client_id, &html);
    }
}

/// `ClanBoard`: the clan list (7 per page), the clan home page, and the
/// notice edit/enable/disable flow.
fn show_clan_board(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    let my_clan = super::guard::clan_of(world, object_id);
    world
        .cb_last_bypass
        .insert(object_id, ("Clan".to_string(), command.to_string()));
    let arg = command
        .split(';')
        .nth(1)
        .and_then(|a| a.parse::<i32>().ok());
    match first_token(command) {
        "_bbsclan" => {
            let eligible = my_clan
                .and_then(|id| world.clans.get(&id))
                .is_some_and(|c| c.level >= 2);
            if eligible {
                clan_home(world, client_id, object_id, my_clan.unwrap());
            } else {
                clan_list(world, client_id, object_id, 1);
            }
        }
        "_bbsclan_clanlist" => clan_list(world, client_id, object_id, arg.unwrap_or(1)),
        "_bbsclan_clanhome" => {
            let target = arg.or(my_clan);
            if let Some(id) = target {
                clan_home(world, client_id, object_id, id);
            }
        }
        "_bbsclan_clannotice_edit" => clan_notice_page(world, client_id, object_id),
        "_bbsclan_clannotice_enable" | "_bbsclan_clannotice_disable" => {
            let enable = command.starts_with("_bbsclan_clannotice_enable");
            if let Some(clan_id) = my_clan {
                let text = world
                    .clan_notices
                    .get(&clan_id)
                    .map(|(_, t)| t.clone())
                    .unwrap_or_default();
                world.clan_notices.insert(clan_id, (enable, text.clone()));
                let _ = world.db.send(crate::db::DbCommand::SaveClanNotice {
                    clan_id,
                    enabled: enable,
                    notice: text,
                });
            }
            clan_notice_page(world, client_id, object_id);
        }
        _ => {}
    }
}

/// Java `ClanBoard.clanList` — 7 clans per page with the paging buttons.
fn clan_list(world: &mut World, client_id: u32, _object_id: i32, page: i32) {
    let page = page.max(1);
    let mut clans: Vec<(i32, String, String, i32, usize)> = world
        .clans
        .values()
        .map(|c| {
            (
                c.id,
                c.name.clone(),
                c.leader_name().to_string(),
                c.level,
                c.members.len(),
            )
        })
        .collect();
    clans.sort_by_key(|c| c.0);
    let mut html = String::from(
        "<html><body><br><br><center><table border=0 width=610><tr><td><a action=\"bypass _bbsclan_clanlist\">CLAN COMMUNITY</a></td></tr></table>\
         <table border=0 cellspacing=0 cellpadding=2 bgcolor=5A5A5A width=610><tr>\
         <td FIXWIDTH=200 align=center>CLAN NAME</td><td FIXWIDTH=200 align=center>CLAN LEADER</td>\
         <td FIXWIDTH=100 align=center>CLAN LEVEL</td><td FIXWIDTH=100 align=center>CLAN MEMBERS</td></tr></table>",
    );
    // Java's window: rows `(page-1)*7 ..` and stop past `(page+1)*7` — its own
    // quirky over-wide bound, kept.
    for (i, (id, name, leader, level, members)) in clans.iter().enumerate() {
        if i as i32 > (page + 1) * 7 {
            break;
        }
        if (i as i32) < (page - 1) * 7 {
            continue;
        }
        html.push_str(&format!(
            "<table border=0 width=610><tr>\
             <td FIXWIDTH=200 align=center><a action=\"bypass _bbsclan_clanhome;{id}\">{name}</a></td>\
             <td FIXWIDTH=200 align=center>{leader}</td>\
             <td FIXWIDTH=100 align=center>{level}</td>\
             <td FIXWIDTH=100 align=center>{members}</td></tr></table>",
        ));
    }
    if page > 1 {
        html.push_str(&format!(
            "<a action=\"bypass _bbsclan_clanlist;{}\">&lt; prev</a> ",
            page - 1
        ));
    }
    if (clans.len() as i32) > page * 7 {
        html.push_str(&format!(
            "<a action=\"bypass _bbsclan_clanlist;{}\">next &gt;</a>",
            page + 1
        ));
    }
    html.push_str("</center></body></html>");
    send_cb_html(world, client_id, &html);
}

/// Java `ClanBoard.clanHome` — the clan info page (level ≥ 2, else back to
/// the list with SM 1050).
fn clan_home(world: &mut World, client_id: u32, object_id: i32, clan_id: i32) {
    let Some((name, level, members, leader, ally)) = world.clans.get(&clan_id).map(|c| {
        (
            c.name.clone(),
            c.level,
            c.members.len(),
            c.leader_name().to_string(),
            c.ally_name.clone(),
        )
    }) else {
        return;
    };
    if level < 2 {
        send_to_client(
            world,
            client_id,
            crate::network::server_packets::system_message_with(
                crate::network::server_packets::sm_ids::NO_CLAN_COMMUNITY_UNDER_LEVEL_2,
                &[],
            ),
        );
        return clan_list(world, client_id, object_id, 1);
    }
    let html = format!(
        "<html><body><br><br><center>\
         <table border=0 width=610><tr><td><a action=\"bypass _bbshome\">HOME</a> &gt; \
         <a action=\"bypass _bbsclan_clanlist\">CLAN COMMUNITY</a></td></tr></table>\
         <table border=0 width=610 bgcolor=434343><tr><td>\
         <a action=\"bypass _bbsclan_clannotice_edit;{clan_id};cnotice\">[CLAN NOTICE]</a></td></tr></table>\
         <table border=0 width=610>\
         <tr><td FIXWIDTH=100>CLAN NAME</td><td FIXWIDTH=195>{name}</td></tr>\
         <tr><td FIXWIDTH=100>CLAN LEVEL</td><td FIXWIDTH=195>{level}</td></tr>\
         <tr><td FIXWIDTH=100>CLAN MEMBERS</td><td FIXWIDTH=195>{members}</td></tr>\
         <tr><td FIXWIDTH=100>CLAN LEADER</td><td FIXWIDTH=195>{leader}</td></tr>\
         <tr><td FIXWIDTH=100>ALLIANCE</td><td FIXWIDTH=195>{ally}</td></tr>\
         </table></center></body></html>",
    );
    send_cb_html(world, client_id, &html);
}

/// Java `ClanBoard.clanNotice` — the leader's edit form (with the on/off
/// toggle and the `Write Notice Set` MultiEdit) or the member's read view.
fn clan_notice_page(world: &mut World, client_id: u32, object_id: i32) {
    let Some(clan_id) = super::guard::clan_of(world, object_id) else {
        return;
    };
    let is_leader = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.leader_id == object_id);
    let (enabled, text) = world
        .clan_notices
        .get(&clan_id)
        .cloned()
        .unwrap_or((false, String::new()));
    let mut html = String::from("<html><body><br><br><center>");
    if is_leader {
        let toggle = if enabled {
            "Clan Notice Function: on / <a action=\"bypass _bbsclan_clannotice_disable\">off</a>"
        } else {
            "Clan Notice Function: <a action=\"bypass _bbsclan_clannotice_enable\">on</a> / off"
        };
        html.push_str(&format!(
            "<table width=610><tr><td>The Clan Notice function allows the clan leader to send \
             messages through a pop-up window to clan members at login.</td></tr>\
             <tr><td>{toggle}</td></tr></table>\
             <table width=610><tr><td>Edit Notice:</td></tr>\
             <tr><td><MultiEdit var=\"Content\" width=610 height=100></td></tr></table>\
             <button value=\"&$140;\" action=\"Write Notice Set _ Content Content Content\" \
             back=\"l2ui_ch3.smallbutton2_down\" width=65 height=20 fore=\"l2ui_ch3.smallbutton2\">",
        ));
    } else {
        html.push_str(
            "<table><tr><td>You are not your clan's leader, and therefore cannot change the clan notice</td></tr></table>",
        );
        if enabled {
            html.push_str(&format!(
                "<table width=610><tr><td>The current clan notice:</td></tr><tr><td>{text}</td></tr></table>",
            ));
        }
    }
    html.push_str("</center></body></html>");
    send_cb_html(world, client_id, &html);
}

/// `//bbs` (AdminBBS) — the GM shortcut onto the board's home page.
pub(crate) fn open_home_for_admin(world: &mut World, client_id: u32, object_id: i32) {
    show_home(world, client_id, object_id, "_bbshome");
}

/// `HomeBoard`'s `_bbshome`/`_bbstop` branch: load the home page (custom or
/// retail) and inject the navigation panel.
fn show_home(world: &mut World, client_id: u32, object_id: i32, command: &str) {
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
    // Java tops up `getPet()` and every `getServitors()` entry alongside the
    // owner — both summonable since G29, so the leg is live now.
    for summon in [
        super::servitor::pet_of(world, object_id),
        super::servitor::servitor_of(world, object_id),
    ]
    .into_iter()
    .flatten()
    {
        super::admin::vitals::heal_creature(world, summon);
    }
    send_message(world, client_id, "You used heal!");
    serve_page(
        world,
        client_id,
        object_id,
        command.strip_prefix("_bbsheal;"),
        "",
    );
}

/// `HomeBoard`'s `_bbsteleport;<x> <y> <z>` branch: charge, hide the board and
/// teleport to the whitelisted destination. Reuses the gatekeeper teleport
/// primitive.
fn do_teleport(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    if !world.cfg.community_board.enable_teleports {
        return;
    }
    let Some(key) = command.strip_prefix("_bbsteleport;") else {
        return;
    };
    let Some(&(x, y, z)) = world
        .cfg
        .community_board
        .available_teleports
        .get(key.trim())
    else {
        warn!("CommunityBoard: teleport [{key}] not in the gatekeeper whitelist.");
        return;
    };
    let price = world.cfg.community_board.teleport_price;
    if !charge(world, client_id, object_id, price) {
        return;
    }
    // Java hides the board (`new ShowBoard()`) and `disableAllSkills()` for
    // 3 s around the teleport; `SkillsDisabled` + the timed re-enable mirror
    // the `enableAllSkills` ThreadPool.schedule.
    send_to_client(world, client_id, sp::show_board_hide());
    world
        .objects
        .add_components(&object_id, crate::model::components::SkillsDisabled);
    world.scheduler.schedule(
        world.tick + 30,
        crate::scheduler::ScheduledTask::SkillsReenable { object_id },
    );
    let dead = is_dead(world, object_id);
    if !dead {
        super::death::teleport_player(world, object_id, x, y, z);
    }
}

/// `HomeBoard`'s `_bbsdelevel` branch — config-off on this dist
/// (`EnableDelevel = False`), ported per the config-disabled rule: pay the
/// currency, drop exactly one level, come back at full HP/MP/CP. Java's
/// refusal order is funds first, then the level-1 floor, and only then the
/// charge; `set_level` carries the `checkPlayerSkills()` re-check.
fn do_delevel(world: &mut World, client_id: u32, object_id: i32) {
    if !world.cfg.community_board.enable_delevel {
        return;
    }
    let price = world.cfg.community_board.delevel_price;
    let currency = world.cfg.community_board.currency_id;
    let have = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map_or(0, |inv| inv.count_of(currency));
    if have < price {
        send_message(world, client_id, "Not enough currency!");
        return;
    }
    let level = world
        .objects
        .get_component::<Player>(&object_id)
        .map_or(0, |p| p.level);
    if level <= 1 {
        send_message(world, client_id, "You are at minimum level!");
        return;
    }
    if !charge(world, client_id, object_id, price) {
        return;
    }
    let new_level = level - 1;
    let exp = world.data.experience.exp_for_level(new_level);
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.exp = exp;
    }
    super::death::set_level(world, object_id, new_level);
    super::admin::vitals::heal_creature(world, object_id);
    if let Some(html) = read_html(
        &world.data.root,
        "data/html/CommunityBoard/Custom/delevel/complete.html",
    ) {
        send_cb_html(world, client_id, &html);
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
    let (page, buffs) = parts
        .split_last()
        .map(|(p, b)| (Some(*p), b))
        .unwrap_or((None, &[]));

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
        let Some(skill) = skill_by_id(world, id, lvl) else {
            warn!("CommunityBoard: buff skill {id}/{lvl} missing from skill data.");
            continue;
        };
        // Java builds one target list — `[player, pet?, servitor…]` — and casts
        // each buff at every member of it, gated on `isSharedWithSummon() ||
        // target.isPlayer()`: a non-shared buff reaches only the player.
        //
        // The servitor is in this list *and* picks the same buff up again
        // through `Skill.applyEffects`' own sharing branch. That double-apply is
        // Java's too, and it refreshes rather than stacks, so the literal target
        // list is kept rather than "optimised" into something that diverges.
        for target in buff_targets(world, object_id) {
            if !skill.shared_with_summon && target != object_id {
                continue;
            }
            crate::game_loop::skills::effects::apply_skill_effects(
                world, object_id, target, &skill,
            );
            // `CommunityCastAnimations`: Java sends this to the **caster only** —
            // its own source carries a commented-out `broadcastPacket` with the
            // note "not recommend broadcast", so onlookers see nothing.
            if world.cfg.community_board.cast_animations {
                cast_animation(world, client_id, object_id, target, &skill);
            }
        }
    }
    serve_page(world, client_id, object_id, page, "");
}

/// Java's `targets` list in `_bbsbuff`: the player, their pet if any, then
/// their servitors. Order matters only for the animation packets.
fn buff_targets(world: &World, object_id: i32) -> Vec<i32> {
    let mut targets = vec![object_id];
    targets.extend(crate::game_loop::servitor::pet_of(world, object_id));
    targets.extend(crate::game_loop::servitor::servitor_of(world, object_id));
    targets
}

/// The `CommunityCastAnimations` `MagicSkillUse`, sent to the buying player
/// only. The caster is the player in every case — including the pet/servitor
/// targets, which Java also credits to the owner rather than to the summon.
fn cast_animation(
    world: &World,
    client_id: u32,
    caster_oid: i32,
    target_oid: i32,
    skill: &crate::model::skill::Skill,
) {
    let Some(caster) = world.objects.get_component::<Player>(&caster_oid) else {
        return;
    };
    let Some(caster_pos) = world
        .objects
        .get_component::<crate::model::components::Position>(&caster_oid)
    else {
        return;
    };
    let Some(target_pos) = world
        .objects
        .get_component::<crate::model::components::Position>(&target_oid)
    else {
        return;
    };
    let pkt = sp::magic_skill_use(
        caster,
        caster_pos,
        (target_oid, target_pos.x, target_pos.y, target_pos.z),
        skill.id,
        skill.level,
        skill.hit_time,
        skill.reuse_delay_group,
        skill.reuse_delay,
    );
    send_to_client(world, client_id, pkt);
}

/// `HomeBoard`'s `_bbspremium;<days>` branch: buy `<days>` (1–30) days of
/// account premium at `premium_price_per_day` each, then serve the thank-you
/// page. Reuses the `PremiumManager` store already ported for `//premium_*`.
fn do_premium(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    use super::admin::premium;
    // `HomeBoard.CUSTOM_COMMANDS` only registers `_bbspremium` when both the
    // global premium system and the community premium option are on.
    if !premium::premium_system_enabled(world)
        || !world.cfg.community_board.community_premium_system
    {
        return;
    }
    // `_bbspremium;<days>` → Java splits the tail on `,` and takes the first field.
    let days: i64 = command
        .strip_prefix("_bbspremium;")
        .and_then(|t| t.split(',').next())
        .and_then(|d| d.trim().parse().ok())
        .unwrap_or(0);
    let price = world
        .cfg
        .community_board
        .premium_price_per_day
        .saturating_mul(days);
    // Java folds the range check into the "Not enough currency!" guard.
    if !(1..=30).contains(&days) {
        send_message(world, client_id, "Not enough currency!");
        return;
    }
    let coin = world.cfg.community_board.premium_coin_id;
    if !charge_item(world, client_id, object_id, coin, price) {
        return;
    }

    let Some(account) = account_of(world, client_id) else {
        return;
    };
    let enddate = premium::add_premium_time(world, &account, days * premium::DAY_MILLIS);
    send_message(
        world,
        client_id,
        &format!(
            "Your account will now have premium status until {}.",
            premium::format_datetime(enddate)
        ),
    );
    // `HomeBoard`: a fresh premium account re-arms the PA-point timer (the
    // `PcCafeOnlyPremium` gate may only now be satisfied).
    super::pc_cafe::run(world, object_id);
    serve_page(
        world,
        client_id,
        object_id,
        Some("premium/thankyou.html"),
        "",
    );
}

/// `HomeBoard`'s `_bbs_buff_scheme_*` branch: create a scheme from the player's
/// active buffs, delete one, or execute (re-cast) one, then re-render the
/// return page with any validation error banner. The bypass carries
/// space-separated args: `<cmd> <name> <returnPath> [self|pet]`.
fn do_scheme(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    // Return page: `parts[2]` when present, else `parts[1]` (Java `parts.length
    // < 3`); only if it names an html.
    let return_path = if parts.len() < 3 {
        parts.get(1)
    } else {
        parts.get(2)
    }
    .copied()
    .filter(|p| p.ends_with(".html"));

    // Java loads the return html first, runs the command (which may set an error
    // message), then re-renders — so we always serve the return page.
    let error = run_scheme_command(world, client_id, object_id, &parts)
        .err()
        .unwrap_or_default();
    serve_page(world, client_id, object_id, return_path, &error);
}

/// Port of `HomeBoard.parseSchemeNameOrError` + the create/delete/execute
/// dispatch. `Err(msg)` becomes the `%errorMessage%` banner.
fn run_scheme_command(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    parts: &[&str],
) -> Result<(), String> {
    if parts.len() < 3 {
        return Err("Please enter scheme name.".to_string());
    }
    let command_name = parts[0];
    let scheme_name = parts[1];
    if scheme_name.chars().count() > 14 {
        return Err("Scheme's name must contain up to 14 chars.".to_string());
    }
    if !is_alphanumeric(scheme_name) {
        return Err("Please use plain alphanumeric characters.".to_string());
    }
    if command_name == "_bbs_buff_scheme_create"
        && let Some(schemes) = world.buffer_schemes.get(&object_id)
    {
        if schemes.len() >= MAX_SCHEMES {
            return Err("Maximum schemes amount is already reached.".to_string());
        }
        if schemes
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case(scheme_name))
        {
            return Err("The scheme name already exists.".to_string());
        }
    }

    match command_name {
        "_bbs_buff_scheme_create" => scheme_create(world, object_id, scheme_name),
        "_bbs_buff_scheme_delete" => {
            scheme_delete(world, object_id, scheme_name);
            Ok(())
        }
        "_bbs_buff_scheme_execute" => {
            let is_pet = parts.get(3) == Some(&"pet");
            apply_scheme(world, client_id, object_id, scheme_name, is_pet)
        }
        _ => Ok(()),
    }
}

/// Java create branch: snapshot the player's currently-active whitelisted buffs
/// into a new scheme, write it through to `buffer_schemes`.
fn scheme_create(world: &mut World, object_id: i32, scheme_name: &str) -> Result<(), String> {
    let buffs: Vec<i32> = world
        .objects
        .get_component::<crate::model::components::Buffs>(&object_id)
        .map(|b| {
            b.0.iter()
                .map(|a| a.skill_id)
                .filter(|id| world.cfg.community_board.available_buffs.contains(id))
                .collect()
        })
        .unwrap_or_default();
    if buffs.is_empty() {
        return Err("You don't have any buffs applied.".to_string());
    }
    let skills = buffs
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    world
        .buffer_schemes
        .entry(object_id)
        .or_default()
        .push((scheme_name.to_string(), buffs));
    let _ = world.db.send(crate::db::DbCommand::StoreBufferScheme {
        object_id,
        scheme_name: scheme_name.to_string(),
        skills,
    });
    Ok(())
}

/// Java `removeScheme` + the shutdown save collapse into an immediate delete.
fn scheme_delete(world: &mut World, object_id: i32, scheme_name: &str) {
    if let Some(schemes) = world.buffer_schemes.get_mut(&object_id) {
        schemes.retain(|(n, _)| !n.eq_ignore_ascii_case(scheme_name));
    }
    let _ = world.db.send(crate::db::DbCommand::DeleteBufferScheme {
        object_id,
        scheme_name: scheme_name.to_string(),
    });
}

/// Port of `HomeBoard.applyBuffs`: re-cast every skill in a scheme onto the
/// player, at the level from the buffer's available-buff table.
pub(crate) fn apply_scheme(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    scheme_name: &str,
    is_pet: bool,
) -> Result<(), String> {
    let scheme: Vec<i32> = world
        .buffer_schemes
        .get(&object_id)
        .and_then(|s| s.iter().find(|(n, _)| n.eq_ignore_ascii_case(scheme_name)))
        .map(|(_, skills)| skills.clone())
        .unwrap_or_default();

    // The "Pet" button buffs the player's summon (Java's `player.getPet()` /
    // `getServitors()`); with no summon it lands on the "no pet" branch. The
    // player still pays, so the cost checks below stay keyed to `object_id`.
    let target = if is_pet {
        match crate::game_loop::servitor::pet_of(world, object_id)
            .or_else(|| crate::game_loop::servitor::servitor_of(world, object_id))
        {
            Some(summon) => summon,
            None => return Err("You don't have a pet.".to_string()),
        }
    } else {
        object_id
    };

    let buff_price = world.cfg.community_board.buff_price;
    let cost = if buff_price > 0 {
        buff_price * scheme.len() as i64
    } else {
        0
    };
    // NOTE: Java's guard is `(cost == 0) || inventoryCount < cost` — an inverted
    // check that applies the scheme for free (dist `BuffPrice = 0`) and, were the
    // price ever positive, would refuse only when the player CAN pay. Ported
    // faithfully ("dist data is the spec"); with the dist price of 0, `cost` is
    // always 0 so the buffs always apply.
    let currency = world.cfg.community_board.currency_id;
    let have = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(|inv| inv.count_of(currency))
        .unwrap_or(0);
    if !(cost == 0 || have < cost) {
        return Err("You don't have enough items for this action.".to_string());
    }
    if cost > 0 {
        // Java `destroyItemByItemId("CB_Buff", CURRENCY, cost, …)` — a no-op in
        // dist; best-effort, never blocks the (already faithful) apply.
        charge(world, client_id, object_id, cost);
    }

    for skill_id in &scheme {
        if !world.cfg.community_board.available_buffs.contains(skill_id) {
            continue;
        }
        let Some(level) = world.data.scheme_buffer.level_of(*skill_id) else {
            continue;
        };
        let Some(skill) = skill_by_id(world, *skill_id, level) else {
            warn!("CommunityBoard: scheme buff {skill_id}/{level} missing from skill data.");
            continue;
        };
        crate::game_loop::skills::effects::apply_skill_effects(world, object_id, target, &skill);
    }
    Ok(())
}

/// Java `Util.isAlphaNumeric` — non-empty and every char a letter or digit.
fn is_alphanumeric(s: &str) -> bool {
    !s.is_empty() && s.chars().all(char::is_alphanumeric)
}

// --- Merchant (multisell / sell) ------------------------------------------

/// `HomeBoard`'s `_bbsmultisell;<id>,<page>` / `_bbsexcmultisell;<id>,<page>`
/// branch: open the multisell window, then re-render the named Custom page (the
/// nav pages pass `_bbstop`, whose file is absent → no re-render, exactly like
/// Java's null `returnHtml`).
fn do_multisell(world: &mut World, client_id: u32, object_id: i32, command: &str, exchange: bool) {
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
    super::multisell::separate_and_send(world, client_id, object_id, None, list_id, exchange);
}

/// `HomeBoard`'s `_bbssell;<page>` branch: open the sell window (BuyList 423 +
/// the sell tab), then re-render the named page. Buylist 423 is absent on this
/// dist (as it is in the Java datapack — the command is unreachable from the
/// shipped htmls), so the window is skipped with a warn rather than NPE'ing.
fn do_sell(world: &mut World, client_id: u32, object_id: i32, command: &str) {
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
            crate::network::trade::buy_list(&list, inv, &world.data, 0.0),
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
fn render_merchant_page(world: &mut World, client_id: u32, object_id: i32, page: Option<&str>) {
    let Some(page) = page.filter(|p| !p.is_empty()) else {
        return;
    };
    let file = if page.ends_with(".html") {
        page.to_string()
    } else {
        format!("{page}.html")
    };
    let rel = format!("data/html/CommunityBoard/Custom/{file}");
    let Some(html) = read_html(&world.data.root, &rel) else {
        return;
    };
    let html = finalize_custom(world, object_id, html, "");
    send_cb_html(world, client_id, &html);
}

// --- FavoriteBoard / HomepageBoard ----------------------------------------

/// Port of `HomepageBoard.parseCommunityBoardCommand` (`_bbslink`): serve the
/// static `homepage.html` verbatim (a plain retail page, no navigation inject).
fn show_homepage(world: &World, client_id: u32) {
    let Some(html) = read_html(&world.data.root, "data/html/CommunityBoard/homepage.html") else {
        warn!("CommunityBoard: missing html [homepage.html].");
        return;
    };
    send_cb_html(world, client_id, &html);
}

/// Port of `FavoriteBoard`'s `_bbsgetfav` branch: render `favorite.html` with
/// one `favorite_list.html` row per stored favorite (newest first, the boot /
/// insert order the mirror keeps). Java re-queries the DB; we render the mirror.
fn show_favorites(world: &World, client_id: u32, object_id: i32) {
    let root = &world.data.root;
    let (Some(page), Some(row_tpl)) = (
        read_html(root, "data/html/CommunityBoard/favorite.html"),
        read_html(root, "data/html/CommunityBoard/favorite_list.html"),
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
fn add_favorite(world: &mut World, client_id: u32, object_id: i32) {
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
fn del_favorite(world: &mut World, client_id: u32, object_id: i32, command: &str) {
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
fn format_fav_date(millis: i64) -> String {
    let (year, month, day, hour, minute, second) = commons::util::civil_from_millis(millis);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

// --- DropSearchBoard --------------------------------------------------------

/// Port of `DropSearchBoard.parseCommunityBoardCommand`. Java splits the raw
/// command on spaces and switches on `params[0]`, so the nav "Search" button
/// (`_bbs_search_item;`, trailing `;`, no space) matches no case and just
/// renders the empty search page.
fn do_drop_search(world: &mut World, client_id: u32, command: &str) {
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
fn render_drop_search(world: &World, client_id: u32, f: impl FnOnce(String) -> String) {
    let root = &world.data.root;
    let Some(html) = read_html(root, "data/html/CommunityBoard/Custom/dropsearch/main.html") else {
        warn!("CommunityBoard: missing dropsearch/main.html.");
        return;
    };
    let navigation =
        read_html(root, "data/html/CommunityBoard/Custom/navigation.html").unwrap_or_default();
    let html = f(html).replace("%navigation%", &navigation);
    send_cb_html(world, client_id, &html);
}

/// Port of `DropSearchBoard.buildItemSearchResult`: the ≤14 lowest-id items
/// whose name contains `item_name` and that appear in the drop index, rendered
/// as a 2-per-row icon grid padded to 7 rows.
fn build_item_search_result(world: &World, item_name: &str) -> String {
    let index = world.data.npc_data.drop_index();
    let needle = item_name.to_lowercase();
    let mut items: Vec<&crate::data::item_data::ItemTemplate> = world
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
fn do_search_drop(world: &World, client_id: u32, params: &[&str]) {
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
fn drop_rate(spoil: f64, raid: f64, death: f64, is_spoil: bool, is_raid: bool) -> f64 {
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
fn fmt_amount(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.2}")
    }
}

/// Port of `DropSearchBoard`'s `_bbs_npc_trace` branch: pick a random live spawn
/// of the NPC and drop a world-map marker on it (Java `Radar.addMarker`), or
/// message the player when none is spawned (bosses / instance mobs).
fn do_npc_trace(world: &mut World, client_id: u32, params: &[&str]) {
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

/// Re-render a Custom sub-page after an action (the `page` tail the action
/// bypasses carry, e.g. `buffer/main` or `buffer/schemes.html`), with an
/// optional error banner. No-op if the tail is missing.
fn serve_page(
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
    let Some(html) = read_html(&world.data.root, &rel) else {
        warn!("CommunityBoard: missing sub-page [{rel}].");
        return;
    };
    let html = finalize_custom(world, object_id, html, error_message);
    send_cb_html(world, client_id, &html);
}

/// Port of `HomeBoard`'s post-build custom substitution: inject the navigation
/// panel, the `%errorMessage%` banner, and the `%schemenames%` scheme buttons.
fn finalize_custom(world: &World, object_id: i32, html: String, error_message: &str) -> String {
    let navigation = read_html(
        &world.data.root,
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

/// The `%schemenames%` block: the player's scheme rows, or Java's empty-state
/// line when the player has no schemes registered (`getPlayerSchemes == null`).
fn render_scheme_names(world: &World, object_id: i32) -> String {
    match world.buffer_schemes.get(&object_id) {
        Some(schemes) => build_scheme_html(schemes),
        None => {
            "No buffer schemes yet, please make sure you have buffs and then click Create Scheme."
                .to_string()
        }
    }
}

/// Java `HomeBoard.buildBufferSchemesHtml`: one execute/pet/delete button row
/// per scheme, names sorted case-insensitively (Java iterates a
/// `TreeMap(CASE_INSENSITIVE_ORDER)`).
fn build_scheme_html(schemes: &[(String, Vec<i32>)]) -> String {
    const ROW: &str = concat!(
        "<td><button value=\"%schemename%\" action=\"bypass _bbs_buff_scheme_execute %schemename% buffer/schemes.html self\" height=\"26\" width=\"130\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\" /></td>",
        "<td><button value=\"%schemename% (Pet)\" action=\"bypass _bbs_buff_scheme_execute %schemename% buffer/schemes.html pet\" height=\"26\" width=\"130\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\" /></td>",
        "<td><button value=\"X\" action=\"bypass _bbs_buff_scheme_delete %schemename% buffer/schemes.html\" height=\"26\" width=\"26\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\" /></td>",
    );
    let mut names: Vec<&str> = schemes.iter().map(|(n, _)| n.as_str()).collect();
    names.sort_by_key(|n| n.to_lowercase());
    let mut out = String::from("<table align=\"center\">");
    for name in names {
        out.push_str("<tr>");
        out.push_str(&ROW.replace("%schemename%", name));
        out.push_str("</tr>");
    }
    out.push_str("</table>");
    out
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
fn in_zone(world: &World, object_id: i32, kind: crate::data::zone_data::ZoneKind) -> bool {
    world
        .objects
        .get_component::<crate::model::components::ZoneFlags>(&object_id)
        .is_some_and(|f| f.contains(kind))
}

fn reputation(world: &World, object_id: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.reputation)
        .unwrap_or(0)
}

/// The account name behind a client (Java `player.getAccountName()`), for the
/// account-scoped premium store.
fn account_of(world: &World, client_id: u32) -> Option<String> {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => Some(s.account().to_string()),
        _ => None,
    }
}

/// `player.destroyItemByItemId(currency, price)` on the board currency with the
/// "not enough" guard.
fn charge(world: &mut World, client_id: u32, object_id: i32, price: i64) -> bool {
    let currency = world.cfg.community_board.currency_id;
    charge_item(world, client_id, object_id, currency, price)
}

/// `player.destroyItemByItemId(item_id, price)` with the "not enough" guard.
/// A zero/negative price is free (no inventory touch). Returns whether the
/// action may proceed.
fn charge_item(
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
    if have < price || !super::quests::take_items(world, client_id, object_id, item_id, price) {
        send_message(world, client_id, "Not enough currency!");
        return false;
    }
    true
}

fn read_html(root: &str, rel: &str) -> Option<String> {
    crate::data::htm_cache::read_htm(format!("{root}{rel}"))
}

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
