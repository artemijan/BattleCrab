//! Recruitment: the board, applicants, rejections, the draft list, and
//! open-joining sign-in.

use super::*;

/// The clan-entry queries the clan window fires on open (ex 0xD3/0xDE):
/// `ExPledgeRecruitInfo` echoes the clan summary with an empty sub-pledge
/// list, `ExPledgeRecruitApplyInfo` always answers DEFAULT until the G18
/// `ClanEntryManager` lands, and an unknown clan id stays silent.
#[test]
fn clan_entry_queries() {
    use crate::model::clan::{Clan, ClanMember};
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    let clan_id = 0x7000_0002;
    let cm = |id: i32| ClanMember {
        char_id: id,
        name: format!("P{id}"),
        level: 40,
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
            name: "Recruiters".into(),
            leader_id: 3001,
            level: 3,
            reputation_score: 0,
            castle_id: 0,
            members: vec![cm(3001), cm(3002)],
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

    // ExPledgeRecruitApplyInfo: status DEFAULT (0) — nothing is registered.
    clans::handle_request_pledge_recruit_apply_info(&world, 1);
    let pkts = drain(&mut rx);
    let apply = pkts
        .iter()
        .find(|p| p[0] == 0xFE && p[1] == 0x40 && p[2] == 0x01)
        .expect("ExPledgeRecruitApplyInfo");
    assert_eq!(&apply[3..7], &0i32.to_le_bytes());

    // ExPledgeRecruitInfo: name, leader name, level, member count, 0 sub-pledges.
    clans::handle_request_pledge_recruit_info(&world, 1, &clan_id.to_le_bytes());
    let pkts = drain(&mut rx);
    let info = pkts
        .iter()
        .find(|p| p[0] == 0xFE && p[1] == 0x3F && p[2] == 0x01)
        .expect("ExPledgeRecruitInfo");
    let mut r = commons::network::PacketReader::new(&info[3..]);
    assert_eq!(r.read_string().unwrap(), "Recruiters");
    assert_eq!(r.read_string().unwrap(), "P3001");
    assert_eq!(r.read_i32().unwrap(), 3); // clan level
    assert_eq!(r.read_i32().unwrap(), 2); // member count
    assert_eq!(r.read_i32().unwrap(), 0); // sub-pledge count

    // Unknown clan id: Java's ClanTable miss returns without an answer.
    clans::handle_request_pledge_recruit_info(&world, 1, &0x7999_9999i32.to_le_bytes());
    assert!(drain(&mut rx).is_empty());
}

/// `RequestPledgeRecruitBoardSearch` (ex 0xD4): the recruit-board tab always
/// gets one `ExPledgeRecruitBoardSearch` page back — empty until the G18
/// `ClanEntryManager` lands (0 total pages, 0 clans, the requested page
/// echoed), and a short/malformed packet is dropped without an answer.
#[test]
fn clan_recruit_board_search() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    // clanLevel=-1, karma=-1, type=0, query="", sort=0, descending=1, page=3,
    // applicationType=0 — the clan window's default "show all" search.
    let mut body = Vec::new();
    body.extend((-1i32).to_le_bytes());
    body.extend((-1i32).to_le_bytes());
    body.extend(0i32.to_le_bytes());
    body.extend(0u16.to_le_bytes()); // empty UTF-16 string (terminator only)
    body.extend(0i32.to_le_bytes());
    body.extend(1i32.to_le_bytes());
    body.extend(3i32.to_le_bytes());
    body.extend(0i32.to_le_bytes());
    clans::handle_request_pledge_recruit_board_search(&world, 1, &body);
    let pkts = drain(&mut rx);
    let page = pkts
        .iter()
        .find(|p| p[0] == 0xFE && p[1] == 0x41 && p[2] == 0x01)
        .expect("ExPledgeRecruitBoardSearch");
    let mut r = commons::network::PacketReader::new(&page[3..]);
    assert_eq!(r.read_i32().unwrap(), 3); // current page echoed
    assert_eq!(r.read_i32().unwrap(), 0); // total pages
    assert_eq!(r.read_i32().unwrap(), 0); // clans on this page

    // Truncated packet (missing the page int): dropped silently.
    clans::handle_request_pledge_recruit_board_search(&world, 1, &body[..body.len() - 8]);
    assert!(drain(&mut rx).is_empty());
}

