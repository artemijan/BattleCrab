use super::*;

/// The `create_clan` bypass: Java's guard matrix (SM ids in `ClanTable.
/// createClan` order), then the success path — clan registered + persisted,
/// leader flags/privileges set, the pledge-window packet trio + SM 189, and
/// duplicate-name/already-in-clan rejects afterwards.
#[test]
fn clan_create_guards_and_success() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);

    let create = |world: &mut World, client: u32, name: &str| {
        handle_request_bypass_to_server(world, client, &bypass_body(&format!("npc_{NPC_OID}_create_clan {name}")));
    };

    // Level < 10.
    create(&mut world, 1, "Myclan");
    let pkts = drain(&mut a_rx);
    assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::YOU_DO_NOT_MEET_THE_CRITERIA_IN_ORDER_TO_CREATE_A_CLAN));

    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 10;

    // Name with a space arrives as two tokens → invalid.
    create(&mut world, 1, "My clan");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::CLAN_NAME_IS_INVALID));
    // Non-alphanumeric.
    create(&mut world, 1, "Cl@n");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::CLAN_NAME_IS_INVALID));
    // Too short / too long.
    create(&mut world, 1, "C");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::CLAN_NAME_IS_INVALID));
    create(&mut world, 1, "Averyveryverylongclanname");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::CLAN_NAME_S_LENGTH_IS_INCORRECT));
    // Recreate cooldown.
    world.objects.get_component_mut::<Player>(&3001).unwrap().clan_create_expiry_time = i64::MAX;
    create(&mut world, 1, "Myclan");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::YOU_MUST_WAIT_10_DAYS_BEFORE_CREATING_A_NEW_CLAN));
    world.objects.get_component_mut::<Player>(&3001).unwrap().clan_create_expiry_time = 0;

    // Success.
    world.id_pool = 0x3000_0000..0x3000_0100;
    drain_db(&mut db_rx);
    create(&mut world, 1, "Myclan");
    let pkts = drain(&mut a_rx);
    let p = world.objects.get_component::<Player>(&3001).unwrap();
    let clan_id = p.clan_id;
    assert_ne!(clan_id, 0);
    assert!(p.clan_leader);
    assert_eq!(p.clan_privs, crate::model::clan::ALL_CLAN_PRIVILEGES);
    let clan = &world.clans[&clan_id];
    assert_eq!((clan.name.as_str(), clan.leader_id, clan.members.len()), ("Myclan", 3001, 1));
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_INFO_UPDATE));
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_ALL));
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_UPDATE));
    assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::YOUR_CLAN_HAS_BEEN_CREATED));
    assert!(pkts.iter().any(|p| p[0] == 0x32), "fresh UserInfo with the clan id");
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::InsertClan { name, leader_id: 3001, .. } if name == "Myclan")));
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::UpdateCharClan { char_id: 3001, clan_privs, .. }
        if *clan_privs == crate::model::clan::ALL_CLAN_PRIVILEGES)));

    // Already in a clan.
    create(&mut world, 1, "Another");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::YOU_HAVE_FAILED_TO_CREATE_A_CLAN));

    // Second player: the name is taken (case-insensitive).
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3002).unwrap().level = 10;
    create(&mut world, 2, "MYCLAN");
    assert!(sm_ids_of(&drain(&mut b_rx)).contains(&server_packets::sm_ids::S1_ALREADY_EXISTS));
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
        let raw = std::fs::read_to_string(format!("{root}data/scripts/village_master/ClanMaster/{name}"))
            .expect(name);
        crate::data::htm_cache::strip_htm(&raw).replace("%objectId%", &NPC_OID.to_string())
    };

    // Talk → the root menu (ClanMaster id -1 ⇒ plain NpcHtmlMessage).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest ClanMaster"));
    let pkts = drain(&mut rx);
    let html = pkts.iter().find_map(|p| decode_npc_html(p)).expect("root menu html");
    assert_eq!(html, page("9000-01.htm"));

    // Leader-gated page as a non-leader → the -no variant.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest ClanMaster 9000-03.htm"));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("gated html");
    assert_eq!(html, page("9000-03-no.htm"));

    // As a leader → the real page.
    world.objects.get_component_mut::<Player>(&3001).unwrap().clan_leader = true;
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest ClanMaster 9000-03.htm"));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("leader html");
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
    let member = |char_id: i32, name: &str| crate::model::clan::ClanMember {
        char_id,
        name: name.into(),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
    };
    world.clans.insert(
        clan_id,
        crate::model::clan::Clan {
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
        },
    );
    for oid in [3001, 3002] {
        world.objects.get_component_mut::<Player>(&oid).unwrap().clan_id = clan_id;
    }

    // B "enters world": pledge window to B, online ping to A.
    clans::on_enter_world(&mut world, 2, 3002);
    let b_pkts = drain(&mut b_rx);
    assert!(b_pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_ALL));
    let a_pkts = drain(&mut a_rx);
    let upd = a_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_UPDATE)
        .expect("online ping to A");
    let mut r = commons::network::PacketReader::new(&upd[1..]);
    assert_eq!(r.read_string().unwrap(), "P3002");

    // Clan chat from A reaches both.
    chat::handle_say2(&mut world, 1, &say2_body("hail", crate::enums::ChatType::Clan.client_id(), None));
    assert!(drain(&mut a_rx).iter().any(|p| p[0] == server_packets::opcodes::SAY2));
    assert!(drain(&mut b_rx).iter().any(|p| p[0] == server_packets::opcodes::SAY2));

    // A clanless player gets SM 4202.
    let mut c_rx = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    chat::handle_say2(&mut world, 3, &say2_body("hail", crate::enums::ChatType::Clan.client_id(), None));
    assert!(sm_ids_of(&drain(&mut c_rx)).contains(&server_packets::sm_ids::YOU_ARE_NOT_IN_A_CLAN));

    // B leaves the world: offline ping to A.
    net::store_and_remove_player(&mut world, 3002);
    let a_pkts = drain(&mut a_rx);
    let upd = a_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_UPDATE)
        .expect("offline ping to A");
    // Online-status byte is the packet tail.
    assert_eq!(*upd.last().unwrap(), 0, "offline");
}

