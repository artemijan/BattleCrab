//! Rank and standing: the pledge class table, the clan level ladder, rank
//! privileges and power grades, the advent aura and the profession-change
//! gate.

use super::*;

/// The pure `calculatePledgeClass` table for a main-clan member (`Clan::
/// pledge_class_of`): a clan below level 4 yields 0 for everyone (no crown);
/// the leader outranks the members from level 4 up.
#[test]
fn pledge_class_table_matches_calculate_pledge_class() {
    use crate::model::clan::Clan;
    let mut clan = Clan {
        id: 1,
        name: "Probe".into(),
        leader_id: 10,
        level: 0,
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
    };
    // (leader, member) expected pledge class per clan level.
    for (level, leader, member) in [
        (0, 0, 0),
        (3, 0, 0),
        (4, 3, 0),
        (5, 4, 2),
        (6, 5, 3),
        (7, 7, 4),
        (8, 8, 5),
        (9, 9, 6),
        (10, 10, 7),
        (11, 11, 8),
    ] {
        clan.level = level;
        assert_eq!(
            clan.pledge_class_of(10),
            leader,
            "leader at clan level {level}"
        );
        assert_eq!(
            clan.pledge_class_of(20),
            member,
            "member at clan level {level}"
        );
    }
}

/// Levelling a clan recomputes each online member's `pledge_class` (the on-head
/// crown) and re-broadcasts UserInfo + CharInfo so the crown appears live.
#[test]
fn set_clan_level_updates_leader_pledge_class_and_rebroadcasts() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 10;
    world.id_pool = 0x3000_0000..0x3000_0100;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_create_clan Myclan")),
    );
    let clan_id = world
        .objects
        .get_component::<Player>(&3001)
        .unwrap()
        .clan_id;
    // A fresh (level 0) clan gives the leader no crown.
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .pledge_class,
        0
    );
    drain(&mut a_rx);

    clans::set_clan_level(&mut world, clan_id, 5);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .pledge_class,
        4,
        "level-5 clan leader is pledge class 4"
    );
    let pkts = drain(&mut a_rx);
    assert!(
        pkts.iter().any(|p| p[0] == 0x32),
        "fresh UserInfo on the level change"
    );
}

/// Clan Advent (skill 19009), Java `ClanMaster`'s login/logout listeners: the
/// aura lands on every online clan member while the leader is logged in and
/// drops when the leader logs out. Leader login buffs all online members; a
/// member logging in with the leader offline gets nothing.
#[test]
fn clan_advent_aura_tracks_leader_online_state() {
    use crate::model::clan::{Clan, ClanMember};

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    // The real 19009 lives in the dist skills the synthetic test data doesn't
    // load — register a stand-in so the login/logout path has one to apply.
    world
        .data
        .skill_data
        .insert_for_test(clan_advent_test_skill());

    // Leader 3001 (client 1) + member 3002 (client 2), both online, one clan.
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0001;
    let cm = |id: i32| ClanMember {
        char_id: id,
        name: format!("P{id}"),
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
            name: "AdventClan".into(),
            leader_id: 3001,
            level: 1,
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
    for oid in [3001, 3002] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .clan_id = clan_id;
    }

    let has_advent = |world: &World, oid: i32| has_buff(world, oid, 19009);

    // Leader logs in → the aura lands on every online member (leader + 3002).
    clans::on_enter_world(&mut world, 1, 3001);
    assert!(
        has_advent(&world, 3001),
        "leader gets Clan Advent on their own login"
    );
    assert!(
        has_advent(&world, 3002),
        "online member gets Clan Advent when the leader logs in"
    );

    // Leader logs out → the aura drops from the remaining online member.
    clans::on_leave_world(&mut world, 3001, clan_id);
    assert!(
        !has_advent(&world, 3002),
        "Clan Advent removed when the leader logs out"
    );

    // Fully take the leader offline (session gone + despawned), then a member
    // login must NOT re-light the aura.
    world.clients.remove(&1);
    world.objects.despawn(&3001);
    clans::on_enter_world(&mut world, 2, 3002);
    assert!(
        !has_advent(&world, 3002),
        "no aura while the leader is offline"
    );
}

