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
        std::fs::read_to_string(format!("{root}data/scripts/village_master/ClanMaster/{name}"))
            .expect(name)
            .replace("%objectId%", &NPC_OID.to_string())
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
    world.clans.insert(clan_id, Clan { id: clan_id, name: "WhClan".into(), leader_id: 3001, level: 1, reputation_score: 0, castle_id: 0, members: vec![cm(3001), cm(3002)], warehouse: Default::default() });
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
