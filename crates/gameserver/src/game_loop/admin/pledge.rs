//! `AdminPledge` + `AdminClan`'s `//clan_show_pending` — the Game panel's "Clan
//! Related" buttons. Clan create/dismiss/level/reputation drive the G11 clan
//! slice's mutation helpers in [`crate::game_loop::clans`]. The "War Related"
//! (castle / clan-hall / fort-siege), cursed-weapon, manor, grand-boss, olympiad
//! and hero buttons on the same panel stay unimplemented until their subsystems
//! land (G21/G24/G25).

use crate::model::Player;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::world::World;

use super::{current_target, send_message, send_sm};

/// Re-show the Game panel (`game_menu.htm`) — Java `AdminPledge.showMainPage`,
/// run at the end of every branch.
fn show_main_page(world: &World, client_id: u32) {
    super::menu::show_admin_html(world, client_id, "game_menu.htm");
}

/// `AdminPledge.useAdminCommand`. `args` is the whitespace-split tail after
/// `admin_pledge`: `args[0]` is the action (create/info/setlevel/dismiss/rep),
/// `args[1]` its parameter. Operates on the GM's current target (which must be a
/// player) and finishes by re-showing the Game panel.
pub(super) fn admin_pledge(world: &mut World, client_id: u32, gm_object_id: i32, args: &[&str]) {
    // Java: the target must be a player, else INVALID_TARGET + showMainPage.
    let Some(target) = current_target(world, gm_object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        show_main_page(world, client_id);
        return;
    };

    // Java requires BOTH an action and a parameter token for every sub-command —
    // including `info`/`dismiss`, whose Game-panel buttons carry no parameter, so
    // those buttons print "Missing parameters!" on retail too (the parameter is
    // then ignored by `info`). Kept faithful to `AdminPledge`.
    let (Some(action), Some(param)) = (args.first().copied(), args.get(1).copied()) else {
        send_message(world, client_id, "Missing parameters!");
        show_main_page(world, client_id);
        return;
    };

    let clan_id = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.clan_id)
        .filter(|&c| c != 0);
    let target_name = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.name.clone())
        .unwrap_or_default();

    match action {
        "create" => {
            if clan_id.is_some() {
                send_message(world, client_id, "Target player has clan!");
            } else {
                // Java: save the recreate cooldown, zero it so `createClan`'s
                // expiry guard passes, then restore it if creation fails.
                let penalty = world
                    .objects
                    .get_component::<Player>(&target)
                    .map(|p| p.clan_create_expiry_time)
                    .unwrap_or(0);
                if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
                    p.clan_create_expiry_time = 0;
                }
                match crate::game_loop::clans::create_clan(world, target, param) {
                    Some(_) => send_message(
                        world,
                        client_id,
                        &format!("Clan {param} created. Leader: {target_name}"),
                    ),
                    None => {
                        if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
                            p.clan_create_expiry_time = penalty;
                        }
                        send_message(
                            world,
                            client_id,
                            "There was a problem while creating the clan.",
                        );
                    }
                }
            }
        }
        "dismiss" => {
            let Some(cid) = clan_id else {
                send_message(world, client_id, "Target player has no clan!");
                show_main_page(world, client_id);
                return;
            };
            if !world.clans.get(&cid).is_some_and(|c| c.leader_id == target) {
                // Java: "$s1 is not a clan leader." then showMainPage + return.
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::system_message_with(
                        sm_ids::S1_IS_NOT_A_CLAN_LEADER,
                        &[SmParam::Text(target_name.clone())],
                    ));
                }
                show_main_page(world, client_id);
                return;
            }
            crate::game_loop::clans::destroy_clan(world, cid);
            // Java re-reads targetPlayer.getClan() after destroyClan.
            let still_in_clan = world
                .objects
                .get_component::<Player>(&target)
                .is_some_and(|p| p.clan_id != 0);
            if still_in_clan {
                send_message(
                    world,
                    client_id,
                    "There was a problem while destroying the clan.",
                );
            } else {
                send_message(world, client_id, "Clan disbanded.");
            }
        }
        "info" => {
            let Some(cid) = clan_id else {
                send_message(world, client_id, "Target player has no clan!");
                show_main_page(world, client_id);
                return;
            };
            if let Some(clan) = world.clans.get(&cid) {
                let pkt = server_packets::gm_view_pledge_info(clan, &target_name, &world.objects);
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(pkt);
                }
            }
        }
        "setlevel" => {
            let Some(cid) = clan_id else {
                send_message(world, client_id, "Target player has no clan!");
                show_main_page(world, client_id);
                return;
            };
            // Java calls Integer.parseInt(param) unguarded here (unlike `rep`'s
            // try/catch): a non-numeric param throws a NumberFormatException that
            // the AdminCommandHandler dispatcher catches — so there is no "Level
            // incorrect." message and no showMainPage. Emulate that abort.
            let Ok(level) = param.parse::<i32>() else {
                send_message(
                    world,
                    client_id,
                    &format!(
                        "Exception during execution of 'admin_pledge {}': invalid number.",
                        args.join(" ")
                    ),
                );
                return;
            };
            // Java: valid range is [0, 12).
            if (0..12).contains(&level) {
                crate::game_loop::clans::set_clan_level(world, cid, level);
                let name = world
                    .clans
                    .get(&cid)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();
                send_message(
                    world,
                    client_id,
                    &format!("You set level {level} for clan {name}"),
                );
            } else {
                send_message(world, client_id, "Level incorrect.");
            }
        }
        "rep" => {
            let Some(cid) = clan_id else {
                send_message(world, client_id, "Target player has no clan!");
                show_main_page(world, client_id);
                return;
            };
            if world.clans.get(&cid).is_some_and(|c| c.level < 5) {
                send_message(
                    world,
                    client_id,
                    "Only clans of level 5 or above may receive reputation points.",
                );
                show_main_page(world, client_id);
                return;
            }
            match param.parse::<i32>() {
                Ok(points) => {
                    if let Some(score) =
                        crate::game_loop::clans::add_clan_reputation(world, cid, points)
                    {
                        let name = world
                            .clans
                            .get(&cid)
                            .map(|c| c.name.clone())
                            .unwrap_or_default();
                        let (verb, dir) = if points > 0 {
                            ("add", "to")
                        } else {
                            ("remove", "from")
                        };
                        send_message(
                            world,
                            client_id,
                            &format!(
                                "You {verb} {} points {dir} {name}'s reputation. Their current score is {score}",
                                points.abs()
                            ),
                        );
                    }
                }
                Err(_) => send_message(world, client_id, "Usage: //pledge <rep> <number>"),
            }
        }
        // Unknown action: Java's inner switch has no default, so it silently
        // falls through to showMainPage.
        _ => {}
    }
    show_main_page(world, client_id);
}

/// `AdminClan`'s `//clan_show_pending` — the "Pen. leaders" button. Lists clans
/// with a pending leadership transfer. The port has no delayed leader-change
/// mechanism (`setNewLeader`/`new_leader_id` unported), so no clan is ever
/// pending and the list renders empty — the panel itself still shows correctly.
pub(super) fn admin_clan_show_pending(world: &mut World, client_id: u32) {
    super::menu::show_admin_html_replace(
        world,
        client_id,
        "clanchanges.htm",
        &[("data", String::new())],
    );
}
