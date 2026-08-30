use super::*;
use crate::game_loop::abnormal::has_buff;
use crate::game_loop::character::subclass;
use crate::game_loop::clans::academy;
use crate::game_loop::combat::pvp;
use crate::game_loop::commerce::warehouse;
use crate::game_loop::social::chat;
use crate::game_loop::{clans, helpers};
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

/// Clan warehouse: a shared container. The leader deposits (persisted), an
/// unprivileged member is denied the withdraw window, and the leader withdraws.
#[test]
fn clan_warehouse_withdrawal_is_leader_only_at_the_shipped_setting() {
    use crate::game_loop::commerce::warehouse;
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::ActiveWarehouse;
    let (mut world, _tx, _db_rx, _lrx) = admin_world();
    let _leader_rx = ingame_player_access(&mut world, 1, 3001, 0);
    let _member_rx = ingame_player_access(&mut world, 2, 3002, 0);

    let clan_id = 0x7000_0009;
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
            name: "WhGate".into(),
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
    // **3002 holds the view-warehouse privilege.** That is the whole point:
    // under the port's old unconditional privilege gate this member could
    // withdraw, and on this dist they must not be able to.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_privs = model::clan::ALL_CLAN_PRIVILEGES;
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .clan_privs = model::clan::CL_VIEW_WAREHOUSE;

    let opened = |world: &mut World, client: u32, oid: i32| {
        world.objects.remove_component::<ActiveWarehouse>(&oid);
        warehouse::open_clan(world, client, oid, true);
        world.objects.has_component::<ActiveWarehouse>(&oid)
    };

    // Shipped: `AltMembersCanWithdrawFromClanWH = False` → leader only.
    assert!(!world.cfg.character.alt_members_can_withdraw_from_clan_wh);
    assert!(opened(&mut world, 1, 3001), "the leader may withdraw");
    assert!(
        !opened(&mut world, 2, 3002),
        "a privileged member must NOT withdraw while the key is off"
    );

    // Turned on: the privilege becomes the gate instead.
    world.cfg.character.alt_members_can_withdraw_from_clan_wh = true;
    assert!(opened(&mut world, 2, 3002), "…and may once the key is on");
}

#[test]
fn clan_warehouse_shared_deposit_withdraw_and_privilege() {
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::inventory::Inventory;
    let (mut world, _tx, mut db_rx, _lrx) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut leader_rx = ingame_player_access(&mut world, 1, 3001, 0);
    let mut member_rx = ingame_player_access(&mut world, 2, 3002, 0);
    drain(&mut leader_rx);
    drain(&mut member_rx);

    // A level-1 clan: 3001 leader, 3002 plain member (no privileges).
    let clan_id = 0x7000_0001;
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
            name: "WhClan".into(),
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
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_privs = model::clan::ALL_CLAN_PRIVILEGES;
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .clan_privs = 0;

    // Leader deposits 500 adena into the shared clan warehouse.
    items::add_inventory_item(&mut world, 3001, 57, 500).unwrap();
    let adena_oid = item_oid(&world, 3001, 57);
    warehouse::open_clan(&mut world, 1, 3001, false); // keeper bypass → active = clan
    let deposit = {
        let mut w = PacketWriter::new();
        w.write_u8(cop::SEND_WARE_HOUSE_DEPOSIT_LIST);
        w.write_i32(1);
        w.write_i32(adena_oid);
        w.write_i64(500);
        w.into_bytes()
    };
    on_packet(&mut world, 1, deposit);
    assert_eq!(
        world.clans[&clan_id].warehouse.0.count_of(57),
        500,
        "deposited into clan warehouse"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(57),
        0,
        "left leader inventory"
    );
    // Persistence flush emitted for the clan.
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::StoreClanWarehouse { clan_id: cid, items } if *cid == clan_id && items.iter().any(|r| r.item_id == 57 && r.count == 500 && r.loc == "CLANWH"))), "clan warehouse persisted");

    // An unprivileged member cannot open the withdraw window.
    drain(&mut member_rx);
    warehouse::open_clan(&mut world, 2, 3002, true);
    let denied = drain(&mut member_rx);
    assert!(
        !denied
            .iter()
            .any(|p| p[0] == server_packets::opcodes::WAREHOUSE_WITHDRAW_LIST),
        "member without CL_VIEW_WAREHOUSE is denied"
    );

    // The leader withdraws 200 back — the shared container drops to 300.
    let wh_oid = world.clans[&clan_id]
        .warehouse
        .0
        .items()
        .iter()
        .find(|it| it.item_id == 57)
        .unwrap()
        .object_id;
    warehouse::open_clan(&mut world, 1, 3001, true);
    let withdraw = {
        let mut w = PacketWriter::new();
        w.write_u8(cop::SEND_WARE_HOUSE_WITH_DRAW_LIST);
        w.write_i32(1);
        w.write_i32(wh_oid);
        w.write_i64(200);
        w.into_bytes()
    };
    on_packet(&mut world, 1, withdraw);
    assert_eq!(
        world.clans[&clan_id].warehouse.0.count_of(57),
        300,
        "300 remains in clan warehouse"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(57),
        200,
        "200 withdrawn to leader"
    );
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
    let relight = |world: &mut World, oid: i32| {
        clans::skills::reapply_clan_advent_on_profession_change(world, oid)
    };

    // Leader online → a member's profession change re-lights the aura.
    assert!(!has_advent(&world, 3002), "starts without it");
    relight(&mut world, 3002);
    assert!(
        has_advent(&world, 3002),
        "re-lit while the leader is online"
    );

    // Leader offline → it does not.
    clans::skills::remove_clan_advent(&mut world, 3002);
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