/// `ClanMaster.onProfessionChange` (`ON_PLAYER_PROFESSION_CHANGE`) — the last
/// of that script's four listeners, and the only one that was unported.
///
/// **This exercises the helper directly rather than driving `set_class_id`.**
/// Going through the class change would be vacuous here: nothing on that path
/// strips buffs in this port, so the aura is still present afterwards whether
/// or not the listener runs — a test written that way passes with the hook
/// deleted. What is worth pinning is the *gate*, which is Java's
/// `isClanLeader() || clan.getLeader().isOnline()`.
#[test]
fn the_profession_change_listener_honours_javas_leader_gate() {
    use crate::model::clan::{Clan, ClanMember};

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    world
        .data
        .skill_data
        .insert_for_test(clan_advent_test_skill());
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    drain_db(&mut db_rx);

    let clan_id = 0x3000_0002;
    let cm = |id: i32| ClanMember {
        char_id: id,
        name: format!("P{id}"),
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
            name: "ProfClan".into(),
            leader_id: 3001,
            level: 1,
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
    for oid in [3001, 3002] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .clan_id = clan_id;
    }
    let has_advent = |world: &World, oid: i32| has_buff(world, oid, 19009);
    let relight =
        |world: &mut World, oid: i32| reapply_clan_advent_on_profession_change(world, oid);

    // Leader online → a member's profession change re-lights the aura.
    assert!(!has_advent(&world, 3002), "starts without it");
    relight(&mut world, 3002);
    assert!(
        has_advent(&world, 3002),
        "re-lit while the leader is online"
    );

    // Leader offline → it does not.
    clans::remove_clan_advent(&mut world, 3002);
    world.clients.remove(&1);
    world.objects.despawn(&3001);
    relight(&mut world, 3002);
    assert!(
        !has_advent(&world, 3002),
        "no re-light for a member while the leader is offline"
    );

    // …but the leader themselves always qualifies, with no online check —
    // they are plainly online to have changed profession at all.
    let _c = ingame_player(&mut world, 3, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = clan_id;
    relight(&mut world, 3001);
    assert!(has_advent(&world, 3001), "the leader re-lights their own");
}

/// The village-master `increase_clan_level` ladder: leader/dissolution gates,
/// the not-met reject, and a successful 0→1 upgrade (SP + adena consumed,
/// level broadcast + FX) and 2→3 (Blood Mark proof items).
#[test]
fn clan_level_up_ladder() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    add_quest_items(
        &mut world,
        &[(57, "Adena", false), (1419, "Blood Mark", false)],
    );
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3002]);
    world.clans.get_mut(&5000).unwrap().level = 0;
    drain_db(&mut db_rx);

    let bypass = |world: &mut World, client: u32| {
        handle_request_bypass_to_server(
            world,
            client,
            &bypass_body(&format!("npc_{NPC_OID}_increase_clan_level")),
        );
    };

    // Non-leader.
    bypass(&mut world, 2);
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT)
    );

    // Pending dissolution.
    world.clans.get_mut(&5000).unwrap().dissolving_expiry_time = i64::MAX;
    bypass(&mut world, 1);
    assert!(ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
        &server_packets::sm_ids::AS_YOU_ARE_SCHEDULED_FOR_CLAN_DISSOLUTION_LEVEL_CANNOT_INCREASE
    ));
    world.clans.get_mut(&5000).unwrap().dissolving_expiry_time = 0;

    // No SP / no adena.
    bypass(&mut world, 1);
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::THE_CONDITIONS_TO_INCREASE_THE_CLAN_S_LEVEL_HAVE_NOT_BEEN_MET
        )
    );

    // 0 → 1: 1000 SP + 150k adena.
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.sp = 5_000;
    }
    world
        .objects
        .get_component_mut::<Inventory>(&3001)
        .unwrap()
        .add_item(&world.data.item_data, 900_001, 57, 200_000);
    bypass(&mut world, 1);
    assert_eq!(world.clans[&5000].level, 1);
    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.sp, 4_000, "1000 SP consumed");
    assert_eq!(adena_of(&world, 3001), 50_000, "150k adena consumed");
    let a_pkts = drain(&mut a_rx);
    let a_sms = ids_after_opcode(&a_pkts, server_packets::opcodes::SYSTEM_MESSAGE);
    assert!(a_sms.contains(&server_packets::sm_ids::S1_ADENA_DISAPPEARED));
    assert!(a_sms.contains(&server_packets::sm_ids::YOUR_SP_HAS_DECREASED_BY_S1));
    assert!(a_sms.contains(&server_packets::sm_ids::YOUR_CLAN_S_LEVEL_HAS_INCREASED));
    assert!(
        a_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "level-up FX"
    );
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOUR_CLAN_S_LEVEL_HAS_INCREASED)
    );
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateClanLevel {
            clan_id: 5000,
            level: 1
        }
    )));

    // 2 → 3: 100k SP + 100 Blood Marks.
    world.clans.get_mut(&5000).unwrap().level = 2;
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.sp = 200_000;
    }
    world
        .objects
        .get_component_mut::<Inventory>(&3001)
        .unwrap()
        .add_item(&world.data.item_data, 900_002, 1419, 150);
    bypass(&mut world, 1);
    assert_eq!(world.clans[&5000].level, 3);
    assert_eq!(
        count_of_item(&world, 3001, 1419),
        50,
        "100 Blood Marks consumed"
    );
    assert_eq!(
        world.objects.get_component::<Player>(&3001).unwrap().sp,
        100_000
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_DISAPPEARED)
    );
}