/// Clan warehouse: a shared container. The leader deposits (persisted), an
/// unprivileged member is denied the withdraw window, and the leader withdraws.
#[test]
fn clan_warehouse_shared_deposit_withdraw_and_privilege() {
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::inventory::Inventory;
    let (mut world, _tx, mut db_rx, _lrx) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut leader_rx = ingame_player_access(&mut world, 1, 3001, 0);
    let mut member_rx = ingame_player_access(&mut world, 2, 3002, 0);
    drain(&mut leader_rx);
    drain(&mut member_rx);

    // A level-1 clan: 3001 leader, 3002 plain member (no privileges).
    let clan_id = 0x7000_0001;
    let cm = |id: i32| ClanMember { char_id: id, name: format!("P{id}"), level: 1, class_id: 0, sex: 0, race: 0 };
    world.clans.insert(clan_id, Clan { id: clan_id, name: "WhClan".into(), leader_id: 3001, level: 1, reputation_score: 0, castle_id: 0, members: vec![cm(3001), cm(3002)], skills: Default::default(), warehouse: Default::default(), char_penalty_expiry_time: 0, dissolving_expiry_time: 0 });
    for oid in [3001, 3002] {
        world.objects.get_component_mut::<Player>(&oid).unwrap().clan_id = clan_id;
    }
    world.objects.get_component_mut::<Player>(&3001).unwrap().clan_privs = crate::model::clan::ALL_CLAN_PRIVILEGES;
    world.objects.get_component_mut::<Player>(&3002).unwrap().clan_privs = 0;

    // Leader deposits 500 adena into the shared clan warehouse.
    super::items::add_inventory_item(&mut world, 3001, 57, 500).unwrap();
    let adena_oid = world.objects.get_component::<Inventory>(&3001).unwrap().items().iter().find(|it| it.item_id == 57).unwrap().object_id;
    super::warehouse::open_clan(&mut world, 1, 3001, false); // keeper bypass → active = clan
    let deposit = { let mut w = PacketWriter::new(); w.write_u8(cop::SEND_WARE_HOUSE_DEPOSIT_LIST); w.write_i32(1); w.write_i32(adena_oid); w.write_i64(500); w.into_bytes() };
    on_packet(&mut world, 1, deposit);
    assert_eq!(world.clans[&clan_id].warehouse.0.count_of(57), 500, "deposited into clan warehouse");
    assert_eq!(world.objects.get_component::<Inventory>(&3001).unwrap().count_of(57), 0, "left leader inventory");
    // Persistence flush emitted for the clan.
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::StoreClanWarehouse { clan_id: cid, items } if *cid == clan_id && items.iter().any(|r| r.item_id == 57 && r.count == 500 && r.loc == "CLANWH"))), "clan warehouse persisted");

    // An unprivileged member cannot open the withdraw window.
    drain(&mut member_rx);
    super::warehouse::open_clan(&mut world, 2, 3002, true);
    let denied = drain(&mut member_rx);
    assert!(!denied.iter().any(|p| p[0] == server_packets::opcodes::WAREHOUSE_WITHDRAW_LIST), "member without CL_VIEW_WAREHOUSE is denied");

    // The leader withdraws 200 back — the shared container drops to 300.
    let wh_oid = world.clans[&clan_id].warehouse.0.items().iter().find(|it| it.item_id == 57).unwrap().object_id;
    super::warehouse::open_clan(&mut world, 1, 3001, true);
    let withdraw = { let mut w = PacketWriter::new(); w.write_u8(cop::SEND_WARE_HOUSE_WITH_DRAW_LIST); w.write_i32(1); w.write_i32(wh_oid); w.write_i64(200); w.into_bytes() };
    on_packet(&mut world, 1, withdraw);
    assert_eq!(world.clans[&clan_id].warehouse.0.count_of(57), 300, "300 remains in clan warehouse");
    assert_eq!(world.objects.get_component::<Inventory>(&3001).unwrap().count_of(57), 200, "200 withdrawn to leader");
}

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
        assert_eq!(clan.pledge_class_of(10), leader, "leader at clan level {level}");
        assert_eq!(clan.pledge_class_of(20), member, "member at clan level {level}");
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
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 10;
    world.id_pool = 0x3000_0000..0x3000_0100;
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_create_clan Myclan")));
    let clan_id = world.objects.get_component::<Player>(&3001).unwrap().clan_id;
    // A fresh (level 0) clan gives the leader no crown.
    assert_eq!(world.objects.get_component::<Player>(&3001).unwrap().pledge_class, 0);
    drain(&mut a_rx);

    crate::game_loop::clans::set_clan_level(&mut world, clan_id, 5);
    assert_eq!(world.objects.get_component::<Player>(&3001).unwrap().pledge_class, 4, "level-5 clan leader is pledge class 4");
    let pkts = drain(&mut a_rx);
    assert!(pkts.iter().any(|p| p[0] == 0x32), "fresh UserInfo on the level change");
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
    world.data.skill_data.insert_for_test(clan_advent_test_skill());

    // Leader 3001 (client 1) + member 3002 (client 2), both online, one clan.
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0001;
    let cm = |id: i32| ClanMember { char_id: id, name: format!("P{id}"), level: 1, class_id: 0, sex: 0, race: 0 };
    world.clans.insert(
        clan_id,
        Clan { id: clan_id, name: "AdventClan".into(), leader_id: 3001, level: 1, reputation_score: 0, castle_id: 0, members: vec![cm(3001), cm(3002)], skills: Default::default(), warehouse: Default::default(), char_penalty_expiry_time: 0, dissolving_expiry_time: 0 },
    );
    for oid in [3001, 3002] {
        world.objects.get_component_mut::<Player>(&oid).unwrap().clan_id = clan_id;
    }

    let has_advent = |world: &World, oid: i32| {
        world.objects.get_component::<Buffs>(&oid).is_some_and(|b| b.0.iter().any(|x| x.skill_id == 19009))
    };

    // Leader logs in → the aura lands on every online member (leader + 3002).
    crate::game_loop::clans::on_enter_world(&mut world, 1, 3001);
    assert!(has_advent(&world, 3001), "leader gets Clan Advent on their own login");
    assert!(has_advent(&world, 3002), "online member gets Clan Advent when the leader logs in");

    // Leader logs out → the aura drops from the remaining online member.
    crate::game_loop::clans::on_leave_world(&mut world, 3001, clan_id);
    assert!(!has_advent(&world, 3002), "Clan Advent removed when the leader logs out");

    // Fully take the leader offline (session gone + despawned), then a member
    // login must NOT re-light the aura.
    world.clients.remove(&1);
    world.objects.despawn(&3001);
    crate::game_loop::clans::on_enter_world(&mut world, 2, 3002);
    assert!(!has_advent(&world, 3002), "no aura while the leader is offline");
}