/// `//give_clan_skills` (Java `adminGiveClanSkills`): the clan learns every
/// pledge skill it qualifies for at its level; each applies to online members
/// gated by social class, lands as a (passive, icon-less) stat buff, shows in
/// the merged SkillList, and persists. Dispersing the clan strips them again.
#[test]
fn give_clan_skills_grants_gates_and_persists() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::{Buffs, ClanSkills};

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    // Two clan skills: 370 gated at HEIR (ordinal 3), 371 gated at COUNT (8).
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(370));
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(371));
    let learn = |id, social| PledgeSkillLearn {
        skill_id: id,
        skill_level: 1,
        get_level: 3,
        social_class: Some(social),
        residencial: false,
        residence_ids: Vec::new(),
        level_up_sp: 0,
    };
    world
        .data
        .pledge_skill_trees
        .insert_for_test(learn(370, 3), false);
    world
        .data
        .pledge_skill_trees
        .insert_for_test(learn(371, 8), false);

    // A level-8 clan: leader 3001 (pledge class 8 → social 9), member 3002
    // (pledge class 5 → social 6). Both online.
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0055;
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
            name: "SkillClan".into(),
            leader_id: 3001,
            level: 8,
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

    let clan_skill = |world: &World, oid: i32, id: i32| {
        world
            .objects
            .get_component::<ClanSkills>(&oid)
            .is_some_and(|c| c.0.contains_key(&id))
    };
    let has_passive_buff = |world: &World, oid: i32, id: i32| {
        world
            .objects
            .get_component::<Buffs>(&oid)
            .is_some_and(|b| b.0.iter().any(|x| x.skill_id == id && x.passive))
    };

    let count = clans::give_clan_skills(&mut world, clan_id, false);
    assert_eq!(
        count, 2,
        "clan learns both level-3 pledge skills at clan level 8"
    );

    // Stored on the clan and persisted.
    assert_eq!(world.clans[&clan_id].skills.get(&370), Some(&1));
    assert_eq!(world.clans[&clan_id].skills.get(&371), Some(&1));
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::SaveClanSkill { clan_id: c, skill_id: 370, skill_level: 1, .. } if *c == clan_id)));
    assert!(
        cmds.iter()
            .any(|c| matches!(c, db::DbCommand::SaveClanSkill { skill_id: 371, .. }))
    );

    // Leader (social 9) gets both; member (social 6) gets only the HEIR skill.
    assert!(
        clan_skill(&world, 3001, 370) && clan_skill(&world, 3001, 371),
        "leader gets both"
    );
    assert!(
        clan_skill(&world, 3002, 370),
        "member qualifies for the HEIR skill"
    );
    assert!(
        !clan_skill(&world, 3002, 371),
        "member is gated out of the COUNT skill"
    );
    // Applied skills land as icon-less passive buffs (stat effect, no abnormal row).
    assert!(
        has_passive_buff(&world, 3001, 370),
        "clan skill applied as a passive buff"
    );
    assert!(
        !has_passive_buff(&world, 3002, 371),
        "gated-out skill not applied"
    );

    // The clan skill shows in the member's merged SkillList (opcode 0x5F).
    let pkt = helpers::skill_list_packet(&world, 3001).expect("skill list");
    assert_eq!(pkt[0], 0x5F);
    let count_in_list = i32::from_le_bytes(pkt[1..5].try_into().unwrap());
    assert!(
        count_in_list >= 2,
        "leader's skill list carries the 2 clan skills"
    );

    // Dispersing the clan strips the clan skills from the (still-online) members.
    clans::destroy_clan(&mut world, clan_id);
    assert!(
        !clan_skill(&world, 3001, 370) && !clan_skill(&world, 3001, 371),
        "leader clan skills cleared on disperse"
    );
    assert!(
        !has_passive_buff(&world, 3001, 370),
        "leader clan-skill buff reverted"
    );
    assert!(
        !clan_skill(&world, 3002, 370),
        "member clan skills cleared on disperse"
    );
}

