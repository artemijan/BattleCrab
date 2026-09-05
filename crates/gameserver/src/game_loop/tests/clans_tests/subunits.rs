//! The sub-pledges — academy, royal guard and knights: creating and joining
//! one, graduation and its reward, sponsors, rename and reorganise.

use super::*;

fn reorganize_body(member_name: &str, new_pledge_type: i32, selected_member: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(1); // isMemberSelected
    w.write_string(member_name);
    w.write_i32(new_pledge_type);
    w.write_string(selected_member);
    w.into_bytes()
}

fn create_academy_bypass(world: &mut World, client: u32, name: &str) {
    handle_request_bypass_to_server(
        world,
        client,
        &bypass_body(&format!("npc_{NPC_OID}_create_academy {name}")),
    );
}

fn create_royal_bypass(world: &mut World, client: u32, name: &str, leader: &str) {
    handle_request_bypass_to_server(
        world,
        client,
        &bypass_body(&format!("npc_{NPC_OID}_create_royal {name} {leader}")),
    );
}

fn create_knight_bypass(world: &mut World, client: u32, name: &str, leader: &str) {
    handle_request_bypass_to_server(
        world,
        client,
        &bypass_body(&format!("npc_{NPC_OID}_create_knight {name} {leader}")),
    );
}

/// Academy creation + the invite/accept flow: the level-5 gate, name clash
/// with an existing sub-unit, and a full accept — pledge type -1, power grade
/// 9, pledge class 1, and the roster count now honoring the real per-type cap.
#[test]
fn academy_create_and_join() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    drain_db(&mut db_rx);

    // Below level 5.
    create_academy_bypass(&mut world, 1, "YoungGuns");
    assert!(ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
        &server_packets::sm_ids::TO_ESTABLISH_A_CLAN_ACADEMY_YOUR_CLAN_MUST_BE_LEVEL_5_OR_HIGHER
    ));
    world.clans.get_mut(&5000).unwrap().level = 5;

    // Success.
    create_academy_bypass(&mut world, 1, "YoungGuns");
    assert!(
        world.clans[&5000]
            .sub_pledges
            .contains_key(&model::clan::SUBUNIT_ACADEMY)
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::CONGRATULATIONS_THE_S1_S_CLAN_ACADEMY_HAS_BEEN_CREATED
        )
    );
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        db::DbCommand::InsertSubPledge {
            clan_id: 5000,
            pledge_type: -1,
            ..
        }
    )));

    // A second academy is refused (already established).
    create_academy_bypass(&mut world, 1, "Another");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOUR_CLAN_HAS_ALREADY_ESTABLISHED_A_CLAN_ACADEMY)
    );

    // A royal-guard name clash with the academy.
    world.clans.get_mut(&5000).unwrap().level = 6;
    create_royal_bypass(&mut world, 1, "YoungGuns", "P3001");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::ANOTHER_MILITARY_UNIT_ALREADY_USES_THAT_NAME)
    );

    // Invite into the academy, accept: pledge type -1, power grade 9, pledge
    // class 1 (per the verified formula).
    clans::handle_request_join_pledge(
        &mut world,
        1,
        &invite_body(3003, model::clan::SUBUNIT_ACADEMY),
    );
    drain(&mut b_rx);
    clans::handle_request_answer_join_pledge(&mut world, 2, &answer_body(1));
    let b = world.objects.get_component::<Player>(&3003).unwrap();
    assert_eq!(
        (b.pledge_type, b.power_grade, b.pledge_class),
        (model::clan::SUBUNIT_ACADEMY, 9, 1)
    );
    assert_eq!(
        world.clans[&5000].member(3003).unwrap().pledge_type,
        model::clan::SUBUNIT_ACADEMY
    );
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateCharPledgeType {
            char_id: 3003,
            pledge_type: -1
        }
    )));

    // The academy's own 20-member cap is now real (not the main pledge's).
    assert_eq!(
        world.clans[&5000].sub_pledge_members_count(model::clan::SUBUNIT_ACADEMY),
        1
    );
}

