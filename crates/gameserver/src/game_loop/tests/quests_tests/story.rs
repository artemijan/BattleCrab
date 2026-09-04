//! Q00001-Q00199 — the newbie village and main story quests.

use super::*;

/// Q00109 In Search of the Nest: the three-NPC cond 1→2→3 chain ends in a
/// one-time completion — the quest survives as COMPLETED and answers with
/// the already-completed page.
#[test]
fn quest_q00109_multi_cond_one_time() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(14858, "Scout's Note", true)]);
    let (pierce, corpse, kahman) = (NPC_OID, NPC_OID + 1, NPC_OID + 2);
    add_test_npc(&mut world, pierce, 31553, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, corpse, 32015, "Folk", 5, 120, 0, 0);
    add_test_npc(&mut world, kahman, 31554, "Folk", 5, 140, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 81;
    drain_db(&mut db_rx);

    let q = "Q00109_InSearchOfTheNest";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pierce}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pierce}_Quest {q} 31553-0.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));

    // The corpse: cond 2 + the note.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{corpse}_Quest {q} 32015-2.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert_eq!(item_count(&world, 3001, 14858), 1);

    // Back to Pierce: cond 3, note taken.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pierce}_Quest {q} 31553-3.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    assert_eq!(item_count(&world, 3001, 14858), 0);

    // Kahman pays out; one-time exit keeps the COMPLETED state.
    let (adena, exp) = (
        item_count(&world, 3001, 57),
        world.objects.get_component::<Player>(&3001).unwrap().exp,
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kahman}_Quest {q} 31554-2.html")),
    );
    assert_eq!(item_count(&world, 3001, 57), adena + 161500);
    assert!(world.objects.get_component::<Player>(&3001).unwrap().exp > exp);
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[q].is_completed(), "one-time quest stays COMPLETED");
    }

    // Talking to Pierce again answers the already-completed page.
    drain(&mut rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pierce}_Quest {q}")),
    );
    let pkts = drain(&mut rx);
    eprintln!(
        "DBG opcodes: {:?}",
        pkts.iter().map(|p| p[0]).collect::<Vec<_>>()
    );
    let html = pkts
        .iter()
        .find_map(|p| decode_npc_html(p))
        .unwrap_or_default();
    eprintln!("DBG html: {html}");
    assert!(
        html.contains("already completed") || html.contains("already been completed"),
        "already-completed message, got: {html}"
    );
}

#[test]
fn quest_q00110_to_the_primeval_isle() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(8777, "Ancient Book", true)]);
    add_test_npc(&mut world, NPC_OID, 31338, "Folk", 70, 100, 0, 0); // Anton
    add_test_npc(&mut world, NPC_OID + 1, 32113, "Folk", 70, 100, 0, 0); // Marquez
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 75;
    let q = "Q00110_ToThePrimevalIsle";
    // Below the min level the empty-html gate blocks state creation.
    let _rx2 = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .level = 70;
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        quest_cond(&world, 3002, q),
        None,
        "level-70 player can't start (empty-html gate)"
    );
    // Level-75 player: accept from Anton, deliver to Marquez.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31338-05.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(
        item_count(&world, 3001, 8777),
        1,
        "Anton hands over the Ancient Book"
    );
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q} 32113-04.html", NPC_OID + 1)),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 189208,
        "Marquez pays 189208 adena"
    );
    assert_eq!(
        item_count(&world, 3001, 8777),
        0,
        "book consumed on the one-time exit"
    );
    // One-time exit → COMPLETED, so a fresh talk is refused (not restarted).
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(1),
        "quest does not restart"
    );
}

#[test]
fn quest_q00127_fishing_specialists_request() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (49510, "Pierre's Letter", true),
            (49504, "Fish Report", true),
            (49505, "Sealed Bottle", true),
            (49507, "Fishing Rod Chest", false),
        ],
    );
    let pierre = NPC_OID;
    let ferma = NPC_OID + 1;
    let baikal = NPC_OID + 2;
    add_test_npc(&mut world, pierre, 30013, "Folk", 30, 100, 0, 0);
    add_test_npc(&mut world, ferma, 30015, "Folk", 30, 100, 0, 0);
    add_test_npc(&mut world, baikal, 30016, "Folk", 30, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 30;
    let q = "Q00127_FishingSpecialistsRequest";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pierre}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pierre}_Quest {q} 30013-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(
        item_count(&world, 3001, 49510),
        1,
        "Pierre hands over his letter"
    );
    // Ferma: letter → report, cond 2.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{ferma}_Quest {q}")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert_eq!(item_count(&world, 3001, 49510), 0, "letter consumed");
    assert_eq!(item_count(&world, 3001, 49504), 1, "fish report received");
    // Baikal: report → sealed bottle, cond 3.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{baikal}_Quest {q}")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    assert_eq!(item_count(&world, 3001, 49505), 1, "sealed bottle received");
    // Pierre: bottle → Fishing Rod Chest, one-time exit.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pierre}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 49507),
        1,
        "Fishing Rod Chest reward"
    );
    assert_eq!(item_count(&world, 3001, 49505), 0, "bottle consumed");
    // One-time exit → does not restart.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pierre}_Quest {q}")),
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(1),
        "quest does not restart"
    );
}

#[test]
fn quest_q00124_meeting_the_elroki() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(8778, "Mantarasa Egg", true)]);
    add_test_npc(&mut world, NPC_OID, 32113, "Folk", 70, 100, 0, 0); // Marquez
    add_test_npc(&mut world, NPC_OID + 1, 32115, "Folk", 70, 100, 0, 0); // Asamah
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 75;
    let q = "Q00124_MeetingTheElroki";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    // Walk the cond chain 1 → 6 (on_event ignores which NPC fires the event).
    for (ev, expect) in [
        ("32113-03.html", 1),
        ("32113-04.html", 2),
        ("32114-04.html", 3),
        ("32115-06.html", 4),
        ("32117-05.html", 5),
        ("32118-04.html", 6),
    ] {
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {q} {ev}")),
        );
        assert_eq!(
            quest_cond(&world, 3001, q),
            Some(expect),
            "after event {ev}"
        );
    }
    assert_eq!(
        item_count(&world, 3001, 8778),
        1,
        "Mantarasa Egg received at cond 6"
    );
    // Asamah pays out and finishes the quest.
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q}", NPC_OID + 1)),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 100013,
        "Asamah pays 100013 adena"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(6),
        "one-time quest is finished"
    );
}