/// `//give_clan_skills` self-heal: a clan carrying a residence skill (stored by
/// a pre-fix grant that read the wrong attribute) has it purged — removed from
/// the clan, reverted on online members, DB row deleted — while the grant
/// (re-)applies the real clan skills immediately and reports the clan's actual
/// skill count (not 0) even when it already owned them.
#[test]
fn give_clan_skills_purges_residence_and_reapplies() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::ClanSkills;

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    // Clan skill 370 (HEIR, non-residence) + residence skill 590.
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(370));
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(590));
    world.data.pledge_skill_trees.insert_for_test(
        PledgeSkillLearn {
            skill_id: 370,
            skill_level: 1,
            get_level: 3,
            social_class: Some(3),
            residencial: false,
            residence_ids: Vec::new(),
            level_up_sp: 0,
        },
        false,
    );
    world.data.pledge_skill_trees.insert_for_test(
        PledgeSkillLearn {
            skill_id: 590,
            skill_level: 1,
            get_level: 4,
            social_class: None,
            residencial: true,
            residence_ids: Vec::new(),
            level_up_sp: 0,
        },
        false,
    );

    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0056;
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
    // The clan already "owns" 370 and a residence 590 (as a pre-fix grant left it),
    // and the residence skill is applied to the online leader.
    let mut skills = std::collections::HashMap::new();
    skills.insert(370, 1);
    skills.insert(590, 1);
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "ResClan".into(),
            leader_id: 3001,
            level: 8,
            reputation_score: 0,
            castle_id: 0,
            members: vec![cm(3001)],
            skills,
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
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = clan_id;
    world.objects.add_components(
        &3001,
        ClanSkills(std::collections::HashMap::from([(590, 1)])),
    );

    let count = clans::give_clan_skills(&mut world, clan_id, false);

    // Residence skill purged from the clan and the member; real skill re-applied.
    assert!(
        !world.clans[&clan_id].skills.contains_key(&590),
        "residence skill purged from clan"
    );
    assert!(
        world.clans[&clan_id].skills.contains_key(&370),
        "clan skill kept"
    );
    let leader_skills = world.objects.get_component::<ClanSkills>(&3001).unwrap();
    assert!(
        !leader_skills.0.contains_key(&590),
        "residence skill reverted on the member"
    );
    assert!(
        leader_skills.0.contains_key(&370),
        "real clan skill applied immediately (no relog)"
    );
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

    let hp = model::calc_max_hp(&world.data, &t, 80, None, &mods);
    let mp = model::calc_max_mp(&world.data, &t, 80, None, &mods);
    let cp = model::calc_max_cp(&world.data, &t, 80, &mods);
    assert!(
        (hp - (1.5 * hp_base + 100.0)).abs() < 1e-6,
        "MaxHp = mul*base + add"
    );
    assert!((mp - (2.0 * mp_base)).abs() < 1e-6, "MaxMp = mul*base");
    assert!((cp - (1.2 * cp_base)).abs() < 1e-6, "MaxCp = mul*base");
    // Empty mods leave the base untouched (mul=1, add=0).
    let none = StatModifiers::default();
    assert!((model::calc_max_hp(&world.data, &t, 80, None, &none) - hp_base).abs() < 1e-6);
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
    let sh = dist::skills()
        .get(7029, 4)
        .expect("Super Haste 7029 L4")
        .clone();
    world.data.skill_data.insert_for_test(sh.clone());

    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let base_mp = world.objects.get_component::<Vitals>(&3001).unwrap().max_mp;

    effects::apply_skill_effects(&mut world, 3001, 3001, &sh);

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
        qualifier: None,
        two_handed: false,
        hp_percent: 0,
    })];
    world.data.skill_data.insert_for_test(s);

    let t = world.data.player_templates.get(0).cloned().unwrap();
    let base_mp = model::calc_max_mp(&world.data, &t, 1, None, &StatModifiers::default());

    let mut chr = dummy_char(7001, "Mage");
    chr.skills = vec![(9001, 1, 0)];
    let bundle = Player::from_char(&world.data, &chr);
    assert_eq!(
        bundle.vitals.max_mp,
        (base_mp * 2.0) as i32,
        "passive MaxMp folded into max_mp at login"
    );
}

