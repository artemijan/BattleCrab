//! Alliances: creating and joining one, leaving, dismissal, and the
//! penalties each carries.

use super::*;

/// Alliance creation + the invite/accept flow: guards, the AskJoinAlly dialog,
/// the target clan folded in (members' ally id synced), the 3-clan cap, the
/// at-war reject, the ally window, and the same-ally war/dissolve interlocks.
#[test]
fn ally_create_join_and_interlocks() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    install_clan(&mut world, 5001, &[3003]);
    drain_db(&mut db_rx);

    let bypass = |world: &mut World, client: u32, cmd: &str| {
        handle_request_bypass_to_server(
            world,
            client,
            &bypass_body(&format!("npc_{NPC_OID}_{cmd}")),
        );
    };

    // Clan level < 5.
    bypass(&mut world, 1, "create_ally GoldenPact");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::TO_CREATE_AN_ALLIANCE_YOUR_CLAN_MUST_BE_LEVEL_5_OR_HIGHER
        )
    );
    world.clans.get_mut(&5000).unwrap().level = 5;
    world.clans.get_mut(&5001).unwrap().level = 5;

    // Bad name.
    bypass(&mut world, 1, "create_ally Bad Name");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::INCORRECT_ALLIANCE_NAME)
    );

    // Success: the clan becomes its own alliance.
    bypass(&mut world, 1, "create_ally GoldenPact");
    let clan = &world.clans[&5000];
    assert_eq!(
        (clan.ally_id, clan.ally_name.as_str()),
        (5000, "GoldenPact")
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .ally_id,
        5000,
        "leader's ally id synced"
    );
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateClanAlly {
            clan_id: 5000,
            ally_id: 5000,
            ..
        }
    )));
    drain(&mut a_rx);

    // A second clan cannot reuse the ally name.
    world.clans.get_mut(&5001).unwrap().level = 5;
    bypass(&mut world, 2, "create_ally GoldenPact");
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THAT_ALLIANCE_NAME_ALREADY_EXISTS)
    );

    // At war → no invite.
    world.clan_wars.push(model::clan::ClanWar {
        attacker_id: 5000,
        attacked_id: 5001,
        state: model::clan::ClanWarState::Mutual,
        winner_id: 0,
        start_time: 1,
        end_time: 0,
        attacker_kills: 0,
        attacked_kills: 0,
    });
    clans::handle_request_join_ally(&mut world, 1, &oid_body(3003));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_MAY_NOT_ALLY_WITH_A_CLAN_YOU_ARE_AT_WAR_WITH)
    );
    world.clan_wars.clear();

    // Invite → dialog; decline → both notified.
    clans::handle_request_join_ally(&mut world, 1, &oid_body(3003));
    let b_pkts = drain(&mut b_rx);
    assert!(
        b_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ASK_JOIN_ALLIANCE)
    );
    assert!(
        ids_after_opcode(&b_pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_LEADER_S2_HAS_REQUESTED_AN_ALLIANCE)
    );
    clans::handle_request_answer_join_ally(&mut world, 2, &answer_body(0));
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::NO_RESPONSE_YOUR_ENTRANCE_TO_THE_ALLIANCE_HAS_BEEN_CANCELLED
        )
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::NO_RESPONSE_INVITATION_TO_JOIN_AN_ALLIANCE_HAS_BEEN_CANCELLED
        )
    );

    // Accept: the whole clan joins.
    clans::handle_request_join_ally(&mut world, 1, &oid_body(3003));
    drain(&mut b_rx);
    clans::handle_request_answer_join_ally(&mut world, 2, &answer_body(1));
    assert_eq!(world.clans[&5001].ally_id, 5000);
    assert_eq!(world.clans[&5001].ally_name, "GoldenPact");
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3003)
            .unwrap()
            .ally_id,
        5000
    );
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_ACCEPTED_THE_ALLIANCE)
    );

    // The 3-clan cap: two more member clans fill the alliance.
    install_clan(&mut world, 5002, &[3005]);
    let mut c_rx = ingame_player(&mut world, 3, 3007, 0, 0, 0);
    install_clan(&mut world, 5003, &[3007]);
    world.clans.get_mut(&5002).unwrap().ally_id = 5000;
    clans::handle_request_join_ally(&mut world, 1, &oid_body(3007));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_EXCEEDED_THE_LIMIT)
    );
    drain(&mut c_rx);

    // The ally window.
    clans::handle_request_ally_info(&world, 2);
    let b_pkts = drain(&mut b_rx);
    assert!(
        b_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ALLIANCE_INFO)
    );
    assert!(
        ids_after_opcode(&b_pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::ALLIANCE_INFORMATION)
    );

    // Same-ally war declaration is refused; an allied clan cannot dissolve.
    world.clans.get_mut(&5000).unwrap().level = 3;
    world.clans.get_mut(&5001).unwrap().level = 3;
    pad_clan(&mut world, 5000, 15);
    pad_clan(&mut world, 5001, 15);
    clans::handle_request_start_pledge_war(&mut world, 1, &name_body("Clan5001"));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CANNOT_DECLARE_WAR_ON_ALLIED_CLAN)
    );
    bypass(&mut world, 1, "dissolve_clan");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CANNOT_DISPERSE_THE_CLANS_IN_YOUR_ALLIANCE)
    );
}

