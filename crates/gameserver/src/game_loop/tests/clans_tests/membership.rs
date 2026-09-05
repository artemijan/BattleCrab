//! Joining and leaving a clan: creating one, the clan master dialog, the
//! roster and its chat, invite/withdraw/oust, dissolve and recover, the
//! delegated leader transfer, and what leaving takes with it.

use super::*;

/// The `create_clan` bypass: Java's guard matrix (SM ids in `ClanTable.
/// createClan` order), then the success path — clan registered + persisted,
/// leader flags/privileges set, the pledge-window packet trio + SM 189, and
/// duplicate-name/already-in-clan rejects afterward.
#[test]
fn clan_create_guards_and_success() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);

    let create = |world: &mut World, client: u32, name: &str| {
        handle_request_bypass_to_server(
            world,
            client,
            &bypass_body(&format!("npc_{NPC_OID}_create_clan {name}")),
        );
    };

    // Level < 10.
    create(&mut world, 1, "Myclan");
    let pkts = drain(&mut a_rx);
    assert!(
        ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::YOU_DO_NOT_MEET_THE_CRITERIA_IN_ORDER_TO_CREATE_A_CLAN
        )
    );

    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 10;

    // Name with a space arrives as two tokens → invalid.
    create(&mut world, 1, "My clan");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CLAN_NAME_IS_INVALID)
    );
    // Non-alphanumeric.
    create(&mut world, 1, "Cl@n");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CLAN_NAME_IS_INVALID)
    );
    // Too short / too long.
    create(&mut world, 1, "C");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CLAN_NAME_IS_INVALID)
    );
    create(&mut world, 1, "Averyveryverylongclanname");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CLAN_NAME_S_LENGTH_IS_INCORRECT)
    );
    // Recreate cooldown.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_create_expiry_time = i64::MAX;
    create(&mut world, 1, "Myclan");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_MUST_WAIT_10_DAYS_BEFORE_CREATING_A_NEW_CLAN)
    );
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_create_expiry_time = 0;

    // Success.
    world.id_pool = 0x3000_0000..0x3000_0100;
    drain_db(&mut db_rx);
    create(&mut world, 1, "Myclan");
    let pkts = drain(&mut a_rx);
    let p = world.objects.get_component::<Player>(&3001).unwrap();
    let clan_id = p.clan_id;
    assert_ne!(clan_id, 0);
    assert!(p.clan_leader);
    assert_eq!(p.clan_privs, model::clan::ALL_CLAN_PRIVILEGES);
    let clan = &world.clans[&clan_id];
    assert_eq!(
        (clan.name.as_str(), clan.leader_id, clan.members.len()),
        ("Myclan", 3001, 1)
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_INFO_UPDATE)
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_ALL)
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_UPDATE)
    );
    assert!(
        ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOUR_CLAN_HAS_BEEN_CREATED)
    );
    assert!(
        pkts.iter().any(|p| p[0] == 0x32),
        "fresh UserInfo with the clan id"
    );
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(
        |c| matches!(c, db::DbCommand::InsertClan { name, leader_id: 3001, .. } if name == "Myclan")
    ));
    assert!(cmds.iter().any(
        |c| matches!(c, db::DbCommand::UpdateCharClan { char_id: 3001, clan_privs, .. }
        if *clan_privs == model::clan::ALL_CLAN_PRIVILEGES)
    ));

    // Already in a clan.
    create(&mut world, 1, "Another");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_FAILED_TO_CREATE_A_CLAN)
    );

    // Second player: the name is taken (case-insensitive).
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .level = 10;
    create(&mut world, 2, "MYCLAN");
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_ALREADY_EXISTS)
    );
}