/// Royal-guard and knight-order creation: leader-eligibility guard,
/// reputation cost, the family-full reject (2 royal slots), and the pledge
/// class a captain/member gets once joined.
#[test]
fn royal_and_knight_creation_and_pledge_class() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    let _c = ingame_player(&mut world, 3, 3005, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3003, 3005]);
    world.clans.get_mut(&5000).unwrap().level = 7;
    drain_db(&mut db_rx);

    // Insufficient reputation.
    create_royal_bypass(&mut world, 1, "Vanguard", "P3003");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_CLAN_REPUTATION_IS_TOO_LOW)
    );

    world.clans.get_mut(&5000).unwrap().reputation_score = 20_000;
    // Unknown / ineligible leader name.
    create_royal_bypass(&mut world, 1, "Vanguard", "Nobody");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_ROYAL_GUARD_CAPTAIN_CANNOT_BE_APPOINTED)
    );

    // Success: reputation spent, captain wired.
    create_royal_bypass(&mut world, 1, "Vanguard", "P3003");
    assert_eq!(world.clans[&5000].reputation_score, 15_000);
    assert_eq!(world.clans[&5000].sub_pledges[&100].leader_id, 3003);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3003)
            .unwrap()
            .pledge_class,
        6,
        "royal captain at level 7"
    );
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateClanReputation {
            clan_id: 5000,
            reputation: 15_000
        }
    )));

    // A second royal unit succeeds (2 slots), a third is refused (family full).
    create_royal_bypass(&mut world, 1, "Rearguard", "P3005");
    assert_eq!(world.clans[&5000].reputation_score, 10_000);
    drain(&mut a_rx);
    create_royal_bypass(&mut world, 1, "Thirdguard", "P3001");
    let a_sms = ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE);
    assert!(
        a_sms.contains(&server_packets::sm_ids::S1_TEXT),
        "family-full plain message"
    );
    assert_eq!(
        world.clans[&5000].reputation_score, 10_000,
        "no charge on the refused attempt"
    );

    // Knight unit: captain must not already lead the royal unit.
    create_knight_bypass(&mut world, 1, "Blades", "P3003");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::THE_CAPTAIN_OF_THE_ORDER_OF_KNIGHTS_CANNOT_BE_APPOINTED
        )
    );
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 40;
    create_knight_bypass(&mut world, 1, "Blades", "P3001");
    // P3001 is the clan leader — Java's `_leader.getObjectId() == leaderId` reject.
    assert!(!world.clans[&5000].sub_pledges.contains_key(&1001));
}

/// Rename, leader assignment, and reorganize between units; a departing
/// captain vacates their unit's leader slot.
#[test]
fn rename_assign_reorganize_and_vacate_on_leave() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    let _c = ingame_player(&mut world, 3, 3005, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3003, 3005]);
    world.clans.get_mut(&5000).unwrap().level = 7;
    world.clans.get_mut(&5000).unwrap().reputation_score = 20_000;
    create_royal_bypass(&mut world, 1, "Vanguard", "P3003");
    drain(&mut a_rx);
    drain_db(&mut db_rx);

    // Rename.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_rename_pledge 100 Sentinels")),
    );
    assert_eq!(world.clans[&5000].sub_pledges[&100].name, "Sentinels");
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateSubPledge {
            clan_id: 5000,
            pledge_type: 100,
            ..
        }
    )));

    // Assign a new captain (P3003 steps down for P3005).
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_assign_subpl_leader Sentinels P3005"
        )),
    );
    assert_eq!(world.clans[&5000].sub_pledges[&100].leader_id, 3005);
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::C1_HAS_BEEN_SELECTED_AS_THE_CAPTAIN_OF_S2)
    );

    // Reorganize: P3003 (main pledge) trades places with the royal unit's
    // plain roster slot — captaincy (a separate `leader_id` lookup) stays with
    // P3005 regardless of P3005's own `pledge_type`, matching Java where a
    // captain's `pledge_type` never changes just because they lead a unit.
    clans::handle_request_pledge_reorganize_member(
        &mut world,
        1,
        &reorganize_body("P3003", 100, "P3005"),
    );
    assert_eq!(world.clans[&5000].member(3003).unwrap().pledge_type, 100);
    assert_eq!(world.clans[&5000].member(3005).unwrap().pledge_type, 0);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3003)
            .unwrap()
            .pledge_type,
        100
    );
    assert_eq!(
        world.clans[&5000].sub_pledges[&100].leader_id, 3005,
        "captaincy untouched by the roster swap"
    );

    // The captain (P3005) leaves the clan — Java clears the vacated
    // captaincy on departure.
    clans::handle_request_withdrawal_pledge(&mut world, 3);
    assert_eq!(
        world.clans[&5000].sub_pledges[&100].leader_id, 0,
        "captaincy vacated on departure"
    );
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateSubPledge {
            clan_id: 5000,
            pledge_type: 100,
            leader_id: 0,
            ..
        }
    )));
}

// --- G18 slice 7: crests ---