/// `//give_clan_skills` (Java `adminGiveClanSkills`): the clan learns every
/// pledge skill it qualifies for at its level; each applies to online members
/// gated by social class, lands as a (passive, icon-less) stat buff, shows in
/// the merged SkillList, and persists. Dispersing the clan strips them again.
#[test]
fn give_clan_skills_grants_gates_and_persists() {
    use crate::model::clan::{Clan, ClanMember};
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::components::{Buffs, ClanSkills};

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    // Two clan skills: 370 gated at HEIR (ordinal 3), 371 gated at COUNT (8).
    world.data.skill_data.insert_for_test(passive_clan_test_skill(370));
    world.data.skill_data.insert_for_test(passive_clan_test_skill(371));
    let learn = |id, social| PledgeSkillLearn { skill_id: id, skill_level: 1, get_level: 3, social_class: Some(social), residencial: false, level_up_sp: 0 };
    world.data.pledge_skill_trees.insert_for_test(learn(370, 3), false);
    world.data.pledge_skill_trees.insert_for_test(learn(371, 8), false);

    // A level-8 clan: leader 3001 (pledge class 8 → social 9), member 3002
    // (pledge class 5 → social 6). Both online.
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0055;
    let cm = |id: i32| ClanMember { char_id: id, name: format!("P{id}"), level: 1, class_id: 0, sex: 0, race: 0 };
    world.clans.insert(
        clan_id,
        Clan { id: clan_id, name: "SkillClan".into(), leader_id: 3001, level: 8, reputation_score: 0, castle_id: 0, members: vec![cm(3001), cm(3002)], skills: Default::default(), warehouse: Default::default(), char_penalty_expiry_time: 0, dissolving_expiry_time: 0 },
    );
    for oid in [3001, 3002] {
        world.objects.get_component_mut::<Player>(&oid).unwrap().clan_id = clan_id;
    }

    let clan_skill = |world: &World, oid: i32, id: i32| {
        world.objects.get_component::<ClanSkills>(&oid).is_some_and(|c| c.0.contains_key(&id))
    };
    let has_passive_buff = |world: &World, oid: i32, id: i32| {
        world.objects.get_component::<Buffs>(&oid).is_some_and(|b| b.0.iter().any(|x| x.skill_id == id && x.passive))
    };

    let count = crate::game_loop::clans::give_clan_skills(&mut world, clan_id, false);
    assert_eq!(count, 2, "clan learns both level-3 pledge skills at clan level 8");

    // Stored on the clan and persisted.
    assert_eq!(world.clans[&clan_id].skills.get(&370), Some(&1));
    assert_eq!(world.clans[&clan_id].skills.get(&371), Some(&1));
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::SaveClanSkill { clan_id: c, skill_id: 370, skill_level: 1, .. } if *c == clan_id)));
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::SaveClanSkill { skill_id: 371, .. })));

    // Leader (social 9) gets both; member (social 6) gets only the HEIR skill.
    assert!(clan_skill(&world, 3001, 370) && clan_skill(&world, 3001, 371), "leader gets both");
    assert!(clan_skill(&world, 3002, 370), "member qualifies for the HEIR skill");
    assert!(!clan_skill(&world, 3002, 371), "member is gated out of the COUNT skill");
    // Applied skills land as icon-less passive buffs (stat effect, no abnormal row).
    assert!(has_passive_buff(&world, 3001, 370), "clan skill applied as a passive buff");
    assert!(!has_passive_buff(&world, 3002, 371), "gated-out skill not applied");

    // The clan skill shows in the member's merged SkillList (opcode 0x5F).
    let pkt = super::helpers::skill_list_packet(&world, 3001).expect("skill list");
    assert_eq!(pkt[0], 0x5F);
    let count_in_list = i32::from_le_bytes(pkt[1..5].try_into().unwrap());
    assert!(count_in_list >= 2, "leader's skill list carries the 2 clan skills");

    // Dispersing the clan strips the clan skills from the (still-online) members.
    crate::game_loop::clans::destroy_clan(&mut world, clan_id);
    assert!(!clan_skill(&world, 3001, 370) && !clan_skill(&world, 3001, 371), "leader clan skills cleared on disperse");
    assert!(!has_passive_buff(&world, 3001, 370), "leader clan-skill buff reverted");
    assert!(!clan_skill(&world, 3002, 370), "member clan skills cleared on disperse");
}

/// `//give_clan_skills` self-heal: a clan carrying a residence skill (stored by
/// a pre-fix grant that read the wrong attribute) has it purged — removed from
/// the clan, reverted on online members, DB row deleted — while the grant
/// (re-)applies the real clan skills immediately and reports the clan's actual
/// skill count (not 0) even when it already owned them.
#[test]
fn give_clan_skills_purges_residence_and_reapplies() {
    use crate::model::clan::{Clan, ClanMember};
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::components::ClanSkills;

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    // Clan skill 370 (HEIR, non-residence) + residence skill 590.
    world.data.skill_data.insert_for_test(passive_clan_test_skill(370));
    world.data.skill_data.insert_for_test(passive_clan_test_skill(590));
    world.data.pledge_skill_trees.insert_for_test(
        PledgeSkillLearn { skill_id: 370, skill_level: 1, get_level: 3, social_class: Some(3), residencial: false, level_up_sp: 0 }, false);
    world.data.pledge_skill_trees.insert_for_test(
        PledgeSkillLearn { skill_id: 590, skill_level: 1, get_level: 4, social_class: None, residencial: true, level_up_sp: 0 }, false);

    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0056;
    let cm = |id: i32| ClanMember { char_id: id, name: format!("P{id}"), level: 1, class_id: 0, sex: 0, race: 0 };
    // The clan already "owns" 370 and a residence 590 (as a pre-fix grant left it),
    // and the residence skill is applied to the online leader.
    let mut skills = std::collections::HashMap::new();
    skills.insert(370, 1);
    skills.insert(590, 1);
    world.clans.insert(
        clan_id,
        Clan { id: clan_id, name: "ResClan".into(), leader_id: 3001, level: 8, reputation_score: 0, castle_id: 0, members: vec![cm(3001)], skills, warehouse: Default::default(), char_penalty_expiry_time: 0, dissolving_expiry_time: 0 },
    );
    world.objects.get_component_mut::<Player>(&3001).unwrap().clan_id = clan_id;
    world.objects.add_components(&3001, ClanSkills(std::collections::HashMap::from([(590, 1)])));

    let count = crate::game_loop::clans::give_clan_skills(&mut world, clan_id, false);

    // Residence skill purged from the clan and the member; real skill re-applied.
    assert!(!world.clans[&clan_id].skills.contains_key(&590), "residence skill purged from clan");
    assert!(world.clans[&clan_id].skills.contains_key(&370), "clan skill kept");
    let leader_skills = world.objects.get_component::<ClanSkills>(&3001).unwrap();
    assert!(!leader_skills.0.contains_key(&590), "residence skill reverted on the member");
    assert!(leader_skills.0.contains_key(&370), "real clan skill applied immediately (no relog)");
    // DB row deleted so a relog can't re-apply it.
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::DeleteClanSkill { clan_id: c, skill_id: 590 } if *c == clan_id)), "residence DB row deleted");
    // Saturated clan still reports its real (non-residence) skill count, not 0.
    assert_eq!(count, 1, "reports the clan's applied skill count");
}

/// `Max{Hp,Mp,Cp}Finalizer`: the buff `mul`/`add` modifiers apply as
/// `mul·(base·statBonus) + add`, with equipped-item bonuses added *after* the
/// mul (Java doesn't scale item bonuses by the buff). Regression for the G7 gap
/// where these finalizers ignored buff modifiers entirely.
#[test]
fn max_vitals_finalizers_apply_buff_modifiers() {
    use crate::model::components::StatModifiers;
    use crate::model::stats::Stat;

    let (world, _db_rx, _link_rx) = quest_test_world();
    let t = world.data.player_templates.get(0).cloned().unwrap();
    let mut mods = StatModifiers::default();
    mods.mul.insert(Stat::MaxHp, 1.5);
    mods.add.insert(Stat::MaxHp, 100.0);
    mods.mul.insert(Stat::MaxMp, 2.0);
    mods.mul.insert(Stat::MaxCp, 1.2);

    let hp_base = t.base_hp_max(80) * world.data.stat_bonus.con_bonus(t.base_con);
    let mp_base = t.base_mp_max(80) * world.data.stat_bonus.men_bonus(t.base_men);
    let cp_base = t.base_cp_max(80) * world.data.stat_bonus.con_bonus(t.base_con);

    let hp = crate::model::calc_max_hp(&world.data, &t, 80, None, &mods);
    let mp = crate::model::calc_max_mp(&world.data, &t, 80, None, &mods);
    let cp = crate::model::calc_max_cp(&world.data, &t, 80, &mods);
    assert!((hp - (1.5 * hp_base + 100.0)).abs() < 1e-6, "MaxHp = mul*base + add");
    assert!((mp - (2.0 * mp_base)).abs() < 1e-6, "MaxMp = mul*base");
    assert!((cp - (1.2 * cp_base)).abs() < 1e-6, "MaxCp = mul*base");
    // Empty mods leave the base untouched (mul=1, add=0).
    let none = StatModifiers::default();
    assert!((crate::model::calc_max_hp(&world.data, &t, 80, None, &none) - hp_base).abs() < 1e-6);
}