/// ClanMaster dialog navigation: `Quest ClanMaster <page>` events render
/// the page (bare bypass resolved through `LastFolkNpc`), with the
/// leader-required remap for non-leaders.
#[test]
fn clan_master_dialog_gates_on_leadership() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    // Click the NPC so LastFolkNpc resolves the bare Quest bypasses.
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    drain(&mut rx);

    let root = world.data.root.clone();
    let page = |name: &str| {
        // The server serves htm through the cache, which strips comments and
        // tabs/newlines exactly as Java's `HtmCache.loadFile` does — so the
        // expectation has to go through the same transform, not the raw file.
        let raw = std::fs::read_to_string(format!(
            "{root}data/scripts/village_master/ClanMaster/{name}"
        ))
        .expect(name);
        crate::data::htm_cache::strip_htm(&raw).replace("%objectId%", &NPC_OID.to_string())
    };

    // Talk → the root menu (ClanMaster id -1 ⇒ plain NpcHtmlMessage).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest ClanMaster"));
    let pkts = drain(&mut rx);
    let html = pkts
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("root menu html");
    assert_eq!(html, page("9000-01.htm"));

    // Leader-gated page as a non-leader → the -no variant.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest ClanMaster 9000-03.htm"));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("gated html");
    assert_eq!(html, page("9000-03-no.htm"));

    // As a leader → the real page.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_leader = true;
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest ClanMaster 9000-03.htm"));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("leader html");
    assert_eq!(html, page("9000-03.htm"));
}

/// Clan roster notifications + clan chat: enter-world sends the pledge
/// window to the member and the online ping to the rest; clan chat reaches
/// every online member; leaving pings offline; the clanless get SM 4202.
#[test]
fn clan_roster_notifications_and_chat() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);

    // A clan with A (leader, online) and B — installed directly; invites
    // are deferred past G11.
    let clan_id = 5000;
    let member = |char_id: i32, name: &str| model::clan::ClanMember {
        char_id,
        name: name.into(),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    };
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "Testers".into(),
            leader_id: 3001,
            level: 0,
            reputation_score: 0,
            castle_id: 0,
            members: vec![member(3001, "P3001"), member(3002, "P3002")],
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    for oid in [3001, 3002] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .clan_id = clan_id;
    }

    // B "enters world": pledge window to B, online ping to A.
    clans::on_enter_world(&mut world, 2, 3002);
    let b_pkts = drain(&mut b_rx);
    assert!(
        b_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_ALL)
    );
    let a_pkts = drain(&mut a_rx);
    let upd = a_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_UPDATE)
        .expect("online ping to A");
    let mut r = commons::network::PacketReader::new(&upd[1..]);
    assert_eq!(r.read_string().unwrap(), "P3002");

    // Clan chat from A reaches both.
    chat::handle_say2(
        &mut world,
        1,
        &say2_body("hail", crate::enums::ChatType::Clan.client_id(), None),
    );
    assert!(
        drain(&mut a_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SAY2)
    );
    assert!(
        drain(&mut b_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SAY2)
    );

    // A clanless player gets SM 4202.
    let mut c_rx = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    chat::handle_say2(
        &mut world,
        3,
        &say2_body("hail", crate::enums::ChatType::Clan.client_id(), None),
    );
    assert!(
        ids_after_opcode(&drain(&mut c_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_ARE_NOT_IN_A_CLAN)
    );

    // B leaves the world: offline ping to A.
    store_and_remove_player(&mut world, 3002);
    let a_pkts = drain(&mut a_rx);
    let upd = a_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_UPDATE)
        .expect("offline ping to A");
    // Online-status byte is the packet tail.
    assert_eq!(*upd.last().unwrap(), 0, "offline");
}

fn oust_body(name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(name);
    w.into_bytes()
}

