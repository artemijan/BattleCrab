use super::add_clan_member;
use super::clan_membership;
use super::send_to_member;
use crate::db::DbCommand;
use crate::game_loop::guard::clan_of_or_zero;
use crate::game_loop::helpers::client_for_player;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::send_sm_to_player as send_sm_with;
use crate::game_loop::helpers::send_to_client;
use crate::model::Player;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::world::World;
use commons::network::PacketReader;
use commons::util::now_millis;

use crate::model::clan::CL_MANAGE_RANKS;

use crate::model::clan_entry::{
    LOCK_TIME_TICKS, PledgeApplicantInfo, PledgeRecruitInfo, PledgeWaitingInfo,
};

fn is_player_recruit_locked(world: &World, player_id: i32) -> bool {
    world
        .recruit_player_lock
        .get(&player_id)
        .is_some_and(|&t| t > world.tick)
}

fn is_clan_recruit_locked(world: &World, clan_id: i32) -> bool {
    world
        .recruit_clan_lock
        .get(&clan_id)
        .is_some_and(|&t| t > world.tick)
}

/// Java `getPlayerLockTime`/`getClanLockTime` — minutes remaining, for the
/// "try again in N minutes" message.
fn player_lock_minutes(world: &World, player_id: i32) -> i64 {
    world
        .recruit_player_lock
        .get(&player_id)
        .map(|&t| (t.saturating_sub(world.tick) / 600) as i64)
        .unwrap_or(0)
}
fn clan_lock_minutes(world: &World, clan_id: i32) -> i64 {
    world
        .recruit_clan_lock
        .get(&clan_id)
        .map(|&t| (t.saturating_sub(world.tick) / 600) as i64)
        .unwrap_or(0)
}

/// `ClanEntryManager.addPlayerApplicationToClan`.
fn add_player_application(world: &mut World, clan_id: i32, info: PledgeApplicantInfo) -> bool {
    if is_player_recruit_locked(world, info.player_id) {
        return false;
    }
    let _ = world.db.send(DbCommand::UpsertPledgeApplicant {
        player_id: info.player_id,
        clan_id,
        karma: info.karma,
        message: info.message.clone(),
    });
    world
        .recruit_applicants
        .entry(clan_id)
        .or_default()
        .insert(info.player_id, info);
    true
}

/// `ClanEntryManager.removePlayerApplication` (no lock — cancelling an
/// application is always allowed, matching Java).
fn remove_player_application(world: &mut World, clan_id: i32, player_id: i32) -> bool {
    let _ = world
        .db
        .send(DbCommand::DeletePledgeApplicant { player_id, clan_id });
    world
        .recruit_applicants
        .get_mut(&clan_id)
        .is_some_and(|m| m.remove(&player_id).is_some())
}

/// `ClanEntryManager.getClanIdForPlayerApplication`.
fn clan_id_for_player_application(world: &World, player_id: i32) -> i32 {
    world
        .recruit_applicants
        .iter()
        .find(|(_, m)| m.contains_key(&player_id))
        .map(|(&clan_id, _)| clan_id)
        .unwrap_or(0)
}

/// `ClanEntryManager.addToWaitingList`.
fn add_to_waiting_list(world: &mut World, info: PledgeWaitingInfo) -> bool {
    if is_player_recruit_locked(world, info.player_id) {
        return false;
    }
    let _ = world.db.send(DbCommand::InsertPledgeWaiting {
        player_id: info.player_id,
        karma: info.karma,
    });
    world.recruit_waiting.insert(info.player_id, info);
    true
}

/// `ClanEntryManager.removeFromWaitingList` — also arms the re-registration
/// lock, unlike removing an application.
fn remove_from_waiting_list(world: &mut World, player_id: i32) -> bool {
    if !world.recruit_waiting.contains_key(&player_id) {
        return false;
    }
    let _ = world.db.send(DbCommand::DeletePledgeWaiting { player_id });
    world.recruit_waiting.remove(&player_id);
    world
        .recruit_player_lock
        .insert(player_id, world.tick + LOCK_TIME_TICKS);
    true
}