#[test]
fn quest_q00111_elrokian_hunters_proof() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (8768, "Diary Fragment", true),
            (8769, "Expedition Member's Letter", true),
            (8770, "Ornithomimus Claw", true),
            (8771, "Deinonychus Bone", true),
            (8772, "Pachycephalosaurus Skin", true),
            (8773, "Practice Elrokian Trap", true),
            (8763, "Elrokian Trap", false),
            (8764, "Trap Stone", false),
        ],
    );
    for id in [22196, 22200] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 75;
        world.data.npc_data.insert_for_test(t);
    }
    let marquez = NPC_OID;
    let mushika = NPC_OID + 1;
    let asamah = NPC_OID + 2;
    let kirikachin = NPC_OID + 3;
    add_test_npc(&mut world, marquez, 32113, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, mushika, 32114, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, asamah, 32115, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, kirikachin, 32116, "Folk", 70, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 75;
    let q = "Q00111_ElrokianHuntersProof";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    // Start (memo 1 / cond 1) and walk the intro dialog steps.
    talk(&mut world, marquez);
    ev(&mut world, marquez, "32113-03.html");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    talk(&mut world, mushika); // memo 1 → 2, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    ev(&mut world, asamah, "32115-03.html"); // memo 2 → 3, cond 3
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    ev(&mut world, marquez, "32113-15.html"); // memo 3 → 4, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // Diary stage: 49 + one kill topping to 50 → cond 5.
    inject(&mut world, 3001, 0x0111_0000, 8768, 49);
    add_test_npc(&mut world, NPC_OID + 10, 22196, "Monster", 75, 30, 0, 0);
    world.force_roll(0); // give_item_randomly roll_f64 (0.0 < 0.51)
    npc::npc_do_die(&mut world, NPC_OID + 10, 3001);
    assert_eq!(item_count(&world, 3001, 8768), 50, "diary tops to 50");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    // Marquez takes the diary (memo 4 → 5); then hands out the Expedition Letter.
    talk(&mut world, marquez);
    assert_eq!(item_count(&world, 3001, 8768), 0, "diary consumed");
    ev(&mut world, marquez, "32113-25.html"); // memo 5 → 6, cond 6, letter
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    assert_eq!(
        item_count(&world, 3001, 8769),
        1,
        "expedition letter received"
    );
    // Kirikachin (memo 6 → 7, takes letter, cond 7) then the flute steps.
    talk(&mut world, kirikachin);
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    assert_eq!(item_count(&world, 3001, 8769), 0, "letter consumed");
    ev(&mut world, kirikachin, "32116-04.html"); // memo 7 → 8 (cond stays 7)
    ev(&mut world, kirikachin, "32116-07.html"); // memo 8 → 9, cond 8
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    ev(&mut world, asamah, "32115-06.html"); // memo 9 → 10, cond 9
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    ev(&mut world, asamah, "32115-09.html"); // memo 10 → 11, cond 10
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    // Trophy stage: 9 claws + one kill to 10, with bone/skin already full → cond 11.
    inject(&mut world, 3001, 0x0111_1000, 8770, 9);
    inject(&mut world, 3001, 0x0111_2000, 8771, 10);
    inject(&mut world, 3001, 0x0111_3000, 8772, 10);
    add_test_npc(&mut world, NPC_OID + 11, 22200, "Monster", 75, 30, 0, 0);
    world.force_roll(0); // give_item_randomly roll_f64 (0.0 < 0.66)
    npc::npc_do_die(&mut world, NPC_OID + 11, 3001);
    assert_eq!(item_count(&world, 3001, 8770), 10, "claws top to 10");
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    // Asamah forges the Practice Elrokian Trap (memo 11 → 12, cond 12, takes trophies).
    talk(&mut world, asamah);
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    assert_eq!(item_count(&world, 3001, 8773), 1, "practice trap forged");
    assert_eq!(
        item_count(&world, 3001, 8770)
            + item_count(&world, 3001, 8771)
            + item_count(&world, 3001, 8772),
        0,
        "trophies consumed"
    );
    // Kirikachin redeems it for the real trap + stones + reward, then exits.
    let a = item_count(&world, 3001, 57);
    ev(&mut world, kirikachin, "32116-10.html");
    assert_eq!(
        item_count(&world, 3001, 8763),
        1,
        "real Elrokian Trap awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 8764),
        100,
        "100 Trap Stones awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 1702800,
        "final adena reward"
    );
    assert_eq!(item_count(&world, 3001, 8773), 0, "practice trap consumed");
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(12),
        "one-time quest finished"
    );
}

/// Parameters for one "Help the …" pet-ticket quest (42/43/44) — they share a
/// single flow, differing only in NPCs, mobs, items and the level gate.
struct HelpQuest {
    q: &'static str,
    start_npc: i32,
    second_npc: i32,
    mob: i32,
    weapon: i32,
    piece: i32,
    artifact: i32, // the assembled Map / Gemstone
    ticket: i32,
    min_level: i32,
    accept: &'static str,
    weapon_ev: &'static str,
    assemble_ev: &'static str,
    second_ev: &'static str,
    final_ev: &'static str,
}

