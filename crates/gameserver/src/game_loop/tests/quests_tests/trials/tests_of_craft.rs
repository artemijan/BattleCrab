//! Q00231-Q00233 — the third class tests for the dwarf and orc classes:
//! Maestro, Lord and War Spirit.

use super::super::*;

#[test]
fn quest_q00231_test_of_the_maestro() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = [
        2864, 2865, 2866, 2868, 2869, 2870, 2871, 2872, 2873, 2874, 2875, 2876, 2877, 2878, 2916,
    ]
    .iter()
    .map(|&id| (id, "Q231", true))
    .collect();
    items.push((2867, "Mark of Maestro", false));
    add_quest_items(&mut world, &items);
    for id in [27133, 20225, 20150] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let lockirin = NPC_OID;
    let balanki = NPC_OID + 1;
    let filaur = NPC_OID + 2;
    let arin = NPC_OID + 3;
    let toma = NPC_OID + 4;
    let croto = NPC_OID + 5;
    let lorain = NPC_OID + 6;
    for (oid, npc) in [
        (lockirin, 30531),
        (balanki, 30533),
        (filaur, 30535),
        (arin, 30536),
        (toma, 30556),
        (croto, 30671),
        (lorain, 30673),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 56; // Artisan
    }
    let q = "Q00231_TestOfTheMaestro";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    talk(&mut world, lockirin);
    ev(&mut world, lockirin, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // --- Balanki's recommendation (memo 2): Croto → Evil Eye Lord → letter. ---
    talk(&mut world, balanki);
    ev(&mut world, balanki, "30533-02.html"); // memo 2
    talk(&mut world, croto);
    ev(&mut world, croto, "30671-02.html"); // Paint of Kamuru
    assert_eq!(item_count(&world, 3001, 2869), 1, "Paint of Kamuru");
    add_test_npc(&mut world, NPC_OID + 20, 27133, "Monster", 40, 30, 0, 0);
    npc::npc_do_die(&mut world, NPC_OID + 20, 3001);
    assert_eq!(
        item_count(&world, 3001, 2870),
        1,
        "Evil Eye Lord drops the Necklace of Kamutu"
    );
    talk(&mut world, croto); // necklace → Letter of Solder Detachment
    assert_eq!(item_count(&world, 3001, 2868), 1, "Letter received");
    talk(&mut world, balanki); // letter → Recommendation of Balanki
    assert_eq!(
        item_count(&world, 3001, 2864),
        1,
        "Recommendation of Balanki"
    );
    // --- Arin's recommendation (memo 3): Toma's teleport-device errand. ---
    talk(&mut world, arin); // Paint of Teleport Device, memo 3
    assert_eq!(
        item_count(&world, 3001, 2871),
        1,
        "Paint of Teleport Device"
    );
    talk(&mut world, toma);
    ev(&mut world, toma, "30556-05.html"); // Broken device + teleport + timer
    assert_eq!(item_count(&world, 3001, 2916), 1, "Broken Teleport Device");
    // The 5-second timer conjures three King Bugbears at the arrival spot.
    advance_ticks(&mut world, 50);
    assert_eq!(
        npcs_of(&mut world, 20150).len(),
        3,
        "on_timer ambush spawns three King Bugbears"
    );
    // The errand teleported the player to Cruma; walk back to the NPCs so the
    // bypass interaction-distance guard lets the remaining turn-ins through.
    {
        let pos = world.objects.get_component_mut::<Position>(&3001).unwrap();
        pos.x = 100;
        pos.y = 0;
        pos.z = 0;
    }
    talk(&mut world, toma); // broken → 5 Teleport Devices
    assert_eq!(item_count(&world, 3001, 2872), 5, "5 Teleport Devices");
    talk(&mut world, arin); // devices → Recommendation of Arin
    assert_eq!(item_count(&world, 3001, 2866), 1, "Recommendation of Arin");
    // --- Filaur's recommendation (memo 4): Lorain's antidote errand. ---
    talk(&mut world, filaur); // Architecture of Cruma, memo 4
    talk(&mut world, lorain); // Ingredients of Antidote
    assert_eq!(item_count(&world, 3001, 2875), 1, "Ingredients of Antidote");
    add_test_npc(&mut world, NPC_OID + 21, 20225, "Monster", 40, 30, 0, 0);
    npc::npc_do_die(&mut world, NPC_OID + 21, 3001);
    assert_eq!(
        item_count(&world, 3001, 2878),
        1,
        "Giant Mist Leech drops Blood of Leech"
    );
    // Fast-forward the rest of the antidote reagents (no kill counter here).
    inject(&mut world, 3001, 0x0231_0000, 2878, 9); // 10 leech blood
    inject(&mut world, 3001, 0x0231_0001, 2876, 10); // wasp needle
    inject(&mut world, 3001, 0x0231_0002, 2877, 10); // spider web
    ev(&mut world, lorain, "30673-04.html"); // → Report of Cruma
    assert_eq!(item_count(&world, 3001, 2874), 1, "Report of Cruma");
    talk(&mut world, filaur); // report → Recommendation of Filaur, cond 2
    assert_eq!(
        item_count(&world, 3001, 2865),
        1,
        "Recommendation of Filaur"
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "all three recommendations → cond 2"
    );
    // --- Lockirin awards the Mark of Maestro. ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, lockirin);
    assert_eq!(item_count(&world, 3001, 2867), 1, "Mark of Maestro awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 372154,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(2),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00232_test_of_the_lord() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (3391..=3416).map(|id| (id, "Q232", true)).collect();
    items.push((3390, "Mark of Lord", false));
    items.push((1341, "Bone Arrow", false));
    add_quest_items(&mut world, &items);
    for id in [20269, 20583, 20233, 20564, 20778] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(30643));
    let kakai = NPC_OID;
    let manakia = NPC_OID + 1;
    let jakal = NPC_OID + 2;
    let sumari = NPC_OID + 3;
    let somak = NPC_OID + 4;
    let varkees = NPC_OID + 5;
    let tantus = NPC_OID + 6;
    let hatos = NPC_OID + 7;
    let takuna = NPC_OID + 8;
    let chianta = NPC_OID + 9;
    let martankus = NPC_OID + 10;
    let first_orc = NPC_OID + 11;
    for (oid, npc) in [
        (kakai, 30565),
        (manakia, 30515),
        (jakal, 30558),
        (sumari, 30564),
        (somak, 30510),
        (varkees, 30566),
        (tantus, 30567),
        (hatos, 30568),
        (takuna, 30641),
        (chianta, 30642),
        (martankus, 30649),
        (first_orc, 30643),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 50; // Orc Shaman
        p.race = 3; // Orc
    }
    let q = "Q00232_TestOfTheLord";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 40;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, kakai);
    ev(&mut world, kakai, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(item_count(&world, 3001, 3391), 1, "Ordeal Necklace");
    // --- Leg: Varkees → Huge Orc Fang ---
    ev(&mut world, varkees, "30566-02.html"); // Varkees Charm
    talk(&mut world, manakia); // Manakia's Orders
    inject(&mut world, 3001, 0x0232_0000, 3398, 18); // 18 Breka Orc Fangs
    kill(&mut world, 20269); // +2 → 20
    talk(&mut world, manakia); // fangs → Manakia's Amulet
    assert_eq!(item_count(&world, 3001, 3399), 1, "Manakia's Amulet");
    talk(&mut world, varkees); // charm + amulet → Huge Orc Fang
    assert_eq!(item_count(&world, 3001, 3400), 1, "Huge Orc Fang");
    // --- Leg: Tantus → Axe of Ceremony ---
    ev(&mut world, tantus, "30567-02.html"); // Tantus Charm
    inject(&mut world, 3001, 0x0232_0001, 57, 1000);
    ev(&mut world, jakal, "30558-02.html"); // 1000 adena → Neruga Axe Blade
    inject(&mut world, 3001, 0x0232_0002, 1341, 1000); // 1000 Bone Arrows
    talk(&mut world, tantus); // blade + arrows → Axe of Ceremony
    assert_eq!(item_count(&world, 3001, 3406), 1, "Axe of Ceremony");
    // --- Leg: Hatos → Sword into Skull ---
    ev(&mut world, hatos, "30568-02.html"); // Hatos Charm
    talk(&mut world, sumari); // Sumari's Letter
    talk(&mut world, somak); // letter → Urutu Blade
    assert_eq!(item_count(&world, 3001, 3402), 1, "Urutu Blade");
    inject(&mut world, 3001, 0x0232_0003, 3403, 9); // 9 Timak Orc Skulls
    kill(&mut world, 20583); // +1 → 10
    talk(&mut world, hatos); // blade + skulls → Sword into Skull
    assert_eq!(item_count(&world, 3001, 3404), 1, "Sword into Skull");
    // --- Leg: Takuna → Handiwork Spider Brooch ---
    ev(&mut world, takuna, "30641-02.html"); // Takuna Charm
    inject(&mut world, 3001, 0x0232_0004, 3407, 8); // 8 feelers
    kill(&mut world, 20233); // +2 → 10 feelers
    inject(&mut world, 3001, 0x0232_0005, 3408, 8); // 8 feet
    kill(&mut world, 20233); // +2 → 10 feet
    talk(&mut world, takuna); // → Handiwork Spider Brooch
    assert_eq!(item_count(&world, 3001, 3409), 1, "Handiwork Spider Brooch");
    // --- Leg: Chianta → Monster Eye Woodcarving (fifth → cond 2) ---
    ev(&mut world, chianta, "30642-02.html"); // Chianta Charm
    inject(&mut world, 3001, 0x0232_0006, 3410, 19); // 19 Cornea
    kill(&mut world, 20564); // +1 → 20
    talk(&mut world, chianta); // → Monster Eye Woodcarving, cond 2
    assert_eq!(item_count(&world, 3001, 3411), 1, "Monster Eye Woodcarving");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // --- Kakai forges the Bear Fang Necklace ---
    talk(&mut world, kakai);
    ev(&mut world, kakai, "30565-08.html"); // all five → Bear Fang Necklace, cond 3
    assert_eq!(item_count(&world, 3001, 3412), 1, "Bear Fang Necklace");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, martankus);
    ev(&mut world, martankus, "30649-04.html"); // → Martankus Charm, cond 4
    assert_eq!(item_count(&world, 3001, 3413), 1, "Martankus Charm");
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    kill(&mut world, 20778); // Ragna → Chief Notice
    kill(&mut world, 20778); // Ragna → Ragna Orc Head, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, martankus); // notice + head → Immortal Flame, cond 6
    assert_eq!(item_count(&world, 3001, 3416), 1, "Immortal Flame");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    talk(&mut world, first_orc); // cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    // --- Completion ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, kakai);
    assert_eq!(
        item_count(&world, 3001, 3390),
        1,
        "Mark of the Lord awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 161806,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(7),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00233_test_of_the_war_spirit() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (2880..=2914).map(|id| (id, "Q233", true)).collect();
    items.push((2879, "Mark of Warspirit", false));
    add_quest_items(&mut world, &items);
    for id in [20089, 20581, 27108, 20158, 20213, 20214, 20215, 20601] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let somak = NPC_OID;
    let racoy = NPC_OID + 1;
    let vivyan = NPC_OID + 2;
    let sarien = NPC_OID + 3;
    let pekiron = NPC_OID + 4;
    let manakia = NPC_OID + 5;
    let orim = NPC_OID + 6;
    let martankus = NPC_OID + 7;
    for (oid, npc) in [
        (somak, 30510),
        (racoy, 30507),
        (vivyan, 30030),
        (sarien, 30436),
        (pekiron, 30682),
        (manakia, 30515),
        (orim, 30630),
        (martankus, 30649),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 50; // Orc Shaman
        p.race = 3; // Orc
    }
    let q = "Q00233_TestOfTheWarSpirit";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 40;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, somak);
    ev(&mut world, somak, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // --- Leg Kiruna (Racoy): totem + insect book + 5 randomly-tiered bones ---
    ev(&mut world, racoy, "30507-02.html"); // Racoy's Totem
    ev(&mut world, vivyan, "30030-04.html"); // Viviante's Letter
    talk(&mut world, sarien); // letter → Insect Diagram Book
    assert_eq!(item_count(&world, 3001, 2904), 1, "Insect Diagram Book");
    for r in [70, 70, 50, 50, 10] {
        world.force_roll(r); // thigh, arm, spine, rib, skull
        kill(&mut world, 20089);
    }
    talk(&mut world, racoy); // bones → Kiruna's Remains 1
    assert_eq!(item_count(&world, 3001, 2910), 1, "Kiruna's Remains 1");
    // --- Leg Tonar (Pekiron): 5 sequential bones off Leto ---
    ev(&mut world, pekiron, "30682-02.html"); // Pekiron's Totem
    for _ in 0..5 {
        kill(&mut world, 20581);
    }
    talk(&mut world, pekiron); // → Tonar's Remains 1
    assert_eq!(item_count(&world, 3001, 2894), 1, "Tonar's Remains 1");
    // --- Leg Hermodt (Manakia): skull off Stenoa + 4 bones off Medusa ---
    ev(&mut world, manakia, "30515-02.html"); // Manakia's Totem
    kill(&mut world, 27108); // Stenoa → Hermodt's Skull
    for _ in 0..4 {
        kill(&mut world, 20158); // Medusa → rib/spine/arm/thigh
    }
    talk(&mut world, manakia); // → Hermodt's Remains 1
    assert_eq!(item_count(&world, 3001, 2901), 1, "Hermodt's Remains 1");
    // --- Leg Brakis (Orim): 30 reagents → fourth remains → cond 2 ---
    ev(&mut world, orim, "30630-04.html"); // Orim's Contract
    inject(&mut world, 3001, 0x0233_0000, 2884, 8); // 8 Porta's Eyes
    kill(&mut world, 20213); // +2 → 10
    inject(&mut world, 3001, 0x0233_0001, 2885, 5); // 5 Excuro's Scales
    kill(&mut world, 20214); // +5 → 10
    inject(&mut world, 3001, 0x0233_0002, 2886, 5); // 5 Mordeo's Talons
    kill(&mut world, 20215); // +5 → 10
    talk(&mut world, orim); // 30 reagents → Brakis's Remains 1, cond 2
    assert_eq!(item_count(&world, 3001, 2887), 1, "Brakis's Remains 1");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // --- Somak forges the Vendetta Totem ---
    talk(&mut world, somak); // 4 remains → Vendetta Totem, cond 3
    assert_eq!(item_count(&world, 3001, 2880), 1, "Vendetta Totem");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    inject(&mut world, 3001, 0x0233_0003, 2881, 12); // 12 Tamlin Orc Heads
    kill(&mut world, 20601); // 13th → cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    talk(&mut world, somak); // 13 heads → War Spirit Totem + remains2, cond 5
    assert_eq!(item_count(&world, 3001, 2882), 1, "War Spirit Totem");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    // --- Completion at Martankus ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, martankus);
    ev(&mut world, martankus, "30649-03.html");
    assert_eq!(
        item_count(&world, 3001, 2879),
        1,
        "Mark of the War Spirit awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 161806,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(5),
        "one-time quest finished"
    );
}
