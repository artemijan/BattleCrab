use super::*;
use crate::game_loop::guard::clan_of_or_zero;

/// Java `Clan.checkClanJoinCondition(player, target, pledgeType)` — the invite
/// guard chain, with each reject's system message sent to the inviter. Run at
/// invite time and re-run when the answer arrives (conditions can change while
/// the dialog is up — Java's "double check").
fn check_clan_join_condition(
    world: &World,
    requestor_oid: i32,
    target_oid: i32,
    pledge_type: i32,
) -> bool {
    let Some(req) = world.objects.get_component::<Player>(&requestor_oid) else {
        return false;
    };
    let clan_id = req.clan_id;
    let requestor_privs = req.clan_privs;
    let Some(clan) = world.clans.get(&clan_id) else {
        return false;
    };
    if !clan.has_privilege(requestor_oid, requestor_privs, CL_JOIN_CLAN) {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return false;
    }
    let Some(target) = world.objects.get_component::<Player>(&target_oid) else {
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
    if clan.char_penalty_expiry_time > now_millis() {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::AFTER_A_CLAN_MEMBER_IS_DISMISSED_THE_CLAN_MUST_WAIT_A_DAY,
            &[],
        );
        return false;
    }
    if target.clan_id != 0 {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::S1_IS_ALREADY_A_MEMBER_OF_ANOTHER_CLAN,
            &[SmParam::Text(target.name.clone())],
        );
        return false;
    }
    if target.clan_join_expiry_time > now_millis() {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::C1_CANNOT_JOIN_THE_CLAN_ONE_DAY_HAS_NOT_PASSED_SINCE_LEAVING,
            &[SmParam::Text(target.name.clone())],
        );
        return false;
    }
    if (target.level > 40 || class_level(world, target.class_id) >= 2) && pledge_type == -1 {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::S1_DOES_NOT_MEET_THE_REQUIREMENTS_TO_JOIN_A_CLAN_ACADEMY,
            &[SmParam::Text(target.name.clone())],
        );
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::IN_ORDER_TO_JOIN_THE_CLAN_ACADEMY_YOU_MUST_BE_UNAFFILIATED,
            &[],
        );
        return false;
    }
    if clan.sub_pledge_members_count(pledge_type) >= clan.max_members_of(pledge_type) {
        if pledge_type == 0 {
            send_sm_with(
                world,
                requestor_oid,
                sm_ids::S1_IS_FULL_AND_CANNOT_ACCEPT_ADDITIONAL_CLAN_MEMBERS,
                &[SmParam::Text(clan.name.clone())],
            );
        } else {
            send_sm_with(world, requestor_oid, sm_ids::THE_CLAN_IS_FULL, &[]);
        }
        return false;
    }
    true
}

/// `RequestJoinPledge` (0x26): a clan member invites the target player. Guards,
/// then parks the invite in the `PendingRequest` slot and puts `AskJoinPledge`
/// on the target's screen.
pub(crate) fn handle_request_join_pledge(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(body);
    let Some(target_oid) = r.read_i32() else {
        return;
    };
    let Some(pledge_type) = r.read_i32() else {
        return;
    };

    let clan_id = clan_of_or_zero(world, player);
    if clan_id == 0 {
        return; // Java: getClan() == null → silent
    }
    // Java resolves the target through `World.getPlayer(objectId)` (online only).
    if client_for_player(world, target_oid).is_none() {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_HAVE_INVITED_THE_WRONG_TARGET,
            &[],
        );
        return;
    }
    if !check_clan_join_condition(world, player, target_oid, pledge_type) {
        return;
    }
    if pledge_type != 0
        && !world
            .clans
            .get(&clan_id)
            .is_some_and(|c| c.sub_pledges.contains_key(&pledge_type))
    {
        // The client only ever offers real sub-units in the invite dialog; a
        // request naming a pledge type the clan hasn't founded is dropped
        // (Java trusts the client here too — this is the port's own guard
        // against corrupting the roster on a malformed/hacked packet).
        warn!("Clan invite with pledge type {pledge_type} refused — no such sub-unit.");
        return;
    }
    // Java `player.getRequest().setRequest(target, this)` — busy targets answer
    // "on another task" (the shared transaction-slot behavior).
    if refuse_if_busy(world, player, target_oid) {
        return;
    }
    crate::game_loop::party::install_request(
        world,
        player,
        target_oid,
        crate::model::components::RequestKind::ClanInvite {
            clan_id,
            pledge_type,
        },
        crate::game_loop::party::REQUEST_TIMEOUT_TICKS,
    );
    let clan_name = clan_name_or_empty(world, clan_id);
    send_to_member(
        world,
        target_oid,
        server_packets::ask_join_pledge(
            player,
            &player_name_or_empty(world, player),
            pledge_type,
            &clan_name,
        ),
    );
}