/// Drive one "Help the …" quest end to end: level gate, weapon hand-in, 30
/// kill-drops assembling the artifact, the second NPC reading it, and the Pet
/// Ticket reward.
fn run_help_quest(p: HelpQuest) {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (p.weapon, "weapon", true),
            (p.piece, "piece", true),
            (p.artifact, "artifact", true),
            (p.ticket, "Pet Ticket", false),
        ],
    );
    let mut mt = crate::data::npc_data::default_template(p.mob);
    mt.type_name = "Monster".into();
    mt.level = 30;
    mt.base_hp_max = 100.0;
    world.data.npc_data.insert_for_test(mt);

    let start = NPC_OID;
    let second = NPC_OID + 1;
    add_test_npc(&mut world, start, p.start_npc, "Folk", 40, 100, 200, 0);
    add_test_npc(&mut world, second, p.second_npc, "Folk", 40, 100, 200, 0);

    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = p.min_level - 1;
    let q = p.q;
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let grab_html = |rx: &mut UnboundedReceiver<bytes::Bytes>| -> Option<String> {
        drain(rx).iter().find_map(|pkt| {
            if pkt[0] == server_packets::opcodes::NPC_HTML_MESSAGE {
                decode_npc_html(pkt)
            } else if pkt[0] == server_packets::opcodes::EX {
                let mut r = commons::network::PacketReader::new(&pkt[1..]);
                r.read_i16()?;
                r.read_i32()?;
                r.read_string()
            } else {
                None
            }
        })
    };

    // --- Level gate: the refusal page carries no accept button. ---
    talk(&mut world, start);
    let refusal = grab_html(&mut rx).expect("under-level greeting");
    assert!(
        !refusal.contains(p.accept),
        "{q}: under-level page offers no start: {refusal}"
    );
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = p.min_level;
    talk(&mut world, start);
    let intro = grab_html(&mut rx).expect("intro");
    assert!(
        intro.contains(p.accept),
        "{q}: at-level intro offers start: {intro}"
    );

    // --- Accept, then hand over the requested weapon (cond 1 → 2). ---
    ev(&mut world, start, p.accept);
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "{q}: started");
    inject(&mut world, 3001, 0x0044_1000, p.weapon, 1);
    ev(&mut world, start, p.weapon_ev);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "{q}: weapon handed in → cond 2"
    );
    assert_eq!(
        item_count(&world, 3001, p.weapon),
        0,
        "{q}: weapon consumed"
    );

    // --- 30 kill-drops assemble the pieces (cond 2 → 3). ---
    let mut mob_oid = NPC_OID + 30;
    for _ in 0..30 {
        mob_oid += 1;
        add_test_npc(&mut world, mob_oid, p.mob, "Monster", 30, 110, 200, 0);
        npc::npc_do_die(&mut world, mob_oid, 3001);
    }
    assert_eq!(item_count(&world, 3001, p.piece), 30, "{q}: 30 pieces");
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(3),
        "{q}: 30th kill → cond 3"
    );
    // A 31st kill drops nothing more (only counts while on cond 2).
    mob_oid += 1;
    add_test_npc(&mut world, mob_oid, p.mob, "Monster", 30, 110, 200, 0);
    npc::npc_do_die(&mut world, mob_oid, 3001);
    assert_eq!(
        item_count(&world, 3001, p.piece),
        30,
        "{q}: no over-collection past cond 2"
    );

    // --- Assemble the artifact (cond 3 → 4), consuming the pieces. ---
    ev(&mut world, start, p.assemble_ev);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(4),
        "{q}: artifact assembled → cond 4"
    );
    assert_eq!(
        item_count(&world, 3001, p.artifact),
        1,
        "{q}: artifact granted"
    );
    assert_eq!(item_count(&world, 3001, p.piece), 0, "{q}: pieces consumed");

    // --- The second NPC reads the artifact (cond 4 → 5). ---
    ev(&mut world, second, p.second_ev);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(5),
        "{q}: artifact read → cond 5"
    );
    assert_eq!(
        item_count(&world, 3001, p.artifact),
        0,
        "{q}: artifact consumed"
    );

    // --- The reward: a Pet Ticket, and the quest completes. ---
    ev(&mut world, start, p.final_ev);
    assert_eq!(
        item_count(&world, 3001, p.ticket),
        1,
        "{q}: Pet Ticket awarded"
    );
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed(), "{q}: completed on reward");
}

#[test]
fn quest_q00042_help_the_uncle() {
    run_help_quest(HelpQuest {
        q: "Q00042_HelpTheUncle",
        start_npc: 30828,  // Waters
        second_npc: 30735, // Sophya
        mob: 20068,        // Monster Eye Destroyer
        weapon: 291,       // Trident
        piece: 7548,
        artifact: 7549, // Map
        ticket: 7583,
        min_level: 25,
        accept: "30828-01.htm",
        weapon_ev: "30828-03.html",
        assemble_ev: "30828-06.html",
        second_ev: "30735-02.html",
        final_ev: "30828-09.html",
    });
}

#[test]
fn quest_q00043_help_the_sister() {
    run_help_quest(HelpQuest {
        q: "Q00043_HelpTheSister",
        start_npc: 30829,  // Cooper
        second_npc: 30097, // Galladucci
        mob: 20203,        // Dion Grizzly
        weapon: 220,       // Crafted Dagger
        piece: 7550,
        artifact: 7551, // Map
        ticket: 7584,
        min_level: 26,
        accept: "30829-01.htm",
        weapon_ev: "30829-03.html",
        assemble_ev: "30829-06.html",
        second_ev: "30097-02.html",
        final_ev: "30829-09.html",
    });
}

#[test]
fn quest_q00044_help_the_son() {
    run_help_quest(HelpQuest {
        q: "Q00044_HelpTheSon",
        start_npc: 30827,  // Lundy
        second_npc: 30505, // Drikus
        mob: 20919,        // Maille Lizardman
        weapon: 168,       // Work Hammer
        piece: 7552,
        artifact: 7553, // Gemstone
        ticket: 7585,
        min_level: 24,
        accept: "30827-01.htm",
        weapon_ev: "30827-03.html",
        assemble_ev: "30827-06.html",
        second_ev: "30505-02.html",
        final_ev: "30827-09.html",
    });
}

// ---------------------------------------------------------------------------
// Formal Wear chain (Q33-Q37), restored to authentic Interlude (level 60).
// ---------------------------------------------------------------------------

/// Seed a started `Q00037_MakeFormalWear` at the given cond, so the component
/// sub-quests (which gate on it) can be entered.
fn seed_formal_wear(world: &mut World, player: i32, cond: i32) {
    let q = world
        .objects
        .get_component_mut::<model::components::social::Quests>(&player)
        .unwrap();
    let qs = q.0.entry("Q00037_MakeFormalWear".to_string()).or_default();
    qs.state = model::quest::state::STARTED;
    qs.vars.insert("cond".to_string(), cond.to_string());
}

