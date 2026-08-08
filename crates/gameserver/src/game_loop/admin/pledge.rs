//! `AdminPledge` + `AdminClan`'s `//clan_show_pending` — the Game panel's "Clan
//! Related" buttons. Clan create/dismiss/level/reputation drive the G11 clan
//! slice's mutation helpers in [`crate::game_loop::clans`]. The "War Related"
//! (castle / clan-hall / fort-siege), cursed-weapon, manor, grand-boss, olympiad
//! and hero buttons on the same panel stay unimplemented until their subsystems
//! land (G21/G24/G25).

use crate::game_loop::clans::clan_name_or_empty;
use crate::game_loop::guard::{self, Guard, OrReject, Reject};
use crate::game_loop::helpers;
use crate::model::Player;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::world::World;

use super::send_message;

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
    let result = pledge_action(world, client_id, gm_object_id, args);
    // Java's `showMainPage` tail runs after every branch — including the failed
    // ones — but NOT when the handler threw, because the throw unwinds past it
    // into `AdminCommandHandler`. That is the whole reason `Reject::Abort`
    // exists, and the reason this tail lives here rather than in `guard`.
    let reached_tail = !matches!(result, Err(Reject::Abort(_)));
    guard::finish(world, client_id, result);
    if reached_tail {
        show_main_page(world, client_id);
    }
}