/// `RequestAnswerJoinPledge` (0x27): the invited player answered the
/// `AskJoinPledge` dialog. Decline notifies both sides; accept re-checks the
/// join condition and runs `Clan.addClanMember` (roster + packets + skills).
pub(crate) fn handle_request_answer_join_pledge(world: &mut World, client_id: u32, body: &[u8]) {
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
    let crate::model::components::RequestKind::ClanInvite {
        clan_id,
        pledge_type,
    } = req.kind
    else {
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
            sm_ids::YOU_DIDN_T_RESPOND_TO_S1_S_INVITATION_JOINING_HAS_BEEN_CANCELLED,
            &[SmParam::Text(player_name_or_empty(world, requestor))],
        );
        send_sm_with(
            world,
            requestor,
            sm_ids::S1_DID_NOT_RESPOND_INVITATION_TO_THE_CLAN_HAS_BEEN_CANCELLED,
            &[SmParam::Text(player_name_or_empty(world, player))],
        );
        return;
    }
    // "conditions can be changed, i.e. another player could join" — re-check,
    // and the requestor must still be in the clan the invite was for.
    if world
        .objects
        .get_component::<Player>(&requestor)
        .map(|p| p.clan_id)
        != Some(clan_id)
    {
        return;
    }
    if !check_clan_join_condition(world, requestor, player, pledge_type) {
        return;
    }
    if clan_of_or_zero(world, player) != 0 {
        return;
    }
    add_clan_member(world, clan_id, player, pledge_type);
}