#[test]
fn quest_q00037_make_formal_wear() {
    const FORMAL_WEAR: i32 = 6408;
    const MYSTERIOUS_CLOTH: i32 = 7076;
    const JEWEL_BOX: i32 = 7077;
    const SEWING_KIT: i32 = 7078;
    const DRESS_SHOES_BOX: i32 = 7113;
    const BOX_OF_COOKIES: i32 = 7159;
    const ICE_WINE: i32 = 7160;
    const SIGNET_RING: i32 = 7164;
    const ALEXIS: i32 = 30842;
    const LEIKAR: i32 = 31520;
    const JEREMY: i32 = 31521;
    const MIST: i32 = 31627;

    let (mut world, _db, _l) = quest_test_world();
    let items: Vec<(i32, &str, bool)> = [
        (SIGNET_RING, true),
        (ICE_WINE, true),
        (BOX_OF_COOKIES, true),
        (MYSTERIOUS_CLOTH, true),
        (JEWEL_BOX, true),
        (SEWING_KIT, true),
        (DRESS_SHOES_BOX, true),
        (FORMAL_WEAR, false),
    ]
    .iter()
    .map(|&(id, q)| (id, "Q37", q))
    .collect();
    add_quest_items(&mut world, &items);
    let alexis = NPC_OID;
    let leikar = NPC_OID + 1;
    let jeremy = NPC_OID + 2;
    let mist = NPC_OID + 3;
    for (oid, npc) in [
        (alexis, ALEXIS),
        (leikar, LEIKAR),
        (jeremy, JEREMY),
        (mist, MIST),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 200, 0);
    }
    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 59;
    let q = "Q00037_MakeFormalWear";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let grab_html = |rx: &mut UnboundedReceiver<bytes::Bytes>| -> Option<String> {
        drain(rx).iter().find_map(|p| {
            if p[0] == server_packets::opcodes::NPC_HTML_MESSAGE {
                decode_npc_html(p)
            } else if p[0] == server_packets::opcodes::EX {
                let mut r = commons::network::PacketReader::new(&p[1..]);
                r.read_i16()?;
                r.read_i32()?;
                r.read_string()
            } else {
                None
            }
        })
    };

    // Level gate: 59 is refused (no accept button to 30842-03).
    talk(&mut world, alexis);
    let html = grab_html(&mut rx).expect("greeting");
    assert!(!html.contains("30842-03.htm"), "under-60 refused: {html}");
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 60;

    // Accept, then the courier chain.
    ev(&mut world, alexis, "30842-03.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    ev(&mut world, leikar, "31520-02.html"); // Signet Ring → cond 2
    assert_eq!(item_count(&world, 3001, SIGNET_RING), 1);
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    ev(&mut world, jeremy, "31521-02.html"); // takes Signet, gives Ice Wine → cond 3
    assert_eq!(
        item_count(&world, 3001, SIGNET_RING),
        0,
        "Signet Ring surrendered"
    );
    assert_eq!(item_count(&world, 3001, ICE_WINE), 1);
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    ev(&mut world, mist, "31627-02.html"); // takes Ice Wine → cond 4
    assert_eq!(item_count(&world, 3001, ICE_WINE), 0);
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    ev(&mut world, jeremy, "31521-05.html"); // Box of Cookies → cond 5
    assert_eq!(item_count(&world, 3001, BOX_OF_COOKIES), 1);
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    ev(&mut world, leikar, "31520-05.html"); // takes Cookies → cond 6
    assert_eq!(quest_cond(&world, 3001, q), Some(6));

    // The components arrive from the sub-quests; Leikar assembles them.
    ev(&mut world, leikar, "31520-08.html"); // no components yet → stays cond 6
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(6),
        "cannot assemble without components"
    );
    inject(&mut world, 3001, 0x0037_1000, MYSTERIOUS_CLOTH, 1);
    inject(&mut world, 3001, 0x0037_2000, JEWEL_BOX, 1);
    inject(&mut world, 3001, 0x0037_3000, SEWING_KIT, 1);
    ev(&mut world, leikar, "31520-08.html"); // takes the 3 components → cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    assert_eq!(
        item_count(&world, 3001, SEWING_KIT),
        0,
        "components consumed"
    );
    inject(&mut world, 3001, 0x0037_4000, DRESS_SHOES_BOX, 1);
    ev(&mut world, leikar, "31520-12.html"); // Dress Shoes Box → Formal Wear
    assert_eq!(
        item_count(&world, 3001, FORMAL_WEAR),
        1,
        "Formal Wear crafted"
    );
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(
        quests.0[q].is_completed(),
        "quest completes on the Formal Wear"
    );
}

