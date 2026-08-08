use super::*;

use crate::model::clan::{SUBUNIT_ACADEMY, SUBUNIT_KNIGHT1, SUBUNIT_ROYAL1, SubPledge};

/// `CreateRoyalGuardCost = 5000` (Feature.ini) — the reputation price of a
/// royal-guard unit.
const ROYAL_GUARD_COST: i32 = 5000;
/// `CreateKnightUnitCost = 10000` — the reputation price of a knight unit.
const KNIGHT_UNIT_COST: i32 = 10_000;

/// `VillageMaster.isValidName`/name-length checks shared by clan/sub-pledge
/// names: alphanumeric, 2..=16 chars (this dist's `ClanNameTemplate = .*`, so
/// the retail regex itself is not ported — same simplification `create_clan`
/// makes).
fn valid_pledge_name(name: &str) -> bool {
    name.chars().all(|c| c.is_ascii_alphanumeric()) && (2..=16).contains(&name.len())
}

/// `VillageMaster.createSubPledge`: the shared academy/royal-guard/knight
/// creation flow. `requested_type` is the *family* id (`SUBUNIT_ACADEMY`,
/// `SUBUNIT_ROYAL1`, or `SUBUNIT_KNIGHT1`) — `Clan.getAvailablePledgeTypes`
/// resolves it to the next open slot in that family.
fn create_sub_pledge(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    requested_type: i32,
    min_clan_lvl: i32,
    name: &str,
    leader_name: Option<&str>,
) {
    let Some(clan_id) = clan_leader_of(world, player_oid) else {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.level < min_clan_lvl {
        let sm = if requested_type == SUBUNIT_ACADEMY {
            sm_ids::TO_ESTABLISH_A_CLAN_ACADEMY_YOUR_CLAN_MUST_BE_LEVEL_5_OR_HIGHER
        } else {
            sm_ids::THE_CONDITIONS_NECESSARY_TO_CREATE_A_MILITARY_UNIT_HAVE_NOT_BEEN_MET
        };
        send_sm(world, client_id, sm);
        return;
    }
    if !valid_pledge_name(name) {
        send_sm(world, client_id, sm_ids::CLAN_NAME_IS_INVALID);
        return;
    }
    if name.len() > 16 {
        send_sm(world, client_id, sm_ids::CLAN_NAME_S_LENGTH_IS_INCORRECT);
        return;
    }
    // Java scans every clan's sub-pledges for a name clash; the port's
    // `ClanTable` equivalent is `World.clans`.
    let name_taken = world.clans.values().any(|c| {
        c.sub_pledges
            .values()
            .any(|sp| sp.name.eq_ignore_ascii_case(name))
    });
    if name_taken {
        if requested_type == SUBUNIT_ACADEMY {
            send_sm_with(
                world,
                player_oid,
                sm_ids::S1_ALREADY_EXISTS,
                &[SmParam::Text(name.to_string())],
            );
        } else {
            send_sm(
                world,
                client_id,
                sm_ids::ANOTHER_MILITARY_UNIT_ALREADY_USES_THAT_NAME,
            );
        }
        return;
    }

    // The leader-designate (royal/knight only): must be a main-pledge member
    // who doesn't already captain a sub-unit.
    let leader_id = if requested_type != SUBUNIT_ACADEMY {
        let Some(leader_name) = leader_name else {
            return;
        };
        let eligible = clan
            .members
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(leader_name))
            .filter(|m| m.pledge_type == 0)
            .filter(|m| clan.leader_sub_pledge_of(m.char_id) == 0);
        let Some(member) = eligible else {
            let sm = if requested_type >= SUBUNIT_KNIGHT1 {
                sm_ids::THE_CAPTAIN_OF_THE_ORDER_OF_KNIGHTS_CANNOT_BE_APPOINTED
            } else {
                sm_ids::THE_ROYAL_GUARD_CAPTAIN_CANNOT_BE_APPOINTED
            };
            send_sm(world, client_id, sm);
            return;
        };
        member.char_id
    } else {
        0
    };
    // `Clan.createSubPledge`'s own reject: the clan leader can't also
    // captain a sub-unit ("Leader is not correct" — a plain message, no SM).
    if leader_id != 0 && leader_id == clan.leader_id {
        send_sm_with(
            world,
            player_oid,
            sm_ids::S1_TEXT,
            &[SmParam::Text("Leader is not correct".to_string())],
        );
        return;
    }

    // `Clan.createSubPledge`'s own guard chain: the resolved slot in the
    // requested family, then (royal/knight only) the reputation price.
    let pledge_type = clan.available_pledge_type(requested_type);
    if pledge_type == 0 {
        if requested_type == SUBUNIT_ACADEMY {
            send_sm(
                world,
                client_id,
                sm_ids::YOUR_CLAN_HAS_ALREADY_ESTABLISHED_A_CLAN_ACADEMY,
            );
        } else {
            send_sm_with(
                world,
                player_oid,
                sm_ids::S1_TEXT,
                &[SmParam::Text(
                    "You can't create any more sub-units of this type".to_string(),
                )],
            );
        }
        return;
    }
    let cost = if requested_type == SUBUNIT_ACADEMY {
        0
    } else if pledge_type < SUBUNIT_KNIGHT1 {
        ROYAL_GUARD_COST
    } else {
        KNIGHT_UNIT_COST
    };
    if cost > 0 && clan.reputation_score < cost {
        send_sm(world, client_id, sm_ids::THE_CLAN_REPUTATION_IS_TOO_LOW);
        return;
    }

    let sub_pledge = SubPledge {
        id: pledge_type,
        name: name.to_string(),
        leader_id,
    };
    let clan = world.clans.get_mut(&clan_id).expect("checked above");
    clan.sub_pledges.insert(pledge_type, sub_pledge);
    if cost > 0 {
        clan.reputation_score -= cost;
    }
    let reputation = clan.reputation_score;
    let _ = world.db.send(DbCommand::InsertSubPledge {
        clan_id,
        pledge_type,
        name: name.to_string(),
        leader_id,
    });
    if cost > 0 {
        let _ = world.db.send(DbCommand::UpdateClanReputation {
            clan_id,
            reputation,
        });
    }
    let info =
        server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("checked above"));
    let leader_display_name = if leader_id != 0 {
        player_name_or_empty(world, leader_id)
    } else {
        String::new()
    };
    let created =
        server_packets::pledge_receive_sub_pledge_created(pledge_type, name, &leader_display_name);
    for oid in online_members(world, clan_id) {
        send_to_member(world, oid, info.clone());
        send_to_member(world, oid, created.clone());
    }

    let clan_name = clan_name_or_empty(world, clan_id);
    let sm = if requested_type == SUBUNIT_ACADEMY {
        server_packets::system_message_with(
            sm_ids::CONGRATULATIONS_THE_S1_S_CLAN_ACADEMY_HAS_BEEN_CREATED,
            &[SmParam::Text(clan_name)],
        )
    } else if pledge_type >= SUBUNIT_KNIGHT1 {
        server_packets::system_message_with(
            sm_ids::THE_KNIGHTS_OF_S1_HAVE_BEEN_CREATED,
            &[SmParam::Text(clan_name)],
        )
    } else {
        server_packets::system_message_with(
            sm_ids::THE_ROYAL_GUARD_OF_S1_HAVE_BEEN_CREATED,
            &[SmParam::Text(clan_name)],
        )
    };
    send_to_member(world, player_oid, sm);

    if leader_id != 0 {
        let pledge_class = world
            .clans
            .get(&clan_id)
            .map(|c| c.pledge_class_of(leader_id))
            .unwrap_or(0);
        if let Some(lp) = world.objects.get_component_mut::<Player>(&leader_id) {
            lp.pledge_class = pledge_class;
        }
        crate::game_loop::party::broadcast_user_info(world, leader_id);
    }
}