/// `ClanEntryManager.addToClanList`.
fn add_to_clan_list(world: &mut World, clan_id: i32, info: PledgeRecruitInfo) -> bool {
    if world.recruit_clans.contains_key(&clan_id) || is_clan_recruit_locked(world, clan_id) {
        return false;
    }
    let _ = world.db.send(DbCommand::InsertPledgeRecruit {
        clan_id,
        karma: info.karma,
        information: info.information.clone(),
        detailed_information: info.detailed_information.clone(),
        application_type: info.application_type,
        recruit_type: info.recruit_type,
    });
    world.recruit_clans.insert(clan_id, info);
    true
}

/// `ClanEntryManager.updateClanList`.
fn update_clan_list(world: &mut World, clan_id: i32, info: PledgeRecruitInfo) -> bool {
    if !world.recruit_clans.contains_key(&clan_id) || is_clan_recruit_locked(world, clan_id) {
        return false;
    }
    let _ = world.db.send(DbCommand::UpdatePledgeRecruit {
        clan_id,
        karma: info.karma,
        information: info.information.clone(),
        detailed_information: info.detailed_information.clone(),
        application_type: info.application_type,
        recruit_type: info.recruit_type,
    });
    world.recruit_clans.insert(clan_id, info);
    true
}

/// `ClanEntryManager.removeFromClanList` — also arms the re-registration lock.
fn remove_from_clan_list(world: &mut World, clan_id: i32) -> bool {
    if !world.recruit_clans.contains_key(&clan_id) {
        return false;
    }
    let _ = world.db.send(DbCommand::DeletePledgeRecruit { clan_id });
    world.recruit_clans.remove(&clan_id);
    world
        .recruit_clan_lock
        .insert(clan_id, world.tick + LOCK_TIME_TICKS);
    true
}

/// `RequestPledgeRecruitBoardAccess` (ex 0xD5): the leader (or a
/// CL_MANAGE_RANKS holder) registers/updates/removes the clan's recruiting
/// listing. `apply_type`: 0 remove, 1 add, 2 update.
pub(crate) fn handle_request_pledge_recruit_board_access(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(ex_body);
    let Some(apply_type) = r.read_i32() else {
        return;
    };
    let Some(karma) = r.read_i32() else { return };
    let Some(information) = r.read_string() else {
        return;
    };
    let Some(detailed_information) = r.read_string() else {
        return;
    };
    let Some(application_type) = r.read_i32() else {
        return;
    };
    let Some(recruit_type) = r.read_i32() else {
        return;
    };

    let Some((clan_id, privs)) = crate::game_loop::guard::clan_and_privs(world, player) else {
        return;
    };
    if clan_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::ONLY_THE_CLAN_LEADER_OR_RANK_MANAGER_MAY_REGISTER_THE_CLAN,
            &[],
        );
        return;
    }
    if !world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.has_privilege(player, privs, CL_MANAGE_RANKS))
    {
        send_sm_with(
            world,
            player,
            sm_ids::ONLY_THE_CLAN_LEADER_OR_RANK_MANAGER_MAY_REGISTER_THE_CLAN,
            &[],
        );
        return;
    }
    let info = PledgeRecruitInfo {
        clan_id,
        karma,
        information,
        detailed_information,
        application_type,
        recruit_type,
    };
    match apply_type {
        0 => {
            remove_from_clan_list(world, clan_id);
        }
        1 => {
            if add_to_clan_list(world, clan_id, info) {
                send_sm_with(
                    world,
                    player,
                    sm_ids::ENTRY_APPLICATION_COMPLETE_AUTO_CANCELLED_AFTER_30_DAYS,
                    &[],
                );
            } else {
                send_sm_with(
                    world,
                    player,
                    sm_ids::YOU_MAY_APPLY_FOR_ENTRY_AFTER_S1_MINUTES_DUE_TO_CANCELLING,
                    &[SmParam::Long(clan_lock_minutes(world, clan_id))],
                );
            }
        }
        2 => {
            if update_clan_list(world, clan_id, info) {
                send_sm_with(
                    world,
                    player,
                    sm_ids::ENTRY_APPLICATION_COMPLETE_AUTO_CANCELLED_AFTER_30_DAYS,
                    &[],
                );
            } else {
                send_sm_with(
                    world,
                    player,
                    sm_ids::YOU_MAY_APPLY_FOR_ENTRY_AFTER_S1_MINUTES_DUE_TO_CANCELLING,
                    &[SmParam::Long(clan_lock_minutes(world, clan_id))],
                );
            }
        }
        _ => {}
    }
}

