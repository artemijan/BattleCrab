//! Clan war: declaring it, making it mutual, the kills that drive its state
//! and reputation, and surrender or timeout.

use super::*;

/// War declaration: the guard chain, the BLOOD_DECLARATION creation with both
/// sides' SMs + persistence, the counter-declaration turning it MUTUAL, and
/// the PvP consequences (lawful kills, war relation bits, free attackability).
#[test]
fn clan_war_declare_and_mutual() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    install_clan(&mut world, 5001, &[3003]);
    drain_db(&mut db_rx);

    // Below level 3 / 15 members.
    clans::handle_request_start_pledge_war(&mut world, 1, &name_body("Clan5001"));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CLAN_WAR_NEEDS_LEVEL_3_AND_15_MEMBERS)
    );

    for id in [5000, 5001] {
        world.clans.get_mut(&id).unwrap().level = 3;
        pad_clan(&mut world, id, 15);
    }

    // Unknown target clan.
    clans::handle_request_start_pledge_war(&mut world, 1, &name_body("Ghosts"));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CLAN_WAR_TARGET_DOES_NOT_EXIST)
    );

    // Own clan.
    clans::handle_request_start_pledge_war(&mut world, 1, &name_body("Clan5000"));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::FOOL_YOU_CANNOT_DECLARE_WAR_AGAINST_YOUR_OWN_CLAN)
    );

    // Dissolving target.
    world.clans.get_mut(&5001).unwrap().dissolving_expiry_time = i64::MAX;
    clans::handle_request_start_pledge_war(&mut world, 1, &name_body("Clan5001"));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CANNOT_DECLARE_WAR_ON_DISSOLVING_CLAN)
    );
    world.clans.get_mut(&5001).unwrap().dissolving_expiry_time = 0;

    // Declare: a BLOOD_DECLARATION war with both sides' messages.
    clans::handle_request_start_pledge_war(&mut world, 1, &name_body("Clan5001"));
    let war = clans::war_between(&world, 5000, 5001).expect("war created");
    assert_eq!(war.state, model::clan::ClanWarState::BloodDeclaration);
    assert_eq!(war.attacker_id, 5000);
    let a_pkts = drain(&mut a_rx);
    assert!(
        ids_after_opcode(&a_pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_DECLARED_A_CLAN_WAR_WITH_S1)
    );
    assert!(a_pkts.iter().any(|p| is_ex(p, 0x40)), "war list sent");
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_HAS_DECLARED_A_CLAN_WAR_KILL_5_TO_START)
    );
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        db::DbCommand::SaveClanWar {
            attacker: 5000,
            attacked: 5001,
            state: 1,
            ..
        }
    )));

    // One-sided war: not yet lawful PvP, single sword only for the attacker.
    assert!(!clans::mutual_war_between(&world, 5000, 5001));
    assert_eq!(
        clans::war_relation_bits(&world, 3001, 3003),
        0x4000,
        "attacker shows the declared sword"
    );
    assert_eq!(
        clans::war_relation_bits(&world, 3003, 3001),
        0,
        "attacked side shows nothing"
    );

    // The attacked side declares back → MUTUAL.
    clans::handle_request_start_pledge_war(&mut world, 2, &name_body("Clan5000"));
    let war = clans::war_between(&world, 5000, 5001).expect("war kept");
    assert_eq!(war.state, model::clan::ClanWarState::Mutual);
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CLAN_WAR_STARTED_WITH_CLAN_S1)
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CLAN_WAR_STARTED_WITH_CLAN_S1)
    );

    // Mutual: lawful PvP, both swords, freely attackable, dissolve blocked.
    assert!(clans::mutual_war_between(&world, 5000, 5001));
    assert!(pvp::check_if_pvp(&world, 3001, 3003));
    assert!(pvp::is_player_auto_attackable(&world, 3001, 3003));
    assert_eq!(clans::war_relation_bits(&world, 3001, 3003), 0xC000);
    assert_eq!(clans::war_relation_bits(&world, 3003, 3001), 0xC000);

    // Redeclaring against a mutual war is refused (plain-text SM 1983).
    clans::handle_request_start_pledge_war(&mut world, 1, &name_body("Clan5001"));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_TEXT)
    );
}

/// The kill pipeline: five attacked-side kills force a blood declaration
/// MUTUAL; mutual-war kills move one reputation point per kill.
#[test]
fn clan_war_kills_drive_state_and_reputation() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0); // attacker clan member
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0); // attacked clan member
    install_clan(&mut world, 5000, &[3001]);
    install_clan(&mut world, 5001, &[3003]);
    world.clan_wars.push(model::clan::ClanWar {
        attacker_id: 5000,
        attacked_id: 5001,
        state: model::clan::ClanWarState::BloodDeclaration,
        winner_id: 0,
        start_time: 1,
        end_time: 0,
        attacker_kills: 0,
        attacked_kills: 0,
    });
    drain_db(&mut db_rx);

    // Four kills of the declaring side: progress messages only.
    for _ in 0..4 {
        clans::clan_war_on_kill(&mut world, 3003, 3001);
    }
    let war = clans::war_between(&world, 5000, 5001).unwrap();
    assert_eq!(
        (war.state, war.attacked_kills),
        (model::clan::ClanWarState::BloodDeclaration, 4)
    );
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_MEMBER_KILLED_S2_MORE_KILLS_TO_START_WAR)
    );

    // The fifth kill forces MUTUAL.
    clans::clan_war_on_kill(&mut world, 3003, 3001);
    assert_eq!(
        clans::war_between(&world, 5000, 5001).unwrap().state,
        model::clan::ClanWarState::Mutual
    );
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CLAN_WAR_STARTED_WITH_CLAN_S1)
    );
    drain(&mut a_rx);

    // A mutual-war kill moves 1 reputation from the victim's clan (level > 4).
    world.clans.get_mut(&5000).unwrap().reputation_score = 10;
    world.clans.get_mut(&5001).unwrap().reputation_score = 20;
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 40;
    drain_db(&mut db_rx);
    clans::clan_war_on_kill(&mut world, 3003, 3001);
    assert_eq!(
        world.clans[&5000].reputation_score, 9,
        "victim clan loses 1"
    );
    assert_eq!(
        world.clans[&5001].reputation_score, 21,
        "killer clan gains 1"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::SaveClanWar { .. }))
    );

    // A victim clan at 0 reputation yields nothing.
    world.clans.get_mut(&5000).unwrap().reputation_score = 0;
    clans::clan_war_on_kill(&mut world, 3003, 3001);
    assert_eq!(world.clans[&5000].reputation_score, 0);
    assert_eq!(world.clans[&5001].reputation_score, 21);
}