/// The admin `//superhaste 4` case (Super Haste 7029 L4, a toggle): its
/// `+100% MaxMp` effect must double the MP bar through the active-buff path
/// (`apply_skill_effects` → `recompute_max_vitals`). This is the modifier that
/// was missing from the Archmage's MP (Java applied it, Rust didn't recompute
/// the vitals for it).
#[test]
fn superhaste_maxmp_doubles_mp() {
    use crate::model::components::Vitals;

    let (mut world, _db_rx, _link_rx) = quest_test_world();
    // The real Super Haste 7029 L4 from the datapack (+100% MaxMp, PER).
    let sh = crate::data::skill_data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"))
        .get(7029, 4)
        .expect("Super Haste 7029 L4")
        .clone();
    world.data.skill_data.insert_for_test(sh.clone());

    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let base_mp = world.objects.get_component::<Vitals>(&3001).unwrap().max_mp;

    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, 3001, &sh);

    let after = world.objects.get_component::<Vitals>(&3001).unwrap().max_mp;
    assert!(
        (after - base_mp * 2).abs() <= 1,
        "Super Haste +100% MaxMp doubles the bar: {base_mp} -> {after}"
    );
}

/// Login path: a passive skill in the character's book that carries a `MaxMp`
/// modifier (a mystic's MP passives — most of an Archmage's MP pool) is folded
/// into the vitals at load. Regression: `from_char` computed the vitals before
/// applying the passive skills, so the boosted MP never reached the first
/// `UserInfo` (the character showed only its base MP).
#[test]
fn passive_max_mp_skill_boosts_mp_at_login() {
    use crate::model::components::StatModifiers;
    use crate::model::skill::{OperateType, SkillEffect, StatModifierEffect};
    use crate::model::stats::{Stat, StatModifierType};

    let (mut world, _db_rx, _link_rx) = quest_test_world();
    // A passive skill that doubles MaxMp (+100%), like a stacked mage MP passive.
    let mut s = passive_clan_test_skill(9001);
    s.operate_type = OperateType::Passive;
    s.effects = vec![SkillEffect::StatModifier(StatModifierEffect {
        stat: Stat::MaxMp,
        mode: StatModifierType::Per,
        amount: 100.0,
        armor_condition: 0,
        weapon_condition: 0,
    })];
    world.data.skill_data.insert_for_test(s);

    let t = world.data.player_templates.get(0).cloned().unwrap();
    let base_mp = crate::model::calc_max_mp(&world.data, &t, 1, None, &StatModifiers::default());

    let mut chr = dummy_char(7001, "Mage");
    chr.skills = vec![(9001, 1)];
    let bundle = Player::from_char(&world.data, &chr);
    assert_eq!(bundle.vitals.max_mp, (base_mp * 2.0) as i32, "passive MaxMp folded into max_mp at login");
}

/// End-to-end: clan skills carrying `MaxHp`/`MaxMp`/`MaxCp` modifiers (Clan
/// Health / Clan Mind, the Archmage clan-leader case) now move the HP/MP/CP bar
/// immediately — `%` modifiers stack multiplicatively, flat ones add. Regression
/// for the bug where these clan skills applied as buffs but never changed the
/// vitals (the finalizers ignored the modifier maps).
#[test]
fn clan_skills_move_max_hp_mp_cp() {
    use crate::model::clan::{Clan, ClanMember};
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::components::{PlayerVitals, StatModifiers, Vitals};
    use crate::model::skill::{SkillEffect, StatModifierEffect};
    use crate::model::stats::{Stat, StatModifierType};

    let (mut world, mut db_rx, _link_rx) = quest_test_world();

    // Skill 370: +100% MaxMp and +300 flat MaxHp. Skill 371: +50% MaxMp, +20% MaxCp.
    for (id, effs) in [
        (370, vec![(Stat::MaxMp, StatModifierType::Per, 100.0), (Stat::MaxHp, StatModifierType::Diff, 300.0)]),
        (371, vec![(Stat::MaxMp, StatModifierType::Per, 50.0), (Stat::MaxCp, StatModifierType::Per, 20.0)]),
    ] {
        let mut s = passive_clan_test_skill(id);
        s.effects = effs
            .into_iter()
            .map(|(stat, mode, amount)| SkillEffect::StatModifier(StatModifierEffect { stat, mode, amount, armor_condition: 0, weapon_condition: 0 }))
            .collect();
        world.data.skill_data.insert_for_test(s);
        world.data.pledge_skill_trees.insert_for_test(
            PledgeSkillLearn { skill_id: id, skill_level: 1, get_level: 1, social_class: None, residencial: false, level_up_sp: 0 }, false);
    }

    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);

    // Exact pre-buff maxima (empty modifier maps).
    let (base_hp, base_mp, base_cp) = {
        let p = world.objects.get_component::<Player>(&3001).unwrap();
        let t = world.data.player_templates.get(p.class_id).cloned().unwrap();
        let inv = world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap();
        let none = StatModifiers::default();
        (
            crate::model::calc_max_hp(&world.data, &t, p.level, Some(inv), &none),
            crate::model::calc_max_mp(&world.data, &t, p.level, Some(inv), &none),
            crate::model::calc_max_cp(&world.data, &t, p.level, &none),
        )
    };

    let clan_id = 0x3000_00AA;
    let cm = |id: i32| ClanMember { char_id: id, name: format!("P{id}"), level: 1, class_id: 0, sex: 0, race: 0 };
    world.clans.insert(
        clan_id,
        Clan { id: clan_id, name: "VitalClan".into(), leader_id: 3001, level: 8, reputation_score: 0, castle_id: 0, members: vec![cm(3001)], skills: Default::default(), warehouse: Default::default(), char_penalty_expiry_time: 0, dissolving_expiry_time: 0 },
    );
    world.objects.get_component_mut::<Player>(&3001).unwrap().clan_id = clan_id;

    crate::game_loop::clans::give_clan_skills(&mut world, clan_id, false);

    let v = *world.objects.get_component::<Vitals>(&3001).unwrap();
    let pv = *world.objects.get_component::<PlayerVitals>(&3001).unwrap();
    // MaxMp: two % buffs stack multiplicatively (2.0 * 1.5 = 3.0).
    assert_eq!(v.max_mp, (base_mp * 3.0) as i32, "MaxMp % buffs stacked onto the bar");
    // MaxHp: flat +300.
    assert_eq!(v.max_hp, (base_hp + 300.0) as i32, "flat MaxHp buff applied");
    // MaxCp: +20%.
    assert_eq!(pv.max_cp, (base_cp * 1.2) as i32, "MaxCp % buff applied");
}