/// Invite guards (Java `checkClanJoinCondition` order), decline, and the full
/// accept burst: JoinPledge + roster + info/count broadcasts + SMs + persistence.
#[test]
fn clan_invite_guards_decline_and_accept() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    drain_db(&mut db_rx);
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Wrong target: the object id is not an online player.
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(9999, 0));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_INVITED_THE_WRONG_TARGET)
    );

    // Self-invite.
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3001, 0));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CANNOT_ASK_YOURSELF_TO_APPLY_TO_A_CLAN)
    );

    // No CL_JOIN_CLAN privilege: a plain member inviting.
    let mut c_rx = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    install_clan(&mut world, 5001, &[3004, 3003]); // 3003 is a plain member of another clan
    clans::handle_request_join_pledge(&mut world, 3, &invite_body(3002, 0));
    assert!(
        ids_after_opcode(&drain(&mut c_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT)
    );

    // Target already clanned.
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3003, 0));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_IS_ALREADY_A_MEMBER_OF_ANOTHER_CLAN)
    );

    // Target under the rejoin penalty.
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .clan_join_expiry_time = i64::MAX;
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3002, 0));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::C1_CANNOT_JOIN_THE_CLAN_ONE_DAY_HAS_NOT_PASSED_SINCE_LEAVING
        )
    );
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .clan_join_expiry_time = 0;

    // Clan under the post-oust penalty.
    world.clans.get_mut(&5000).unwrap().char_penalty_expiry_time = i64::MAX;
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3002, 0));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::AFTER_A_CLAN_MEMBER_IS_DISMISSED_THE_CLAN_MUST_WAIT_A_DAY
        )
    );
    world.clans.get_mut(&5000).unwrap().char_penalty_expiry_time = 0;

    // Clan full (level 1 main pledge caps at 15).
    for i in 0..14 {
        let cm = model::clan::ClanMember {
            char_id: 8000 + i,
            name: format!("F{i}"),
            level: 1,
            class_id: 0,
            sex: 0,
            race: 0,
            power_grade: 5,
            title: String::new(),
            pledge_type: 0,
            apprentice: 0,
            sponsor: 0,
        };
        world.clans.get_mut(&5000).unwrap().members.push(cm);
    }
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3002, 0));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::S1_IS_FULL_AND_CANNOT_ACCEPT_ADDITIONAL_CLAN_MEMBERS
        )
    );
    world
        .clans
        .get_mut(&5000)
        .unwrap()
        .members
        .retain(|m| m.char_id < 8000);

    // Valid invite → AskJoinPledge on B, the request slot armed on both sides.
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3002, 0));
    let pkts = drain(&mut b_rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::ASK_JOIN_PLEDGE)
    );
    assert!(world.objects.has_component::<PendingRequest>(&3001));
    assert!(world.objects.has_component::<PendingRequest>(&3002));

    // A second invite while the slot is busy → "on another task".
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3002, 0));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::C1_IS_ON_ANOTHER_TASK_PLEASE_TRY_AGAIN_LATER)
    );

    // Decline: both sides notified, slots freed.
    clans::handle_request_answer_join_pledge(&mut world, 2, &answer_body(0));
    assert!(ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
        &server_packets::sm_ids::YOU_DIDN_T_RESPOND_TO_S1_S_INVITATION_JOINING_HAS_BEEN_CANCELLED
    ));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::S1_DID_NOT_RESPOND_INVITATION_TO_THE_CLAN_HAS_BEEN_CANCELLED
        )
    );
    assert!(!world.objects.has_component::<PendingRequest>(&3001));
    assert!(!world.objects.has_component::<PendingRequest>(&3002));

    // Accept: the join burst.
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .clan_join_expiry_time = 0;
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3002, 0));
    drain(&mut b_rx);
    drain_db(&mut db_rx);
    clans::handle_request_answer_join_pledge(&mut world, 2, &answer_body(1));

    let b = world.objects.get_component::<Player>(&3002).unwrap();
    assert_eq!((b.clan_id, b.clan_privs, b.clan_leader), (5000, 0, false));
    assert!(world.clans[&5000].member(3002).is_some());
    let b_pkts = drain(&mut b_rx);
    assert!(
        b_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::JOIN_PLEDGE)
    );
    assert!(
        b_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_ALL)
    );
    let b_sms = ids_after_opcode(&b_pkts, server_packets::opcodes::SYSTEM_MESSAGE);
    assert!(b_sms.contains(&server_packets::sm_ids::ENTERED_THE_CLAN));
    assert!(b_sms.contains(&server_packets::sm_ids::S1_HAS_JOINED_THE_CLAN));
    let a_pkts = drain(&mut a_rx);
    assert!(
        a_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_ADD)
    );
    assert!(
        a_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_INFO_UPDATE)
    );
    assert!(
        ids_after_opcode(&a_pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_HAS_JOINED_THE_CLAN)
    );
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateCharClan {
            char_id: 3002,
            clan_id: 5000,
            clan_privs: 0
        }
    )));
    assert!(cmds.iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateCharClanJoinExpiry {
            char_id: 3002,
            expiry: 0
        }
    )));
}