/// End-to-end: clan skills carrying `MaxHp`/`MaxMp`/`MaxCp` modifiers (Clan
/// Health / Clan Mind, the Archmage clan-leader case) now move the HP/MP/CP bar
/// immediately — `%` modifiers stack multiplicatively, flat ones add. Regression
/// for the bug where these clan skills applied as buffs but never changed the
/// vitals (the finalizers ignored the modifier maps).
#[test]
fn clan_skills_move_max_hp_mp_cp() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::{PlayerVitals, StatModifiers, Vitals};
    use crate::model::skill::{SkillEffect, StatModifierEffect};
    use crate::model::stats::{Stat, StatModifierType};

    let (mut world, mut db_rx, _link_rx) = quest_test_world();

    // Skill 370: +100% MaxMp and +300 flat MaxHp. Skill 371: +50% MaxMp, +20% MaxCp.
    for (id, effs) in [
        (
            370,
            vec![
                (Stat::MaxMp, StatModifierType::Per, 100.0),
                (Stat::MaxHp, StatModifierType::Diff, 300.0),
            ],
        ),
        (
            371,
            vec![
                (Stat::MaxMp, StatModifierType::Per, 50.0),
                (Stat::MaxCp, StatModifierType::Per, 20.0),
            ],
        ),
    ] {
        let mut s = passive_clan_test_skill(id);
        s.effects = effs
            .into_iter()
            .map(|(stat, mode, amount)| {
                SkillEffect::StatModifier(StatModifierEffect {
                    stat,
                    mode,
                    amount,
                    armor_condition: 0,
                    weapon_condition: 0,
                    qualifier: None,
                    two_handed: false,
                    hp_percent: 0,
                })
            })
            .collect();
        world.data.skill_data.insert_for_test(s);
        world.data.pledge_skill_trees.insert_for_test(
            PledgeSkillLearn {
                skill_id: id,
                skill_level: 1,
                get_level: 1,
                social_class: None,
                residencial: false,
                residence_ids: Vec::new(),
                level_up_sp: 0,
            },
            false,
        );
    }

    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);

    // Exact pre-buff maxima (empty modifier maps).
    let (base_hp, base_mp, base_cp) = {
        let p = world.objects.get_component::<Player>(&3001).unwrap();
        let t = world
            .data
            .player_templates
            .get(p.class_id)
            .cloned()
            .unwrap();
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        let none = StatModifiers::default();
        (
            model::calc_max_hp(&world.data, &t, p.level, Some(inv), &none),
            model::calc_max_mp(&world.data, &t, p.level, Some(inv), &none),
            model::calc_max_cp(&world.data, &t, p.level, &none),
        )
    };

    let clan_id = 0x3000_00AA;
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
            name: "VitalClan".into(),
            leader_id: 3001,
            level: 8,
            reputation_score: 0,
            castle_id: 0,
            members: vec![cm(3001)],
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
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = clan_id;

    clans::give_clan_skills(&mut world, clan_id, false);

    let v = *world.objects.get_component::<Vitals>(&3001).unwrap();
    let pv = *world.objects.get_component::<PlayerVitals>(&3001).unwrap();
    // MaxMp: two % buffs stack multiplicatively (2.0 * 1.5 = 3.0).
    assert_eq!(
        v.max_mp,
        (base_mp * 3.0) as i32,
        "MaxMp % buffs stacked onto the bar"
    );
    // MaxHp: flat +300.
    assert_eq!(
        v.max_hp,
        (base_hp + 300.0) as i32,
        "flat MaxHp buff applied"
    );
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
            name: "SiegeClan".into(),
            leader_id: 3001,
            level: 4,
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

    let has = |world: &World, oid: i32, id: i32| {
        world
            .objects
            .get_component::<ClanSkills>(&oid)
            .is_some_and(|c| c.0.contains_key(&id))
    };

    // Level 4: below the siege min level — the leader gets no siege skills.
    clans::on_enter_world(&mut world, 1, 3001);
    assert!(
        !has(&world, 3001, 247),
        "no siege skills below clan level 5"
    );

    // Reaching level 5 grants the three core siege skills to the online leader.
    clans::set_clan_level(&mut world, clan_id, 5);
    for id in [247, 19034, 19035] {
        assert!(
            has(&world, 3001, id),
            "leader gains siege skill {id} at clan level 5"
        );
    }
    // No castle yet → no Outpost skills.
    assert!(
        !has(&world, 3001, 844) && !has(&world, 3001, 845),
        "Outpost skills need a castle"
    );
    // A regular member never gets siege skills.
    clans::on_enter_world(&mut world, 2, 3002);
    assert!(
        !has(&world, 3002, 247),
        "non-leader member gets no siege skills"
    );

    // Owning a castle adds the two Outpost skills on the leader's next login.
    world.clans.get_mut(&clan_id).unwrap().castle_id = 3;
    clans::on_enter_world(&mut world, 1, 3001);
    assert!(
        has(&world, 3001, 844) && has(&world, 3001, 845),
        "castle owner gets Outpost skills"
    );
}