// --- G18 slice 1: membership lifecycle ---

fn draft_list_apply_body(apply_type: i32, karma: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(apply_type);
    w.write_i32(karma);
    w.into_bytes()
}

fn board_access_body(
    apply_type: i32,
    karma: i32,
    info: &str,
    detail: &str,
    app_type: i32,
    recruit_type: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(apply_type);
    w.write_i32(karma);
    w.write_string(info);
    w.write_string(detail);
    w.write_i32(app_type);
    w.write_i32(recruit_type);
    w.into_bytes()
}

fn waiting_apply_body(karma: i32, clan_id: i32, message: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(karma);
    w.write_i32(clan_id);
    w.write_string(message);
    w.into_bytes()
}

/// A clan leader registers/updates/removes the clan on the recruitment
/// board: privilege gate, the re-registration lock after cancelling, and a
/// search that actually finds the listing.
#[test]
fn recruit_board_register_update_remove_and_search() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3003]);
    world.clans.get_mut(&5000).unwrap().level = 4;
    drain_db(&mut db_rx);

    // A plain member without CL_MANAGE_RANKS cannot register the clan.
    clans::handle_request_pledge_recruit_board_access(
        &mut world,
        2,
        &board_access_body(1, 0, "Hi", "Details", 0, 0),
    );
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::ONLY_THE_CLAN_LEADER_OR_RANK_MANAGER_MAY_REGISTER_THE_CLAN
        )
    );

    // The leader registers.
    clans::handle_request_pledge_recruit_board_access(
        &mut world,
        1,
        &board_access_body(1, 3, "LookingForMembers", "Come raid with us", 1, 0),
    );
    assert!(world.recruit_clans.contains_key(&5000));
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::ENTRY_APPLICATION_COMPLETE_AUTO_CANCELLED_AFTER_30_DAYS
        )
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::InsertPledgeRecruit { clan_id: 5000, .. }))
    );

    // A search with no filters finds it.
    let mut body = Vec::new();
    body.extend((-1i32).to_le_bytes());
    body.extend((-1i32).to_le_bytes());
    body.extend(0i32.to_le_bytes());
    body.extend(0u16.to_le_bytes());
    body.extend(0i32.to_le_bytes());
    body.extend(1i32.to_le_bytes());
    body.extend(1i32.to_le_bytes());
    body.extend(0i32.to_le_bytes());
    clans::handle_request_pledge_recruit_board_search(&world, 1, &body);
    let pkts = drain(&mut a_rx);
    let page = pkts
        .iter()
        .find(|p| p[0] == 0xFE && p[1] == 0x41 && p[2] == 0x01)
        .expect("board search");
    let mut r = commons::network::PacketReader::new(&page[3..]);
    assert_eq!(r.read_i32().unwrap(), 1); // page
    assert_eq!(r.read_i32().unwrap(), 1); // total pages
    assert_eq!(r.read_i32().unwrap(), 1); // 1 clan on the page
    assert_eq!(r.read_i32().unwrap(), 5000); // clan id

    // The detail pane.
    let mut db2 = Vec::new();
    db2.extend(5000i32.to_le_bytes());
    clans::handle_request_pledge_recruit_board_detail(&world, 1, &db2);
    let pkts = drain(&mut a_rx);
    let detail = pkts
        .iter()
        .find(|p| p[0] == 0xFE && p[1] == 0x42 && p[2] == 0x01)
        .expect("board detail");
    let mut r = commons::network::PacketReader::new(&detail[3..]);
    assert_eq!(r.read_i32().unwrap(), 5000);
    assert_eq!(r.read_i32().unwrap(), 3);
    assert_eq!(r.read_string().unwrap(), "LookingForMembers");

    // ApplyInfo now answers ORDERED for the leader.
    clans::handle_request_pledge_recruit_apply_info(&world, 1);
    let apply = drain(&mut a_rx)
        .into_iter()
        .find(|p| p[0] == 0xFE && p[1] == 0x40 && p[2] == 0x01)
        .expect("ExPledgeRecruitApplyInfo");
    assert_eq!(&apply[3..7], &1i32.to_le_bytes(), "ORDERED");

    // Remove, then re-register is locked for 5 minutes.
    clans::handle_request_pledge_recruit_board_access(
        &mut world,
        1,
        &board_access_body(0, 0, "", "", 0, 0),
    );
    assert!(!world.recruit_clans.contains_key(&5000));
    clans::handle_request_pledge_recruit_board_access(
        &mut world,
        1,
        &board_access_body(1, 3, "Again", "Details", 1, 0),
    );
    assert!(!world.recruit_clans.contains_key(&5000), "locked out");
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::YOU_MAY_APPLY_FOR_ENTRY_AFTER_S1_MINUTES_DUE_TO_CANCELLING
        )
    );
}