/// `VillageMaster`'s `create_academy <name>` bypass.
pub(crate) fn handle_create_academy(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    args: &str,
) {
    let mut it = args.split_whitespace();
    let Some(name) = it.next() else { return };
    create_sub_pledge(world, client_id, player_oid, SUBUNIT_ACADEMY, 5, name, None);
}

/// `VillageMaster`'s `create_royal <name> <leaderName>` bypass.
pub(crate) fn handle_create_royal(world: &mut World, client_id: u32, player_oid: i32, args: &str) {
    let mut it = args.split_whitespace();
    let Some(name) = it.next() else { return };
    let leader = it.next();
    create_sub_pledge(
        world,
        client_id,
        player_oid,
        SUBUNIT_ROYAL1,
        6,
        name,
        leader,
    );
}

/// `VillageMaster`'s `create_knight <name> <leaderName>` bypass.
pub(crate) fn handle_create_knight(world: &mut World, client_id: u32, player_oid: i32, args: &str) {
    let mut it = args.split_whitespace();
    let Some(name) = it.next() else { return };
    let leader = it.next();
    create_sub_pledge(
        world,
        client_id,
        player_oid,
        SUBUNIT_KNIGHT1,
        7,
        name,
        leader,
    );
}

/// `VillageMaster.renameSubPledge` (`rename_pledge <pledgeTypeId> <newName>`).
pub(crate) fn handle_rename_pledge(world: &mut World, client_id: u32, player_oid: i32, args: &str) {
    let mut it = args.split_whitespace();
    let Some(Ok(pledge_type)) = it.next().map(str::parse::<i32>) else {
        return;
    };
    let Some(new_name) = it.next() else { return };
    let Some(clan_id) = clan_leader_of(world, player_oid) else {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    };
    if !world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.sub_pledges.contains_key(&pledge_type))
    {
        return; // "Pledge don't exists." (Java's own plain-text message)
    }
    if !valid_pledge_name(new_name) {
        send_sm(world, client_id, sm_ids::CLAN_NAME_IS_INVALID);
        return;
    }
    if new_name.len() > 16 {
        send_sm(world, client_id, sm_ids::CLAN_NAME_S_LENGTH_IS_INCORRECT);
        return;
    }
    let leader_id = {
        let Some(c) = world.clans.get_mut(&clan_id) else {
            return;
        };
        let Some(sp) = c.sub_pledges.get_mut(&pledge_type) else {
            return;
        };
        sp.name = new_name.to_string();
        sp.leader_id
    };
    let _ = world.db.send(DbCommand::UpdateSubPledge {
        clan_id,
        pledge_type,
        name: new_name.to_string(),
        leader_id,
    });
    broadcast_clan_status(world, clan_id);
}

