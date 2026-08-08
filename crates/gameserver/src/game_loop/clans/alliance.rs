use super::*;

use crate::model::clan::{
    ALLY_PENALTY_TYPE_CLAN_DISMISSED, ALLY_PENALTY_TYPE_CLAN_LEAVED,
    ALLY_PENALTY_TYPE_DISMISS_CLAN, ALLY_PENALTY_TYPE_DISSOLVE_ALLY,
};

/// `AltMaxNumOfClansInAlly = 3` on this dist.
const MAX_CLANS_IN_ALLY: usize = 3;

/// The ally penalties all run `DaysBefore… = 1` day on this dist.
const ALLY_PENALTY_MS: i64 = 86_400_000;

/// Persist a clan's ally membership + penalty stamps (the ally half of
/// `Clan.updateClanInDB`).
fn store_clan_ally(world: &World, clan_id: i32) {
    let Some(c) = world.clans.get(&clan_id) else {
        return;
    };
    let _ = world.db.send(DbCommand::UpdateClanAlly {
        clan_id,
        ally_id: c.ally_id,
        ally_name: c.ally_name.clone(),
        penalty_expiry: c.ally_penalty_expiry_time,
        penalty_type: c.ally_penalty_type,
    });
}

/// Sync every online member's denormalized `Player.ally_id` with the clan and
/// re-broadcast their UserInfo/CharInfo (the ally id rides both).
fn refresh_ally_on_members(world: &mut World, clan_id: i32) {
    let (ally_id, ally_crest_id) = world
        .clans
        .get(&clan_id)
        .map(|c| (c.ally_id, c.ally_crest_id))
        .unwrap_or((0, 0));
    for oid in online_members(world, clan_id) {
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.ally_id = ally_id;
            p.ally_crest_id = ally_crest_id;
        }
        crate::game_loop::party::broadcast_user_info(world, oid);
    }
}

/// The clans of one alliance (Java `ClanTable.getClanAllies`).
pub(crate) fn ally_clan_ids(world: &World, ally_id: i32) -> Vec<i32> {
    if ally_id == 0 {
        return Vec::new();
    }
    world
        .clans
        .values()
        .filter(|c| c.ally_id == ally_id)
        .map(|c| c.id)
        .collect()
}

/// `Clan.broadcastToOnlineAllyMembers`.
fn broadcast_to_ally(world: &World, ally_id: i32, pkt: &[u8]) {
    for clan_id in ally_clan_ids(world, ally_id) {
        broadcast_to_clan(world, clan_id, pkt);
    }
}

/// `VillageMaster.onBypassFeedback`'s `create_ally` branch → `Clan.createAlly`:
/// the guard chain, then the clan becomes its own alliance's leader.
pub(crate) fn handle_create_ally(world: &mut World, client_id: u32, player_oid: i32, args: &str) {
    let name = args.trim();
    let Some(clan_id) = clan_leader_of(world, player_oid) else {
        send_sm(
            world,
            client_id,
            sm_ids::ONLY_CLAN_LEADERS_MAY_CREATE_ALLIANCES,
        );
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.ally_id != 0 {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_ALREADY_BELONG_TO_ANOTHER_ALLIANCE,
        );
        return;
    }
    if clan.level < 5 {
        send_sm(
            world,
            client_id,
            sm_ids::TO_CREATE_AN_ALLIANCE_YOUR_CLAN_MUST_BE_LEVEL_5_OR_HIGHER,
        );
        return;
    }
    if clan.ally_penalty_expiry_time > now_millis()
        && clan.ally_penalty_type == ALLY_PENALTY_TYPE_DISSOLVE_ALLY
    {
        send_sm(
            world,
            client_id,
            sm_ids::CANNOT_CREATE_A_NEW_ALLIANCE_WITHIN_1_DAY_OF_DISSOLUTION,
        );
        return;
    }
    if clan.dissolving_expiry_time > now_millis() {
        send_sm(
            world,
            client_id,
            sm_ids::SCHEDULED_FOR_CLAN_DISSOLUTION_NO_ALLIANCE_CAN_BE_CREATED,
        );
        return;
    }
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
        send_sm(world, client_id, sm_ids::INCORRECT_ALLIANCE_NAME);
        return;
    }
    if name.len() > 16 || name.len() < 2 {
        send_sm(
            world,
            client_id,
            sm_ids::INCORRECT_LENGTH_FOR_AN_ALLIANCE_NAME,
        );
        return;
    }
    if world
        .clans
        .values()
        .any(|c| c.ally_name.eq_ignore_ascii_case(name))
    {
        send_sm(world, client_id, sm_ids::THAT_ALLIANCE_NAME_ALREADY_EXISTS);
        return;
    }

    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.ally_id = clan_id;
        c.ally_name = name.to_string();
        c.ally_penalty_expiry_time = 0;
        c.ally_penalty_type = 0;
    }
    store_clan_ally(world, clan_id);
    refresh_ally_on_members(world, clan_id);
    // Java notes it does not know the right message id here and sends a plain
    // text line instead; ported as written.
    send_sm_with(
        world,
        player_oid,
        sm_ids::S1_TEXT,
        &[SmParam::Text(format!("Alliance {name} has been created."))],
    );
}