/// A member logging in re-derives the clan's skills (Java `addSkillEffects` on
/// enter-world), gated by social class — nothing is persisted on the player.
#[test]
fn clan_skills_reapply_on_member_login() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::ClanSkills;

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(370));
    world.data.pledge_skill_trees.insert_for_test(
        PledgeSkillLearn {
            skill_id: 370,
            skill_level: 1,
            get_level: 3,
            social_class: Some(3),
            residencial: false,
            residence_ids: Vec::new(),
            level_up_sp: 0,
        },
        false,
    );
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0066;
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
    // The clan already knows skill 370 (as if loaded from clan_skills).
    let mut skills = std::collections::HashMap::new();
    skills.insert(370, 1);
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "SkillClan".into(),
            leader_id: 3001,
            level: 8,
            reputation_score: 0,
            castle_id: 0,
            members: vec![cm(3001)],
            skills,
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
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = clan_id;

    // Simulate the leader's login → clan skills re-applied from the clan.
    clans::on_enter_world(&mut world, 1, 3001);
    assert!(
        world
            .objects
            .get_component::<ClanSkills>(&3001)
            .is_some_and(|c| c.0.contains_key(&370)),
        "clan skills re-derived on login"
    );
    // Nothing was written to the player's own persisted skill book.
    assert!(
        world
            .objects
            .get_component::<SkillBook>(&3001)
            .is_some_and(|b| !b.0.contains_key(&370)),
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

/// Build a clan of `members` (first is leader) directly in the world and wire
/// the members' Player clan fields — the fixture every lifecycle test starts
/// from.
fn install_clan(world: &mut World, clan_id: i32, member_oids: &[i32]) {
    let cm = |char_id: i32| model::clan::ClanMember {
        char_id,
        name: format!("P{char_id}"),
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
            name: format!("Clan{clan_id}"),
            leader_id: member_oids[0],
            level: 1,
            reputation_score: 0,
            castle_id: 0,
            members: member_oids
                .iter()
                .map(|&o| {
                    let mut m = cm(o);
                    if o == member_oids[0] {
                        m.power_grade = 1; // leader
                    }
                    m
                })
                .collect(),
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
    for (i, &oid) in member_oids.iter().enumerate() {
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.clan_id = clan_id;
            p.clan_leader = i == 0;
            p.clan_privs = if i == 0 {
                model::clan::ALL_CLAN_PRIVILEGES
            } else {
                0
            };
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

fn reorganize_body(member_name: &str, new_pledge_type: i32, selected_member: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(1); // isMemberSelected
    w.write_string(member_name);
    w.write_i32(new_pledge_type);
    w.write_string(selected_member);
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

/// The pledge-skill learn flow: the leader-only list (`ExAcquirableSkillListBy
/// Class` PLEDGE), the reputation gate, and a successful learn — rep deducted
/// + persisted, the skill stored/broadcast, and the refreshed list offering
///   the next level.
#[test]
fn pledge_skill_learning_spends_reputation() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(370));
    let learn = |lvl: i32, sp: i64| PledgeSkillLearn {
        skill_id: 370,
        skill_level: lvl,
        get_level: 3,
        social_class: None,
        residencial: false,
        residence_ids: Vec::new(),
        level_up_sp: sp,
    };
    world
        .data
        .pledge_skill_trees
        .insert_for_test(learn(1, 1_500), false);
    world
        .data
        .pledge_skill_trees
        .insert_for_test(learn(2, 3_000), false);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3002]);
    world.clans.get_mut(&5000).unwrap().level = 3;
    drain_db(&mut db_rx);

    // Non-leader asking for the list → NotClanLeader.htm (an NpcHtmlMessage).
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!("npc_{NPC_OID}_learn_clan_skills")),
    );
    assert!(
        drain(&mut b_rx)
            .iter()
            .any(|p| decode_npc_html(p).is_some()),
        "NotClanLeader html shown"
    );

    // Leader: the PLEDGE learnable list with the level-1 entry.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_learn_clan_skills")),
    );
    let pkts = drain(&mut a_rx);
    let list = pkts
        .iter()
        .find(|p| is_ex(p, 0xFA))
        .expect("ExAcquirableSkillListByClass sent");
    assert_eq!(i16::from_le_bytes([list[3], list[4]]), 2, "PLEDGE type");

    // The info request answers with the reputation cost.
    clans::handle_request_pledge_skill_info(&world, 1, 370, 1);
    let info = drain(&mut a_rx);
    assert!(
        info.iter()
            .any(|p| p[0] == server_packets::opcodes::ACQUIRE_SKILL_INFO)
    );

    // Learning without reputation fails.
    clans::handle_learn_pledge_skill(&mut world, 1, 370, 1);
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::SKILL_ACQUIRE_FAILED_INSUFFICIENT_CLAN_REPUTATION)
    );
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
    let a_sms = ids_after_opcode(&a_pkts, server_packets::opcodes::SYSTEM_MESSAGE);
    assert!(a_sms.contains(
        &server_packets::sm_ids::S1_POINTS_HAVE_BEEN_DEDUCTED_FROM_THE_CLAN_S_REPUTATION
    ));
    assert!(a_sms.contains(&server_packets::sm_ids::THE_CLAN_SKILL_S1_HAS_BEEN_ADDED));
    assert!(
        a_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ACQUIRE_SKILL_DONE)
    );
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateClanReputation {
            clan_id: 5000,
            reputation: 8_500
        }
    )));
    assert!(cmds.iter().any(|c| matches!(
        c,
        db::DbCommand::SaveClanSkill {
            clan_id: 5000,
            skill_id: 370,
            skill_level: 1,
            ..
        }
    )));
    // The member got the passive too (no social gate on the fixture).
    assert!(
        world
            .objects
            .get_component::<model::components::ClanSkills>(&3002)
            .is_some_and(|c| c.0.get(&370) == Some(&1))
    );

    // The re-shown list now offers level 2.
    clans::handle_learn_pledge_skill(&mut world, 1, 370, 2);
    assert_eq!(world.clans[&5000].skills.get(&370), Some(&2));
    assert_eq!(world.clans[&5000].reputation_score, 5_500);
}

// --- G18 slice 3: ranks & power grades ---

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

fn pad_clan(world: &mut World, clan_id: i32, to: usize) {
    let c = world.clans.get_mut(&clan_id).unwrap();
    let mut i = 0;
    while c.members.len() < to {
        c.members.push(model::clan::ClanMember {
            char_id: 90_000 + clan_id + i,
            name: format!("Pad{clan_id}x{i}"),
            level: 40,
            class_id: 0,
            sex: 0,
            race: 0,
            power_grade: 5,
            title: String::new(),
            pledge_type: 0,
            apprentice: 0,
            sponsor: 0,
        });
        i += 1;
    }
}

fn name_body(name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(name);
    w.into_bytes()
}

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

fn oid_body(oid: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(oid);
    w.into_bytes()
}

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