#[test]
fn quest_q00036_make_a_sewing_kit() {
    const FERRIS: i32 = 30847;
    const IRON_GOLEM: i32 = 20566;
    const REINFORCED_STEEL: i32 = 7163;
    const ORIHARUKON: i32 = 1893;
    const ARTISANS_FRAME: i32 = 1891;
    const SEWING_KIT: i32 = 7078;

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (REINFORCED_STEEL, "q", true),
            (ORIHARUKON, "q", true),
            (ARTISANS_FRAME, "q", true),
            (SEWING_KIT, "reward", false),
        ],
    );
    let mut gt = crate::data::npc_data::default_template(IRON_GOLEM);
    gt.type_name = "Monster".into();
    gt.level = 60;
    gt.base_hp_max = 100.0;
    world.data.npc_data.insert_for_test(gt);
    let ferris = NPC_OID;
    add_test_npc(&mut world, ferris, FERRIS, "Folk", 60, 100, 200, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 60;
    let mut rx2 = _rx;
    let q = "Q00036_MakeASewingKit";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let grab = |rx: &mut UnboundedReceiver<bytes::Bytes>| -> String {
        drain(rx)
            .iter()
            .find_map(|p| {
                if p[0] == server_packets::opcodes::NPC_HTML_MESSAGE {
                    decode_npc_html(p)
                } else if p[0] == server_packets::opcodes::EX {
                    let mut r = commons::network::PacketReader::new(&p[1..]);
                    r.read_i16()?;
                    r.read_i32()?;
                    r.read_string()
                } else {
                    None
                }
            })
            .unwrap_or_default()
    };

    // Prereq: without Make Formal Wear at cond 6, Ferris offers no accept button.
    talk(&mut world, ferris);
    let html = grab(&mut rx2);
    assert!(
        !html.contains("30847-03.htm"),
        "no accept offered without parent: {html}"
    );

    // With the parent at cond 6, accept and gather.
    seed_formal_wear(&mut world, 3001, 6);
    talk(&mut world, ferris);
    ev(&mut world, ferris, "30847-03.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    // Five Reinforced Steel from Iron Golems (force the 50% roll).
    let mut mob = NPC_OID + 20;
    for _ in 0..5 {
        mob += 1;
        add_test_npc(&mut world, mob, IRON_GOLEM, "Monster", 60, 110, 200, 0);
        world.force_roll(0); // roll(2)==0 → the peel succeeds
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(
        item_count(&world, 3001, REINFORCED_STEEL),
        5,
        "5 steel scraps"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "5th scrap → cond 2");
    ev(&mut world, ferris, "30847-06.html"); // hand in 5 steel → cond 3
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    assert_eq!(
        item_count(&world, 3001, REINFORCED_STEEL),
        0,
        "steel consumed"
    );
    // Insufficient mats: no craft.
    ev(&mut world, ferris, "30847-09.html");
    assert_eq!(
        item_count(&world, 3001, SEWING_KIT),
        0,
        "no craft without mats"
    );
    inject(&mut world, 3001, 0x0036_1000, ORIHARUKON, 10);
    inject(&mut world, 3001, 0x0036_2000, ARTISANS_FRAME, 10);
    ev(&mut world, ferris, "30847-09.html"); // craft
    assert_eq!(
        item_count(&world, 3001, SEWING_KIT),
        1,
        "Sewing Kit crafted"
    );
    assert_eq!(
        item_count(&world, 3001, ORIHARUKON),
        0,
        "oriharukon consumed"
    );
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed());
}

#[test]
fn quest_q00035_find_glittering_jewelry() {
    const ELLIE: i32 = 30091;
    const FELTON: i32 = 30879;
    const ALLIGATOR: i32 = 20135;
    const ROUGH_JEWEL: i32 = 7162;
    const ORIHARUKON: i32 = 1893;
    const SILVER_NUGGET: i32 = 1873;
    const THONS: i32 = 4044;
    const JEWEL_BOX: i32 = 7077;

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (ROUGH_JEWEL, "q", true),
            (ORIHARUKON, "q", true),
            (SILVER_NUGGET, "q", true),
            (THONS, "q", true),
            (JEWEL_BOX, "reward", false),
        ],
    );
    let mut at = crate::data::npc_data::default_template(ALLIGATOR);
    at.type_name = "Monster".into();
    at.level = 60;
    at.base_hp_max = 100.0;
    world.data.npc_data.insert_for_test(at);
    let ellie = NPC_OID;
    let felton = NPC_OID + 1;
    add_test_npc(&mut world, ellie, ELLIE, "Folk", 60, 100, 200, 0);
    add_test_npc(&mut world, felton, FELTON, "Folk", 60, 100, 200, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 60;
    seed_formal_wear(&mut world, 3001, 6);
    let q = "Q00035_FindGlitteringJewelry";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    talk(&mut world, ellie); // create the state (shows the accept page)
    ev(&mut world, ellie, "30091-03.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    ev(&mut world, felton, "30879-02.html"); // → cond 2, start hunting
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    let mut mob = NPC_OID + 20;
    for _ in 0..10 {
        mob += 1;
        add_test_npc(&mut world, mob, ALLIGATOR, "Monster", 60, 110, 200, 0);
        world.force_roll(0);
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, ROUGH_JEWEL), 10, "10 rough jewels");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    ev(&mut world, ellie, "30091-07.html"); // hand in jewels → cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    ev(&mut world, ellie, "30091-11.html"); // no mats → no box
    assert_eq!(item_count(&world, 3001, JEWEL_BOX), 0);
    inject(&mut world, 3001, 0x0035_1000, ORIHARUKON, 5);
    inject(&mut world, 3001, 0x0035_2000, SILVER_NUGGET, 500);
    inject(&mut world, 3001, 0x0035_3000, THONS, 150);
    ev(&mut world, ellie, "30091-11.html");
    assert_eq!(item_count(&world, 3001, JEWEL_BOX), 1, "Jewel Box crafted");
    assert_eq!(item_count(&world, 3001, THONS), 0, "thons consumed");
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed());
}

#[test]
fn quest_q00034_in_search_of_cloth() {
    const RADIA: i32 = 30088;
    const RALFORD: i32 = 30165;
    const VARAN: i32 = 30294;
    const TRISALIM_SPIDER: i32 = 20560;
    const SPINNERET: i32 = 7528;
    const SPIDERSILK: i32 = 7161;
    const SUEDE: i32 = 1866;
    const THREAD: i32 = 1868;
    const MYSTERIOUS_CLOTH: i32 = 7076;

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (SPINNERET, "q", true),
            (SPIDERSILK, "q", true),
            (SUEDE, "q", true),
            (THREAD, "q", true),
            (MYSTERIOUS_CLOTH, "reward", false),
        ],
    );
    let mut st = crate::data::npc_data::default_template(TRISALIM_SPIDER);
    st.type_name = "Monster".into();
    st.level = 46;
    st.base_hp_max = 100.0;
    world.data.npc_data.insert_for_test(st);
    let radia = NPC_OID;
    let ralford = NPC_OID + 1;
    let varan = NPC_OID + 2;
    add_test_npc(&mut world, radia, RADIA, "Folk", 60, 100, 200, 0);
    add_test_npc(&mut world, ralford, RALFORD, "Folk", 60, 100, 200, 0);
    add_test_npc(&mut world, varan, VARAN, "Folk", 60, 100, 200, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 60;
    seed_formal_wear(&mut world, 3001, 6);
    let q = "Q00034_InSearchOfCloth";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    talk(&mut world, radia);
    ev(&mut world, radia, "30088-03.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    ev(&mut world, varan, "30294-02.html"); // → cond 2
    ev(&mut world, radia, "30088-06.html"); // → cond 3
    ev(&mut world, ralford, "30165-02.html"); // → cond 4, hunt spiders
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    let mut mob = NPC_OID + 20;
    for _ in 0..10 {
        mob += 1;
        add_test_npc(&mut world, mob, TRISALIM_SPIDER, "Monster", 46, 110, 200, 0);
        world.force_roll(0);
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, SPINNERET), 10, "10 spinnerets");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    ev(&mut world, ralford, "30165-05.html"); // spin into Spidersilk → cond 6
    assert_eq!(item_count(&world, 3001, SPIDERSILK), 1, "Spidersilk spun");
    assert_eq!(
        item_count(&world, 3001, SPINNERET),
        0,
        "spinnerets consumed"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    ev(&mut world, radia, "30088-10.html"); // no cloth materials yet
    assert_eq!(item_count(&world, 3001, MYSTERIOUS_CLOTH), 0);
    inject(&mut world, 3001, 0x0034_1000, SUEDE, 3000);
    inject(&mut world, 3001, 0x0034_2000, THREAD, 5000);
    ev(&mut world, radia, "30088-10.html");
    assert_eq!(
        item_count(&world, 3001, MYSTERIOUS_CLOTH),
        1,
        "Mysterious Cloth woven"
    );
    assert_eq!(item_count(&world, 3001, SUEDE), 0, "suede consumed");
    assert_eq!(
        item_count(&world, 3001, SPIDERSILK),
        0,
        "spidersilk consumed"
    );
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed());
}