/// Siege/leader skills (Java `SiegeManager.addSiegeSkills`): a clan leader gains
/// Build Headquarters (247) + Imprint of Light/Darkness (19034/19035) once the
/// clan reaches level 5, the two Outpost skills (844/845) only with a castle;
/// regular members get none. Delivered through the transient [`ClanSkills`]
/// channel so they show in the merged SkillList without persisting.
#[test]
fn siege_skills_granted_to_level5_clan_leader_only() {
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::ClanSkills;

    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let clan_id = 0x3000_0077;
    let cm = |id: i32| ClanMember { char_id: id, name: format!("P{id}"), level: 1, class_id: 0, sex: 0, race: 0 };
    world.clans.insert(
        clan_id,
        Clan { id: clan_id, name: "SiegeClan".into(), leader_id: 3001, level: 4, reputation_score: 0, castle_id: 0, members: vec![cm(3001), cm(3002)], skills: Default::default(), warehouse: Default::default(), char_penalty_expiry_time: 0, dissolving_expiry_time: 0 },
    );
    for oid in [3001, 3002] {
        world.objects.get_component_mut::<Player>(&oid).unwrap().clan_id = clan_id;
    }

    let has = |world: &World, oid: i32, id: i32| {
        world.objects.get_component::<ClanSkills>(&oid).is_some_and(|c| c.0.contains_key(&id))
    };

    // Level 4: below the siege min level — the leader gets no siege skills.
    crate::game_loop::clans::on_enter_world(&mut world, 1, 3001);
    assert!(!has(&world, 3001, 247), "no siege skills below clan level 5");

    // Reaching level 5 grants the three core siege skills to the online leader.
    crate::game_loop::clans::set_clan_level(&mut world, clan_id, 5);
    for id in [247, 19034, 19035] {
        assert!(has(&world, 3001, id), "leader gains siege skill {id} at clan level 5");
    }
    // No castle yet → no Outpost skills.
    assert!(!has(&world, 3001, 844) && !has(&world, 3001, 845), "Outpost skills need a castle");
    // A regular member never gets siege skills.
    crate::game_loop::clans::on_enter_world(&mut world, 2, 3002);
    assert!(!has(&world, 3002, 247), "non-leader member gets no siege skills");

    // Owning a castle adds the two Outpost skills on the leader's next login.
    world.clans.get_mut(&clan_id).unwrap().castle_id = 3;
    crate::game_loop::clans::on_enter_world(&mut world, 1, 3001);
    assert!(has(&world, 3001, 844) && has(&world, 3001, 845), "castle owner gets Outpost skills");
}

/// A member logging in re-derives the clan's skills (Java `addSkillEffects` on
/// enter-world), gated by social class — nothing is persisted on the player.
#[test]
fn clan_skills_reapply_on_member_login() {
    use crate::model::clan::{Clan, ClanMember};
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::components::ClanSkills;

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    world.data.skill_data.insert_for_test(passive_clan_test_skill(370));
    world.data.pledge_skill_trees.insert_for_test(
        PledgeSkillLearn { skill_id: 370, skill_level: 1, get_level: 3, social_class: Some(3), residencial: false, level_up_sp: 0 },
        false,
    );
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0066;
    let cm = |id: i32| ClanMember { char_id: id, name: format!("P{id}"), level: 1, class_id: 0, sex: 0, race: 0 };
    // The clan already knows skill 370 (as if loaded from clan_skills).
    let mut skills = std::collections::HashMap::new();
    skills.insert(370, 1);
    world.clans.insert(
        clan_id,
        Clan { id: clan_id, name: "SkillClan".into(), leader_id: 3001, level: 8, reputation_score: 0, castle_id: 0, members: vec![cm(3001)], skills, warehouse: Default::default(), char_penalty_expiry_time: 0, dissolving_expiry_time: 0 },
    );
    world.objects.get_component_mut::<Player>(&3001).unwrap().clan_id = clan_id;

    // Simulate the leader's login → clan skills re-applied from the clan.
    crate::game_loop::clans::on_enter_world(&mut world, 1, 3001);
    assert!(
        world.objects.get_component::<ClanSkills>(&3001).is_some_and(|c| c.0.contains_key(&370)),
        "clan skills re-derived on login"
    );
    // Nothing was written to the player's own persisted skill book.
    assert!(
        world.objects.get_component::<crate::model::components::SkillBook>(&3001).is_some_and(|b| !b.0.contains_key(&370)),
        "clan skill is transient — never in the persisted SkillBook"
    );
}

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
    let cm = |id: i32| ClanMember { char_id: id, name: format!("P{id}"), level: 40, class_id: 0, sex: 0, race: 0 };
    world.clans.insert(clan_id, Clan { id: clan_id, name: "Recruiters".into(), leader_id: 3001, level: 3, reputation_score: 0, castle_id: 0, members: vec![cm(3001), cm(3002)], skills: Default::default(), warehouse: Default::default(), char_penalty_expiry_time: 0, dissolving_expiry_time: 0 });

    // ExPledgeRecruitApplyInfo: status DEFAULT (0) — nothing is registered.
    super::clans::handle_request_pledge_recruit_apply_info(&world, 1);
    let pkts = drain(&mut rx);
    let apply = pkts.iter().find(|p| p[0] == 0xFE && p[1] == 0x40 && p[2] == 0x01).expect("ExPledgeRecruitApplyInfo");
    assert_eq!(&apply[3..7], &0i32.to_le_bytes());

    // ExPledgeRecruitInfo: name, leader name, level, member count, 0 sub-pledges.
    super::clans::handle_request_pledge_recruit_info(&world, 1, &clan_id.to_le_bytes());
    let pkts = drain(&mut rx);
    let info = pkts.iter().find(|p| p[0] == 0xFE && p[1] == 0x3F && p[2] == 0x01).expect("ExPledgeRecruitInfo");
    let mut r = commons::network::PacketReader::new(&info[3..]);
    assert_eq!(r.read_string().unwrap(), "Recruiters");
    assert_eq!(r.read_string().unwrap(), "P3001");
    assert_eq!(r.read_i32().unwrap(), 3); // clan level
    assert_eq!(r.read_i32().unwrap(), 2); // member count
    assert_eq!(r.read_i32().unwrap(), 0); // sub-pledge count

    // Unknown clan id: Java's ClanTable miss returns without an answer.
    super::clans::handle_request_pledge_recruit_info(&world, 1, &0x7999_9999i32.to_le_bytes());
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
    super::clans::handle_request_pledge_recruit_board_search(&world, 1, &body);
    let pkts = drain(&mut rx);
    let page = pkts.iter().find(|p| p[0] == 0xFE && p[1] == 0x41 && p[2] == 0x01).expect("ExPledgeRecruitBoardSearch");
    let mut r = commons::network::PacketReader::new(&page[3..]);
    assert_eq!(r.read_i32().unwrap(), 3); // current page echoed
    assert_eq!(r.read_i32().unwrap(), 0); // total pages
    assert_eq!(r.read_i32().unwrap(), 0); // clans on this page

    // Truncated packet (missing the page int): dropped silently.
    super::clans::handle_request_pledge_recruit_board_search(&world, 1, &body[..body.len() - 8]);
    assert!(drain(&mut rx).is_empty());
}