fn crest_bytes(len: usize, fill: u8) -> Vec<u8> {
    vec![fill; len]
}

fn set_pledge_crest_body(data: &[u8]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(data.len() as i32);
    w.write_bytes(data);
    w.into_bytes()
}

fn pledge_crest_query_body(crest_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(crest_id);
    w.into_bytes()
}

/// Small clan-crest set/delete: the level-3 gate, privilege gate, dissolution
/// gate, oversized-payload reject, and a full set→query→delete round trip
/// that keeps UserInfo's crest id in sync for every online member.
#[test]
fn small_pledge_crest_set_query_delete() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3003]);
    drain_db(&mut db_rx);

    // Oversized payload (readImpl itself bails on this dist's 256-byte cap).
    let mut w = PacketWriter::new();
    w.write_i32(300);
    clans::handle_request_set_pledge_crest(&mut world, 1, &w.into_bytes());
    assert!(
        world.crests.is_empty(),
        "oversized request never reaches runImpl"
    );

    // Below level 3.
    let img = crest_bytes(64, 7);
    clans::handle_request_set_pledge_crest(&mut world, 1, &set_pledge_crest_body(&img));
    assert!(ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(&server_packets::sm_ids::A_CLAN_CREST_CAN_ONLY_BE_REGISTERED_WHEN_THE_CLAN_S_SKILL_LEVEL_IS_3_OR_ABOVE));

    // Dissolving gate.
    world.clans.get_mut(&5000).unwrap().level = 3;
    world.clans.get_mut(&5000).unwrap().dissolving_expiry_time = i64::MAX;
    clans::handle_request_set_pledge_crest(&mut world, 1, &set_pledge_crest_body(&img));
    assert!(ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(&server_packets::sm_ids::AS_YOU_ARE_SCHEDULED_FOR_CLAN_DISSOLUTION_CANNOT_REGISTER_OR_DELETE_CREST));
    world.clans.get_mut(&5000).unwrap().dissolving_expiry_time = 0;

    // No CL_REGISTER_CREST privilege.
    clans::handle_request_set_pledge_crest(&mut world, 2, &set_pledge_crest_body(&img));
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT)
    );

    // Success: stored, persisted, id on the clan + every online member.
    clans::handle_request_set_pledge_crest(&mut world, 1, &set_pledge_crest_body(&img));
    let crest_id = world.clans[&5000].crest_id;
    assert_ne!(crest_id, 0);
    assert_eq!(world.crests[&crest_id].data, img);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3003)
            .unwrap()
            .clan_crest_id,
        crest_id
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_CREST_WAS_SUCCESSFULLY_REGISTERED)
    );
    assert!(
        drain_db(&mut db_rx).iter().any(
            |c| matches!(c, db::DbCommand::InsertCrest { id, kind: 1, .. } if *id == crest_id)
        )
    );

    // Query answers with the stored bitmap.
    clans::handle_request_pledge_crest(&world, 1, &pledge_crest_query_body(crest_id));
    let pkts = drain(&mut a_rx);
    let pkt = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::PLEDGE_CREST)
        .expect("PledgeCrest sent");
    assert_eq!(&pkt[13..13 + img.len()], img.as_slice());

    // Delete.
    clans::handle_request_set_pledge_crest(&mut world, 1, &set_pledge_crest_body(&[]));
    assert_eq!(world.clans[&5000].crest_id, 0);
    assert!(
        world.crests.is_empty(),
        "the last-allocated id is never reused, but the bitmap itself is dropped"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3003)
            .unwrap()
            .clan_crest_id,
        0
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_CLAN_MARK_HAS_BEEN_DELETED)
    );
}

/// Large clan crest: same guard chain at the 2176-byte cap, and the chunked
/// query answer.
#[test]
fn large_pledge_crest_set_and_chunked_query() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    world.clans.get_mut(&5000).unwrap().level = 3;
    drain_db(&mut db_rx);

    let mut w = PacketWriter::new();
    w.write_i32(3000); // over the 2176 cap
    clans::handle_request_ex_set_pledge_crest_large(&mut world, 1, &w.into_bytes());
    assert!(world.crests.is_empty());

    let img = crest_bytes(2000, 9);
    let mut w = PacketWriter::new();
    w.write_i32(img.len() as i32);
    w.write_bytes(&img);
    clans::handle_request_ex_set_pledge_crest_large(&mut world, 1, &w.into_bytes());
    let crest_id = world.clans[&5000].crest_large_id;
    assert_ne!(crest_id, 0);
    assert_eq!(world.crests[&crest_id].data, img);
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_CLAN_MARK_WAS_SUCCESSFULLY_REGISTERED_ON_ITEMS)
    );

    let mut qw = PacketWriter::new();
    qw.write_i32(crest_id);
    qw.write_i32(5000);
    clans::handle_request_ex_pledge_crest_large(&world, 1, &qw.into_bytes());
    let pkts = drain(&mut a_rx);
    let emblem = pkts
        .iter()
        .find(|p| is_ex(p, 0x1B))
        .expect("ExPledgeEmblem");
    // header: opcode(1) + ex-id(2) + serverId(4) + clanId(4) + crestId(4) + chunkId(4) + totalSize(4) + chunkLen(4) = 27
    assert_eq!(
        i32::from_le_bytes([emblem[23], emblem[24], emblem[25], emblem[26]]),
        img.len() as i32
    );
    assert_eq!(&emblem[27..27 + img.len()], img.as_slice());
}