#[test]
fn quest_q00033_make_a_pair_of_dress_shoes() {
    const WOODLEY: i32 = 30838;
    const IAN: i32 = 30164;
    const LEIKAR: i32 = 31520;
    const LEATHER: i32 = 1882;
    const THREAD: i32 = 1868;
    const ADENA: i32 = 57;
    const DRESS_SHOES_BOX: i32 = 7113;

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (LEATHER, "q", true),
            (THREAD, "q", true),
            (DRESS_SHOES_BOX, "reward", false),
        ],
    );
    let woodley = NPC_OID;
    let ian = NPC_OID + 1;
    let leikar = NPC_OID + 2;
    add_test_npc(&mut world, woodley, WOODLEY, "Folk", 60, 100, 200, 0);
    add_test_npc(&mut world, ian, IAN, "Folk", 60, 100, 200, 0);
    add_test_npc(&mut world, leikar, LEIKAR, "Folk", 60, 100, 200, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 60;
    // Q33 gates on the parent being all the way to cond 7.
    seed_formal_wear(&mut world, 3001, 7);
    let q = "Q00033_MakeAPairOfDressShoes";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    talk(&mut world, woodley);
    ev(&mut world, woodley, "30838-03.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    ev(&mut world, leikar, "31520-02.html"); // → cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    ev(&mut world, woodley, "30838-06.html"); // → cond 3
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    // Ian sells 360 Leather + 90 Thread for 300k adena.
    ev(&mut world, ian, "30164-02.html"); // no adena → refused
    assert_eq!(
        item_count(&world, 3001, LEATHER),
        0,
        "no sale without the fee"
    );
    inject(&mut world, 3001, 0x0033_1000, ADENA, 500_000);
    ev(&mut world, ian, "30164-02.html");
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(5),
        "materials bought → cond 5"
    );
    assert_eq!(item_count(&world, 3001, LEATHER), 360, "leather bought");
    assert_eq!(item_count(&world, 3001, THREAD), 90, "thread bought");
    assert_eq!(item_count(&world, 3001, ADENA), 200_000, "300k paid to Ian");
    // Woodley crafts the shoes for the remaining 200k.
    ev(&mut world, woodley, "30838-13.html");
    assert_eq!(
        item_count(&world, 3001, DRESS_SHOES_BOX),
        1,
        "Dress Shoes Box crafted"
    );
    assert_eq!(item_count(&world, 3001, LEATHER), 0, "leather consumed");
    assert_eq!(item_count(&world, 3001, ADENA), 0, "200k paid to Woodley");
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed());
}

// ---------------------------------------------------------------------------
// Primeval Isle hunts (Q641 Attack Sailren, Q642 A Powerful Primeval Creature).
// ---------------------------------------------------------------------------

/// The Name of Evil - 1 (125): the Primeval Isle letter-puzzle story quest.
/// Drives the Q124 prereq gate, the claw/bone grind, and all three Kaimu
/// pillar puzzles through to the Epitaph of Wisdom and completion.
#[test]
fn quest_q00125_the_name_of_evil_1() {
    const MUSHIKA: i32 = 32114;
    const KARAKAWEI: i32 = 32117;
    const ULU: i32 = 32119;
    const BALU: i32 = 32120;
    const CHUTA: i32 = 32121;
    const CLAW: i32 = 8779;
    const BONE: i32 = 8780;
    const EPITAPH: i32 = 8781;
    const GAZKH_FRAGMENT: i32 = 8782;
    const ORNITHO: i32 = 22200; // 661 claw chance
    const DEINO: i32 = 22203; // 651 bone chance

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (CLAW, "q", true),
            (BONE, "q", true),
            (EPITAPH, "q", true),
            (GAZKH_FRAGMENT, "q", true),
        ],
    );
    for id in [ORNITHO, DEINO] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 78;
        t.base_hp_max = 100.0;
        world.data.npc_data.insert_for_test(t);
    }
    let mushika = NPC_OID;
    let karakawei = NPC_OID + 1;
    let ulu = NPC_OID + 2;
    let balu = NPC_OID + 3;
    let chuta = NPC_OID + 4;
    for (oid, npc) in [
        (mushika, MUSHIKA),
        (karakawei, KARAKAWEI),
        (ulu, ULU),
        (balu, BALU),
        (chuta, CHUTA),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 78, 100, 200, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 78;
    let q = "Q00125_TheNameOfEvil1";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut rx = _rx;

    // Prereq: Meeting the Elroki (124) must be complete. (Talking creates a
    // CREATED state but does not start the quest.)
    talk(&mut world, mushika);
    {
        let quests = world
            .objects
            .get_component_mut::<model::components::social::Quests>(&3001)
            .unwrap();
        quests
            .0
            .entry("Q00124_MeetingTheElroki".to_string())
            .or_default()
            .state = model::quest::state::COMPLETED;
    }

    // Accept → Gazkh Fragment → Karakawei sends hunting.
    ev(&mut world, mushika, "32114-05.html");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    ev(&mut world, mushika, "32114-08.html");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert_eq!(
        item_count(&world, 3001, GAZKH_FRAGMENT),
        1,
        "Gazkh Fragment"
    );
    ev(&mut world, karakawei, "32117-09.html");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));

    // Grind 2 claws + 2 bones (forced drops) → cond 4.
    let mut mob = NPC_OID + 20;
    for id in [ORNITHO, ORNITHO, DEINO, DEINO] {
        mob += 1;
        add_test_npc(&mut world, mob, id, "Monster", 78, 110, 200, 0);
        world.force_roll(0); // roll(1000)==0 < chance → drop
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, CLAW), 2, "2 claws");
    assert_eq!(item_count(&world, 3001, BONE), 2, "2 bones");
    assert_eq!(quest_cond(&world, 3001, q), Some(4), "materials → cond 4");

    // Karakawei takes the materials; then sends to the pillars.
    talk(&mut world, karakawei); // cond 4: consumes claws+bones
    assert_eq!(item_count(&world, 3001, CLAW), 0, "claws consumed");
    ev(&mut world, karakawei, "32117-15.html");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));

    // The puzzle sets the "Memo" quest var only on the full correct word.
    let memo = |w: &World| -> i32 {
        w.objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap()
            .0[q]
            .get_int("Memo")
    };
    // A wrong Ulu attempt (skip letters) does not solve it.
    ev(&mut world, ulu, "T_One");
    ev(&mut world, ulu, "U_One"); // T and U set, but not E/P → fail
    assert_eq!(memo(&world), 0, "an incomplete word is rejected");
    let _ = drain(&mut rx);

    // Ulu Kaimu puzzle T-E-P-U → solves, then cond 6.
    ev(&mut world, ulu, "T_One");
    ev(&mut world, ulu, "E_One");
    ev(&mut world, ulu, "P_One");
    ev(&mut world, ulu, "U_One"); // solves → Memo set
    assert_eq!(memo(&world), 1, "the full word is accepted");
    ev(&mut world, ulu, "32119-18.html");
    assert_eq!(quest_cond(&world, 3001, q), Some(6), "Ulu solved → cond 6");

    // Balu Kaimu puzzle T-O-O2-N → cond 7.
    ev(&mut world, balu, "T_Two");
    ev(&mut world, balu, "O_Two");
    ev(&mut world, balu, "O2_Two");
    ev(&mut world, balu, "N_Two");
    ev(&mut world, balu, "32120-17.html");
    assert_eq!(quest_cond(&world, 3001, q), Some(7), "Balu solved → cond 7");

    // Chuta Kaimu puzzle W-A-G-U → Epitaph, cond 8.
    ev(&mut world, chuta, "W_Three");
    ev(&mut world, chuta, "A_Three");
    ev(&mut world, chuta, "G_Three");
    ev(&mut world, chuta, "U_Three");
    ev(&mut world, chuta, "32121-18.html"); // Gazkh → Epitaph of Wisdom
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(8),
        "Chuta solved → cond 8"
    );
    assert_eq!(item_count(&world, 3001, EPITAPH), 1, "Epitaph of Wisdom");
    assert_eq!(
        item_count(&world, 3001, GAZKH_FRAGMENT),
        0,
        "Gazkh consumed"
    );

    // Mushika completes the quest for the Epitaph.
    talk(&mut world, mushika);
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed(), "completed at Mushika");
}