/// Cease-fire, surrender, and the 7-day timeout; a war also blocks dissolve.
#[test]
fn clan_war_stop_surrender_timeout() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    install_clan(&mut world, 5001, &[3003]);
    let mutual_war = || model::clan::ClanWar {
        attacker_id: 5000,
        attacked_id: 5001,
        state: model::clan::ClanWarState::Mutual,
        winner_id: 0,
        start_time: 1,
        end_time: 0,
        attacker_kills: 0,
        attacked_kills: 0,
    };
    world.clan_wars.push(mutual_war());
    drain_db(&mut db_rx);

    // An active war blocks dissolution.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_dissolve_clan")),
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CANNOT_DISSOLVE_A_CLAN_WHILE_ENGAGED_IN_A_WAR)
    );

    // Stop: too little reputation first, then a successful cease-fire.
    clans::handle_request_stop_pledge_war(&mut world, 1, &name_body("Clan5001"));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_CLAN_REPUTATION_IS_TOO_LOW)
    );
    world.clans.get_mut(&5000).unwrap().reputation_score = 1_000;
    clans::handle_request_stop_pledge_war(&mut world, 1, &name_body("Clan5001"));
    assert!(
        clans::war_between(&world, 5000, 5001).is_none(),
        "war deleted"
    );
    assert_eq!(
        world.clans[&5000].reputation_score, 500,
        "500 reputation paid"
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::YOUR_CLAN_LOST_500_REPUTATION_FOR_WITHDRAWING_FROM_THE_WAR
        )
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::DeleteClanWar { .. }))
    );

    // Surrender: refused during BLOOD_DECLARATION, accepted in MUTUAL.
    let mut blood = mutual_war();
    blood.state = model::clan::ClanWarState::BloodDeclaration;
    world.clan_wars.push(blood);
    clans::handle_request_surrender_pledge_war(&mut world, 1, &name_body("Clan5001"));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CANNOT_DECLARE_DEFEAT_BEFORE_7_DAYS_WITH_CLAN_S1)
    );
    world.clan_wars.last_mut().unwrap().state = model::clan::ClanWarState::Mutual;
    world.clans.get_mut(&5000).unwrap().reputation_score = 1_000;
    clans::handle_request_surrender_pledge_war(&mut world, 1, &name_body("Clan5001"));
    let war = clans::war_between(&world, 5000, 5001).expect("kept until the delete task");
    assert_eq!(war.winner_id, 5001);
    assert!(war.end_time > 0);
    assert_eq!(world.clans[&5000].reputation_score, 500);
    let a_pkts = drain(&mut a_rx);
    assert!(
        a_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SURRENDER_PLEDGE_WAR)
    );
    assert!(
        ids_after_opcode(&a_pkts, server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::THE_WAR_ENDED_BY_YOUR_DEFEAT_DECLARATION_WITH_THE_S1_CLAN
        )
    );
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_WAR_ENDED_BY_THE_S1_CLAN_S_DEFEAT_DECLARATION)
    );
    // The WIN state blocks a redeclaration by the loser… er, by the winner side
    // (Java: state_for == WIN → the 21-day message).
    world.clans.get_mut(&5001).unwrap().level = 3;
    world.clans.get_mut(&5000).unwrap().level = 3;
    pad_clan(&mut world, 5000, 15);
    pad_clan(&mut world, 5001, 15);
    clans::handle_request_start_pledge_war(&mut world, 2, &name_body("Clan5000"));
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CANNOT_DECLARE_WAR_21_DAYS_AFTER_DEFEAT_WITH_S1)
    );
    // The delete task tears it down.
    clans::delete_clan_wars(&mut world, 5000, 5001);
    assert!(clans::war_between(&world, 5000, 5001).is_none());

    // Timeout: a blood declaration goes TIE; mutual is untouched.
    let mut blood = mutual_war();
    blood.state = model::clan::ClanWarState::BloodDeclaration;
    world.clan_wars.push(blood);
    clans::handle_clan_war_timeout(&mut world, 5000, 5001);
    let war = clans::war_between(&world, 5000, 5001).unwrap();
    assert_eq!(war.state, model::clan::ClanWarState::Tie);
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::BECAUSE_CLAN_S1_DID_NOT_FIGHT_BACK_THE_WAR_WAS_CANCELLED
        )
    );
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::A_CLAN_WAR_DECLARED_BY_CLAN_S1_WAS_CANCELLED)
    );
    world.clan_wars.clear();
    world.clan_wars.push(mutual_war());
    clans::handle_clan_war_timeout(&mut world, 5000, 5001);
    assert_eq!(
        clans::war_between(&world, 5000, 5001).unwrap().state,
        model::clan::ClanWarState::Mutual
    );
}

// --- G18 slice 5: alliances ---