/// A clanless player applies to a specific clan; the leader gets the alarm,
/// views the queue, and accepts — reusing the shared join path.
#[test]
fn recruit_applicant_apply_and_accept() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0); // clan leader
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0); // clanless applicant
    install_clan(&mut world, 5000, &[3001]);
    drain_db(&mut db_rx);
    drain(&mut a_rx);

    clans::handle_request_pledge_waiting_apply(
        &mut world,
        2,
        &waiting_apply_body(0, 5000, "Let me in!"),
    );
    assert!(
        world
            .recruit_applicants
            .get(&5000)
            .is_some_and(|m| m.contains_key(&3003))
    );
    let b_pkts = drain(&mut b_rx);
    assert!(
        b_pkts
            .iter()
            .any(|p| p[0] == 0xFE && p[1] == 0x40 && p[2] == 0x01),
        "WAITING status ack"
    );
    assert!(
        drain(&mut a_rx)
            .iter()
            .any(|p| p[0] == 0xFE && p[1] == 0x47 && p[2] == 0x01),
        "leader gets the alarm"
    );
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        db::DbCommand::UpsertPledgeApplicant {
            player_id: 3003,
            clan_id: 5000,
            ..
        }
    )));

    // The applicant checks their own status.
    clans::handle_request_pledge_waiting_applied(&world, 2);
    let applied = drain(&mut b_rx)
        .into_iter()
        .find(|p| p[0] == 0xFE && p[1] == 0x43 && p[2] == 0x01)
        .expect("applied");
    let mut r = commons::network::PacketReader::new(&applied[3..]);
    assert_eq!(r.read_i32().unwrap(), 5000);

    // The leader views the queue.
    let mut lb = Vec::new();
    lb.extend(5000i32.to_le_bytes());
    clans::handle_request_pledge_waiting_list(&world, 1, &lb);
    let list = drain(&mut a_rx)
        .into_iter()
        .find(|p| p[0] == 0xFE && p[1] == 0x44 && p[2] == 0x01)
        .expect("waiting list");
    let mut r = commons::network::PacketReader::new(&list[3..]);
    assert_eq!(r.read_i32().unwrap(), 1);
    assert_eq!(r.read_i32().unwrap(), 3003);

    // Accept: the applicant joins through the shared path, and the queue empties.
    let mut acc = Vec::new();
    acc.extend(1i32.to_le_bytes());
    acc.extend(3003i32.to_le_bytes());
    acc.extend(5000i32.to_le_bytes());
    clans::handle_request_pledge_waiting_user_accept(&mut world, 1, &acc);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3003)
            .unwrap()
            .clan_id,
        5000
    );
    assert!(
        !world
            .recruit_applicants
            .get(&5000)
            .is_some_and(|m| m.contains_key(&3003))
    );
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::ENTERED_THE_CLAN)
    );
}