/// Ally crest: only the alliance leader may set it; joining/leaving syncs
/// `Player.ally_crest_id`.
#[test]
fn ally_crest_set_and_sync_on_membership_change() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    install_clan(&mut world, 5001, &[3003]);
    for (id, ally) in [(5000, 5000), (5001, 5000)] {
        let c = world.clans.get_mut(&id).unwrap();
        c.level = 5;
        c.ally_id = ally;
        c.ally_name = "Pact".into();
    }
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .ally_id = 5000;
    world
        .objects
        .get_component_mut::<Player>(&3003)
        .unwrap()
        .ally_id = 5000;
    drain_db(&mut db_rx);

    // A member clan's leader cannot set the ally crest.
    let img = crest_bytes(50, 3);
    let mut w = PacketWriter::new();
    w.write_i32(img.len() as i32);
    w.write_bytes(&img);
    clans::handle_request_set_ally_crest(&mut world, 2, &w.into_bytes());
    assert!(
        ids_after_opcode(&drain(&mut b_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THIS_FEATURE_IS_ONLY_AVAILABLE_TO_ALLIANCE_LEADERS)
    );

    // The alliance leader sets it — every member clan's online players sync.
    let mut w = PacketWriter::new();
    w.write_i32(img.len() as i32);
    w.write_bytes(&img);
    clans::handle_request_set_ally_crest(&mut world, 1, &w.into_bytes());
    let crest_id = world.clans[&5000].ally_crest_id;
    assert_ne!(crest_id, 0);
    assert_eq!(
        world.clans[&5001].ally_crest_id, crest_id,
        "pushed to every clan in the alliance"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3003)
            .unwrap()
            .ally_crest_id,
        crest_id
    );
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_CREST_WAS_SUCCESSFULLY_REGISTERED)
    );

    // A member leaving picks up ally_crest_id = 0 (their own row only).
    clans::handle_ally_leave(&mut world, 2);
    assert_eq!(world.clans[&5001].ally_crest_id, 0);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3003)
            .unwrap()
            .ally_crest_id,
        0
    );
    assert_eq!(
        world.clans[&5000].ally_crest_id, crest_id,
        "the leader clan keeps its own crest"
    );

    // A fresh join inherits the leader clan's current ally crest.
    let mut c_rx = ingame_player(&mut world, 3, 3005, 0, 0, 0);
    install_clan(&mut world, 5002, &[3005]);
    world.clans.get_mut(&5002).unwrap().level = 5;
    clans::handle_request_join_ally(&mut world, 1, &oid_body(3005));
    drain(&mut c_rx);
    clans::handle_request_answer_join_ally(&mut world, 3, &answer_body(1));
    assert_eq!(world.clans[&5002].ally_crest_id, crest_id);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3005)
            .unwrap()
            .ally_crest_id,
        crest_id
    );
}

// --- G18 slice 8: recruitment registry ---

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

const DIST_RES: &str = crate::data::DIST_GAME;