fn pledge_power_body(rank: i32, action: i32, privs: Option<i32>) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(rank);
    w.write_i32(action);
    if let Some(p) = privs {
        w.write_i32(p);
    }
    w.into_bytes()
}

/// The rank-privilege editor: non-leader edits are ignored (answer only), the
/// leader's edit stores + persists the mask, refreshes online holders of that
/// rank, and rank 9 is clamped to the academy-bestowable subset.
#[test]
fn rank_privs_edit_and_refresh() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3002]);
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .power_grade = 5;
    drain_db(&mut db_rx);
    drain(&mut a_rx);
    drain(&mut b_rx);

    // A plain member's action-2 edit is answered but not stored.
    clans::handle_request_pledge_power(&mut world, 2, &pledge_power_body(5, 2, Some(0xFF)));
    assert!(world.clans[&5000].rank_privs.is_empty());
    assert!(
        drain(&mut b_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MANAGE_PLEDGE_POWER)
    );

    // The leader stores rank 5 → the online grade-5 member gets the mask.
    let privs = model::clan::CL_JOIN_CLAN | model::clan::CL_VIEW_WAREHOUSE;
    clans::handle_request_pledge_power(&mut world, 1, &pledge_power_body(5, 2, Some(privs)));
    assert_eq!(world.clans[&5000].rank_privs_of(5), privs);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3002)
            .unwrap()
            .clan_privs,
        privs
    );
    assert!(drain_db(&mut db_rx)
        .iter()
        .any(|c| matches!(c, db::DbCommand::SaveClanRankPrivs { clan_id: 5000, rank: 5, privs: p } if *p == privs)));
    // broadcastClanStatus resets the clan windows.
    assert!(
        drain(&mut b_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_ALL)
    );

    // Rank 9 is clamped to the bestowable subset.
    clans::handle_request_pledge_power(&mut world, 1, &pledge_power_body(9, 2, Some(-1)));
    assert_eq!(
        world.clans[&5000].rank_privs_of(9),
        model::clan::RANK9_PRIVS_MASK
    );

    // The grade list answers all 9 ranks.
    clans::handle_request_pledge_power_grade_list(&world, 1);
    let list = drain(&mut a_rx)
        .into_iter()
        .find(|p| is_ex(p, 0x3D))
        .expect("PledgePowerGradeList");
    assert_eq!(i32::from_le_bytes([list[3], list[4], list[5], list[6]]), 9);
}

