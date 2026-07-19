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
    world.clans.insert(clan_id, Clan { id: clan_id, name: "WhClan".into(), leader_id: 3001, level: 1, reputation_score: 0, castle_id: 0, members: vec![cm(3001), cm(3002)], skills: Default::default(), warehouse: Default::default() });
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
        Clan { id: clan_id, name: "AdventClan".into(), leader_id: 3001, level: 1, reputation_score: 0, castle_id: 0, members: vec![cm(3001), cm(3002)], skills: Default::default(), warehouse: Default::default() },
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
    let learn = |id, social| PledgeSkillLearn { skill_id: id, skill_level: 1, get_level: 3, social_class: Some(social), residencial: false };
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
        Clan { id: clan_id, name: "SkillClan".into(), leader_id: 3001, level: 8, reputation_score: 0, castle_id: 0, members: vec![cm(3001), cm(3002)], skills: Default::default(), warehouse: Default::default() },
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
        PledgeSkillLearn { skill_id: 370, skill_level: 1, get_level: 3, social_class: Some(3), residencial: false }, false);
    world.data.pledge_skill_trees.insert_for_test(
        PledgeSkillLearn { skill_id: 590, skill_level: 1, get_level: 4, social_class: None, residencial: true }, false);

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
        Clan { id: clan_id, name: "ResClan".into(), leader_id: 3001, level: 8, reputation_score: 0, castle_id: 0, members: vec![cm(3001)], skills, warehouse: Default::default() },
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
            PledgeSkillLearn { skill_id: id, skill_level: 1, get_level: 1, social_class: None, residencial: false }, false);
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
        Clan { id: clan_id, name: "VitalClan".into(), leader_id: 3001, level: 8, reputation_score: 0, castle_id: 0, members: vec![cm(3001)], skills: Default::default(), warehouse: Default::default() },
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
        Clan { id: clan_id, name: "SiegeClan".into(), leader_id: 3001, level: 4, reputation_score: 0, castle_id: 0, members: vec![cm(3001), cm(3002)], skills: Default::default(), warehouse: Default::default() },
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
        PledgeSkillLearn { skill_id: 370, skill_level: 1, get_level: 3, social_class: Some(3), residencial: false },
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
        Clan { id: clan_id, name: "SkillClan".into(), leader_id: 3001, level: 8, reputation_score: 0, castle_id: 0, members: vec![cm(3001)], skills, warehouse: Default::default() },
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
    world.clans.insert(clan_id, Clan { id: clan_id, name: "Recruiters".into(), leader_id: 3001, level: 3, reputation_score: 0, castle_id: 0, members: vec![cm(3001), cm(3002)], skills: Default::default(), warehouse: Default::default() });

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