/// `Clan.dissolveAlly` (the `dissolve_ally` bypass and `RequestDismissAlly`).
pub(crate) fn handle_dissolve_ally(world: &mut World, client_id: u32, player_oid: i32) {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    let is_leader = p.clan_leader;
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let ally_id = clan.ally_id;
    if ally_id == 0 {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_ARE_NOT_CURRENTLY_ALLIED_WITH_ANY_CLANS,
        );
        return;
    }
    if !is_leader || clan_id != ally_id {
        send_sm(
            world,
            client_id,
            sm_ids::THIS_FEATURE_IS_ONLY_AVAILABLE_TO_ALLIANCE_LEADERS,
        );
        return;
    }
    if in_siege_zone(world, player_oid) {
        send_sm(
            world,
            client_id,
            sm_ids::CANNOT_DISSOLVE_ALLIANCE_WHILE_AFFILIATED_CLAN_IN_SIEGE,
        );
        return;
    }

    let dissolved =
        crate::network::enter_world::system_message(sm_ids::THE_ALLIANCE_HAS_BEEN_DISSOLVED);
    broadcast_to_ally(world, ally_id, &dissolved);

    for cid in ally_clan_ids(world, ally_id) {
        if cid == clan_id {
            continue;
        }
        if let Some(c) = world.clans.get_mut(&cid) {
            c.ally_id = 0;
            c.ally_name.clear();
            c.ally_penalty_expiry_time = 0;
            c.ally_penalty_type = 0;
        }
        store_clan_ally(world, cid);
        refresh_ally_on_members(world, cid);
    }
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.ally_id = 0;
        c.ally_name.clear();
        c.ally_crest_id = 0; // `changeAllyCrest(0, false)`
        c.ally_penalty_expiry_time = now_millis() + ALLY_PENALTY_MS;
        c.ally_penalty_type = ALLY_PENALTY_TYPE_DISSOLVE_ALLY;
    }
    store_clan_ally(world, clan_id);
    refresh_ally_on_members(world, clan_id);
}

