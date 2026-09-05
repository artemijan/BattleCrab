//! Q00210 — obtaining a wolf pet. Not a trial; it sits in this block only
//! because its quest number falls between them.

use super::super::*;

/// Q00210 Obtain a Wolf Pet: the four-NPC dialog chain (Lundy → Bella → Bynn
/// → Sydnia → Lundy) advances cond 1→4 and hands over the Wolf Collar (2375),
/// one-time.
#[test]
fn quest_q00210_wolf_pet_chain() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(2375, "Wolf Collar", false)]);
    world.id_pool = 0x3000_0000..0x3000_0100; // the reward allocates the collar's oid
    let (lundy, bella, bynn, sydnia) = (NPC_OID, NPC_OID + 1, NPC_OID + 2, NPC_OID + 3);
    add_test_npc(&mut world, lundy, 30827, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, bella, 30256, "Folk", 5, 120, 0, 0);
    add_test_npc(&mut world, bynn, 30335, "Folk", 5, 140, 0, 0);
    add_test_npc(&mut world, sydnia, 30321, "Folk", 5, 160, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 15;
    drain_db(&mut db_rx);

    let q = "Q00210_ObtainAWolfPet";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lundy}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lundy}_Quest {q} 30827-03.htm")),
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(1),
        "Lundy started the quest"
    );

    // An out-of-order click is refused: Bynn (cond 2) while still at cond 1.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{bynn}_Quest {q} 30335-02.html")),
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(1),
        "cond guard holds — no skipping ahead"
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{bella}_Quest {q} 30256-03.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{bynn}_Quest {q} 30335-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{sydnia}_Quest {q} 30321-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(4));

    assert_eq!(
        item_count(&world, 3001, 2375),
        0,
        "no collar until the payout"
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lundy}_Quest {q} 30827-05.html")),
    );
    assert_eq!(item_count(&world, 3001, 2375), 1, "Wolf Collar rewarded");
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed(), "one-time quest stays COMPLETED");
}

/// Q00210 refuses a starter below level 15 with `no_level.htm` and does not
/// start (Java `addCondMinLevel(15, "no_level.htm")`).
#[test]
fn quest_q00210_refused_below_level_15() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let lundy = NPC_OID;
    add_test_npc(&mut world, lundy, 30827, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 14;
    drain(&mut rx);

    let q = "Q00210_ObtainAWolfPet";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lundy}_Quest {q}")),
    );
    // `no_level.htm` is a `.htm` file, so it ships as ExNpcQuestHtmlMessage
    // (the quest window), not a plain NpcHtmlMessage.
    let decode_quest_html = |pkt: &[u8]| -> Option<String> {
        if pkt[0] != server_packets::opcodes::EX
            || i16::from_le_bytes([pkt[1], pkt[2]])
                != server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE
        {
            return None;
        }
        let mut r = commons::network::PacketReader::new(&pkt[3..]);
        r.read_i32()?;
        r.read_string()
    };
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_quest_html(p))
        .expect("quest html");
    assert!(
        html.contains("level requirements") || html.contains("level 15"),
        "the level gate, got: {html}"
    );
    // The talk creates a CREATED state (Java `getQuestState(player, true)`) but
    // the gate keeps it un-started (cond 0, never `startQuest`).
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(!quests.0[q].is_started(), "the quest never started");
}