/// `VillageMaster.assignSubPledgeLeader` (`assign_subpl_leader <unitName>
/// <memberName>`).
pub(crate) fn handle_assign_subpledge_leader(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    args: &str,
) {
    let mut it = args.split_whitespace();
    let Some(unit_name) = it.next() else { return };
    let Some(member_name) = it.next() else { return };
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    let player_display_name = p.name.clone();
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    if member_name.len() > 16 {
        send_sm(
            world,
            client_id,
            sm_ids::YOUR_TITLE_CANNOT_EXCEED_16_CHARACTERS,
        );
        return;
    }
    if player_display_name.eq_ignore_ascii_case(member_name) {
        send_sm(
            world,
            client_id,
            sm_ids::THE_ROYAL_GUARD_CAPTAIN_CANNOT_BE_APPOINTED,
        );
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let Some(sub_pledge) = clan
        .sub_pledges
        .values()
        .find(|sp| sp.name.eq_ignore_ascii_case(unit_name))
    else {
        send_sm(world, client_id, sm_ids::CLAN_NAME_IS_INVALID);
        return;
    };
    if sub_pledge.id == SUBUNIT_ACADEMY {
        send_sm(world, client_id, sm_ids::CLAN_NAME_IS_INVALID);
        return;
    }
    let sub_pledge_id = sub_pledge.id;
    let eligible = clan
        .members
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(member_name))
        .filter(|m| m.pledge_type == 0)
        .filter(|m| clan.leader_sub_pledge_of(m.char_id) == 0);
    let Some(member) = eligible.cloned() else {
        let sm = if sub_pledge_id >= SUBUNIT_KNIGHT1 {
            sm_ids::THE_CAPTAIN_OF_THE_ORDER_OF_KNIGHTS_CANNOT_BE_APPOINTED
        } else {
            sm_ids::THE_ROYAL_GUARD_CAPTAIN_CANNOT_BE_APPOINTED
        };
        send_sm(world, client_id, sm);
        return;
    };

    if let Some(c) = world.clans.get_mut(&clan_id)
        && let Some(sp) = c.sub_pledges.get_mut(&sub_pledge_id)
    {
        sp.leader_id = member.char_id;
    }
    let _ = world.db.send(DbCommand::UpdateSubPledge {
        clan_id,
        pledge_type: sub_pledge_id,
        name: unit_name.to_string(),
        leader_id: member.char_id,
    });

    let pledge_class = world
        .clans
        .get(&clan_id)
        .map(|c| c.pledge_class_of(member.char_id))
        .unwrap_or(0);
    if let Some(lp) = world.objects.get_component_mut::<Player>(&member.char_id) {
        lp.pledge_class = pledge_class;
    }
    crate::game_loop::party::broadcast_user_info(world, member.char_id);
    broadcast_clan_status(world, clan_id);
    let sm = server_packets::system_message_with(
        sm_ids::C1_HAS_BEEN_SELECTED_AS_THE_CAPTAIN_OF_S2,
        &[
            SmParam::Text(member.name.clone()),
            SmParam::Text(unit_name.to_string()),
        ],
    );
    broadcast_to_clan(world, clan_id, &sm);
}

// --- G18 slice 7: crests ----------------------------------------------------