/// Java `Clan.checkAllyJoinCondition` — the invite guard chain (each reject's
/// message goes to the inviting alliance leader).
fn check_ally_join_condition(world: &World, requestor_oid: i32, target_oid: i32) -> bool {
    let Some(rp) = world.objects.get_component::<Player>(&requestor_oid) else {
        return false;
    };
    let leader_clan_id = rp.clan_id;
    let Some(leader_clan) = world.clans.get(&leader_clan_id) else {
        return false;
    };
    if leader_clan.ally_id == 0 || !rp.clan_leader || leader_clan_id != leader_clan.ally_id {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::THIS_FEATURE_IS_ONLY_AVAILABLE_TO_ALLIANCE_LEADERS,
            &[],
        );
        return false;
    }
    let now = now_millis();
    if leader_clan.ally_penalty_expiry_time > now
        && leader_clan.ally_penalty_type == ALLY_PENALTY_TYPE_DISMISS_CLAN
    {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::MAY_NOT_ACCEPT_ANY_CLAN_WITHIN_A_DAY_AFTER_EXPELLING,
            &[],
        );
        return false;
    }
    let Some(tp) = world.objects.get_component::<Player>(&target_oid) else {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::YOU_HAVE_INVITED_THE_WRONG_TARGET,
            &[],
        );
        return false;
    };
    if requestor_oid == target_oid {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::YOU_CANNOT_ASK_YOURSELF_TO_APPLY_TO_A_CLAN,
            &[],
        );
        return false;
    }
    if tp.clan_id == 0 {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::THE_TARGET_MUST_BE_A_CLAN_MEMBER,
            &[],
        );
        return false;
    }
    if !tp.clan_leader {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::S1_IS_NOT_A_CLAN_LEADER,
            &[SmParam::Text(tp.name.clone())],
        );
        return false;
    }
    let Some(target_clan) = world.clans.get(&tp.clan_id) else {
        return false;
    };
    if target_clan.ally_id != 0 {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::S1_CLAN_IS_ALREADY_A_MEMBER_OF_S2_ALLIANCE,
            &[
                SmParam::Text(target_clan.name.clone()),
                SmParam::Text(target_clan.ally_name.clone()),
            ],
        );
        return false;
    }
    if target_clan.ally_penalty_expiry_time > now {
        if target_clan.ally_penalty_type == ALLY_PENALTY_TYPE_CLAN_LEAVED {
            send_sm_with(
                world,
                requestor_oid,
                sm_ids::S1_CLAN_CANNOT_JOIN_ALLIANCE_ONE_DAY_NOT_PASSED,
                &[
                    SmParam::Text(target_clan.name.clone()),
                    SmParam::Text(target_clan.ally_name.clone()),
                ],
            );
            return false;
        }
        if target_clan.ally_penalty_type == ALLY_PENALTY_TYPE_CLAN_DISMISSED {
            send_sm_with(
                world,
                requestor_oid,
                sm_ids::WITHDRAWN_OR_EXPELLED_CLAN_CANNOT_ENTER_ALLIANCE_FOR_A_DAY,
                &[],
            );
            return false;
        }
    }
    // Both standing in a siege zone.
    let both_in_siege = [requestor_oid, target_oid].iter().all(|&oid| {
        world
            .objects
            .get_component::<crate::model::components::Position>(&oid)
            .is_some_and(|pos| {
                world
                    .data
                    .zone_data
                    .siege_castle_at(pos.x, pos.y, pos.z)
                    .is_some()
            })
    });
    if both_in_siege {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::THE_OPPOSING_CLAN_IS_PARTICIPATING_IN_A_SIEGE_BATTLE,
            &[],
        );
        return false;
    }
    if at_war_between(world, leader_clan_id, tp.clan_id) {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::YOU_MAY_NOT_ALLY_WITH_A_CLAN_YOU_ARE_AT_WAR_WITH,
            &[],
        );
        return false;
    }
    if ally_clan_ids(world, leader_clan.ally_id).len() >= MAX_CLANS_IN_ALLY {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::YOU_HAVE_EXCEEDED_THE_LIMIT,
            &[],
        );
        return false;
    }
    true
}

/// `RequestJoinAlly` (0x8C): the alliance leader invites another clan's leader.
pub(crate) fn handle_request_join_ally(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(target_oid) = PacketReader::new(body).read_i32() else {
        return;
    };
    if client_for_player(world, target_oid).is_none() {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_HAVE_INVITED_THE_WRONG_TARGET,
            &[],
        );
        return;
    }
    let clan_id = world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    if clan_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION,
            &[],
        );
        return;
    }
    if !check_ally_join_condition(world, player, target_oid) {
        return;
    }
    if refuse_if_busy(world, player, target_oid) {
        return;
    }
    let ally_id = world.clans.get(&clan_id).map(|c| c.ally_id).unwrap_or(0);
    let ally_name = world
        .clans
        .get(&clan_id)
        .map(|c| c.ally_name.clone())
        .unwrap_or_default();
    crate::game_loop::party::install_request(
        world,
        player,
        target_oid,
        crate::model::components::RequestKind::AllyInvite { ally_id },
        crate::game_loop::party::REQUEST_TIMEOUT_TICKS,
    );
    if let Some(cs) = client_for_player(world, target_oid).and_then(|cid| world.clients.get(&cid)) {
        cs.send(server_packets::system_message_with(
            sm_ids::S1_LEADER_S2_HAS_REQUESTED_AN_ALLIANCE,
            &[
                SmParam::Text(ally_name),
                SmParam::Text(player_name_or_empty(world, player)),
            ],
        ));
        cs.send(server_packets::ask_join_ally(
            player,
            &player_name_or_empty(world, player),
        ));
    }
}