/// Java `RequestAnswerJoinPledge`'s accept half + `Clan.addClanMember`: put the
/// new member in the roster, wire their clan fields, and send the join burst.
/// New members start at power grade 5 with no rank privileges (the rank-privs
/// table is a later slice — Java's fresh-clan `getRankPrivs(5)` is CP_NOTHING).
pub(crate) fn add_clan_member(world: &mut World, clan_id: i32, player_oid: i32, pledge_type: i32) {
    send_to_member(world, player_oid, server_packets::join_pledge(clan_id));

    // Java: academy members start at power grade 9, everyone else at 5
    // ("not confirmed" per Java's own comment, kept faithfully).
    let grade = if pledge_type == crate::model::clan::SUBUNIT_ACADEMY {
        9
    } else {
        5
    };
    let member = {
        let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
            return;
        };
        ClanMember {
            char_id: player_oid,
            name: p.name.clone(),
            level: p.level,
            class_id: p.class_id,
            sex: p.is_female as i32,
            race: p.race,
            power_grade: grade,
            title: p.title.clone(),
            pledge_type,
            apprentice: 0,
            sponsor: 0,
        }
    };
    let Some(clan) = world.clans.get_mut(&clan_id) else {
        return;
    };
    clan.members.push(member.clone());
    let pledge_class = clan.pledge_class_of(player_oid);
    // Java `player.setClanPrivileges(clan.getRankPrivs(player.getPowerGrade()))`.
    let privs = clan.rank_privs_of(grade);
    sync_clan_insignia(world, clan_id, player_oid);
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.clan_id = clan_id;
        p.clan_privs = privs;
        p.clan_leader = false;
        p.power_grade = grade;
        p.pledge_type = pledge_type;
        p.pledge_class = pledge_class;
        p.clan_join_expiry_time = 0; // Java `setClanJoinExpiryTime(0)`
    }
    let _ = world.db.send(DbCommand::UpdateCharClan {
        char_id: player_oid,
        clan_id,
        clan_privs: privs,
    });
    let _ = world.db.send(DbCommand::UpdateCharPowerGrade {
        char_id: player_oid,
        power_grade: grade,
    });
    let _ = world.db.send(DbCommand::UpdateCharPledgeType {
        char_id: player_oid,
        pledge_type,
    });
    let _ = world.db.send(DbCommand::UpdateCharClanJoinExpiry {
        char_id: player_oid,
        expiry: 0,
    });
    // Java `RequestAnswerJoinPledge`: an academy invite also stamps the level
    // the recruit joined at — the graduation reward scales off it.
    academy::on_join(world, player_oid, pledge_type);

    send_sm_with(world, player_oid, sm_ids::ENTERED_THE_CLAN, &[]);
    let joined = server_packets::system_message_with(
        sm_ids::S1_HAS_JOINED_THE_CLAN,
        &[SmParam::Text(player_name_or_empty(world, player_oid))],
    );
    broadcast_to_clan(world, clan_id, &joined);

    // Clan skills + the merged skill list (Java `addClanMember` →
    // `addSkillEffects(player)` + `PledgeSkillList`). This also grants the
    // clan's castle residential skills, so a member who joins mid-ownership
    // gets them now rather than only at their next login.
    apply_clan_skills_to_member(world, clan_id, player_oid);
    // Clan Advent — Java fires ON_PLAYER_CLAN_JOIN, the ClanMaster script
    // lights the aura on the joiner when the leader is online.
    let leader_online = world
        .clans
        .get(&clan_id)
        .map(|c| c.leader_id)
        .is_some_and(|lid| client_for_player(world, lid).is_some());
    if leader_online {
        apply_clan_advent(world, player_oid);
    }

    let add = server_packets::pledge_show_member_list_add(&member);
    let info =
        server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("inserted above"));
    let count = server_packets::ex_pledge_count(
        world
            .clans
            .get(&clan_id)
            .map(|c| c.members.len())
            .unwrap_or(0) as i32,
    );
    for oid in online_members(world, clan_id) {
        if oid != player_oid {
            send_to_member(world, oid, add.clone());
        }
        send_to_member(world, oid, info.clone());
        send_to_member(world, oid, count.clone());
    }
    // "this activates the clan tab on the new member".
    for pkt in server_packets::pledge_show_member_list_all_tabs(
        world.clans.get(&clan_id).expect("inserted above"),
        &world.objects,
    ) {
        send_to_member(world, player_oid, pkt);
    }
    crate::game_loop::player_info::broadcast_user_info(world, player_oid);
}

