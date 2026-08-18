//! The retail clan and region boards: clan list, clan home and the clan
//! notice page.

use super::*;
/// `RegionBoard`: `_bbsloc` renders the nine regions off the castles — name
/// fstring, owning clan + alliance, buy-tax. The per-region detail
/// (`_bbsloc;id`) is left unimplemented in Java itself, so a valid id gets
/// Java's silent nothing (an invalid one, Java's warn).
pub(super) fn show_region_board(world: &mut World, client_id: u32, object_id: i32, command: &str) {
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
    let Some(row_tpl) = read_html(
        world,
        client_id,
        "data/html/CommunityBoard/region_list.html",
    ) else {
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
    let Some(html) = read_html(world, client_id, "data/html/CommunityBoard/region.html") else {
        return;
    };
    send_cb_html(world, client_id, &html.replace("%region_list%", &rows));
}

/// `ClanBoard`: the clan list (7 per page), the clan home page, and the
/// notice edit/enable/disable flow.
pub(super) fn show_clan_board(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    let my_clan = crate::game_loop::guard::clan_of(world, object_id);
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
pub(super) fn clan_list(world: &mut World, client_id: u32, _object_id: i32, page: i32) {
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
pub(super) fn clan_home(world: &mut World, client_id: u32, object_id: i32, clan_id: i32) {
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
pub(super) fn clan_notice_page(world: &mut World, client_id: u32, object_id: i32) {
    let Some(clan_id) = crate::game_loop::guard::clan_of(world, object_id) else {
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