/// `RequestAnswerJoinAlly` (0x8D): the invited clan leader answered.
pub(crate) fn handle_request_answer_join_ally(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let answer = PacketReader::new(body).read_i32().unwrap_or(0);
    let Some(req) = world
        .objects
        .get_component::<crate::model::components::PendingRequest>(&player)
        .copied()
    else {
        return;
    };
    let crate::model::components::RequestKind::AllyInvite { ally_id } = req.kind else {
        return;
    };
    if !req.answerer {
        return;
    }
    crate::game_loop::party::clear_linked_request(world, player);
    let requestor = req.other;

    if answer == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::NO_RESPONSE_YOUR_ENTRANCE_TO_THE_ALLIANCE_HAS_BEEN_CANCELLED,
            &[],
        );
        send_sm_with(
            world,
            requestor,
            sm_ids::NO_RESPONSE_INVITATION_TO_JOIN_AN_ALLIANCE_HAS_BEEN_CANCELLED,
            &[],
        );
        return;
    }
    // Re-check (the requestor must still lead the same alliance).
    if world
        .objects
        .get_component::<Player>(&requestor)
        .map(|p| p.clan_id)
        != Some(ally_id)
    {
        return;
    }
    if !check_ally_join_condition(world, requestor, player) {
        return;
    }
    let ally_name = world
        .clans
        .get(&ally_id)
        .map(|c| c.ally_name.clone())
        .unwrap_or_default();
    let target_clan_id = world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    let leader_crest = world
        .clans
        .get(&ally_id)
        .map(|c| c.ally_crest_id)
        .unwrap_or(0);
    if let Some(c) = world.clans.get_mut(&target_clan_id) {
        c.ally_id = ally_id;
        c.ally_name = ally_name;
        c.ally_penalty_expiry_time = 0;
        c.ally_penalty_type = 0;
        c.ally_crest_id = leader_crest; // `changeAllyCrest(leaderCrest, true)`
    }
    store_clan_ally(world, target_clan_id);
    refresh_ally_on_members(world, target_clan_id);
    // Java sends the (wrong) friend-added message to the requestor — kept.
    send_sm_with(
        world,
        requestor,
        sm_ids::SUCCESSFULLY_ADDED_TO_YOUR_FRIEND_LIST,
        &[],
    );
    send_sm_with(world, player, sm_ids::YOU_HAVE_ACCEPTED_THE_ALLIANCE, &[]);
}

/// `AllyLeave` (0x8E): a member clan's leader withdraws their clan.
pub(crate) fn handle_ally_leave(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION,
            &[],
        );
        return;
    }
    if !p.clan_leader {
        send_sm_with(
            world,
            player,
            sm_ids::ONLY_THE_CLAN_LEADER_MAY_APPLY_FOR_WITHDRAWAL_FROM_THE_ALLIANCE,
            &[],
        );
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.ally_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_CURRENTLY_ALLIED_WITH_ANY_CLANS,
            &[],
        );
        return;
    }
    if clan.id == clan.ally_id {
        send_sm_with(world, player, sm_ids::ALLIANCE_LEADERS_CANNOT_WITHDRAW, &[]);
        return;
    }
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.ally_id = 0;
        c.ally_name.clear();
        c.ally_crest_id = 0; // `changeAllyCrest(0, true)`
        c.ally_penalty_expiry_time = now_millis() + ALLY_PENALTY_MS;
        c.ally_penalty_type = ALLY_PENALTY_TYPE_CLAN_LEAVED;
    }
    store_clan_ally(world, clan_id);
    refresh_ally_on_members(world, clan_id);
    send_sm_with(
        world,
        player,
        sm_ids::YOU_HAVE_WITHDRAWN_FROM_THE_ALLIANCE,
        &[],
    );
}

/// `AllyDismiss` (0x8F): the alliance leader expels a member clan by name.
pub(crate) fn handle_ally_dismiss(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(name) = PacketReader::new(body).read_string() else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let is_leader = p.clan_leader;
    if clan_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION,
            &[],
        );
        return;
    }
    let Some(leader_clan) = world.clans.get(&clan_id) else {
        return;
    };
    let ally_id = leader_clan.ally_id;
    if ally_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_CURRENTLY_ALLIED_WITH_ANY_CLANS,
            &[],
        );
        return;
    }
    if !is_leader || clan_id != ally_id {
        send_sm_with(
            world,
            player,
            sm_ids::THIS_FEATURE_IS_ONLY_AVAILABLE_TO_ALLIANCE_LEADERS,
            &[],
        );
        return;
    }
    let Some(target) = world
        .clans
        .values()
        .find(|c| c.name.eq_ignore_ascii_case(&name))
    else {
        send_sm_with(world, player, sm_ids::THAT_CLAN_DOES_NOT_EXIST, &[]);
        return;
    };
    let target_id = target.id;
    if target_id == clan_id {
        send_sm_with(world, player, sm_ids::ALLIANCE_LEADERS_CANNOT_WITHDRAW, &[]);
        return;
    }
    if target.ally_id != ally_id {
        send_sm_with(world, player, sm_ids::DIFFERENT_ALLIANCE, &[]);
        return;
    }

    let now = now_millis();
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.ally_penalty_expiry_time = now + ALLY_PENALTY_MS;
        c.ally_penalty_type = ALLY_PENALTY_TYPE_DISMISS_CLAN;
    }
    store_clan_ally(world, clan_id);
    if let Some(c) = world.clans.get_mut(&target_id) {
        c.ally_id = 0;
        c.ally_name.clear();
        c.ally_crest_id = 0; // `changeAllyCrest(0, true)`
        c.ally_penalty_expiry_time = now + ALLY_PENALTY_MS;
        c.ally_penalty_type = ALLY_PENALTY_TYPE_CLAN_DISMISSED;
    }
    store_clan_ally(world, target_id);
    refresh_ally_on_members(world, target_id);
    send_sm_with(
        world,
        player,
        sm_ids::YOU_HAVE_SUCCEEDED_IN_EXPELLING_THE_CLAN,
        &[],
    );
}