/// Java `Clan.removeClanMember(objectId, clanJoinExpiryTime)`, narrowed to the
/// main pledge: drop the roster row, tear the member's clan state down (online)
/// or push the column reset (offline), and stamp the rejoin penalty. The
/// caller sends the leave/oust messages and the roster-delete broadcasts.
/// The academy trio (`lvl_joined_academy` + the apprentice/sponsor pair) is
/// cleared here, as Java's `setClan(null)` does. **The residential skills need
/// no separate teardown**: they ride the same transient `ClanSkills` component
/// as the pledge skills, which `remove_clan_skills_from_member` below clears
/// wholesale — Java has to name them separately only because it keeps them in a
/// different collection. A departing sub-pledge leader also vacates their
/// unit's leader slot (`Clan.removeClanMember`'s `getLeaderSubPledge` leg).
///
/// The castle circlet goes with the member (Java `Clan.removeClanMember` →
/// `CastleManager.removeCirclet(exMember, getCastleId())`, gated on
/// `RemoveCastleCirclets`): leaving a castle-owning clan costs you the crown.
pub(crate) fn remove_clan_member(
    world: &mut World,
    clan_id: i32,
    member_oid: i32,
    clan_join_expiry: i64,
) {
    // Java `Player.setClan(null)` clears `lvlJoinedAcademy` + the mentorship
    // pair; run it first, while the member is still on the roster (the
    // mentorship lookup reads it).
    academy::on_leave_clan(world, member_oid);
    // `CastleManager.removeCirclet(exMember, getCastleId())` — before the
    // roster edit below, while the clan still reports its castle. A clan with
    // no castle has id 0, which `circlet_of` maps to "no circlet".
    if world.cfg.character.remove_castle_circlets {
        let castle_id = world.clans.get(&clan_id).map_or(0, |c| c.castle_id);
        crate::game_loop::castle::remove_circlet(world, member_oid, castle_id);
    }
    let Some(clan) = world.clans.get_mut(&clan_id) else {
        return;
    };
    let Some(idx) = clan.members.iter().position(|m| m.char_id == member_oid) else {
        warn!("Member {member_oid} not found in clan {clan_id} while trying to remove.");
        return;
    };
    clan.members.remove(idx);
    // `Clan.removeClanMember`'s `getLeaderSubPledge` leg: a departing
    // sub-pledge leader vacates their unit's slot (0 = vacant), persisted
    // like a leader reassignment.
    let vacated: Vec<(i32, String)> = clan
        .sub_pledges
        .values_mut()
        .filter(|sp| sp.leader_id == member_oid)
        .map(|sp| {
            sp.leader_id = 0;
            (sp.id, sp.name.clone())
        })
        .collect();
    for (pledge_type, name) in vacated {
        let _ = world.db.send(DbCommand::UpdateSubPledge {
            clan_id,
            pledge_type,
            name,
            leader_id: 0,
        });
    }
    // Read before the mutable borrow below.
    let create_cooldown_ms = clan_create_cooldown_ms(world);
    let Some(clan) = world.clans.get_mut(&clan_id) else {
        return;
    };
    let was_leader = clan.leader_id == member_oid;
    let leader_expiry = if was_leader {
        now_millis() + create_cooldown_ms
    } else {
        0
    };

    // Java `removeClanMember`: a departing sub-unit captain leaves the slot
    // vacant ("position becomes vacant and leader should appoint new via NPC").
    let vacated_sub_pledge = clan.leader_sub_pledge_of(member_oid);
    if vacated_sub_pledge != 0 {
        if let Some(sp) = clan.sub_pledges.get_mut(&vacated_sub_pledge) {
            sp.leader_id = 0;
        }
        let (name, leader_id) = clan
            .sub_pledges
            .get(&vacated_sub_pledge)
            .map(|sp| (sp.name.clone(), sp.leader_id))
            .unwrap_or_default();
        let _ = world.db.send(DbCommand::UpdateSubPledge {
            clan_id,
            pledge_type: vacated_sub_pledge,
            name,
            leader_id,
        });
    }

    let online = world.objects.get_component::<Player>(&member_oid).is_some();
    if online {
        // Java: title cleared unless noble, clan skills + Clan Advent stripped,
        // clan fields zeroed, join penalty stamped, window closed.
        remove_clan_advent(world, member_oid);
        remove_clan_skills_from_member(world, member_oid);
        if let Some(p) = world.objects.get_component_mut::<Player>(&member_oid) {
            if !p.is_noble {
                p.title.clear();
            }
            p.clan_id = 0;
            p.clan_privs = 0;
            p.clan_leader = false;
            p.pledge_class = 0;
            p.ally_id = 0;
            p.clan_join_expiry_time = clan_join_expiry;
            if was_leader {
                p.clan_create_expiry_time = leader_expiry;
            }
        }
        send_to_member(
            world,
            member_oid,
            server_packets::pledge_show_member_list_delete_all(),
        );
        crate::game_loop::player_info::broadcast_user_info(world, member_oid);
    }
    let _ = world.db.send(DbCommand::RemoveClanMember {
        char_id: member_oid,
        clan_join_expiry,
        clan_create_expiry: leader_expiry,
    });
}