/// Put `oid` into clan 5000's academy at `joined_level`, as a real accept would.
fn put_in_academy(world: &mut World, oid: i32, joined_level: i32) {
    if let Some(c) = world.clans.get_mut(&5000)
        && let Some(m) = c.members.iter_mut().find(|m| m.char_id == oid)
    {
        m.pledge_type = model::clan::SUBUNIT_ACADEMY;
        m.power_grade = 9;
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
        p.pledge_type = model::clan::SUBUNIT_ACADEMY;
        p.power_grade = 9;
        p.lvl_joined_academy = joined_level;
    }
}

/// **Joining the academy stamps the level the reward is computed from.** It is
/// the *joining* level, not the graduating one, so it has to be captured here
/// or the reward is unknowable later.
#[test]
fn joining_the_academy_records_the_joining_level() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    world.clans.get_mut(&5000).unwrap().level = 5;
    create_academy_bypass(&mut world, 1, "YoungGuns");
    world
        .objects
        .get_component_mut::<Player>(&3003)
        .unwrap()
        .level = 23;
    drain(&mut a_rx);
    drain_db(&mut db_rx);

    clans::handle_request_join_pledge(
        &mut world,
        1,
        &invite_body(3003, model::clan::SUBUNIT_ACADEMY),
    );
    drain(&mut b_rx);
    clans::handle_request_answer_join_pledge(&mut world, 2, &answer_body(1));

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3003)
            .unwrap()
            .lvl_joined_academy,
        23,
        "the level at join time is what the graduation reward reads"
    );
    assert!(
        drain_db(&mut db_rx).iter().any(|c| matches!(
            c,
            db::DbCommand::UpdateCharAcademyLevel {
                char_id: 3003,
                lvl_joined_academy: 23
            }
        )),
        "and it is persisted — it outlives the session"
    );

    // A main-pledge recruit is not an academy member.
    let mut c_rx = ingame_player(&mut world, 3, 3004, 0, 0, 0);
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3004, 0));
    drain(&mut c_rx);
    clans::handle_request_answer_join_pledge(&mut world, 3, &answer_body(1));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3004)
            .unwrap()
            .lvl_joined_academy,
        0
    );
}

/// **Graduation is the whole point of the academy**: the 2nd class transfer
/// pays the clan on a sliding scale, expels the graduate with *no* rejoin
/// penalty, and hands over the circlet.
#[test]
fn graduating_pays_the_clan_and_frees_the_graduate() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    world.data.item_data = dist::items_owned();
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[2]);
    world.id_pool = 0x4300_0000..0x4300_0100;
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3003]);
    put_in_academy(&mut world, 3003, 20);
    let before = world.clans[&5000].reputation_score;
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain_db(&mut db_rx);

    // Class 2 (gladiator) is in THIRD_CLASS_GROUP — Java's name for the
    // *2nd*-transfer classes.
    assert!(subclass::set_class_id(&mut world, 3003, 2));

    // 650 - (20 - 16) * 20 = 570.
    assert_eq!(
        world.clans[&5000].reputation_score - before,
        570,
        "the sliding reward scales off the joining level"
    );
    assert!(
        world.clans[&5000].member(3003).is_none(),
        "the graduate is expelled"
    );
    let p = world.objects.get_component::<Player>(&3003).unwrap();
    assert_eq!(p.clan_id, 0);
    assert_eq!(p.lvl_joined_academy, 0, "no longer an academy member");
    assert_eq!(
        p.clan_join_expiry_time, 0,
        "and free to join another clan at once — the other half of the reward"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3003)
            .unwrap()
            .count_of(8181),
        1,
        "the Clan Academy Circlet"
    );
    let sms = ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE);
    assert!(sms.contains(
        &server_packets::sm_ids::CONGRATULATIONS_YOU_WILL_NOW_GRADUATE_FROM_THE_CLAN_ACADEMY
    ));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CLAN_MEMBER_S1_HAS_BEEN_EXPELLED),
        "the clan is told"
    );
}

/// The reward brackets, at their edges. Java's three arms are not a clamp —
/// the middle formula would give a different number at both ends.
#[test]
fn the_graduation_reward_brackets_are_javas() {
    for (joined, expected) in [
        (10, 650),
        (16, 650),
        (17, 630),
        (38, 210),
        (39, 190),
        (45, 190),
    ] {
        let (mut world, ..) = quest_test_world();
        world
            .data
            .categories
            .insert_for_test("THIRD_CLASS_GROUP", &[2]);
        let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        let _rx2 = ingame_player(&mut world, 2, 3003, 0, 0, 0);
        install_clan(&mut world, 5000, &[3001, 3003]);
        put_in_academy(&mut world, 3003, joined);
        let before = world.clans[&5000].reputation_score;

        academy::graduate(&mut world, 3003);

        assert_eq!(
            world.clans[&5000].reputation_score - before,
            expected,
            "joined at {joined} → {expected} reputation"
        );
    }
}