/// The body of [`admin_pledge`]; every precondition bails through `?`.
fn pledge_action(world: &mut World, client_id: u32, gm_object_id: i32, args: &[&str]) -> Guard<()> {
    // Java: the target must be a player, else INVALID_TARGET + showMainPage.
    let target = guard::player_target(world, gm_object_id).or_sm(sm_ids::INVALID_TARGET)?;

    // Java requires BOTH an action and a parameter token for every sub-command —
    // including `info`/`dismiss`, whose Game-panel buttons carry no parameter, so
    // those buttons print "Missing parameters!" on retail too (the parameter is
    // then ignored by `info`). Kept faithful to `AdminPledge`.
    let (Some(action), Some(param)) = (args.first().copied(), args.get(1).copied()) else {
        return Err(Reject::Msg("Missing parameters!".to_string()));
    };

    let clan_id = guard::clan_of(world, target);
    let target_name = helpers::player_name_or_empty(world, target);

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
            let cid = clan_id.or_msg("Target player has no clan!")?;
            // Java: "$s1 is not a clan leader." then showMainPage + return.
            world
                .clans
                .get(&cid)
                .is_some_and(|c| c.leader_id == target)
                .then_some(())
                .or_sm_with(
                    sm_ids::S1_IS_NOT_A_CLAN_LEADER,
                    vec![SmParam::Text(target_name.clone())],
                )?;
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
            let cid = clan_id.or_msg("Target player has no clan!")?;
            if let Some(clan) = world.clans.get(&cid) {
                let pkt = server_packets::gm_view_pledge_info(clan, &target_name, &world.objects);
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(pkt);
                }
            }
        }
        "setlevel" => {
            let cid = clan_id.or_msg("Target player has no clan!")?;
            // Java calls Integer.parseInt(param) unguarded here (unlike `rep`'s
            // try/catch): a non-numeric param throws a NumberFormatException that
            // the AdminCommandHandler dispatcher catches — so there is no "Level
            // incorrect." message and no showMainPage. Emulate that abort.
            let level = param.parse::<i32>().ok().or_abort(format!(
                "Exception during execution of 'admin_pledge {}': invalid number.",
                args.join(" ")
            ))?;
            // Java: valid range is [0, 12).
            if (0..12).contains(&level) {
                crate::game_loop::clans::set_clan_level(world, cid, level);
                let name = clan_name_or_empty(world, cid);
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
            let cid = clan_id.or_msg("Target player has no clan!")?;
            if world.clans.get(&cid).is_some_and(|c| c.level < 5) {
                return Err(Reject::Msg(
                    "Only clans of level 5 or above may receive reputation points.".to_string(),
                ));
            }
            match param.parse::<i32>() {
                Ok(points) => {
                    if let Some(score) =
                        crate::game_loop::clans::add_clan_reputation(world, cid, points)
                    {
                        let name = clan_name_or_empty(world, cid);
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
    Ok(())
}

/// `AdminClan`'s `//clan_show_pending` — the "Pen. leaders" button: one row per
/// clan carrying a `new_leader_id` (stamped by the village master's delegated
/// transfer, applied at the Wednesday reset), with a Force link that runs the
/// transfer now.
pub(super) fn admin_clan_show_pending(world: &mut World, client_id: u32) {
    let mut clans: Vec<&crate::model::clan::Clan> = world
        .clans
        .values()
        .filter(|c| c.new_leader_id != 0)
        .collect();
    // Java iterates `ClanTable.getClans()`; sort by id so the panel is stable.
    clans.sort_by_key(|c| c.id);
    let mut data = String::new();
    for clan in clans {
        // Java `getNewLeaderName()` reads the member roster, so a nominee who
        // has left the clan renders blank — the same row Java would draw.
        let new_leader = clan
            .members
            .iter()
            .find(|m| m.char_id == clan.new_leader_id)
            .map(|m| m.name.as_str())
            .unwrap_or("");
        data.push_str(&format!(
            "<tr><td>{}</td><td>{new_leader}</td><td><a action=\"bypass -h admin_clan_force_pending {}\">Force</a></td></tr>",
            clan.name, clan.id
        ));
    }
    super::menu::show_admin_html_replace(world, client_id, "clanchanges.htm", &[("data", data)]);
}

/// `AdminClan`'s `//clan_force_pending <clanId>` — run a pending transfer at
/// once instead of waiting for Wednesday. Java bails silently on a non-numeric
/// argument, an unknown clan, or a nominee who is no longer a member.
pub(super) fn admin_clan_force_pending(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(clan_id) = args.first().and_then(|a| a.parse::<i32>().ok()) else {
        return;
    };
    let Some(new_leader) = world
        .clans
        .get(&clan_id)
        .filter(|c| c.new_leader_id != 0)
        .filter(|c| c.members.iter().any(|m| m.char_id == c.new_leader_id))
        .map(|c| c.new_leader_id)
    else {
        return;
    };
    super::super::clans::force_new_leader(world, clan_id, new_leader);
    send_message(world, client_id, "Task have been forcely executed.");
}

// ---------------------------------------------------------------------------
// Category-4 sweep: `AdminClan` leader override + `AdminSkill` clan skill
// ---------------------------------------------------------------------------

/// `//clan_changeleader [name]` — make the (named or targeted) player their
/// clan's leader immediately (Java `Clan.setNewLeader` via `AdminClan`).
pub(super) fn admin_clan_changeleader(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let result = clan_changeleader(world, client_id, object_id, args);
    guard::finish(world, client_id, result);
}

fn clan_changeleader(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) -> Guard<()> {
    let target = args
        .first()
        .and_then(|name| super::find_online_player(world, name))
        .or_else(|| guard::player_target(world, object_id))
        .or_sm(sm_ids::INVALID_TARGET)?;
    let clan_id = guard::clan_of(world, target).or_sm(sm_ids::THE_TARGET_MUST_BE_A_CLAN_MEMBER)?;
    if crate::game_loop::clans::force_new_leader(world, clan_id, target) {
        send_message(world, client_id, "Clan leader changed.");
    } else {
        send_message(world, client_id, "That player already leads the clan.");
    }
    Ok(())
}

/// `//add_clan_skill <id> <level>` — grant one pledge skill to the targeted
/// leader's clan (Java `AdminSkill.adminAddClanSkill`; the target must be the
/// clan leader).
pub(super) fn admin_add_clan_skill(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let result = add_clan_skill(world, client_id, object_id, args);
    guard::finish(world, client_id, result);
}

fn add_clan_skill(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) -> Guard<()> {
    let (Some(skill_id), Some(level)) = (
        args.first().and_then(|a| a.parse::<i32>().ok()),
        args.get(1).and_then(|a| a.parse::<i32>().ok()),
    ) else {
        return Err(Reject::Msg(
            "Usage: //add_clan_skill <skillId> <level>".to_string(),
        ));
    };
    world
        .data
        .skill_data
        .get(skill_id, level)
        .or_msg("No such skill/level.")?;
    let target = guard::player_target(world, object_id).or_sm(sm_ids::INVALID_TARGET)?;
    let clan_id = guard::clan_of(world, target).or_sm(sm_ids::THE_TARGET_MUST_BE_A_CLAN_MEMBER)?;
    // Java `AdminSkill` sends the same message for "no clan" and "not the
    // leader", so both spellings of the check keep that one id.
    world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.leader_id == target)
        .then_some(())
        .or_sm(sm_ids::THE_TARGET_MUST_BE_A_CLAN_MEMBER)?;
    crate::game_loop::clans::admin_add_clan_skill(world, clan_id, skill_id, level);
    send_message(world, client_id, "Clan skill added.");
    Ok(())
}