/// Withdraw and oust: the leader/combat/membership guards, the rejoin penalty
/// on the leaver, and the clan-side invite penalty on an oust.
#[test]
fn clan_withdraw_and_oust() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3002]);
    drain_db(&mut db_rx);

    // Clanless player withdrawing.
    let mut c_rx = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    clans::handle_request_withdrawal_pledge(&mut world, 3);
    assert!(
        ids_after_opcode(&drain(&mut c_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION
        )
    );

    // The leader cannot withdraw.
    clans::handle_request_withdrawal_pledge(&mut world, 1);
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::A_CLAN_LEADER_CANNOT_WITHDRAW_FROM_THEIR_OWN_CLAN)
    );

    // A member in combat cannot withdraw / be dismissed.
    combat::refresh_attack_stance(&mut world, 3002);
    clans::handle_request_withdrawal_pledge(&mut world, 2);
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CANNOT_LEAVE_A_CLAN_WHILE_ENGAGED_IN_COMBAT)
    );
    clans::handle_request_oust_pledge_member(&mut world, 1, &oust_body("P3002"));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::A_CLAN_MEMBER_MAY_NOT_BE_DISMISSED_DURING_COMBAT)
    );
    world.tick += 10_000; // combat stance expires

    // Self-oust.
    clans::handle_request_oust_pledge_member(&mut world, 1, &oust_body("P3001"));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CANNOT_DISMISS_YOURSELF)
    );

    // Withdraw: penalty stamped, roster shrinks, both sides messaged.
    clans::handle_request_withdrawal_pledge(&mut world, 2);
    let b = world.objects.get_component::<Player>(&3002).unwrap();
    assert_eq!(b.clan_id, 0);
    assert!(b.clan_join_expiry_time > 0, "rejoin penalty stamped");
    assert!(world.clans[&5000].member(3002).is_none());
    let b_pkts = drain(&mut b_rx);
    assert!(
        b_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_DELETE_ALL)
    );
    let b_sms = ids_after_opcode(&b_pkts, server_packets::opcodes::SYSTEM_MESSAGE);
    assert!(b_sms.contains(&server_packets::sm_ids::YOU_HAVE_WITHDRAWN_FROM_THE_CLAN));
    assert!(b_sms.contains(
        &server_packets::sm_ids::AFTER_LEAVING_A_CLAN_YOU_MUST_WAIT_A_DAY_BEFORE_JOINING_ANOTHER
    ));
    let a_pkts = drain(&mut a_rx);
    assert!(
        ids_after_opcode(&a_pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_HAS_WITHDRAWN_FROM_THE_CLAN)
    );
    assert!(
        a_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_DELETE)
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::RemoveClanMember { char_id: 3002, .. }))
    );

    // Oust an offline member: roster row goes, the clan takes the invite
    // penalty, the DB reset covers the offline character.
    world
        .clans
        .get_mut(&5000)
        .unwrap()
        .members
        .push(model::clan::ClanMember {
            char_id: 3005,
            name: "P3005".into(),
            level: 1,
            class_id: 0,
            sex: 0,
            race: 0,
            power_grade: 5,
            title: String::new(),
            pledge_type: 0,
            apprentice: 0,
            sponsor: 0,
        });
    clans::handle_request_oust_pledge_member(&mut world, 1, &oust_body("P3005"));
    assert!(world.clans[&5000].member(3005).is_none());
    assert!(
        world.clans[&5000].char_penalty_expiry_time > 0,
        "clan invite penalty stamped"
    );
    let a_sms = ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE);
    assert!(a_sms.contains(&server_packets::sm_ids::CLAN_MEMBER_S1_HAS_BEEN_EXPELLED));
    assert!(
        a_sms.contains(&server_packets::sm_ids::YOU_HAVE_SUCCEEDED_IN_EXPELLING_THE_CLAN_MEMBER)
    );
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, db::DbCommand::RemoveClanMember { char_id: 3005, .. }))
    );
    assert!(
        cmds.iter()
            .any(|c| matches!(c, db::DbCommand::UpdateClanPenalties { clan_id: 5000, .. }))
    );
}