/// **An academy member is a second-class citizen**: they cannot be re-ranked,
/// cannot be nominated leader, and their kills don't count for a clan war.
#[test]
fn academy_members_are_barred_from_rank_and_leadership() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3003]);
    put_in_academy(&mut world, 3003, 20);
    drain(&mut a_rx);
    drain_db(&mut db_rx);

    // Re-rank: refused with SM 1754, and the grade does not move.
    let mut w = PacketWriter::new();
    w.write_string("P3003");
    w.write_i32(6);
    clans::handle_request_pledge_set_member_power_grade(&mut world, 1, &w.into_bytes());
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::THAT_PRIVILEGE_CANNOT_BE_GRANTED_TO_A_CLAN_ACADEMY_MEMBER
        )
    );
    assert_eq!(
        world.clans[&5000].member(3003).unwrap().power_grade,
        9,
        "still rank 9"
    );

    // A full member *can* be re-ranked — the refusal is about the academy, not
    // about the handler being broken.
    let mut w = PacketWriter::new();
    w.write_string("P3001");
    w.write_i32(6);
    clans::handle_request_pledge_set_member_power_grade(&mut world, 1, &w.into_bytes());
    drain(&mut a_rx);
    assert_ne!(world.clans[&5000].member(3001).unwrap().power_grade, 9);
}

/// Sponsor pairing: the packet names the two players in either order and the
/// academy side becomes the apprentice. Both rows are written immediately.
#[test]
fn a_sponsor_can_be_paired_and_unpaired() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3003]);
    put_in_academy(&mut world, 3003, 20);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain_db(&mut db_rx);

    // Deliberately "wrong" order: the sponsor first, the apprentice second.
    let pair = |set: i32| {
        let mut w = PacketWriter::new();
        w.write_i32(set);
        w.write_string("P3001");
        w.write_string("P3003");
        w.into_bytes()
    };
    academy::handle_set_academy_master(&mut world, 1, &pair(1));

    let sponsor = world.objects.get_component::<Player>(&3001).unwrap();
    let apprentice = world.objects.get_component::<Player>(&3003).unwrap();
    assert_eq!(
        (sponsor.apprentice, sponsor.sponsor),
        (3003, 0),
        "the non-academy side sponsors"
    );
    assert_eq!(
        (apprentice.apprentice, apprentice.sponsor),
        (0, 3001),
        "…and the academy side is the apprentice, whichever order they arrived in"
    );
    let cmds = drain_db(&mut db_rx);
    assert_eq!(
        cmds.iter()
            .filter(|c| matches!(c, db::DbCommand::UpdateCharApprenticeSponsor { .. }))
            .count(),
        2,
        "both rows are written at once — Java: 'both must match'"
    );
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::S2_HAS_BEEN_DESIGNATED_AS_THE_APPRENTICE_OF_CLAN_MEMBER_S1
        )
    );

    // A second pairing is refused while the first stands.
    drain(&mut a_rx);
    academy::handle_set_academy_master(&mut world, 1, &pair(1));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .apprentice,
        3003,
        "unchanged"
    );

    // Unpair.
    academy::handle_set_academy_master(&mut world, 1, &pair(0));
    let sponsor = world.objects.get_component::<Player>(&3001).unwrap();
    let apprentice = world.objects.get_component::<Player>(&3003).unwrap();
    assert_eq!((sponsor.apprentice, sponsor.sponsor), (0, 0));
    assert_eq!((apprentice.apprentice, apprentice.sponsor), (0, 0));
}

/// Leaving the clan clears the academy trio — otherwise a graduate's next clan
/// would inherit a phantom sponsor, and `isAcademyMember()` would stay true.
#[test]
fn leaving_the_clan_clears_the_academy_state() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let _a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3003]);
    put_in_academy(&mut world, 3003, 20);
    let mut w = PacketWriter::new();
    w.write_i32(1);
    w.write_string("P3001");
    w.write_string("P3003");
    academy::handle_set_academy_master(&mut world, 1, &w.into_bytes());
    drain(&mut b_rx);
    drain_db(&mut db_rx);

    clans::handle_request_withdrawal_pledge(&mut world, 2);

    let p = world.objects.get_component::<Player>(&3003).unwrap();
    assert_eq!(p.lvl_joined_academy, 0, "no longer an academy member");
    assert_eq!((p.apprentice, p.sponsor), (0, 0), "mentorship broken");
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .apprentice,
        0,
        "…on the sponsor's side too"
    );
}