// --- G18 slice 1: membership lifecycle ---

/// Build a clan of `members` (first is leader) directly in the world and wire
/// the members' Player clan fields — the fixture every lifecycle test starts
/// from.
fn install_clan(world: &mut World, clan_id: i32, member_oids: &[i32]) {
    let cm = |char_id: i32| crate::model::clan::ClanMember {
        char_id,
        name: format!("P{char_id}"),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
    };
    world.clans.insert(
        clan_id,
        crate::model::clan::Clan {
            id: clan_id,
            name: format!("Clan{clan_id}"),
            leader_id: member_oids[0],
            level: 1,
            reputation_score: 0,
            castle_id: 0,
            members: member_oids.iter().map(|&o| cm(o)).collect(),
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
        },
    );
    for (i, &oid) in member_oids.iter().enumerate() {
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.clan_id = clan_id;
            p.clan_leader = i == 0;
            p.clan_privs = if i == 0 { crate::model::clan::ALL_CLAN_PRIVILEGES } else { 0 };
        }
    }
}

fn invite_body(target_oid: i32, pledge_type: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(target_oid);
    w.write_i32(pledge_type);
    w.into_bytes()
}

fn answer_body(answer: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(answer);
    w.into_bytes()
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
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::YOU_HAVE_INVITED_THE_WRONG_TARGET));

    // Self-invite.
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3001, 0));
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::YOU_CANNOT_ASK_YOURSELF_TO_APPLY_TO_A_CLAN));

    // No CL_JOIN_CLAN privilege: a plain member inviting.
    let mut c_rx = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    install_clan(&mut world, 5001, &[3004, 3003]); // 3003 is a plain member of another clan
    clans::handle_request_join_pledge(&mut world, 3, &invite_body(3002, 0));
    assert!(sm_ids_of(&drain(&mut c_rx)).contains(&server_packets::sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT));

    // Target already clanned.
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3003, 0));
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::S1_IS_ALREADY_A_MEMBER_OF_ANOTHER_CLAN));

    // Target under the rejoin penalty.
    world.objects.get_component_mut::<Player>(&3002).unwrap().clan_join_expiry_time = i64::MAX;
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3002, 0));
    assert!(sm_ids_of(&drain(&mut a_rx))
        .contains(&server_packets::sm_ids::C1_CANNOT_JOIN_THE_CLAN_ONE_DAY_HAS_NOT_PASSED_SINCE_LEAVING));
    world.objects.get_component_mut::<Player>(&3002).unwrap().clan_join_expiry_time = 0;

    // Clan under the post-oust penalty.
    world.clans.get_mut(&5000).unwrap().char_penalty_expiry_time = i64::MAX;
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3002, 0));
    assert!(sm_ids_of(&drain(&mut a_rx))
        .contains(&server_packets::sm_ids::AFTER_A_CLAN_MEMBER_IS_DISMISSED_THE_CLAN_MUST_WAIT_A_DAY));
    world.clans.get_mut(&5000).unwrap().char_penalty_expiry_time = 0;

    // Clan full (level 1 main pledge caps at 15).
    for i in 0..14 {
        let cm = crate::model::clan::ClanMember {
            char_id: 8000 + i,
            name: format!("F{i}"),
            level: 1,
            class_id: 0,
            sex: 0,
            race: 0,
        };
        world.clans.get_mut(&5000).unwrap().members.push(cm);
    }
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3002, 0));
    assert!(sm_ids_of(&drain(&mut a_rx))
        .contains(&server_packets::sm_ids::S1_IS_FULL_AND_CANNOT_ACCEPT_ADDITIONAL_CLAN_MEMBERS));
    world.clans.get_mut(&5000).unwrap().members.retain(|m| m.char_id < 8000);

    // Valid invite → AskJoinPledge on B, the request slot armed on both sides.
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3002, 0));
    let pkts = drain(&mut b_rx);
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::ASK_JOIN_PLEDGE));
    assert!(world.objects.has_component::<crate::model::components::PendingRequest>(&3001));
    assert!(world.objects.has_component::<crate::model::components::PendingRequest>(&3002));

    // A second invite while the slot is busy → "on another task".
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3002, 0));
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::C1_IS_ON_ANOTHER_TASK_PLEASE_TRY_AGAIN_LATER));

    // Decline: both sides notified, slots freed.
    clans::handle_request_answer_join_pledge(&mut world, 2, &answer_body(0));
    assert!(sm_ids_of(&drain(&mut b_rx))
        .contains(&server_packets::sm_ids::YOU_DIDN_T_RESPOND_TO_S1_S_INVITATION_JOINING_HAS_BEEN_CANCELLED));
    assert!(sm_ids_of(&drain(&mut a_rx))
        .contains(&server_packets::sm_ids::S1_DID_NOT_RESPOND_INVITATION_TO_THE_CLAN_HAS_BEEN_CANCELLED));
    assert!(!world.objects.has_component::<crate::model::components::PendingRequest>(&3001));
    assert!(!world.objects.has_component::<crate::model::components::PendingRequest>(&3002));

    // Accept: the join burst.
    world.objects.get_component_mut::<Player>(&3002).unwrap().clan_join_expiry_time = 0;
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3002, 0));
    drain(&mut b_rx);
    drain_db(&mut db_rx);
    clans::handle_request_answer_join_pledge(&mut world, 2, &answer_body(1));

    let b = world.objects.get_component::<Player>(&3002).unwrap();
    assert_eq!((b.clan_id, b.clan_privs, b.clan_leader), (5000, 0, false));
    assert!(world.clans[&5000].member(3002).is_some());
    let b_pkts = drain(&mut b_rx);
    assert!(b_pkts.iter().any(|p| p[0] == server_packets::opcodes::JOIN_PLEDGE));
    assert!(b_pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_ALL));
    let b_sms = sm_ids_of(&b_pkts);
    assert!(b_sms.contains(&server_packets::sm_ids::ENTERED_THE_CLAN));
    assert!(b_sms.contains(&server_packets::sm_ids::S1_HAS_JOINED_THE_CLAN));
    let a_pkts = drain(&mut a_rx);
    assert!(a_pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_ADD));
    assert!(a_pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_INFO_UPDATE));
    assert!(sm_ids_of(&a_pkts).contains(&server_packets::sm_ids::S1_HAS_JOINED_THE_CLAN));
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::UpdateCharClan { char_id: 3002, clan_id: 5000, clan_privs: 0 })));
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::UpdateCharClanJoinExpiry { char_id: 3002, expiry: 0 })));
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
    assert!(sm_ids_of(&drain(&mut c_rx))
        .contains(&server_packets::sm_ids::YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION));

    // The leader cannot withdraw.
    clans::handle_request_withdrawal_pledge(&mut world, 1);
    assert!(sm_ids_of(&drain(&mut a_rx))
        .contains(&server_packets::sm_ids::A_CLAN_LEADER_CANNOT_WITHDRAW_FROM_THEIR_OWN_CLAN));

    // A member in combat cannot withdraw / be dismissed.
    super::combat::refresh_attack_stance(&mut world, 3002);
    clans::handle_request_withdrawal_pledge(&mut world, 2);
    assert!(sm_ids_of(&drain(&mut b_rx)).contains(&server_packets::sm_ids::YOU_CANNOT_LEAVE_A_CLAN_WHILE_ENGAGED_IN_COMBAT));
    clans::handle_request_oust_pledge_member(&mut world, 1, &oust_body("P3002"));
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::A_CLAN_MEMBER_MAY_NOT_BE_DISMISSED_DURING_COMBAT));
    world.tick += 10_000; // combat stance expires

    // Self-oust.
    clans::handle_request_oust_pledge_member(&mut world, 1, &oust_body("P3001"));
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::YOU_CANNOT_DISMISS_YOURSELF));

    // Withdraw: penalty stamped, roster shrinks, both sides messaged.
    clans::handle_request_withdrawal_pledge(&mut world, 2);
    let b = world.objects.get_component::<Player>(&3002).unwrap();
    assert_eq!(b.clan_id, 0);
    assert!(b.clan_join_expiry_time > 0, "rejoin penalty stamped");
    assert!(world.clans[&5000].member(3002).is_none());
    let b_pkts = drain(&mut b_rx);
    assert!(b_pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_DELETE_ALL));
    let b_sms = sm_ids_of(&b_pkts);
    assert!(b_sms.contains(&server_packets::sm_ids::YOU_HAVE_WITHDRAWN_FROM_THE_CLAN));
    assert!(b_sms.contains(&server_packets::sm_ids::AFTER_LEAVING_A_CLAN_YOU_MUST_WAIT_A_DAY_BEFORE_JOINING_ANOTHER));
    let a_pkts = drain(&mut a_rx);
    assert!(sm_ids_of(&a_pkts).contains(&server_packets::sm_ids::S1_HAS_WITHDRAWN_FROM_THE_CLAN));
    assert!(a_pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_DELETE));
    assert!(drain_db(&mut db_rx)
        .iter()
        .any(|c| matches!(c, db::DbCommand::RemoveClanMember { char_id: 3002, .. })));

    // Oust an offline member: roster row goes, the clan takes the invite
    // penalty, the DB reset covers the offline character.
    world.clans.get_mut(&5000).unwrap().members.push(crate::model::clan::ClanMember {
        char_id: 3005,
        name: "P3005".into(),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
    });
    clans::handle_request_oust_pledge_member(&mut world, 1, &oust_body("P3005"));
    assert!(world.clans[&5000].member(3005).is_none());
    assert!(world.clans[&5000].char_penalty_expiry_time > 0, "clan invite penalty stamped");
    let a_sms = sm_ids_of(&drain(&mut a_rx));
    assert!(a_sms.contains(&server_packets::sm_ids::CLAN_MEMBER_S1_HAS_BEEN_EXPELLED));
    assert!(a_sms.contains(&server_packets::sm_ids::YOU_HAVE_SUCCEEDED_IN_EXPELLING_THE_CLAN_MEMBER));
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::RemoveClanMember { char_id: 3005, .. })));
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::UpdateClanPenalties { clan_id: 5000, .. })));
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
        handle_request_bypass_to_server(world, client, &bypass_body(&format!("npc_{NPC_OID}_{verb}")));
    };

    // A non-leader asking for dissolution.
    bypass(&mut world, 2, "dissolve_clan");
    assert!(sm_ids_of(&drain(&mut b_rx)).contains(&server_packets::sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT));

    // The leader: stamp + persistence + the scheduled removal armed.
    bypass(&mut world, 1, "dissolve_clan");
    assert!(world.clans[&5000].dissolving_expiry_time > 0);
    assert!(drain_db(&mut db_rx)
        .iter()
        .any(|c| matches!(c, db::DbCommand::UpdateClanPenalties { clan_id: 5000, dissolving_expiry_time, .. }
            if *dissolving_expiry_time > 0)));

    // Asking again while pending.
    bypass(&mut world, 1, "dissolve_clan");
    assert!(sm_ids_of(&drain(&mut a_rx))
        .contains(&server_packets::sm_ids::YOU_HAVE_ALREADY_REQUESTED_THE_DISSOLUTION_OF_YOUR_CLAN));

    // Recover: the stamp is zeroed and a firing task no-ops.
    bypass(&mut world, 1, "recover_clan");
    assert_eq!(world.clans[&5000].dissolving_expiry_time, 0);
    clans::handle_clan_dissolve_task(&mut world, 5000);
    assert!(world.clans.contains_key(&5000), "recovered clan survives the stale task");

    // A due stamp destroys the clan (members reset, windows closed).
    world.clans.get_mut(&5000).unwrap().dissolving_expiry_time = 1;
    drain(&mut a_rx);
    drain(&mut b_rx);
    clans::handle_clan_dissolve_task(&mut world, 5000);
    assert!(!world.clans.contains_key(&5000));
    assert_eq!(world.objects.get_component::<Player>(&3001).unwrap().clan_id, 0);
    assert_eq!(world.objects.get_component::<Player>(&3002).unwrap().clan_id, 0);
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::CLAN_HAS_DISPERSED));
    assert!(drain(&mut b_rx).iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_DELETE_ALL));
}