/// The Name of Evil - 2 (126): the level-77 conclusion — the singing Kaimu
/// ladder, the three-melody Warrior's Grave puzzle, and the reward. Completing
/// it is what unlocks Q641 Attack Sailren.
#[test]
fn quest_q00126_the_name_of_evil_2() {
    const ASAMAH: i32 = 32115;
    const ULU: i32 = 32119;
    const BALU: i32 = 32120;
    const CHUTA: i32 = 32121;
    const GRAVE: i32 = 32122;
    const STATUE: i32 = 32109;
    const MUSHIKA: i32 = 32114;
    const GAZKH_FRAGMENT: i32 = 8782;
    const BONE_POWDER: i32 = 8783;
    const ENCHANT_WEAPON_A: i32 = 729;

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (GAZKH_FRAGMENT, "q", true),
            (BONE_POWDER, "q", true),
            (ENCHANT_WEAPON_A, "reward", false),
        ],
    );
    let asamah = NPC_OID;
    let ulu = NPC_OID + 1;
    let balu = NPC_OID + 2;
    let chuta = NPC_OID + 3;
    let grave = NPC_OID + 4;
    let statue = NPC_OID + 5;
    let mushika = NPC_OID + 6;
    for (oid, npc) in [
        (asamah, ASAMAH),
        (ulu, ULU),
        (balu, BALU),
        (chuta, CHUTA),
        (grave, GRAVE),
        (statue, STATUE),
        (mushika, MUSHIKA),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 78, 100, 200, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 78;
    let q = "Q00126_TheNameOfEvil2";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let cond = |w: &World| quest_cond(w, 3001, q);

    // Prereq: The Name of Evil - 1 (125) complete.
    talk(&mut world, asamah);
    {
        let quests = world
            .objects
            .get_component_mut::<model::components::social::Quests>(&3001)
            .unwrap();
        quests
            .0
            .entry("Q00125_TheNameOfEvil1".to_string())
            .or_default()
            .state = model::quest::state::COMPLETED;
    }

    // Accept and walk the singing-Kaimu ladder (cond 1 → 11).
    ev(&mut world, asamah, "32115-1.html");
    assert_eq!(cond(&world), Some(1));
    for (npc, e, expect) in [
        (asamah, "32115-1b.html", 2),
        (ulu, "32119-3.html", 3),
        (ulu, "32119-4.html", 4),
        (ulu, "32119-5.html", 5),
        (balu, "32120-3.html", 6),
        (balu, "32120-4.html", 7),
        (balu, "32120-5.html", 8),
        (chuta, "32121-3.html", 9),
        (chuta, "32121-4.html", 10),
        (chuta, "32121-5.html", 11),
    ] {
        ev(&mut world, npc, e);
        assert_eq!(cond(&world), Some(expect), "advance via {e}");
    }
    assert_eq!(
        item_count(&world, 3001, GAZKH_FRAGMENT),
        1,
        "Gazkh Fragment from Chuta"
    );

    // Warrior's Grave: talking advances to cond 12, then to the melodies.
    talk(&mut world, grave); // cond 11 → 12
    assert_eq!(cond(&world), Some(12));
    ev(&mut world, grave, "32122-3.html"); // → 13
    ev(&mut world, grave, "32122-4.html"); // → 14
    assert_eq!(cond(&world), Some(14), "at the first melody");

    // Melody 1 rejects an incomplete tune, accepts the full one.
    ev(&mut world, grave, "DO_One");
    ev(&mut world, grave, "FA2_One"); // DO + FA2 only → fail
    assert_eq!(cond(&world), Some(14), "an incomplete melody is rejected");
    for note in ["DO_One", "MI_One", "FA_One", "SOL_One", "FA2_One"] {
        ev(&mut world, grave, note);
    }
    assert_eq!(cond(&world), Some(15), "melody 1 → cond 15");
    // Melody 2.
    for note in ["FA_Two", "SOL_Two", "TI_Two", "SOL2_Two", "FA2_Two"] {
        ev(&mut world, grave, note);
    }
    assert_eq!(cond(&world), Some(16), "melody 2 → cond 16");
    // Melody 3.
    for note in [
        "SOL_Three",
        "FA_Three",
        "MI_Three",
        "FA2_Three",
        "MI2_Three",
    ] {
        ev(&mut world, grave, note);
    }
    assert_eq!(cond(&world), Some(17), "melody 3 → cond 17");

    // The grave raises the Bone Powder; on to the statue and back.
    ev(&mut world, grave, "32122-7.html");
    assert_eq!(
        item_count(&world, 3001, BONE_POWDER),
        1,
        "Bone Powder raised"
    );
    ev(&mut world, grave, "32122-8.html"); // → 18
    ev(&mut world, statue, "32109-2.html"); // → 19
    ev(&mut world, statue, "32109-3.html"); // → 20, takes Bone Powder
    assert_eq!(item_count(&world, 3001, BONE_POWDER), 0, "Bone Powder read");
    ev(&mut world, asamah, "32115-4.html"); // → 21
    ev(&mut world, asamah, "32115-5.html"); // → 22
    ev(&mut world, mushika, "32114-2.html"); // → 23
    assert_eq!(cond(&world), Some(23));

    // Mushika's reward completes the chain.
    ev(&mut world, mushika, "32114-3.html");
    assert_eq!(
        item_count(&world, 3001, ENCHANT_WEAPON_A),
        1,
        "A-grade Weapon Enchant"
    );
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed(), "The Name of Evil - 2 complete");
}