/// Leave, dismiss, and dissolution: the penalty types 1–4 and their gates.
#[test]
fn ally_leave_dismiss_dissolve_penalties() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    install_clan(&mut world, 5001, &[3003]);
    for (id, ally) in [(5000, 5000), (5001, 5000)] {
        let c = world.clans.get_mut(&id).unwrap();
        c.level = 5;
        c.ally_id = ally;
        c.ally_name = "GoldenPact".into();
    }
    drain_db(&mut db_rx);

    // The alliance leader cannot leave their own alliance.
    clans::handle_ally_leave(&mut world, 1);
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::ALLIANCE_LEADERS_CANNOT_WITHDRAW)
    );

    // A member clan leaves: penalty type 1 stamped.
    clans::handle_ally_leave(&mut world, 2);
    let b = &world.clans[&5001];
    assert_eq!(b.ally_id, 0);
    assert_eq!(
        b.ally_penalty_type,
        model::clan::ALLY_PENALTY_TYPE_CLAN_LEAVED
    );
    assert!(b.ally_penalty_expiry_time > 0);
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_WITHDRAWN_FROM_THE_ALLIANCE)
    );

    // The leave penalty blocks rejoining.
    clans::handle_request_join_ally(&mut world, 1, &oid_body(3003));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_CLAN_CANNOT_JOIN_ALLIANCE_ONE_DAY_NOT_PASSED)
    );

    // Re-admit directly, then the leader dismisses the clan: penalties 2 + 3.
    {
        let c = world.clans.get_mut(&5001).unwrap();
        c.ally_id = 5000;
        c.ally_name = "GoldenPact".into();
        c.ally_penalty_expiry_time = 0;
        c.ally_penalty_type = 0;
    }
    clans::handle_ally_dismiss(&mut world, 1, &name_body("Clan5001"));
    assert_eq!(world.clans[&5001].ally_id, 0);
    assert_eq!(
        world.clans[&5001].ally_penalty_type,
        model::clan::ALLY_PENALTY_TYPE_CLAN_DISMISSED
    );
    assert_eq!(
        world.clans[&5000].ally_penalty_type,
        model::clan::ALLY_PENALTY_TYPE_DISMISS_CLAN
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_SUCCEEDED_IN_EXPELLING_THE_CLAN)
    );

    // The dismiss penalty blocks inviting anyone.
    {
        let c = world.clans.get_mut(&5001).unwrap();
        c.ally_penalty_expiry_time = 0;
        c.ally_penalty_type = 0;
    }
    clans::handle_request_join_ally(&mut world, 1, &oid_body(3003));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::MAY_NOT_ACCEPT_ANY_CLAN_WITHIN_A_DAY_AFTER_EXPELLING
        )
    );

    // Dissolution: everyone out, penalty 4 on the ex-leader clan, and the
    // re-create gate.
    {
        let a = world.clans.get_mut(&5000).unwrap();
        a.ally_penalty_expiry_time = 0;
        a.ally_penalty_type = 0;
        let b = world.clans.get_mut(&5001).unwrap();
        b.ally_id = 5000;
        b.ally_name = "GoldenPact".into();
    }
    clans::handle_request_dismiss_ally(&mut world, 1);
    assert_eq!(world.clans[&5000].ally_id, 0);
    assert_eq!(world.clans[&5001].ally_id, 0);
    assert_eq!(
        world.clans[&5000].ally_penalty_type,
        model::clan::ALLY_PENALTY_TYPE_DISSOLVE_ALLY
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_ALLIANCE_HAS_BEEN_DISSOLVED)
    );
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_ALLIANCE_HAS_BEEN_DISSOLVED)
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_create_ally NewPact")),
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::CANNOT_CREATE_A_NEW_ALLIANCE_WITHIN_1_DAY_OF_DISSOLUTION
        )
    );
}

// --- G18 slice 6: sub-pledges & academy ---