/// `RequestPledgeRecruitBoardDetail` (ex 0xD6): the full detail pane for one
/// recruiting clan.
pub(crate) fn handle_request_pledge_recruit_board_detail(
    world: &World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(clan_id) = PacketReader::new(ex_body).read_i32() else {
        return;
    };
    let Some(info) = world.recruit_clans.get(&clan_id) else {
        return;
    };
    send_to_client(
        world,
        client_id,
        server_packets::ex_pledge_recruit_board_detail(
            info.clan_id,
            info.karma,
            &info.information,
            &info.detailed_information,
            info.application_type,
            info.recruit_type,
        ),
    );
}

/// `RequestPledgeRecruitBoardSearch` (ex 0xD4): the recruit-board search,
/// with Java's real unsorted/sorted/by-name branches and 12-per-page paging.
pub(crate) fn handle_request_pledge_recruit_board_search(
    world: &World,
    client_id: u32,
    ex_body: &[u8],
) {
    let mut r = PacketReader::new(ex_body);
    let Some(clan_level) = r.read_i32() else {
        return;
    };
    let Some(karma) = r.read_i32() else { return };
    let Some(search_type) = r.read_i32() else {
        return;
    };
    let Some(query) = r.read_string() else { return };
    let Some(sort) = r.read_i32() else { return };
    let Some(descending_raw) = r.read_i32() else {
        return;
    };
    let Some(page) = r.read_i32() else { return };
    let Some(_application_type) = r.read_i32() else {
        return;
    }; // read, unused (Java: "Helios")
    let descending = descending_raw == 2;

    let mut matches: Vec<&PledgeRecruitInfo> = if query.is_empty() {
        if karma < 0 && clan_level < 0 {
            world.recruit_clans.values().collect()
        } else {
            world
                .recruit_clans
                .values()
                .filter(|info| {
                    let level = world.clans.get(&info.clan_id).map(|c| c.level).unwrap_or(0);
                    let level_ok = clan_level < 0 || clan_level == level;
                    let karma_ok = karma < 0 || karma == info.karma;
                    level_ok && karma_ok
                })
                .collect()
        }
    } else {
        let q = query.to_lowercase();
        world
            .recruit_clans
            .values()
            .filter(|info| {
                let Some(c) = world.clans.get(&info.clan_id) else {
                    return false;
                };
                if search_type == 1 {
                    c.name.to_lowercase().contains(&q)
                } else {
                    c.leader_name().to_lowercase().contains(&q)
                }
            })
            .collect()
    };
    if query.is_empty() && !(karma < 0 && clan_level < 0) {
        let sort_by = sort.clamp(1, 4);
        matches.sort_by(|a, b| {
            let ord = match sort_by {
                1 => world
                    .clans
                    .get(&a.clan_id)
                    .map(|c| c.name.clone())
                    .cmp(&world.clans.get(&b.clan_id).map(|c| c.name.clone())),
                2 => world
                    .clans
                    .get(&a.clan_id)
                    .map(|c| c.leader_name().to_string())
                    .cmp(
                        &world
                            .clans
                            .get(&b.clan_id)
                            .map(|c| c.leader_name().to_string()),
                    ),
                3 => world
                    .clans
                    .get(&a.clan_id)
                    .map(|c| c.level)
                    .cmp(&world.clans.get(&b.clan_id).map(|c| c.level)),
                _ => a.karma.cmp(&b.karma),
            };
            if descending { ord.reverse() } else { ord }
        });
    }

    const PER_PAGE: usize = 12;
    let total = matches.len();
    let start = ((page.max(1) as usize) - 1) * PER_PAGE;
    let page_entries: Vec<_> = matches
        .into_iter()
        .skip(start)
        .take(PER_PAGE)
        .filter_map(|info| {
            let c = world.clans.get(&info.clan_id)?;
            Some((
                c.id,
                c.ally_id,
                c.crest_id,
                c.ally_crest_id,
                c.name.clone(),
                c.leader_name().to_string(),
                c.level,
                c.members.len() as i32,
                info.karma,
                info.information.clone(),
                info.application_type,
                info.recruit_type,
            ))
        })
        .collect();
    send_to_client(
        world,
        client_id,
        server_packets::ex_pledge_recruit_board_search(page, total, &page_entries),
    );
}