/// An Obvious Lie (32): Maximilian → Gentler's map → Miki → farm 20 Medicinal
/// Herbs from Alligators → hand over herbs, Spirit Ore, and Thread + Suede for
/// a pair of Cat Ears.
#[test]
fn quest_q00032_an_obvious_lie() {
    const MAXIMILIAN: i32 = 30120;
    const GENTLER: i32 = 30094;
    const MIKI: i32 = 31706;
    const ALLIGATOR: i32 = 20135;
    const MAP_OF_GENTLER: i32 = 7165;
    const MEDICINAL_HERB: i32 = 7166;
    const SPIRIT_ORE: i32 = 3031;
    const THREAD: i32 = 1868;
    const SUEDE: i32 = 1866;
    const CAT_EARS: i32 = 6843;

    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> =
        [MAP_OF_GENTLER, MEDICINAL_HERB, SPIRIT_ORE, THREAD, SUEDE]
            .iter()
            .map(|&i| (i, "q", true))
            .collect();
    items.push((CAT_EARS, "ears", false));
    add_quest_items(&mut world, &items);
    let mut at = crate::data::npc_data::default_template(ALLIGATOR);
    at.type_name = "Monster".into();
    at.level = 46;
    at.base_hp_max = 100.0;
    world.data.npc_data.insert_for_test(at);
    let maximilian = NPC_OID;
    let gentler = NPC_OID + 1;
    let miki = NPC_OID + 2;
    for (oid, npc) in [(maximilian, MAXIMILIAN), (gentler, GENTLER), (miki, MIKI)] {
        add_test_npc(&mut world, oid, npc, "Folk", 46, 100, 200, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 46;
    let q = "Q00032_AnObviousLie";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let cond = |w: &World| quest_cond(w, 3001, q);
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };

    // Accept → Gentler's map → Miki takes it (cond 3).
    talk(&mut world, maximilian);
    ev(&mut world, maximilian, "30120-02.html");
    assert_eq!(cond(&world), Some(1), "started");
    ev(&mut world, gentler, "30094-02.html");
    assert_eq!(cond(&world), Some(2));
    assert_eq!(item_count(&world, 3001, MAP_OF_GENTLER), 1, "map given");
    ev(&mut world, miki, "31706-02.html");
    assert_eq!(cond(&world), Some(3));
    assert_eq!(
        item_count(&world, 3001, MAP_OF_GENTLER),
        0,
        "map surrendered"
    );

    // Farm 20 Medicinal Herbs from Alligators → cond 4.
    let mut mob = NPC_OID + 20;
    for _ in 0..20 {
        mob += 1;
        add_test_npc(&mut world, mob, ALLIGATOR, "Monster", 46, 110, 200, 0);
        world.force_roll(0); // give_item_randomly → drop
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, MEDICINAL_HERB), 20, "20 herbs");
    assert_eq!(cond(&world), Some(4), "20th herb → cond 4");

    // Gentler takes the herbs (cond 5), then Spirit Ore (cond 6).
    ev(&mut world, gentler, "30094-06.html");
    assert_eq!(cond(&world), Some(5));
    assert_eq!(
        item_count(&world, 3001, MEDICINAL_HERB),
        0,
        "herbs consumed"
    );
    inject(&mut world, 3001, 0x0032_1000, SPIRIT_ORE, 500);
    ev(&mut world, gentler, "30094-09.html");
    assert_eq!(cond(&world), Some(6));
    assert_eq!(
        item_count(&world, 3001, SPIRIT_ORE),
        0,
        "spirit ore consumed"
    );

    // Miki (cond 7), Gentler (cond 8).
    ev(&mut world, miki, "31706-05.html");
    assert_eq!(cond(&world), Some(7));
    ev(&mut world, gentler, "30094-12.html");
    assert_eq!(cond(&world), Some(8));

    // Without Thread + Suede, no ears.
    ev(&mut world, gentler, "cat");
    assert_eq!(
        item_count(&world, 3001, CAT_EARS),
        0,
        "no ears without materials"
    );
    // With them, the Cat Ears are crafted and the quest completes.
    inject(&mut world, 3001, 0x0032_2000, THREAD, 1000);
    inject(&mut world, 3001, 0x0032_3000, SUEDE, 500);
    ev(&mut world, gentler, "cat");
    assert_eq!(item_count(&world, 3001, CAT_EARS), 1, "Cat Ears crafted");
    assert_eq!(item_count(&world, 3001, THREAD), 0, "thread consumed");
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed(), "one-time quest completes");
}