// --- G18 slice 2: clan level-up + pledge skill learning ---

/// The village-master `increase_clan_level` ladder: leader/dissolution gates,
/// the not-met reject, and a successful 0→1 upgrade (SP + adena consumed,
/// level broadcast + FX) and 2→3 (Blood Mark proof items).
#[test]
fn clan_level_up_ladder() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    add_quest_items(&mut world, &[(57, "Adena", false), (1419, "Blood Mark", false)]);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3002]);
    world.clans.get_mut(&5000).unwrap().level = 0;
    drain_db(&mut db_rx);

    let bypass = |world: &mut World, client: u32| {
        handle_request_bypass_to_server(world, client, &bypass_body(&format!("npc_{NPC_OID}_increase_clan_level")));
    };

    // Non-leader.
    bypass(&mut world, 2);
    assert!(sm_ids_of(&drain(&mut b_rx)).contains(&server_packets::sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT));

    // Pending dissolution.
    world.clans.get_mut(&5000).unwrap().dissolving_expiry_time = i64::MAX;
    bypass(&mut world, 1);
    assert!(sm_ids_of(&drain(&mut a_rx))
        .contains(&server_packets::sm_ids::AS_YOU_ARE_SCHEDULED_FOR_CLAN_DISSOLUTION_LEVEL_CANNOT_INCREASE));
    world.clans.get_mut(&5000).unwrap().dissolving_expiry_time = 0;

    // No SP / no adena.
    bypass(&mut world, 1);
    assert!(sm_ids_of(&drain(&mut a_rx))
        .contains(&server_packets::sm_ids::THE_CONDITIONS_TO_INCREASE_THE_CLAN_S_LEVEL_HAVE_NOT_BEEN_MET));

    // 0 → 1: 1000 SP + 150k adena.
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.sp = 5_000;
    }
    world.objects.get_component_mut::<crate::model::inventory::Inventory>(&3001).unwrap().add_item(
        &world.data.item_data,
        900_001,
        57,
        200_000,
    );
    bypass(&mut world, 1);
    assert_eq!(world.clans[&5000].level, 1);
    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.sp, 4_000, "1000 SP consumed");
    assert_eq!(adena_of(&world, 3001), 50_000, "150k adena consumed");
    let a_pkts = drain(&mut a_rx);
    let a_sms = sm_ids_of(&a_pkts);
    assert!(a_sms.contains(&server_packets::sm_ids::S1_ADENA_DISAPPEARED));
    assert!(a_sms.contains(&server_packets::sm_ids::YOUR_SP_HAS_DECREASED_BY_S1));
    assert!(a_sms.contains(&server_packets::sm_ids::YOUR_CLAN_S_LEVEL_HAS_INCREASED));
    assert!(a_pkts.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE), "level-up FX");
    assert!(sm_ids_of(&drain(&mut b_rx)).contains(&server_packets::sm_ids::YOUR_CLAN_S_LEVEL_HAS_INCREASED));
    assert!(drain_db(&mut db_rx)
        .iter()
        .any(|c| matches!(c, db::DbCommand::UpdateClanLevel { clan_id: 5000, level: 1 })));

    // 2 → 3: 100k SP + 100 Blood Marks.
    world.clans.get_mut(&5000).unwrap().level = 2;
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.sp = 200_000;
    }
    world.objects.get_component_mut::<crate::model::inventory::Inventory>(&3001).unwrap().add_item(
        &world.data.item_data,
        900_002,
        1419,
        150,
    );
    bypass(&mut world, 1);
    assert_eq!(world.clans[&5000].level, 3);
    assert_eq!(count_of_item(&world, 3001, 1419), 50, "100 Blood Marks consumed");
    assert_eq!(world.objects.get_component::<Player>(&3001).unwrap().sp, 100_000);
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::S1_DISAPPEARED));
}