/// Rejecting an applicant just drops the row; the draft list (global
/// "looking for clan" registry) supports add/remove/search with the
/// re-registration lock.
#[test]
fn recruit_reject_and_draft_list_lifecycle() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    drain_db(&mut db_rx);

    clans::handle_request_pledge_waiting_apply(
        &mut world,
        2,
        &waiting_apply_body(0, 5000, "Pick me"),
    );
    drain(&mut a_rx);
    drain(&mut b_rx);
    let mut rej = Vec::new();
    rej.extend(0i32.to_le_bytes());
    rej.extend(3003i32.to_le_bytes());
    rej.extend(5000i32.to_le_bytes());
    clans::handle_request_pledge_waiting_user_accept(&mut world, 1, &rej);
    assert!(
        !world
            .recruit_applicants
            .get(&5000)
            .is_some_and(|m| m.contains_key(&3003))
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3003)
            .unwrap()
            .clan_id,
        0,
        "rejected, not joined"
    );

    // Draft list: apply, search finds it, remove locks re-entry for 5 minutes.
    clans::handle_request_pledge_draft_list_apply(&mut world, 2, &draft_list_apply_body(1, 0));
    assert!(world.recruit_waiting.contains_key(&3003));
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::ENTERED_INTO_WAITING_LIST_AUTO_DELETED_AFTER_30_DAYS
        )
    );

    let mut search = Vec::new();
    search.extend(0i32.to_le_bytes());
    search.extend(107i32.to_le_bytes());
    search.extend(0i32.to_le_bytes());
    search.extend(0u16.to_le_bytes());
    search.extend(0i32.to_le_bytes());
    search.extend(1i32.to_le_bytes());
    clans::handle_request_pledge_draft_list_search(&world, 1, &search);
    let found = drain(&mut a_rx)
        .into_iter()
        .find(|p| p[0] == 0xFE && p[1] == 0x46 && p[2] == 0x01)
        .expect("draft search");
    let mut r = commons::network::PacketReader::new(&found[3..]);
    assert_eq!(r.read_i32().unwrap(), 1);
    assert_eq!(r.read_i32().unwrap(), 3003);

    clans::handle_request_pledge_draft_list_apply(&mut world, 2, &draft_list_apply_body(0, 0));
    assert!(!world.recruit_waiting.contains_key(&3003));
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::ENTRY_APPLICATION_CANCELLED_YOU_MAY_APPLY_AFTER_5_MINUTES
        )
    );
    clans::handle_request_pledge_draft_list_apply(&mut world, 2, &draft_list_apply_body(1, 0));
    assert!(
        !world.recruit_waiting.contains_key(&3003),
        "locked out for 5 minutes"
    );
}

/// Open-joining sign-in: instant self-join when the clan's recruitment
/// listing allows it, gated on the usual clan-full/penalty checks.
#[test]
fn recruit_open_joining_sign_in() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    drain_db(&mut db_rx);
    drain(&mut a_rx);
    clans::handle_request_pledge_recruit_board_access(
        &mut world,
        1,
        &board_access_body(1, 0, "Open", "Join us", 1, 0),
    );
    drain(&mut a_rx);
    drain_db(&mut db_rx);

    let mut body = Vec::new();
    body.extend(5000i32.to_le_bytes());
    clans::handle_request_pledge_sign_in_for_open_joining_method(&mut world, 2, &body);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3003)
            .unwrap()
            .clan_id,
        5000
    );
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::ENTERED_THE_CLAN)
    );

    // A full clan (level-1 cap 15) refuses, and reuses the applicant-removal
    // path even though this player was never actually in the queue (a no-op).
    let mut c_rx = ingame_player(&mut world, 3, 3005, 0, 0, 0);
    install_clan(&mut world, 5001, &[3007]);
    pad_clan(&mut world, 5001, 15); // level-1 main-pledge cap
    clans::handle_request_pledge_recruit_board_access(
        &mut world,
        1,
        &board_access_body(1, 0, "Open2", "Full", 1, 0),
    );
    drain_db(&mut db_rx);
    let mut body2 = Vec::new();
    body2.extend(5001i32.to_le_bytes());
    // Register the second clan's leader (3007 offline) — bypass: install a
    // recruit entry directly for 5001, since only 3001 has a client here.
    world.recruit_clans.insert(
        5001,
        model::clan_entry::PledgeRecruitInfo {
            clan_id: 5001,
            karma: 0,
            information: "Full".into(),
            detailed_information: "Full".into(),
            application_type: 1,
            recruit_type: 0,
        },
    );
    clans::handle_request_pledge_sign_in_for_open_joining_method(&mut world, 3, &body2);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3005)
            .unwrap()
            .clan_id,
        0,
        "clan full, join refused"
    );
    assert!(
        ids_after_opcode(&drain(&mut c_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::S1_IS_FULL_AND_CANNOT_ACCEPT_ADDITIONAL_CLAN_MEMBERS
        )
    );
}

// ---------------------------------------------------------------------------
// Residential (castle) skills
// ---------------------------------------------------------------------------
