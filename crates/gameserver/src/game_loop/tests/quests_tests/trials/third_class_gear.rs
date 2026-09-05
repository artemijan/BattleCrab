//! Q00234 Fate's Whisper and Q00235 Mimir's Elixir — the two quests that
//! gate the third class change rather than certifying for it.

use super::super::*;

#[test]
fn quest_q00235_mimirs_elixir() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (5011, "Star of Destiny", false),
            (6320, "Pure Silver", true),
            (6321, "True Gold", true),
            (6322, "Sage Stone", true),
            (6318, "Blood Fire", true),
            (6319, "Mimir's Elixir", true),
            (5905, "Magister Mixing Stone", true),
            (729, "Scroll: Enchant Weapon (A)", false),
        ],
    );
    for id in [20965, 21090] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 75;
        world.data.npc_data.insert_for_test(t);
    }
    let ladd = NPC_OID;
    let joan = NPC_OID + 1;
    let urn = NPC_OID + 2;
    add_test_npc(&mut world, ladd, 30721, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, joan, 30718, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, urn, 31149, "Folk", 70, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 76;
    inject(&mut world, 3001, 0x0235_9000, 5011, 1); // Star of Destiny (Fate's Whisper prereq)
    let q = "Q00235_MimirsElixir";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    talk(&mut world, ladd);
    ev(&mut world, ladd, "30721-06.htm"); // start, cond 1
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // cond 1 → 2 needs Pure Silver (the Q00373 product).
    inject(&mut world, 3001, 0x0235_0000, 6320, 1);
    talk(&mut world, ladd);
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // Joan → cond 3, then a Sage Stone drop → cond 4.
    ev(&mut world, joan, "30718-03.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    add_test_npc(&mut world, NPC_OID + 10, 20965, "Monster", 75, 30, 0, 0);
    world.force_roll(0); // roll(10) < 2 → Sage Stone
    npc::npc_do_die(&mut world, NPC_OID + 10, 3001);
    assert_eq!(item_count(&world, 3001, 6322), 1, "Sage Stone drops");
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // Joan forges True Gold (cond 4 → 5).
    talk(&mut world, joan);
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    assert_eq!(item_count(&world, 3001, 6321), 1, "True Gold forged");
    assert_eq!(item_count(&world, 3001, 6322), 0, "Sage Stone consumed");
    // Ladd hands over the Magister Mixing Stone (cond 5 → 6).
    ev(&mut world, ladd, "30721-12.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    assert_eq!(item_count(&world, 3001, 5905), 1, "Mixing Stone received");
    // A Blood Fire drop → cond 7.
    add_test_npc(&mut world, NPC_OID + 11, 21090, "Monster", 75, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 11, 3001);
    assert_eq!(item_count(&world, 3001, 6318), 1, "Blood Fire drops");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    // Mix at the Urn → Mimir's Elixir (cond 7 → 8), consuming silver/gold/fire.
    ev(&mut world, urn, "31149-success.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    assert_eq!(item_count(&world, 3001, 6319), 1, "Mimir's Elixir brewed");
    assert_eq!(
        item_count(&world, 3001, 6320)
            + item_count(&world, 3001, 6321)
            + item_count(&world, 3001, 6318),
        0,
        "silver/gold/fire consumed"
    );
    // Ladd redeems the elixir for the A-grade enchant scroll and finishes.
    ev(&mut world, ladd, "30721-16.htm");
    assert_eq!(
        item_count(&world, 3001, 729),
        1,
        "Scroll: Enchant Weapon (A) awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 5011),
        0,
        "Star of Destiny consumed"
    );
    assert_eq!(item_count(&world, 3001, 6319), 0, "elixir consumed");
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(8),
        "one-time quest finished"
    );
}

/// Fate's Whisper (234) end to end: the level-75 gate, the boss-chest material
/// chain (Soul Orb + three Infernium Scepters), the Varnish/Hammer/Mold forging
/// steps, the Pipette-on-Baium fill, and Reorin's templated B→A grade weapon
/// upgrade that awards the Star of Destiny. Also proves `onKill` drops a chest.
#[test]
fn quest_q00234_fates_whisper() {
    // Items
    const REIRIA_SOUL_ORB: i32 = 4666;
    const KERMON: i32 = 4667;
    const GOLKONDA: i32 = 4668;
    const HALLATE: i32 = 4669;
    const INFERNIUM_VARNISH: i32 = 4672;
    const REORIN_HAMMER: i32 = 4670;
    const REORIN_MOLD: i32 = 4671;
    const PIPETTE_KNIFE: i32 = 4665;
    const RED_PIPETTE_KNIFE: i32 = 4673;
    const CRYSTAL_B: i32 = 1460;
    const STAR_OF_DESTINY: i32 = 5011;
    const B_GRADE: i32 = 79; // Sword of Damascus
    const A_GRADE: i32 = 6580; // the upgraded weapon
    // NPCs
    const REORIN: i32 = 31002;
    const N30182: i32 = 30182;
    const N30847: i32 = 30847;
    const N30178: i32 = 30178;
    const N30833: i32 = 30833;
    const BAIUM: i32 = 29020;
    const BOSS_25035: i32 = 25035;

    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = [
        REIRIA_SOUL_ORB,
        KERMON,
        GOLKONDA,
        HALLATE,
        INFERNIUM_VARNISH,
        REORIN_HAMMER,
        REORIN_MOLD,
        PIPETTE_KNIFE,
        RED_PIPETTE_KNIFE,
        CRYSTAL_B,
        B_GRADE,
    ]
    .iter()
    .map(|&id| (id, "Q234", true))
    .collect();
    items.push((STAR_OF_DESTINY, "Star of Destiny", false));
    items.push((A_GRADE, "Sirra's Blade", false));
    add_quest_items(&mut world, &items);

    // Baium + the chest-dropping boss.
    for id in [BAIUM, BOSS_25035] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 80;
        t.base_hp_max = 1_000_000.0;
        world.data.npc_data.insert_for_test(t);
    }

    let reorin = NPC_OID;
    let n30182 = NPC_OID + 1;
    let n30847 = NPC_OID + 2;
    let n30178 = NPC_OID + 3;
    let n30833 = NPC_OID + 4;
    let chest27 = NPC_OID + 5;
    let chest28 = NPC_OID + 6;
    let chest29 = NPC_OID + 7;
    let chest30 = NPC_OID + 8;
    for (oid, npc) in [
        (reorin, REORIN),
        (n30182, N30182),
        (n30847, N30847),
        (n30178, N30178),
        (n30833, N30833),
        (chest27, 31027),
        (chest28, 31028),
        (chest29, 31029),
        (chest30, 31030),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 200, 0);
    }

    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 74;
    let q = "Q00234_FatesWhisper";
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

    // --- Level gate: below 75 is refused (the 31002-01 page has no start link). ---
    talk(&mut world, reorin);
    let html = grab_html(&mut rx).expect("Reorin greeting");
    assert!(
        !html.contains("31002-03.htm"),
        "no start offered below 75: {html}"
    );
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 76;

    // --- Accept. ---
    ev(&mut world, reorin, "31002-03.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started at cond 1");

    // --- onKill drops a chest beside the boss. ---
    let before = npcs_of(&mut world, 31027).len();
    add_test_npc(
        &mut world,
        NPC_OID + 40,
        BOSS_25035,
        "Monster",
        80,
        300,
        300,
        0,
    );
    npc::npc_do_die(&mut world, NPC_OID + 40, 3001);
    assert!(
        npcs_of(&mut world, 31027).len() > before,
        "killing boss 25035 spawns a 31027 chest"
    );
    // Java's `addSpawn(…, true, 120000)`: the chest is on a two-minute fuse,
    // so a drop nobody collects clears itself instead of standing for ever.
    // Just short of two minutes it is still there…
    advance_ticks(&mut world, 1190);
    assert!(
        npcs_of(&mut world, 31027).len() > before,
        "the chest is still standing before its 120 s are up"
    );
    // …and past it, gone.
    advance_ticks(&mut world, 30);
    assert_eq!(
        npcs_of(&mut world, 31027).len(),
        before,
        "the chest despawns after two minutes"
    );
    // Re-spawn one for the rest of the walkthrough, which opens it.
    add_test_npc(
        &mut world,
        NPC_OID + 41,
        BOSS_25035,
        "Monster",
        80,
        300,
        300,
        0,
    );
    npc::npc_do_die(&mut world, NPC_OID + 41, 3001);

    // --- Chest 31027 → Reiria Soul Orb, then Reorin advances to cond 2. ---
    talk(&mut world, chest27);
    assert_eq!(
        item_count(&world, 3001, REIRIA_SOUL_ORB),
        1,
        "Soul Orb from chest"
    );
    talk(&mut world, reorin);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "orb turned in → cond 2"
    );
    assert_eq!(item_count(&world, 3001, REIRIA_SOUL_ORB), 0, "orb consumed");

    // --- Chests 31028/29/30 → the three Infernium Scepters, Reorin → cond 3. ---
    talk(&mut world, chest28);
    talk(&mut world, chest29);
    talk(&mut world, chest30);
    assert_eq!(item_count(&world, 3001, KERMON), 1);
    assert_eq!(item_count(&world, 3001, GOLKONDA), 1);
    assert_eq!(item_count(&world, 3001, HALLATE), 1);
    talk(&mut world, reorin);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(3),
        "scepters turned in → cond 3"
    );

    // --- 30182 hands over the Infernium Varnish (via its 01c event). ---
    ev(&mut world, n30182, "30182-01c.htm");
    assert_eq!(
        item_count(&world, 3001, INFERNIUM_VARNISH),
        1,
        "Varnish received"
    );
    talk(&mut world, reorin);
    assert_eq!(quest_cond(&world, 3001, q), Some(4), "varnish → cond 4");

    // --- 30847 gives the Reorin Hammer. ---
    talk(&mut world, n30847);
    assert_eq!(
        item_count(&world, 3001, REORIN_HAMMER),
        1,
        "Hammer received"
    );
    talk(&mut world, reorin);
    assert_eq!(quest_cond(&world, 3001, q), Some(5), "hammer → cond 5");

    // --- 30178 (cond 5) → its 01a event advances to cond 6. ---
    ev(&mut world, n30178, "30178-01a.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));

    // --- 30833 (cond 6) → its 01b event gives the Pipette Knife, cond 7. ---
    ev(&mut world, n30833, "30833-01b.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    assert_eq!(
        item_count(&world, 3001, PIPETTE_KNIFE),
        1,
        "Pipette Knife received"
    );

    // --- Baium: striking with the Pipette Knife equipped fills it (Red). ---
    equip_weapon_row(&mut world, 3001, PIPETTE_KNIFE); // RHand = Pipette Knife
    add_test_npc(&mut world, NPC_OID + 51, BAIUM, "Monster", 80, 120, 200, 0);
    combat::npc_receive_damage(&mut world, NPC_OID + 51, 3001, 10.0, false);
    assert_eq!(
        item_count(&world, 3001, RED_PIPETTE_KNIFE),
        1,
        "Pipette filled on Baium"
    );
    assert_eq!(
        item_count(&world, 3001, PIPETTE_KNIFE),
        0,
        "empty Pipette consumed"
    );

    // --- 30833 (cond 7, holding Red Pipette) → Reorin Mold, cond 8. ---
    talk(&mut world, n30833);
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    assert_eq!(item_count(&world, 3001, REORIN_MOLD), 1, "Mold received");

    // --- Reorin: cond 8 → cond 9 (mold consumed), then 984 crystals → cond 10. ---
    talk(&mut world, reorin);
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    assert_eq!(item_count(&world, 3001, REORIN_MOLD), 0, "mold consumed");
    inject(&mut world, 3001, 0x0234_1000, CRYSTAL_B, 984);
    talk(&mut world, reorin);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(10),
        "984 crystals → cond 10"
    );
    assert_eq!(item_count(&world, 3001, CRYSTAL_B), 0, "crystals consumed");

    // --- The weapon UI: select a B-grade, confirm, then upgrade to A-grade. ---
    let _ = drain(&mut rx); // clear the queued BGradeList page
    ev(&mut world, reorin, "selectBGrade_79");
    let html = grab_html(&mut rx).expect("B-grade confirm page");
    assert!(
        html.contains("Sword of Damascus"),
        "%weaponname% substituted into 31002-13: {html}"
    );
    ev(&mut world, reorin, "confirmWeapon");
    let _ = drain(&mut rx);
    // The A-grade trade needs the B-grade weapon in the bag.
    inject(&mut world, 3001, 0x0234_2000, B_GRADE, 1);
    ev(&mut world, reorin, &format!("selectAGrade_{A_GRADE}"));
    assert_eq!(item_count(&world, 3001, B_GRADE), 0, "B-grade consumed");
    assert_eq!(
        item_count(&world, 3001, A_GRADE),
        1,
        "A-grade weapon forged"
    );
    assert_eq!(
        item_count(&world, 3001, STAR_OF_DESTINY),
        1,
        "Star of Destiny awarded"
    );
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed(), "quest completes on the upgrade");
}