/// The pledge-skill learn flow: the leader-only list (`ExAcquirableSkillListBy
/// Class` PLEDGE), the reputation gate, and a successful learn — rep deducted
/// + persisted, the skill stored/broadcast, and the refreshed list offering
/// the next level.
#[test]
fn pledge_skill_learning_spends_reputation() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    world.data.skill_data.insert_for_test(passive_clan_test_skill(370));
    let learn = |lvl: i32, sp: i64| PledgeSkillLearn {
        skill_id: 370,
        skill_level: lvl,
        get_level: 3,
        social_class: None,
        residencial: false,
        level_up_sp: sp,
    };
    world.data.pledge_skill_trees.insert_for_test(learn(1, 1_500), false);
    world.data.pledge_skill_trees.insert_for_test(learn(2, 3_000), false);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3002]);
    world.clans.get_mut(&5000).unwrap().level = 3;
    drain_db(&mut db_rx);

    // Non-leader asking for the list → NotClanLeader.htm (an NpcHtmlMessage).
    handle_request_bypass_to_server(&mut world, 2, &bypass_body(&format!("npc_{NPC_OID}_learn_clan_skills")));
    assert!(drain(&mut b_rx).iter().any(|p| decode_npc_html(p).is_some()), "NotClanLeader html shown");

    // Leader: the PLEDGE learnable list with the level-1 entry.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_learn_clan_skills")));
    let pkts = drain(&mut a_rx);
    let list = pkts
        .iter()
        .find(|p| p[0] == 0xFE && p.len() > 3 && i16::from_le_bytes([p[1], p[2]]) == 0xFA)
        .expect("ExAcquirableSkillListByClass sent");
    assert_eq!(i16::from_le_bytes([list[3], list[4]]), 2, "PLEDGE type");

    // The info request answers with the reputation cost.
    clans::handle_request_pledge_skill_info(&world, 1, 370, 1);
    let info = drain(&mut a_rx);
    assert!(info.iter().any(|p| p[0] == server_packets::opcodes::ACQUIRE_SKILL_INFO));

    // Learning without reputation fails.
    clans::handle_learn_pledge_skill(&mut world, 1, 370, 1);
    assert!(sm_ids_of(&drain(&mut a_rx))
        .contains(&server_packets::sm_ids::SKILL_ACQUIRE_FAILED_INSUFFICIENT_CLAN_REPUTATION));
    assert!(world.clans[&5000].skills.is_empty());

    // Skipping a level is silently refused (Java's prev-level hack check).
    world.clans.get_mut(&5000).unwrap().reputation_score = 10_000;
    clans::handle_learn_pledge_skill(&mut world, 1, 370, 2);
    assert!(world.clans[&5000].skills.is_empty());

    // A successful learn: rep −1500 (persisted), skill stored + pushed.
    drain_db(&mut db_rx);
    clans::handle_learn_pledge_skill(&mut world, 1, 370, 1);
    assert_eq!(world.clans[&5000].skills.get(&370), Some(&1));
    assert_eq!(world.clans[&5000].reputation_score, 8_500);
    let a_pkts = drain(&mut a_rx);
    let a_sms = sm_ids_of(&a_pkts);
    assert!(a_sms.contains(&server_packets::sm_ids::S1_POINTS_HAVE_BEEN_DEDUCTED_FROM_THE_CLAN_S_REPUTATION));
    assert!(a_sms.contains(&server_packets::sm_ids::THE_CLAN_SKILL_S1_HAS_BEEN_ADDED));
    assert!(a_pkts.iter().any(|p| p[0] == server_packets::opcodes::ACQUIRE_SKILL_DONE));
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::UpdateClanReputation { clan_id: 5000, reputation: 8_500 })));
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::SaveClanSkill { clan_id: 5000, skill_id: 370, skill_level: 1, .. })));
    // The member got the passive too (no social gate on the fixture).
    assert!(world
        .objects
        .get_component::<crate::model::components::ClanSkills>(&3002)
        .is_some_and(|c| c.0.get(&370) == Some(&1)));

    // The re-shown list now offers level 2.
    clans::handle_learn_pledge_skill(&mut world, 1, 370, 2);
    assert_eq!(world.clans[&5000].skills.get(&370), Some(&2));
    assert_eq!(world.clans[&5000].reputation_score, 5_500);
}

/// `RequestPledgeDraftListSearch` (ex 0xDC): the draft-list tab always gets
/// an `ExPledgeDraftListSearch` back — empty until the G18 `ClanEntryManager`
/// lands (0 waiting-list entries), and a short/malformed packet is dropped
/// without an answer.
#[test]
fn clan_draft_list_search() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    // levelMin=0, levelMax=107, classId=-1, query="", sortBy=1, descending=1
    // — the clan window's default "show all" draft search.
    let mut body = Vec::new();
    body.extend(0i32.to_le_bytes());
    body.extend(107i32.to_le_bytes());
    body.extend((-1i32).to_le_bytes());
    body.extend(0u16.to_le_bytes()); // empty UTF-16 string (terminator only)
    body.extend(1i32.to_le_bytes());
    body.extend(1i32.to_le_bytes());
    super::clans::handle_request_pledge_draft_list_search(&world, 1, &body);
    let pkts = drain(&mut rx);
    let list = pkts.iter().find(|p| p[0] == 0xFE && p[1] == 0x46 && p[2] == 0x01).expect("ExPledgeDraftListSearch");
    let mut r = commons::network::PacketReader::new(&list[3..]);
    assert_eq!(r.read_i32().unwrap(), 0); // waiting-list entries

    // Truncated packet (missing the sort ints): dropped silently.
    super::clans::handle_request_pledge_draft_list_search(&world, 1, &body[..body.len() - 8]);
    assert!(drain(&mut rx).is_empty());
}