/// Village-master dissolve/recover: the leader gate, the 7-day stamp, the
/// double-request reject, recover zeroing the stamp (the scheduled task then
/// no-ops), and the actual destruction when the stamp comes due.
#[test]
fn clan_dissolve_and_recover() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3002]);
    drain_db(&mut db_rx);

    let bypass = |world: &mut World, client: u32, verb: &str| {
        handle_request_bypass_to_server(
            world,
            client,
            &bypass_body(&format!("npc_{NPC_OID}_{verb}")),
        );
    };

    // A non-leader asking for dissolution.
    bypass(&mut world, 2, "dissolve_clan");
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT)
    );

    // The leader: stamp + persistence + the scheduled removal armed.
    bypass(&mut world, 1, "dissolve_clan");
    assert!(world.clans[&5000].dissolving_expiry_time > 0);
    assert!(drain_db(&mut db_rx)
        .iter()
        .any(|c| matches!(c, db::DbCommand::UpdateClanPenalties { clan_id: 5000, dissolving_expiry_time, .. }
            if *dissolving_expiry_time > 0)));

    // Asking again while pending.
    bypass(&mut world, 1, "dissolve_clan");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::YOU_HAVE_ALREADY_REQUESTED_THE_DISSOLUTION_OF_YOUR_CLAN
        )
    );

    // Recover: the stamp is zeroed and a firing task no-ops.
    bypass(&mut world, 1, "recover_clan");
    assert_eq!(world.clans[&5000].dissolving_expiry_time, 0);
    clans::handle_clan_dissolve_task(&mut world, 5000);
    assert!(
        world.clans.contains_key(&5000),
        "recovered clan survives the stale task"
    );

    // A due stamp destroys the clan (members reset, windows closed).
    world.clans.get_mut(&5000).unwrap().dissolving_expiry_time = 1;
    drain(&mut a_rx);
    drain(&mut b_rx);
    clans::handle_clan_dissolve_task(&mut world, 5000);
    assert!(!world.clans.contains_key(&5000));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .clan_id,
        0
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3002)
            .unwrap()
            .clan_id,
        0
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CLAN_HAS_DISPERSED)
    );
    assert!(
        drain(&mut b_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_DELETE_ALL)
    );
}

// --- G18 slice 2: clan level-up + pledge skill learning ---

/// The delegated leader transfer: stamp + persist + confirmation htmls;
/// a second request answers in-progress; cancel zeroes the stamp.
#[test]
fn delegated_leader_transfer() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3002]);
    drain_db(&mut db_rx);

    let bypass = |world: &mut World, client: u32, cmd: &str| {
        handle_request_bypass_to_server(
            world,
            client,
            &bypass_body(&format!("npc_{NPC_OID}_{cmd}")),
        );
    };

    // Non-leader.
    bypass(&mut world, 2, "change_clan_leader P3001");
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT)
    );

    // Unknown member.
    bypass(&mut world, 1, "change_clan_leader Nobody");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_DOES_NOT_EXIST)
    );

    // Success: stamp + persist + html.
    bypass(&mut world, 1, "change_clan_leader P3002");
    assert_eq!(world.clans[&5000].new_leader_id, 3002);
    assert!(
        drain(&mut a_rx)
            .iter()
            .any(|p| decode_npc_html(p).is_some_and(|h| h.contains("delegation") || !h.is_empty()))
    );
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateClanNewLeader {
            clan_id: 5000,
            new_leader_id: 3002
        }
    )));

    // A second request while pending → in-progress page, stamp unchanged.
    bypass(&mut world, 1, "change_clan_leader P3002");
    assert_eq!(world.clans[&5000].new_leader_id, 3002);

    // Cancel zeroes the stamp.
    bypass(&mut world, 1, "cancel_clan_leader_change");
    assert_eq!(world.clans[&5000].new_leader_id, 0);
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateClanNewLeader {
            clan_id: 5000,
            new_leader_id: 0
        }
    )));
}

// --- G18 slice 4: clan wars ---

/// `Clan.removeClanMember`: `if (!player.isNoble()) player.setTitle("")`.
///
/// A noble's title is their own standing, not the clan's, so it outlives the
/// clan. The single-member leave path already honoured this; the **dissolve**
/// path stripped every ex-member's title unconditionally, which is what this
/// pins — both directions in one test so a blanket "never clear" would fail too.
#[test]
fn dissolving_a_clan_spares_a_nobles_title() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let _a_rx = ingame_player(&mut world, 1, 3101, 0, 0, 0);
    let _b_rx = ingame_player(&mut world, 2, 3102, 0, 0, 0);
    install_clan(&mut world, 5100, &[3101, 3102]);
    drain_db(&mut db_rx);

    for (oid, noble) in [(3101, true), (3102, false)] {
        let p = world.objects.get_component_mut::<Player>(&oid).unwrap();
        p.is_noble = noble;
        p.title = "Titled".to_string();
    }

    world.clans.get_mut(&5100).unwrap().dissolving_expiry_time = 1;
    clans::handle_clan_dissolve_task(&mut world, 5100);

    assert_eq!(
        world.objects.get_component::<Player>(&3101).unwrap().title,
        "Titled",
        "a noble keeps their title through the clan dissolving"
    );
    assert_eq!(
        world.objects.get_component::<Player>(&3102).unwrap().title,
        "",
        "a non-noble still loses theirs"
    );
}