/// `RequestDismissAlly` (0x90): the alliance leader dissolves the whole ally.
pub(crate) fn handle_request_dismiss_ally(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let is_leader = world
        .objects
        .get_component::<Player>(&player)
        .is_some_and(|p| p.clan_leader);
    if !is_leader {
        send_sm_with(
            world,
            player,
            sm_ids::THIS_FEATURE_IS_ONLY_AVAILABLE_TO_ALLIANCE_LEADERS,
            &[],
        );
        return;
    }
    handle_dissolve_ally(world, client_id, player);
}

/// `RequestAllyInfo` (0x2E): the ally window (`AllianceInfo`) + the SM cascade.
pub(crate) fn handle_request_ally_info(world: &World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let ally_id = world.clans.get(&p.clan_id).map(|c| c.ally_id).unwrap_or(0);
    if ally_id == 0 {
        send_sm_with(world, player, sm_ids::YOU_ARE_NOT_IN_AN_ALLIANCE, &[]);
        return;
    }
    let clans = ally_clan_ids(world, ally_id);
    let rows: Vec<(String, i32, String, i32, i32)> = clans
        .iter()
        .filter_map(|&cid| {
            let c = world.clans.get(&cid)?;
            let online = online_members(world, cid).len() as i32;
            Some((
                c.name.clone(),
                c.level,
                c.leader_name().to_string(),
                c.members.len() as i32,
                online,
            ))
        })
        .collect();
    let total: i32 = rows.iter().map(|r| r.3).sum();
    let online: i32 = rows.iter().map(|r| r.4).sum();
    let (ally_name, leader_clan_name, leader_player_name) = world
        .clans
        .get(&ally_id)
        .map(|c| {
            (
                c.ally_name.clone(),
                c.name.clone(),
                c.leader_name().to_string(),
            )
        })
        .unwrap_or_default();
    let Some(cs) = world.clients.get(&client_id) else {
        return;
    };
    cs.send(server_packets::alliance_info(
        &ally_name,
        total,
        online,
        &leader_clan_name,
        &leader_player_name,
        &rows,
    ));
    cs.send(server_packets::system_message_with(
        sm_ids::ALLIANCE_INFORMATION,
        &[],
    ));
    cs.send(server_packets::system_message_with(
        sm_ids::ALLIANCE_NAME_S1,
        &[SmParam::Text(ally_name)],
    ));
    cs.send(server_packets::system_message_with(
        sm_ids::ALLIANCE_LEADER_S2_OF_S1,
        &[
            SmParam::Text(leader_clan_name),
            SmParam::Text(leader_player_name),
        ],
    ));
    cs.send(server_packets::system_message_with(
        sm_ids::CONNECTION_S1_TOTAL_S2,
        &[SmParam::Int(online), SmParam::Int(total)],
    ));
    cs.send(server_packets::system_message_with(
        sm_ids::AFFILIATED_CLANS_TOTAL_S1_CLAN_S,
        &[SmParam::Int(rows.len() as i32)],
    ));
    for (name, level, leader, c_total, c_online) in &rows {
        cs.send(server_packets::system_message_with(
            sm_ids::CLAN_INFORMATION,
            &[],
        ));
        cs.send(server_packets::system_message_with(
            sm_ids::CLAN_NAME_S1,
            &[SmParam::Text(name.clone())],
        ));
        cs.send(server_packets::system_message_with(
            sm_ids::CLAN_LEADER_S1,
            &[SmParam::Text(leader.clone())],
        ));
        cs.send(server_packets::system_message_with(
            sm_ids::CLAN_LEVEL_S1,
            &[SmParam::Int(*level)],
        ));
        cs.send(server_packets::system_message_with(
            sm_ids::CONNECTION_S1_TOTAL_S2,
            &[SmParam::Int(*c_online), SmParam::Int(*c_total)],
        ));
        cs.send(server_packets::system_message_with(sm_ids::EMPTY_4, &[]));
    }
}

// --- G18 slice 6: sub-pledges & academy ------------------------------------
