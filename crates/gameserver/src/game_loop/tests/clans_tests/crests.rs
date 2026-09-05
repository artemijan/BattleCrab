//! Crests: the pledge crest, the chunked large crest, and the ally crest
//! syncing on a membership change.

use super::*;

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