// ---------------------------------------------------------------------------
// Castle circlets (G24) — `CastleManager.removeCirclet`
// ---------------------------------------------------------------------------

/// Gludio's circlet, from Java's `_castleCirclets` table (index 1).
const GLUDIO_CIRCLET: i32 = 6838;

/// **Leaving a castle-owning clan costs you the circlet** (Java
/// `Clan.removeClanMember` → `removeCirclet(exMember, getCastleId())`, gated on
/// `RemoveCastleCirclets`).
#[test]
fn leaving_a_castle_owning_clan_takes_the_circlet() {
    use crate::game_loop::siege::treasury::circlet_of;

    let (mut world, ..) = quest_test_world();
    world.cfg.character.remove_castle_circlets = true;
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    add_quest_items(&mut world, &[(GLUDIO_CIRCLET, "Circlet of Gludio", false)]);
    inventory::add_inventory_item(&mut world, 3001, GLUDIO_CIRCLET, 1);

    // A clan that owns Gludio, with 3001 on the roster.
    let mut clan = castle_owning_clan(700);
    clan.castle_id = 1;
    clan.members.push(model::clan::ClanMember {
        char_id: 3001,
        name: "P3001".into(),
        level: 20,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    });
    world.clans.insert(700, clan);

    assert_eq!(circlet_of(1), GLUDIO_CIRCLET, "Gludio's circlet id");
    assert_eq!(
        item_count(&world, 3001, GLUDIO_CIRCLET),
        1,
        "worn before leaving"
    );

    clans::remove_clan_member(&mut world, 700, 3001, 0);
    assert_eq!(
        item_count(&world, 3001, GLUDIO_CIRCLET),
        0,
        "the circlet leaves with the clan"
    );
}

/// **With `RemoveCastleCirclets` off, the circlet stays** — Java gates both
/// call sites on that flag, so an operator can let members keep them.
#[test]
fn the_circlet_survives_when_the_config_says_so() {
    let (mut world, ..) = quest_test_world();
    world.cfg.character.remove_castle_circlets = false;
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    add_quest_items(&mut world, &[(GLUDIO_CIRCLET, "Circlet of Gludio", false)]);
    inventory::add_inventory_item(&mut world, 3001, GLUDIO_CIRCLET, 1);

    let mut clan = castle_owning_clan(701);
    clan.castle_id = 1;
    clan.members.push(model::clan::ClanMember {
        char_id: 3001,
        name: "P3001".into(),
        level: 20,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    });
    world.clans.insert(701, clan);

    clans::remove_clan_member(&mut world, 701, 3001, 0);
    assert_eq!(
        item_count(&world, 3001, GLUDIO_CIRCLET),
        1,
        "config off: the circlet is kept"
    );
}

/// A clan with **no** castle has id 0, which maps to "no circlet" — so an
/// ordinary clan leaver keeps whatever headgear they own.
#[test]
fn a_castleless_clan_takes_nothing() {
    use crate::game_loop::siege::treasury::circlet_of;
    assert_eq!(circlet_of(0), 0, "no castle, no circlet");
    assert_eq!(circlet_of(10), 0, "out of range, as Java's bounds check is");
    // The nine real castles all map to a distinct item.
    let ids: Vec<i32> = (1..=9).map(circlet_of).collect();
    assert!(ids.iter().all(|&i| i != 0), "every castle has one: {ids:?}");
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 9, "and they are distinct: {ids:?}");
}

/// A minimal clan for the circlet tests.
fn castle_owning_clan(id: i32) -> Clan {
    Clan {
        id,
        name: format!("Clan{id}"),
        leader_id: 3001,
        level: 5,
        reputation_score: 0,
        castle_id: 0,
        members: Vec::new(),
        skills: Default::default(),
        warehouse: Default::default(),
        char_penalty_expiry_time: 0,
        dissolving_expiry_time: 0,
        rank_privs: Default::default(),
        new_leader_id: 0,
        sub_pledges: Default::default(),
        ally_id: 0,
        ally_name: String::new(),
        ally_penalty_expiry_time: 0,
        ally_penalty_type: 0,
        crest_id: 0,
        crest_large_id: 0,
        ally_crest_id: 0,
        blood_alliance_count: 0,
    }
}