/// `RequestWithdrawalPledge` (0x28): a member (never the leader) leaves their
/// clan, taking the 1-day rejoin penalty.
pub(crate) fn handle_request_withdrawal_pledge(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    if refuse_if_clanless(world, player, clan_id) {
        return;
    }
    if p.clan_leader {
        send_sm_with(
            world,
            player,
            sm_ids::A_CLAN_LEADER_CANNOT_WITHDRAW_FROM_THEIR_OWN_CLAN,
            &[],
        );
        return;
    }
    if crate::game_loop::combat::has_attack_stance(world, player) {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_CANNOT_LEAVE_A_CLAN_WHILE_ENGAGED_IN_COMBAT,
            &[],
        );
        return;
    }

    let name = player_name_or_empty(world, player);
    let penalty = now_millis() + clan_join_penalty_ms(world);
    remove_clan_member(world, clan_id, player, penalty);

    let withdrew = server_packets::system_message_with(
        sm_ids::S1_HAS_WITHDRAWN_FROM_THE_CLAN,
        &[SmParam::Text(name.clone())],
    );
    broadcast_to_clan(world, clan_id, &withdrew);
    broadcast_to_clan(
        world,
        clan_id,
        &server_packets::pledge_show_member_list_delete(&name),
    );
    let count = server_packets::ex_pledge_count(
        world
            .clans
            .get(&clan_id)
            .map(|c| c.members.len())
            .unwrap_or(0) as i32,
    );
    broadcast_to_clan(world, clan_id, &count);
    send_sm_with(world, player, sm_ids::YOU_HAVE_WITHDRAWN_FROM_THE_CLAN, &[]);
    send_sm_with(
        world,
        player,
        sm_ids::AFTER_LEAVING_A_CLAN_YOU_MUST_WAIT_A_DAY_BEFORE_JOINING_ANOTHER,
        &[],
    );
}

/// `RequestOustPledgeMember` (0x29): a member with CL_DISMISS expels another
/// member by name. Both sides take a 1-day penalty: the oustee cannot join a
/// clan, the clan cannot invite (`setCharPenaltyExpiryTime`).
pub(crate) fn handle_request_oust_pledge_member(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(target_name) = PacketReader::new(body).read_string() else {
        return;
    };
    let Some((clan_id, privs)) = crate::game_loop::guard::clan_and_privs(world, player) else {
        return;
    };
    if refuse_if_clanless(world, player, clan_id) {
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if !clan.has_privilege(player, privs, CL_DISMISS) {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return;
    }
    if player_name_or_empty(world, player).eq_ignore_ascii_case(&target_name) {
        send_sm_with(world, player, sm_ids::YOU_CANNOT_DISMISS_YOURSELF, &[]);
        return;
    }
    let Some(member) = clan.member_by_name(&target_name).cloned() else {
        warn!("Oust target ({target_name}) is not a member of clan {clan_id}.");
        return;
    };
    let member_online = client_for_player(world, member.char_id).is_some();
    if member_online && crate::game_loop::combat::has_attack_stance(world, member.char_id) {
        send_sm_with(
            world,
            player,
            sm_ids::A_CLAN_MEMBER_MAY_NOT_BE_DISMISSED_DURING_COMBAT,
            &[],
        );
        return;
    }

    let penalty_until = now_millis() + clan_join_penalty_ms(world);
    remove_clan_member(world, clan_id, member.char_id, penalty_until);
    let dissolving = world
        .clans
        .get(&clan_id)
        .map(|c| c.dissolving_expiry_time)
        .unwrap_or(0);
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.char_penalty_expiry_time = penalty_until;
    }
    let _ = world.db.send(DbCommand::UpdateClanPenalties {
        clan_id,
        char_penalty_expiry_time: penalty_until,
        dissolving_expiry_time: dissolving,
    });

    let expelled = server_packets::system_message_with(
        sm_ids::CLAN_MEMBER_S1_HAS_BEEN_EXPELLED,
        &[SmParam::Text(member.name.clone())],
    );
    broadcast_to_clan(world, clan_id, &expelled);
    send_sm_with(
        world,
        player,
        sm_ids::YOU_HAVE_SUCCEEDED_IN_EXPELLING_THE_CLAN_MEMBER,
        &[],
    );
    send_sm_with(
        world,
        player,
        sm_ids::AFTER_A_CLAN_MEMBER_IS_DISMISSED_THE_CLAN_MUST_WAIT_A_DAY,
        &[],
    );
    broadcast_to_clan(
        world,
        clan_id,
        &server_packets::pledge_show_member_list_delete(&member.name),
    );
    let count = server_packets::ex_pledge_count(
        world
            .clans
            .get(&clan_id)
            .map(|c| c.members.len())
            .unwrap_or(0) as i32,
    );
    broadcast_to_clan(world, clan_id, &count);
    if member_online {
        send_sm_with(
            world,
            member.char_id,
            sm_ids::YOU_HAVE_RECENTLY_BEEN_DISMISSED_FROM_A_CLAN,
            &[],
        );
    }
}