/// `RequestPledgeWaitingApply` (ex 0xD7): a clanless player applies to a
/// specific clan.
pub(crate) fn handle_request_pledge_waiting_apply(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(ex_body);
    let Some(karma) = r.read_i32() else { return };
    let Some(clan_id) = r.read_i32() else { return };
    let Some(message) = r.read_string() else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    if p.clan_id != 0 || !world.clans.contains_key(&clan_id) {
        return;
    }
    let info = PledgeApplicantInfo {
        player_id: player,
        name: p.name.clone(),
        level: p.level,
        karma,
        clan_id,
        message,
    };
    if add_player_application(world, clan_id, info) {
        send_to_client(
            world,
            client_id,
            server_packets::ex_pledge_recruit_apply_info(4),
        ); // ClanEntryStatus::WAITING
        let leader_id = world.clans.get(&clan_id).map(|c| c.leader_id).unwrap_or(0);
        send_to_member(
            world,
            leader_id,
            server_packets::ex_pledge_waiting_list_alarm(),
        );
    } else {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_MAY_APPLY_FOR_ENTRY_AFTER_S1_MINUTES_DUE_TO_CANCELLING,
            &[SmParam::Long(player_lock_minutes(world, player))],
        );
    }
}

/// `RequestPledgeWaitingApplied` (ex 0xD8): a clanless player checks their
/// own pending application.
pub(crate) fn handle_request_pledge_waiting_applied(world: &World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if clan_of_or_zero(world, player) != 0 {
        return;
    }
    let clan_id = clan_id_for_player_application(world, player);
    if clan_id == 0 {
        return;
    }
    let Some(app) = world
        .recruit_applicants
        .get(&clan_id)
        .and_then(|m| m.get(&player))
    else {
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let recruit = world.recruit_clans.get(&clan_id);
    send_to_client(
        world,
        client_id,
        server_packets::ex_pledge_waiting_list_applied(
            clan.id,
            &clan.name,
            clan.leader_name(),
            clan.level,
            clan.members.len() as i32,
            recruit.map(|r| r.karma).unwrap_or(0),
            recruit.map(|r| r.information.as_str()).unwrap_or(""),
            &app.message,
        ),
    );
}

/// `RequestPledgeWaitingList` (ex 0xD9): the clan's applicant queue.
pub(crate) fn handle_request_pledge_waiting_list(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(clan_id) = PacketReader::new(ex_body).read_i32() else {
        return;
    };
    if clan_of_or_zero(world, player) != clan_id {
        return;
    }
    send_waiting_list(world, client_id, clan_id);
}

fn send_waiting_list(world: &World, client_id: u32, clan_id: i32) {
    let rows: Vec<_> = world
        .recruit_applicants
        .get(&clan_id)
        .map(|m| {
            m.values()
                .map(|a| (a.player_id, a.name.clone(), 0, a.level))
                .collect()
        })
        .unwrap_or_default();
    send_to_client(
        world,
        client_id,
        server_packets::ex_pledge_waiting_list(&rows),
    );
}

/// `RequestPledgeWaitingUser` (ex 0xDA): one applicant's detail, or the whole
/// queue when that player has no application (Java's own fallback).
pub(crate) fn handle_request_pledge_waiting_user(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(ex_body);
    let Some(clan_id) = r.read_i32() else { return };
    let Some(player_id) = r.read_i32() else {
        return;
    };
    if clan_of_or_zero(world, player) != clan_id {
        return;
    }
    match world
        .recruit_applicants
        .get(&clan_id)
        .and_then(|m| m.get(&player_id))
    {
        Some(app) => {
            send_to_client(
                world,
                client_id,
                server_packets::ex_pledge_waiting_user(app.player_id, &app.message),
            );
        }
        None => send_waiting_list(world, client_id, clan_id),
    }
}

/// `RequestPledgeWaitingUserAccept` (ex 0xDB): accept (join the applicant
/// through the shared `add_clan_member` path) or reject an application.
pub(crate) fn handle_request_pledge_waiting_user_accept(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(ex_body);
    let Some(accept) = r.read_i32() else { return };
    let Some(player_id) = r.read_i32() else {
        return;
    };
    let Some(_clan_id_echo) = r.read_i32() else {
        return;
    };
    let Some((clan_id, _, _)) = clan_membership(world, player) else {
        return;
    };
    if accept != 1 {
        remove_player_application(world, clan_id, player_id);
        return;
    }
    if client_for_player(world, player_id).is_none() {
        return;
    }
    let target_ok = world
        .objects
        .get_component::<Player>(&player_id)
        .is_some_and(|t| t.clan_id == 0 && t.clan_join_expiry_time < now_millis());
    if !target_ok {
        let expiry = world
            .objects
            .get_component::<Player>(&player_id)
            .map(|t| t.clan_join_expiry_time)
            .unwrap_or(0);
        if expiry > now_millis() {
            send_sm_with(
                world,
                player,
                sm_ids::C1_CANNOT_JOIN_THE_CLAN_ONE_DAY_HAS_NOT_PASSED_SINCE_LEAVING,
                &[SmParam::Text(player_name_or_empty(world, player_id))],
            );
        }
        return;
    }
    add_clan_member(world, clan_id, player_id, 0);
    remove_player_application(world, clan_id, player_id);
}

/// `RequestPledgeDraftListSearch` (ex 0xDC): the leader's search of clanless
/// waiting players.
pub(crate) fn handle_request_pledge_draft_list_search(
    world: &World,
    client_id: u32,
    ex_body: &[u8],
) {
    let mut r = PacketReader::new(ex_body);
    let Some(level_min) = r.read_i32() else {
        return;
    };
    let Some(level_max) = r.read_i32() else {
        return;
    };
    let Some(_class_id) = r.read_i32() else {
        return;
    }; // read but unused — the role filter is unhandled in Java too
    let Some(query) = r.read_string() else { return };
    let Some(sort) = r.read_i32() else { return };
    let Some(descending_raw) = r.read_i32() else {
        return;
    };
    let descending = descending_raw == 2;

    let mut rows: Vec<&PledgeWaitingInfo> = if query.is_empty() {
        world
            .recruit_waiting
            .values()
            .filter(|p| p.level >= level_min && p.level <= level_max)
            .collect()
    } else {
        let q = query.to_lowercase();
        world
            .recruit_waiting
            .values()
            .filter(|p| p.name.to_lowercase().contains(&q))
            .collect()
    };
    if query.is_empty() {
        let sort_by = sort.clamp(1, 4);
        rows.sort_by(|a, b| {
            let ord = match sort_by {
                1 => a.name.cmp(&b.name),
                2 => a.karma.cmp(&b.karma),
                3 => a.level.cmp(&b.level),
                _ => a.class_id.cmp(&b.class_id),
            };
            if descending { ord.reverse() } else { ord }
        });
    }
    let out: Vec<_> = rows
        .iter()
        .map(|p| (p.player_id, p.name.clone(), p.karma, p.class_id, p.level))
        .collect();
    send_to_client(
        world,
        client_id,
        server_packets::ex_pledge_draft_list_search(&out),
    );
}

/// `RequestPledgeDraftListApply` (ex 0xDD): a clanless player adds/removes
/// themselves from the global waiting list. `apply_type`: 0 remove, 1 add.
pub(crate) fn handle_request_pledge_draft_list_apply(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(ex_body);
    let Some(apply_type) = r.read_i32() else {
        return;
    };
    let Some(karma) = r.read_i32() else { return };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    if p.clan_id != 0 {
        send_sm_with(
            world,
            player,
            sm_ids::ONLY_THE_CLAN_LEADER_OR_RANK_MANAGER_MAY_REGISTER_THE_CLAN,
            &[],
        );
        return;
    }
    match apply_type {
        0 => {
            if remove_from_waiting_list(world, player) {
                send_sm_with(
                    world,
                    player,
                    sm_ids::ENTRY_APPLICATION_CANCELLED_YOU_MAY_APPLY_AFTER_5_MINUTES,
                    &[],
                );
            }
        }
        1 => {
            let info = PledgeWaitingInfo {
                player_id: player,
                level: p.level,
                karma,
                class_id: p.class_id,
                name: p.name.clone(),
            };
            if add_to_waiting_list(world, info) {
                send_sm_with(
                    world,
                    player,
                    sm_ids::ENTERED_INTO_WAITING_LIST_AUTO_DELETED_AFTER_30_DAYS,
                    &[],
                );
            } else {
                send_sm_with(
                    world,
                    player,
                    sm_ids::YOU_MAY_APPLY_FOR_ENTRY_AFTER_S1_MINUTES_DUE_TO_CANCELLING,
                    &[SmParam::Long(player_lock_minutes(world, player))],
                );
            }
        }
        _ => {}
    }
}

/// `RequestPledgeSignInForOpenJoiningMethod` (ex 0x111): instant self-join
/// into a clan whose recruitment listing is `application_type` open (no
/// leader approval needed).
pub(crate) fn handle_request_pledge_sign_in_for_open_joining_method(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(clan_id) = PacketReader::new(ex_body).read_i32() else {
        return;
    };
    let Some(recruit) = world.recruit_clans.get(&clan_id) else {
        return;
    };
    let _ = recruit;
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    if p.clan_id != 0 {
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.char_penalty_expiry_time > now_millis() {
        send_sm_with(
            world,
            player,
            sm_ids::AFTER_A_CLAN_MEMBER_IS_DISMISSED_THE_CLAN_MUST_WAIT_A_DAY,
            &[],
        );
        return;
    }
    if p.clan_join_expiry_time > now_millis() {
        send_sm_with(
            world,
            player,
            sm_ids::C1_CANNOT_JOIN_THE_CLAN_ONE_DAY_HAS_NOT_PASSED_SINCE_LEAVING,
            &[SmParam::Text(p.name.clone())],
        );
        return;
    }
    if clan.sub_pledge_members_count(0) >= clan.max_members_of(0) {
        send_sm_with(
            world,
            player,
            sm_ids::S1_IS_FULL_AND_CANNOT_ACCEPT_ADDITIONAL_CLAN_MEMBERS,
            &[SmParam::Text(clan.name.clone())],
        );
        return;
    }
    add_clan_member(world, clan_id, player, 0);
    remove_player_application(world, clan_id, player);
}