fn member_grade_body(name: &str, grade: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(name);
    w.write_i32(grade);
    w.into_bytes()
}

fn member_query_body(name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(0);
    w.write_string(name);
    w.into_bytes()
}

/// Setting a member's power grade: CL_MANAGE_RANKS gate, leader untouchable,
/// grade + persistence + the SM 1761 broadcast; the member-detail and
/// power-info panes answer with the new rank.
#[test]
fn member_power_grade_and_info_panes() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3002]);
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .power_grade = 5;
    drain_db(&mut db_rx);

    // A plain member without CL_MANAGE_RANKS cannot re-rank.
    clans::handle_request_pledge_set_member_power_grade(
        &mut world,
        2,
        &member_grade_body("P3001", 7),
    );
    assert_eq!(world.clans[&5000].member(3001).unwrap().power_grade, 1);

    // The leader cannot be re-ranked.
    clans::handle_request_pledge_set_member_power_grade(
        &mut world,
        1,
        &member_grade_body("P3001", 7),
    );
    assert_eq!(world.clans[&5000].member(3001).unwrap().power_grade, 1);

    // The leader re-ranks the member to grade 4.
    drain(&mut a_rx);
    drain(&mut b_rx);
    clans::handle_request_pledge_set_member_power_grade(
        &mut world,
        1,
        &member_grade_body("P3002", 4),
    );
    assert_eq!(world.clans[&5000].member(3002).unwrap().power_grade, 4);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3002)
            .unwrap()
            .power_grade,
        4
    );
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::CLAN_MEMBER_C1_S_PRIVILEGE_LEVEL_HAS_BEEN_CHANGED_TO_S2
        )
    );
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateCharPowerGrade {
            char_id: 3002,
            power_grade: 4
        }
    )));

    // The info panes reflect the new grade.
    world
        .clans
        .get_mut(&5000)
        .unwrap()
        .rank_privs
        .insert(4, 0x0A);
    clans::handle_request_pledge_member_power_info(&world, 1, &member_query_body("P3002"));
    let pkts = drain(&mut a_rx);
    let power = pkts
        .iter()
        .find(|p| is_ex(p, 0x3E))
        .expect("PledgeReceivePowerInfo");
    assert_eq!(
        i32::from_le_bytes([power[3], power[4], power[5], power[6]]),
        4,
        "grade in the pane"
    );
    clans::handle_request_pledge_member_info(&world, 1, &member_query_body("P3002"));
    assert!(drain(&mut a_rx).iter().any(|p| is_ex(p, 0x3F)));
}

/// Enter-world derives privileges from the rank table: the leader gets the
/// all-bits mask + grade 1; a member's stored `clan_privs` never wins over
/// their rank's current mask (grade defaulting to 5).
#[test]
fn enter_world_derives_privs_from_rank_table() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3002]);
    world
        .clans
        .get_mut(&5000)
        .unwrap()
        .rank_privs
        .insert(5, 0x0C);

    // Stale stored values, as if loaded from an out-of-date characters row.
    {
        let p = world.objects.get_component_mut::<Player>(&3002).unwrap();
        p.clan_privs = 999;
        p.power_grade = 0;
        let l = world.objects.get_component_mut::<Player>(&3001).unwrap();
        l.clan_privs = 0;
        l.power_grade = 0;
    }
    clans::on_enter_world(&mut world, 1, 3001);
    clans::on_enter_world(&mut world, 2, 3002);
    let l = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(
        (l.clan_privs, l.power_grade),
        (model::clan::ALL_CLAN_PRIVILEGES, 1)
    );
    let p = world.objects.get_component::<Player>(&3002).unwrap();
    assert_eq!((p.clan_privs, p.power_grade), (0x0C, 5));
}