/// `VillageMaster.dissolveClan` (the `dissolve_clan` bypass): guard chain,
/// then stamp `dissolving_expiry_time`, hit the leader with a full death-XP
/// penalty, and schedule the delayed removal.
pub(crate) fn handle_dissolve_clan(world: &mut World, client_id: u32, player_oid: i32) {
    let Some(clan_id) = clan_leader_of(world, player_oid) else {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.ally_id != 0 {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_CANNOT_DISPERSE_THE_CLANS_IN_YOUR_ALLIANCE,
        );
        return;
    }
    if clan_is_at_war(world, clan_id) {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_CANNOT_DISSOLVE_A_CLAN_WHILE_ENGAGED_IN_A_WAR,
        );
        return;
    }
    if clan.castle_id != 0 {
        // Java folds castle/clan-hall/fort ownership into SM 266.
        send_sm(
            world,
            client_id,
            sm_ids::YOU_CANNOT_DISSOLVE_A_CLAN_WHILE_OWNING_A_CLAN_HALL_OR_CASTLE,
        );
        return;
    }
    if world.sieges.values().any(|s| s.is_registered(clan_id)) {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_CANNOT_DISSOLVE_A_CLAN_DURING_A_SIEGE,
        );
        return;
    }
    if in_siege_zone(world, player_oid) {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_CANNOT_DISSOLVE_A_CLAN_DURING_A_SIEGE,
        );
        return;
    }
    if clan.dissolving_expiry_time > now_millis() {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_HAVE_ALREADY_REQUESTED_THE_DISSOLUTION_OF_YOUR_CLAN,
        );
        return;
    }

    let due = now_millis() + clan_dissolve_delay_ms(world);
    let char_penalty = clan.char_penalty_expiry_time;
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.dissolving_expiry_time = due;
    }
    let _ = world.db.send(DbCommand::UpdateClanPenalties {
        clan_id,
        char_penalty_expiry_time: char_penalty,
        dissolving_expiry_time: due,
    });
    // "The clan leader should take the XP penalty of a full death."
    crate::game_loop::death::apply_death_exp_penalty(world, player_oid);
    schedule_clan_dissolve(world, clan_id, due);
}

/// `VillageMaster.recoverClan` (the `recover_clan` bypass): the leader cancels
/// a pending dissolution — the stamp is zeroed, the scheduled removal no-ops.
pub(crate) fn handle_recover_clan(world: &mut World, client_id: u32, player_oid: i32) {
    let Some(clan_id) = clan_leader_of(world, player_oid) else {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    };
    let char_penalty = {
        let Some(c) = world.clans.get_mut(&clan_id) else {
            return;
        };
        c.dissolving_expiry_time = 0;
        c.char_penalty_expiry_time
    };
    let _ = world.db.send(DbCommand::UpdateClanPenalties {
        clan_id,
        char_penalty_expiry_time: char_penalty,
        dissolving_expiry_time: 0,
    });
}

/// Arm the `ClanDissolve` task for `due` (wall clock) — used by the dissolve
/// bypass and re-armed at boot for persisted stamps (`ClanTable`'s constructor
/// schedules past-due dissolutions to fire immediately).
pub(crate) fn schedule_clan_dissolve(world: &mut World, clan_id: i32, due: i64) {
    let delay_ticks = ((due - now_millis()).max(0) / MS_PER_TICK) as u64;
    world.scheduler.schedule(
        world.tick + delay_ticks,
        crate::scheduler::ScheduledTask::ClanDissolve { clan_id },
    );
}

/// `ClanTable.scheduleRemoveClan`'s body at fire time: destroy only if the
/// dissolution is still requested and has come due (a `recover_clan` in the
/// meantime zeroes the stamp and turns this into a no-op).
pub(crate) fn handle_clan_dissolve_task(world: &mut World, clan_id: i32) {
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.dissolving_expiry_time == 0 || clan.dissolving_expiry_time > now_millis() {
        return;
    }
    destroy_clan(world, clan_id);
}

// --- G18 slice 2: clan level-up + rep-gated pledge skill learning ----------