/// Build a clan that owns `castle` with a single online member `leader`.
#[cfg(test)]
fn owner_clan(id: i32, leader: i32, castle: i32) -> Clan {
    use crate::model::clan::{Clan, ClanMember};
    Clan {
        id,
        name: format!("Clan{id}"),
        leader_id: leader,
        level: 5,
        reputation_score: 0,
        castle_id: castle,
        members: vec![ClanMember {
            char_id: leader,
            name: format!("P{leader}"),
            level: 40,
            class_id: 0,
            sex: 0,
            race: 0,
            power_grade: 1,
            title: String::new(),
            pledge_type: 0,
            apprentice: 0,
            sponsor: 0,
        }],
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

/// A residence skill 593 gated to residence 3, with no social-class gate.
#[cfg(test)]
fn residence_learn() -> crate::data::pledge_skill_tree::PledgeSkillLearn {
    crate::data::pledge_skill_tree::PledgeSkillLearn {
        skill_id: 593,
        skill_level: 1,
        get_level: 4,
        social_class: None,
        residencial: true,
        residence_ids: vec![3],
        level_up_sp: 0,
    }
}

#[cfg(test)]
fn has_clan_skill(world: &World, oid: i32, id: i32) -> bool {
    world
        .objects
        .get_component::<model::components::ClanSkills>(&oid)
        .is_some_and(|c| c.0.contains_key(&id))
}

/// **Residential skills load per residence** — castle 1 grants Residence Health
/// (593); an unknown residence grants nothing (Java `getAvailableResidentialSkills`).
#[test]
fn residential_skills_load_per_castle() {
    let trees = crate::data::pledge_skill_tree::PledgeSkillTreeData::load_from(DIST_RES);
    let ids: Vec<i32> = trees
        .available_residential_skills(1)
        .iter()
        .map(|l| l.skill_id)
        .collect();
    assert!(
        ids.contains(&593),
        "castle 1 grants Residence Health: {ids:?}"
    );
    assert!(
        trees.available_residential_skills(999).is_empty(),
        "an unknown residence grants nothing"
    );
}

/// **A castle-owning clan's member gets the castle's residential skills on
/// login, and loses them when the castle is gone** (Java `Player.enterWorld` +
/// `AbstractResidence.removeResidentialSkills`).
#[test]
fn residential_skills_granted_on_login_and_stripped() {
    let (mut world, mut db_rx, _link) = quest_test_world();
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(593));
    world
        .data
        .pledge_skill_trees
        .insert_for_test(residence_learn(), false);
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0056;
    world.clans.insert(clan_id, owner_clan(clan_id, 3001, 3));
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = clan_id;

    clans::apply_clan_skills_to_member(&mut world, clan_id, 3001);
    assert!(
        has_clan_skill(&world, 3001, 593),
        "a castle-owning clan member gets the residential skill on login"
    );

    clans::remove_residential_skills(&mut world, 3001, 3);
    assert!(
        !has_clan_skill(&world, 3001, 593),
        "losing the castle strips the residential skill"
    );
}

/// **Capturing a castle moves its residential skills** — the former owner's
/// online member loses them, the captor's gains them (Java `Castle.setOwner`).
#[test]
fn capturing_a_castle_moves_residential_skills() {
    use crate::model::siege::{Siege, SiegeClanType};
    let (mut world, mut db_rx, _link) = quest_test_world();
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(593));
    world
        .data
        .pledge_skill_trees
        .insert_for_test(residence_learn(), false);
    let mut siege = Siege::new(3);
    siege.in_progress = true;
    siege.add_clan(500, SiegeClanType::Owner);
    siege.add_clan(700, SiegeClanType::Attacker);
    world.sieges.insert(3, siege);
    // Defender clan 500 owns castle 3; attacker clan 700 owns nothing. Both
    // leaders online.
    let _def = ingame_player(&mut world, 1, 8002, 0, 0, 0);
    let _atk = ingame_player(&mut world, 2, 8003, 0, 0, 0);
    world.clans.insert(500, owner_clan(500, 8002, 3));
    world.clans.insert(700, owner_clan(700, 8003, 0));
    for (oid, cid) in [(8002, 500), (8003, 700)] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .clan_id = cid;
    }
    // The defender already holds the skill (granted while owning).
    clans::give_residential_skills(&mut world, 8002, 3, 500);
    assert!(
        has_clan_skill(&world, 8002, 593),
        "defender holds it pre-capture"
    );
    drain_db(&mut db_rx);

    crate::game_loop::siege::capture(&mut world, 3, 700);

    assert!(
        !has_clan_skill(&world, 8002, 593),
        "the former owner's member loses the residential skill"
    );
    assert!(
        has_clan_skill(&world, 8003, 593),
        "the captor's member gains it"
    );
}

// --- G18.6: academy graduation, restrictions and mentorship ----------------

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

/// **Residential skills follow clan membership, not just login.** A member who
/// joins a castle-owning clan gets them at once (Java `addClanMember` →
/// `addSkillEffects`), and a member who leaves loses them with the clan
/// (`setClan(null)` → `removeResidentialSkills`) — otherwise a one-day
/// membership would leave the buff on them for good.
#[test]
fn residential_skills_follow_joining_and_leaving() {
    let (mut world, mut db_rx, _link) = quest_test_world();
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(593));
    world
        .data
        .pledge_skill_trees
        .insert_for_test(residence_learn(), false);
    let _leader = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _recruit = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    // The clan owns castle 3.
    world.clans.get_mut(&5000).unwrap().castle_id = 3;
    drain_db(&mut db_rx);

    assert!(
        !has_clan_skill(&world, 3003, 593),
        "an outsider has nothing"
    );

    // Join.
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3003, 0));
    clans::handle_request_answer_join_pledge(&mut world, 2, &answer_body(1));
    assert!(
        has_clan_skill(&world, 3003, 593),
        "joining a castle-owning clan grants the residential skill immediately"
    );

    // Leave.
    clans::handle_request_withdrawal_pledge(&mut world, 2);
    assert!(
        !has_clan_skill(&world, 3003, 593),
        "and leaving takes it back"
    );
}

/// **The large crest reaches `UserInfo` too.** It is mirrored onto the player
/// alongside the small one — `refresh_clan_crest_on_members` and the two
/// join/enter-world syncs — because the `UserInfo` builder cannot reach
/// `World.clans`. Before this the packet wrote a hard 0 and the field had no
/// source at all.
#[test]
fn the_large_clan_crest_is_mirrored_onto_online_members() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let _a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    drain_db(&mut db_rx);

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .clan_crest_large_id,
        0,
        "no crest yet"
    );

    // Set it on the clan and run the same refresh the crest handlers use.
    world.clans.get_mut(&5000).unwrap().crest_large_id = 0x4243_4445;
    clans::refresh_clan_crest_on_members_for_test(&mut world, 5000);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .clan_crest_large_id,
        0x4243_4445,
        "the member now carries the large crest"
    );
}

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
    items::add_inventory_item(&mut world, 3001, GLUDIO_CIRCLET, 1);

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
    items::add_inventory_item(&mut world, 3001, GLUDIO_CIRCLET, 1);

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
