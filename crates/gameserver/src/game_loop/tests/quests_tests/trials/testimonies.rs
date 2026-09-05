//! Q00217-Q00221 — the second class testimonies: Trust, Life, Fate, Glory
//! and Prosperity.

use super::super::*;

#[test]
fn quest_q00217_testimony_of_trust() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (2735..=2761).map(|id| (id, "Q217", true)).collect();
    items.push((2734, "Mark of Trust", false));
    add_quest_items(&mut world, &items);
    for id in [
        20013, 27121, 20036, 27120, 20550, 20082, 20157, 20553, 20213,
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let hollint = NPC_OID;
    let biotin = NPC_OID + 1;
    let asterios = NPC_OID + 2;
    let thifiell = NPC_OID + 3;
    let clayton = NPC_OID + 4;
    let manakia = NPC_OID + 5;
    let lockirin = NPC_OID + 6;
    let kakai = NPC_OID + 7;
    let nikola = NPC_OID + 8;
    let seresin = NPC_OID + 9;
    for (oid, npc) in [
        (hollint, 30191),
        (biotin, 30031),
        (asterios, 30154),
        (thifiell, 30358),
        (clayton, 30464),
        (manakia, 30515),
        (lockirin, 30531),
        (kakai, 30565),
        (nikola, 30621),
        (seresin, 30657),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 1; // Warrior (Human, HUMAN_2ND_GROUP)
        p.race = 0; // Human
    }
    world
        .data
        .categories
        .insert_for_test("HUMAN_2ND_GROUP", &[1]);
    let q = "Q00217_TestimonyOfTrust";
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
    talk(&mut world, hollint);
    ev(&mut world, hollint, "ACCEPT");
    assert_eq!(quest_memo(&world, 3001, q), 1);
    talk(&mut world, asterios);
    ev(&mut world, asterios, "30154-03.html"); // → Order of Asterios, memo 2, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // --- Elf leg: conjure and slay Actea and Luell ---
    world.force_roll(0); // roll(100)=0 < 33 → spawn Actea
    kill(&mut world, 20013); // Dryad
    assert_eq!(npcs_of(&mut world, 27121).len(), 1, "Actea conjured");
    kill(&mut world, 27121); // Actea → Seed of Verdure
    assert_eq!(item_count(&world, 3001, 2747), 1, "Seed of Verdure");
    world.force_roll(0);
    kill(&mut world, 20036); // Lirein → spawn Luell
    kill(&mut world, 27120); // Luell → Breath of Winds, memo 3, cond 3
    assert_eq!(item_count(&world, 3001, 2746), 1, "Breath of Winds");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, asterios); // → Scroll of Elf Trust, memo 4, cond 4
    assert_eq!(item_count(&world, 3001, 2741), 1, "Scroll of Elf Trust");
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // --- Dark Elf leg: Thifiell → Clayton → three reagents ---
    talk(&mut world, thifiell);
    ev(&mut world, thifiell, "30358-02.html"); // → Letter of Thifiell, memo 5, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, clayton); // → Order of Clayton, memo 6, cond 6
    assert_eq!(item_count(&world, 3001, 2755), 1, "Order of Clayton");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    inject(&mut world, 3001, 0x0217_0000, 2749, 4); // 4 basilisk blood
    kill(&mut world, 20550); // 5th → Basilisk Plasma
    inject(&mut world, 3001, 0x0217_0001, 2750, 4); // 4 giant aphid
    kill(&mut world, 20082); // 5th → Honey Dew
    inject(&mut world, 3001, 0x0217_0002, 2751, 4); // 4 stakato fluids
    kill(&mut world, 20157); // 5th → Stakato Ichor, cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, clayton); // cond 8
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, thifiell); // → Scroll of Dark Elf Trust, memo 7, cond 9
    assert_eq!(
        item_count(&world, 3001, 2740),
        1,
        "Scroll of Dark Elf Trust"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    talk(&mut world, hollint); // → Letter to Seresin, memo 8, cond 10
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, seresin);
    ev(&mut world, seresin, "30657-03.html"); // → Letters to Dwarf + Orc, memo 9, cond 12
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    // --- Orc leg: Kakai → Manakia → windsus parasites ---
    talk(&mut world, kakai);
    ev(&mut world, kakai, "30565-02.html"); // → Letter to Manakia, memo 10, cond 13
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    talk(&mut world, manakia);
    ev(&mut world, manakia, "30515-02.html"); // memo 11, cond 14
    assert_eq!(quest_cond(&world, 3001, q), Some(14));
    inject(&mut world, 3001, 0x0217_0003, 2756, 8); // 8 parasites
    kill(&mut world, 20553); // +2 → 10, memo 12, cond 15
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    talk(&mut world, manakia); // → Letter of Manakia, memo 13, cond 16
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    talk(&mut world, kakai); // → Scroll of Orc Trust, memo 14, cond 17
    assert_eq!(item_count(&world, 3001, 2743), 1, "Scroll of Orc Trust");
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    // --- Dwarf leg: Lockirin → Nikola → Porta heart ---
    talk(&mut world, lockirin);
    ev(&mut world, lockirin, "30531-02.html"); // → Letter to Nichola, memo 15, cond 18
    assert_eq!(quest_cond(&world, 3001, q), Some(18));
    talk(&mut world, nikola);
    ev(&mut world, nikola, "30621-02.html"); // → Order of Nichola, memo 16, cond 19
    assert_eq!(quest_cond(&world, 3001, q), Some(19));
    kill(&mut world, 20213); // Porta → Heart of Porta, cond 20
    assert_eq!(quest_cond(&world, 3001, q), Some(20));
    talk(&mut world, nikola); // heart → memo 17, cond 21
    assert_eq!(quest_cond(&world, 3001, q), Some(21));
    talk(&mut world, lockirin); // → Scroll of Dwarf Trust, memo 18, cond 22
    assert_eq!(item_count(&world, 3001, 2742), 1, "Scroll of Dwarf Trust");
    assert_eq!(quest_cond(&world, 3001, q), Some(22));
    talk(&mut world, hollint); // → Recommendation of Hollin, memo 19, cond 23
    assert_eq!(
        item_count(&world, 3001, 2744),
        1,
        "Recommendation of Hollin"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(23));
    // --- Completion at Biotin ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, biotin);
    assert_eq!(item_count(&world, 3001, 2734), 1, "Mark of Trust awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 252212,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(23),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00219_testimony_of_fate() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (3173..=3202).map(|id| (id, "Q219", true)).collect();
    items.push((3172, "Mark of Fate", false));
    add_quest_items(&mut world, &items);
    for id in [
        20144, 20158, 20233, 20202, 20192, 20157, 20270, 20554, 20582, 20600, 27079,
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let kaira = NPC_OID;
    let metheus = NPC_OID + 1;
    let ixia = NPC_OID + 2;
    let roa = NPC_OID + 3;
    let norman = NPC_OID + 4;
    let thifiell = NPC_OID + 5;
    let arkenia = NPC_OID + 6;
    let pixy = NPC_OID + 7;
    let treant = NPC_OID + 8;
    for (oid, npc) in [
        (kaira, 30476),
        (metheus, 30614),
        (ixia, 30463),
        (roa, 30114),
        (norman, 30210),
        (thifiell, 30358),
        (arkenia, 30419),
        (pixy, 31845),
        (treant, 31850),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 32; // Palus Knight (Dark Elf, DELF_2ND_GROUP)
        p.race = 2; // Dark Elf
    }
    world
        .data
        .categories
        .insert_for_test("DELF_2ND_GROUP", &[32]);
    let q = "Q00219_TestimonyOfFate";
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
    talk(&mut world, kaira);
    ev(&mut world, kaira, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    talk(&mut world, metheus); // → Metheus's Funeral Jar, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    kill(&mut world, 20144); // Hangman → Kasandra's Remains, cond 3
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, metheus); // → Herbalism Textbook, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    talk(&mut world, ixia); // textbook → Ixia's List, cond 6
    assert_eq!(item_count(&world, 3001, 3177), 1, "Ixia's List");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    // Five poison reagents, each to 10 → cond 7.
    inject(&mut world, 3001, 0x0219_0000, 3178, 9);
    kill(&mut world, 20158); // Medusa's Ichor → 10
    inject(&mut world, 3001, 0x0219_0001, 3179, 9);
    kill(&mut world, 20233); // Marsh Spider Fluids → 10
    inject(&mut world, 3001, 0x0219_0002, 3180, 9);
    kill(&mut world, 20202); // Dead Seeker Dung → 10
    inject(&mut world, 3001, 0x0219_0003, 3181, 9);
    kill(&mut world, 20192); // Tyrant's Blood → 10
    inject(&mut world, 3001, 0x0219_0004, 3182, 9);
    kill(&mut world, 20157); // Nightshade Root → 10, cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, ixia); // reagents → Belladonna, cond 8
    assert_eq!(item_count(&world, 3001, 3183), 1, "Belladonna");
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, metheus); // Belladonna → Alder's Skull 1, cond 9
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    talk(&mut world, kaira); // → Alder's Skull 2, cond 10
    assert_eq!(item_count(&world, 3001, 3185), 1, "Alder's Skull 2");
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, kaira); // cond 11
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    talk(&mut world, roa);
    ev(&mut world, roa, "30114-04.html"); // → Alder's Receipt, cond 12
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    talk(&mut world, norman); // receipt → Revelations Manuscript, cond 13
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    talk(&mut world, kaira);
    ev(&mut world, kaira, "30476-12.html"); // → Kaira's Recommendation, cond 15
    assert_eq!(item_count(&world, 3001, 3189), 1, "Kaira's Recommendation");
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    talk(&mut world, thifiell); // → Palus Charm + Thifiell's Letter, cond 16
    assert_eq!(item_count(&world, 3001, 3190), 1, "Palus Charm");
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    talk(&mut world, arkenia);
    ev(&mut world, arkenia, "30419-02.html"); // → Arkenia's Note, cond 17
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    // --- Alchemy: Red Fairy Dust from four overlord skulls ---
    ev(&mut world, pixy, "31845-02.html"); // Pixy Garnet
    kill(&mut world, 20554); // Grandis's Skull
    kill(&mut world, 20600); // Karul Bugbear Skull
    kill(&mut world, 20270); // Breka Overlord Skull
    kill(&mut world, 20582); // Leto Overlord Skull
    talk(&mut world, pixy); // skulls → Red Fairy Dust
    assert_eq!(item_count(&world, 3001, 3198), 1, "Red Fairy Dust");
    // --- Alchemy: Blight Treant Sap ---
    ev(&mut world, treant, "31850-02.html"); // Timiriran Seed
    kill(&mut world, 27079); // Black Willow Lurker → Black Willow Leaf
    talk(&mut world, treant); // → Blight Treant Sap
    assert_eq!(item_count(&world, 3001, 3201), 1, "Blight Treant Sap");
    // --- Arkenia's Letter, cond 18 ---
    talk(&mut world, arkenia);
    ev(&mut world, arkenia, "30419-05.html"); // dust + sap → Arkenia's Letter, cond 18
    assert_eq!(item_count(&world, 3001, 3202), 1, "Arkenia's Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(18));
    // --- Completion at Thifiell ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, thifiell);
    assert_eq!(item_count(&world, 3001, 3172), 1, "Mark of Fate awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 247708,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(18),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00218_testimony_of_life() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (3141..=3171).map(|id| (id, "Q218", true)).collect();
    items.push((3026, "Talin's Spear", true));
    items.push((3140, "Mark of Life", false));
    add_quest_items(&mut world, &items);
    for id in [20550, 20082, 20176, 20145, 20233, 20581, 27077] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let cardien = NPC_OID;
    let asterios = NPC_OID + 1;
    let pushkin = NPC_OID + 2;
    let thalia = NPC_OID + 3;
    let arkenia = NPC_OID + 4;
    let adonius = NPC_OID + 5;
    let isael = NPC_OID + 6;
    for (oid, npc) in [
        (cardien, 30460),
        (asterios, 30154),
        (pushkin, 30300),
        (thalia, 30371),
        (arkenia, 30419),
        (adonius, 30375),
        (isael, 30655),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 19; // Elven Knight (ELF_2ND_GROUP)
        p.race = 1; // Elf
    }
    world
        .data
        .categories
        .insert_for_test("ELF_2ND_GROUP", &[19]);
    let q = "Q00218_TestimonyOfLife";
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
    talk(&mut world, cardien);
    ev(&mut world, cardien, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    talk(&mut world, asterios);
    ev(&mut world, asterios, "30154-07.html"); // → Hierarch's Letter + Moonflower Charm, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    talk(&mut world, thalia);
    ev(&mut world, thalia, "30371-03.html"); // → Grail Diagram, cond 3
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, pushkin);
    ev(&mut world, pushkin, "30300-06.html"); // → Pushkin's List, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    inject(&mut world, 3001, 0x0218_0000, 3161, 8); // mithril ore
    kill(&mut world, 20550); // Basilisk +2 → 10
    inject(&mut world, 3001, 0x0218_0001, 3162, 18); // ant acid
    kill(&mut world, 20082); // Ant +2 → 20
    inject(&mut world, 3001, 0x0218_0002, 3163, 16); // wyrm talon
    kill(&mut world, 20176); // Wyrm +4 → 20, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, pushkin);
    ev(&mut world, pushkin, "30300-10.html"); // → Pure Mithril Cup, cond 6
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    talk(&mut world, thalia); // cup → Thalia's 1st Letter, cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, arkenia);
    ev(&mut world, arkenia, "30419-04.html"); // → Arkenia's Contract + Instructions, cond 8
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, adonius);
    ev(&mut world, adonius, "30375-02.html"); // → Adonius's List, cond 9
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    inject(&mut world, 3001, 0x0218_0003, 3165, 16); // harpy down
    kill(&mut world, 20145); // Harpy +4 → 20
    inject(&mut world, 3001, 0x0218_0004, 3164, 16); // spider ichor
    kill(&mut world, 20233); // Spider +4 → 20, cond 10
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, adonius); // → Andariel Scripture Copy, cond 11
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    talk(&mut world, arkenia); // → Stardust, cond 12
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    talk(&mut world, thalia);
    ev(&mut world, thalia, "30371-11.html"); // → Thalia's 2nd Letter, cond 14
    assert_eq!(quest_cond(&world, 3001, q), Some(14));
    talk(&mut world, isael);
    ev(&mut world, isael, "30655-02.html"); // → Isael's Instructions, cond 15
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    for _ in 0..6 {
        kill(&mut world, 20581); // Leto → six spear parts
    }
    talk(&mut world, isael); // parts → Talin's Spear + Isael's Letter, cond 17
    assert_eq!(item_count(&world, 3001, 3026), 1, "Talin's Spear assembled");
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    talk(&mut world, thalia); // spear + letter → Grail of Purity, cond 18
    assert_eq!(item_count(&world, 3001, 3158), 1, "Grail of Purity");
    assert_eq!(quest_cond(&world, 3001, q), Some(18));
    // Slay the Unicorn with Talin's Spear equipped.
    equip_weapon_row(&mut world, 3001, 3026);
    inject(&mut world, 3001, 0x0218_0005, 3144, 1); // re-supply moonflower/spear/grail
    inject(&mut world, 3001, 0x0218_0006, 3026, 1);
    inject(&mut world, 3001, 0x0218_0007, 3158, 1);
    kill(&mut world, 27077); // Unicorn (spear-struck) → Tears of Unicorn, cond 19
    assert_eq!(item_count(&world, 3001, 3159), 1, "Tears of Unicorn");
    assert_eq!(quest_cond(&world, 3001, q), Some(19));
    talk(&mut world, thalia); // tears → Water of Life, cond 20
    assert_eq!(item_count(&world, 3001, 3160), 1, "Water of Life");
    assert_eq!(quest_cond(&world, 3001, q), Some(20));
    talk(&mut world, asterios); // moonflower + water → Camomile Charm, cond 21
    assert_eq!(item_count(&world, 3001, 3142), 1, "Camomile Charm");
    assert_eq!(quest_cond(&world, 3001, q), Some(21));
    // --- Completion at Cardien ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, cardien);
    assert_eq!(item_count(&world, 3001, 3140), 1, "Mark of Life awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 342288,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(21),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00220_testimony_of_glory() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (3204..=3237).map(|id| (id, "Q220", true)).collect();
    items.push((3203, "Mark of Glory", false));
    add_quest_items(&mut world, &items);
    for id in [
        20563, 20192, 20550, 20583, 20601, 20778, 27080, 27081, 27082, 27083, 27086,
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let vokian = NPC_OID;
    let chianta = NPC_OID + 1;
    let kasman = NPC_OID + 2;
    let manakia = NPC_OID + 3;
    let driko = NPC_OID + 4;
    let burai = NPC_OID + 5;
    let harak = NPC_OID + 6;
    let voltar = NPC_OID + 7;
    let kepra = NPC_OID + 8;
    let tanapi = NPC_OID + 9;
    let kakai = NPC_OID + 10;
    for (oid, npc) in [
        (vokian, 30514),
        (chianta, 30642),
        (kasman, 30501),
        (manakia, 30515),
        (driko, 30619),
        (burai, 30617),
        (harak, 30618),
        (voltar, 30615),
        (kepra, 30616),
        (tanapi, 30571),
        (kakai, 30565),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 45; // Orc Raider
        p.race = 3; // Orc
    }
    world
        .data
        .categories
        .insert_for_test("ORC_2ND_GROUP", &[45]);
    let q = "Q00220_TestimonyOfGlory";
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
    talk(&mut world, vokian);
    ev(&mut world, vokian, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Vokian's three subjugation reagents → cond 2.
    inject(&mut world, 3001, 0x0220_0000, 3205, 9);
    kill(&mut world, 20563); // Manashen shard → 10
    inject(&mut world, 3001, 0x0220_0001, 3206, 9);
    kill(&mut world, 20192); // Tyrant talon → 10
    inject(&mut world, 3001, 0x0220_0002, 3207, 9);
    kill(&mut world, 20550); // Basilisk fang → 10, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    talk(&mut world, vokian); // → Order2 + Necklace of Authority, cond 3
    assert_eq!(item_count(&world, 3001, 3209), 1, "Necklace of Authority");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, chianta);
    ev(&mut world, chianta, "30642-03.html"); // → Chianta's 1st Order, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // --- Five scepter legs ---
    // Vuku (Driko): letter → contract → 30 husks
    ev(&mut world, kasman, "30501-02.html"); // Kasman's 1st Letter
    ev(&mut world, driko, "30619-03.html"); // → Driko's Contract
    inject(&mut world, 3001, 0x0220_0003, 3234, 30); // 30 stakato drone husks
    talk(&mut world, driko); // → Scepter of Vuku
    assert_eq!(item_count(&world, 3001, 3213), 1, "Scepter of Vuku");
    // Turek (Burai): letter → glove → 2 makum heads
    ev(&mut world, kasman, "30501-05.html"); // Kasman's 2nd Letter
    ev(&mut world, burai, "30617-03.html"); // → Glove of Burai
    kill(&mut world, 27083); // Makum head 1
    kill(&mut world, 27083); // Makum head 2
    talk(&mut world, burai); // → Scepter of Turek
    assert_eq!(item_count(&world, 3001, 3214), 1, "Scepter of Turek");
    // Tunath (Harak): letter → scepter directly
    ev(&mut world, kasman, "30501-08.html"); // Kasman's 3rd Letter
    ev(&mut world, harak, "30618-03.html"); // → Scepter of Tunath
    assert_eq!(item_count(&world, 3001, 3215), 1, "Scepter of Tunath");
    // Breka (Voltar): letter → glove → Pashika + Vultus heads
    ev(&mut world, manakia, "30515-04.html"); // Manakia's 1st Letter
    ev(&mut world, voltar, "30615-04.html"); // → Glove of Voltar
    kill(&mut world, 27080); // Pashika's Head
    kill(&mut world, 27081); // Vultus's Head
    talk(&mut world, voltar); // → Scepter of Breka
    assert_eq!(item_count(&world, 3001, 3211), 1, "Scepter of Breka");
    // Enku (Kepra): letter → glove → 4 Enku heads (fifth scepter → cond 5)
    ev(&mut world, manakia, "30515-05.html"); // Manakia's 2nd Letter
    ev(&mut world, kepra, "30616-04.html"); // → Glove of Kepra
    for _ in 0..4 {
        kill(&mut world, 27082); // Enku overlord heads
    }
    talk(&mut world, kepra); // → Scepter of Enku, cond 5
    assert_eq!(item_count(&world, 3001, 3212), 1, "Scepter of Enku");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    // Chianta assembles the scepters → 3rd Order.
    talk(&mut world, chianta);
    ev(&mut world, chianta, "30642-07.html"); // scepters → Chianta's 3rd Order, cond 6
    assert_eq!(item_count(&world, 3001, 3217), 1, "Chianta's 3rd Order");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    // Timak heads + Tamlin skulls → cond 7 (via Tamlin, per the faithful bug).
    inject(&mut world, 3001, 0x0220_0004, 3219, 19); // timak heads
    kill(&mut world, 20583); // → 20
    inject(&mut world, 3001, 0x0220_0005, 3218, 19); // tamlin skulls
    kill(&mut world, 20601); // → 20, cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, chianta); // → Scepter Box, cond 8
    assert_eq!(item_count(&world, 3001, 3220), 1, "Scepter Box");
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, tanapi);
    ev(&mut world, tanapi, "30571-03.html"); // → Tanapi's Order, cond 9
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    // Ragna summons the Revenant; slay it for the Scepter of Tantos.
    kill(&mut world, 20778); // Ragna → spawns Revenant
    assert!(
        !npcs_of(&mut world, 27086).is_empty(),
        "Revenant of Tantos conjured"
    );
    kill(&mut world, 27086); // Revenant → Scepter of Tantos, cond 10
    assert_eq!(item_count(&world, 3001, 3236), 1, "Scepter of Tantos");
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, tanapi); // → Ritual Box, cond 11
    assert_eq!(item_count(&world, 3001, 3237), 1, "Ritual Box");
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    // --- Completion at Kakai ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, kakai);
    assert_eq!(item_count(&world, 3001, 3203), 1, "Mark of Glory awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 262720,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(11),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00221_testimony_of_prosperity() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (3239..=3275).map(|id| (id, "Q221", true)).collect();
    items.push((3023, "Recipe Titan Key", true));
    items.push((3030, "Key of Titan", true));
    items.push((3428, "Crystal Brooch", true));
    items.push((3238, "Mark of Prosperity", false));
    items.push((1867, "Animal Skin", false));
    add_quest_items(&mut world, &items);
    for id in [20154, 20228, 20157, 20231, 20233] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let n = |i| NPC_OID + i;
    let (parman, wilford, lilith, bright, lockirin, shari, mion, toma) =
        (n(0), n(1), n(2), n(3), n(4), n(5), n(6), n(7));
    let (spiron, balanki, keef, filaur, arin, maryse, bolter, torocco) =
        (n(8), n(9), n(10), n(11), n(12), n(13), n(14), n(15));
    let (piotur, emily, nikola, boxt) = (n(16), n(17), n(18), n(19));
    for (oid, npc) in [
        (parman, 30104),
        (wilford, 30005),
        (lilith, 30368),
        (bright, 30466),
        (lockirin, 30531),
        (shari, 30517),
        (mion, 30519),
        (toma, 30556),
        (spiron, 30532),
        (balanki, 30533),
        (keef, 30534),
        (filaur, 30535),
        (arin, 30536),
        (maryse, 30553),
        (bolter, 30554),
        (torocco, 30555),
        (piotur, 30597),
        (emily, 30620),
        (nikola, 30621),
        (boxt, 30622),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 54; // Scavenger
        p.race = 4; // Dwarf
    }
    world
        .data
        .categories
        .insert_for_test("DWARF_2ND_GROUP", &[54]);
    let q = "Q00221_TestimonyOfProsperity";
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
    talk(&mut world, parman);
    ev(&mut world, parman, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // --- Proof 1: Blessed Seed ---
    ev(&mut world, piotur, "30597-02.html");
    assert_eq!(item_count(&world, 3001, 3242), 1, "Blessed Seed");
    // --- Proof 2: Lilith's Elven Wafer ---
    ev(&mut world, wilford, "30005-04.html"); // Crystal Brooch
    ev(&mut world, lilith, "30368-03.html"); // → Lilith's Elven Wafer
    assert_eq!(item_count(&world, 3001, 3244), 1, "Lilith's Elven Wafer");
    // --- Proof 3: Emily's Recipe ---
    ev(&mut world, bright, "30466-03.html"); // Bright's List
    inject(&mut world, 3001, 0x0221_0000, 3265, 19);
    kill(&mut world, 20154); // Mandragora petal → 20
    inject(&mut world, 3001, 0x0221_0001, 3266, 9);
    kill(&mut world, 20228); // Crimson moss → 10
    talk(&mut world, bright); // → Mandragora Bouquet
    assert_eq!(item_count(&world, 3001, 3267), 1, "Mandragora Bouquet");
    ev(&mut world, emily, "30620-03.html"); // → Emily's Recipe
    assert_eq!(item_count(&world, 3001, 3243), 1, "Emily's Recipe");
    // --- Proof 4: Old Account Book (five guild contributions) ---
    ev(&mut world, lockirin, "30531-03.html"); // license + 5 notices
    // Receipt 1 (Spiron/Shari)
    talk(&mut world, spiron); // takes 1st notice
    talk(&mut world, shari); // Contribution of Shari
    talk(&mut world, spiron); // → Receipt 1st
    assert_eq!(item_count(&world, 3001, 3258), 1, "Receipt 1st");
    // Receipt 2 (Balanki/Mion+Maryse)
    talk(&mut world, balanki); // takes 2nd notice
    talk(&mut world, mion); // Contribution of Mion
    talk(&mut world, maryse); // Maryse's Request
    inject(&mut world, 3001, 0x0221_0002, 1867, 10); // 10 animal skin
    talk(&mut world, maryse); // → Contribution of Maryse
    talk(&mut world, balanki); // → Receipt 2nd
    assert_eq!(item_count(&world, 3001, 3259), 1, "Receipt 2nd");
    // Receipt 3 (Keef/Torocco, 5000 adena)
    talk(&mut world, keef); // takes 3rd notice
    ev(&mut world, torocco, "30555-02.html"); // Procuration of Torocco
    inject(&mut world, 3001, 0x0221_0003, 57, 5000);
    ev(&mut world, keef, "30534-03a.html"); // 5000 adena → Receipt 3rd
    assert_eq!(item_count(&world, 3001, 3260), 1, "Receipt 3rd");
    // Receipt 4 (Filaur/Bolter)
    talk(&mut world, filaur); // takes 4th notice
    talk(&mut world, bolter); // Receipt of Bolter
    talk(&mut world, filaur); // → Receipt 4th
    assert_eq!(item_count(&world, 3001, 3261), 1, "Receipt 4th");
    // Receipt 5 (Arin/Toma)
    talk(&mut world, arin); // takes 5th notice
    talk(&mut world, toma); // Contribution of Toma
    talk(&mut world, arin); // → Receipt 5th
    assert_eq!(item_count(&world, 3001, 3262), 1, "Receipt 5th");
    talk(&mut world, lockirin); // 5 receipts → Old Account Book, cond 2
    assert_eq!(item_count(&world, 3001, 3241), 1, "Old Account Book");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // --- Parman: First Ring → Second Ring ---
    talk(&mut world, parman);
    ev(&mut world, parman, "30104-08.html"); // → Ring 2nd + Parman's Letter, cond 4
    assert_eq!(
        item_count(&world, 3001, 3240),
        1,
        "Second Ring of Testimony"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // --- Phase 2: Key of Titan ---
    talk(&mut world, nikola);
    ev(&mut world, nikola, "30621-04.html"); // → Clay Dough, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, boxt);
    ev(&mut world, boxt, "30622-02.html"); // → Pattern of Keyhole, cond 6
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    talk(&mut world, nikola); // → Recipe + Nikola's List, cond 7
    assert_eq!(item_count(&world, 3001, 3272), 1, "Nikola's List");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    inject(&mut world, 3001, 0x0221_0004, 3273, 19);
    kill(&mut world, 20157); // Stakato shell → 20
    inject(&mut world, 3001, 0x0221_0005, 3274, 9);
    kill(&mut world, 20231); // Toad lord sac → 10
    inject(&mut world, 3001, 0x0221_0006, 3275, 9);
    kill(&mut world, 20233); // Marsh spider thorn → 10, cond 8
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    inject(&mut world, 3001, 0x0221_0007, 3030, 1); // crafted Key of Titan
    talk(&mut world, boxt);
    ev(&mut world, boxt, "30622-04.html"); // key → Maphr's Tablet Fragment, cond 9
    assert_eq!(item_count(&world, 3001, 3245), 1, "Maphr's Tablet Fragment");
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    // --- Completion at Parman ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, parman);
    assert_eq!(
        item_count(&world, 3001, 3238),
        1,
        "Mark of Prosperity awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 217682,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(9),
        "one-time quest finished"
    );
}
