//! Q00226-Q00229 — the third class tests for the mystic classes: Healer,
//! Reformer, Magus and Witchcraft.

use super::super::*;

#[test]
fn quest_q00226_test_of_the_healer() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (2810..=2819).map(|id| (id, "Q226", true)).collect();
    items.push((2820, "Mark of Healer", false));
    add_quest_items(&mut world, &items);
    for id in [27122, 27123, 27124, 27125, 27126, 27127, 27134] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let bandellos = NPC_OID;
    let perrin = NPC_OID + 1;
    let allana = NPC_OID + 2;
    let gupu = NPC_OID + 3;
    let windy = NPC_OID + 4;
    let sorius = NPC_OID + 5;
    let daurin = NPC_OID + 6;
    let mde = NPC_OID + 7;
    let piper = NPC_OID + 8;
    let kristina = NPC_OID + 9;
    for (oid, npc) in [
        (bandellos, 30473),
        (perrin, 30428),
        (allana, 30424),
        (gupu, 30658),
        (windy, 30660),
        (sorius, 30327),
        (daurin, 30674),
        (mde, 30661),
        (piper, 30662),
        (kristina, 30665),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 15; // Cleric (WHITE_MAGIC_GROUP)
    }
    world
        .data
        .categories
        .insert_for_test("WHITE_MAGIC_GROUP", &[15]);
    let q = "Q00226_TestOfTheHealer";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, bandellos);
    ev(&mut world, bandellos, "ACCEPT");
    assert_eq!(quest_memo(&world, 3001, q), 1);
    talk(&mut world, perrin);
    ev(&mut world, perrin, "30428-02.html"); // cond 2 + Tatoma ambush
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert!(
        !npcs_of(&mut world, 27134).is_empty(),
        "Tatoma ambush spawned"
    );
    kill(&mut world, 27134); // Tatoma → memo 2, cond 3
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, perrin); // memo 3, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    talk(&mut world, allana); // memo 4, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, gupu); // cond 6
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    inject(&mut world, 3001, 0x0226_0000, 57, 1000);
    ev(&mut world, gupu, "30658-02.html"); // 1000 adena → Picture of Windy, cond 7
    assert_eq!(item_count(&world, 3001, 2812), 1, "Picture of Windy");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, windy);
    ev(&mut world, windy, "30660-03.html"); // → Windy's Pebbles, cond 8
    assert_eq!(item_count(&world, 3001, 2814), 1, "Windy's Pebbles");
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, gupu); // pebbles → Golden Statue, memo 5
    assert_eq!(item_count(&world, 3001, 2813), 1, "Golden Statue");
    talk(&mut world, gupu); // memo 5 → cond 9
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    talk(&mut world, sorius); // Order of Sorius, memo 6, cond 10
    assert_eq!(item_count(&world, 3001, 2815), 1, "Order of Sorius");
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, daurin);
    ev(&mut world, daurin, "30674-02.html"); // cond 11, spawn Leros
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    kill(&mut world, 27123); // Lero Leader → Secret Letter 1, cond 12
    assert_eq!(item_count(&world, 3001, 2816), 1, "Secret Letter 1");
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    talk(&mut world, daurin); // memo 8, cond 13
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    // Four secret letters: the Mysterious Dark Elf deletes itself each step.
    let re_add_mde = |w: &mut World| add_test_npc(w, mde, 30661, "Folk", 40, 100, 0, 0);
    re_add_mde(&mut world);
    talk(&mut world, mde); // spawn assassins, cond 14
    assert_eq!(quest_cond(&world, 3001, q), Some(14));
    kill(&mut world, 27124); // Assassin → Secret Letter 2, cond 15
    assert_eq!(item_count(&world, 3001, 2817), 1, "Secret Letter 2");
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    re_add_mde(&mut world);
    talk(&mut world, mde); // spawn snipers, cond 16
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    kill(&mut world, 27125); // Sniper → Secret Letter 3, cond 17
    assert_eq!(item_count(&world, 3001, 2818), 1, "Secret Letter 3");
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    re_add_mde(&mut world);
    talk(&mut world, mde); // spawn wizards + lord, cond 18
    assert_eq!(quest_cond(&world, 3001, q), Some(18));
    kill(&mut world, 27127); // Lord → Secret Letter 4, cond 19
    assert_eq!(item_count(&world, 3001, 2819), 1, "Secret Letter 4");
    assert_eq!(quest_cond(&world, 3001, q), Some(19));
    re_add_mde(&mut world);
    talk(&mut world, mde); // all four → cond 20
    assert_eq!(quest_cond(&world, 3001, q), Some(20));
    talk(&mut world, piper); // directions → cond 21
    assert_eq!(quest_cond(&world, 3001, q), Some(21));
    talk(&mut world, kristina);
    ev(&mut world, kristina, "30665-02.html"); // 4 letters → Cristina's Letter, memo 9, cond 22
    assert_eq!(item_count(&world, 3001, 2811), 1, "Cristina's Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(22));
    talk(&mut world, sorius); // Cristina's Letter → memo 10, cond 23
    assert_eq!(quest_cond(&world, 3001, q), Some(23));
    // Completion at Bandellos (healer kept the Golden Statue → lesser reward path).
    talk(&mut world, bandellos); // memo 10 + statue → 30473-07
    ev(&mut world, bandellos, "30473-09.html");
    assert_eq!(
        item_count(&world, 3001, 2820),
        1,
        "Mark of the Healer awarded"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(23),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00228_test_of_magus() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (2841..=2863).map(|id| (id, "Q228", true)).collect();
    items.push((2840, "Mark of Magus", false));
    add_quest_items(&mut world, &items);
    for id in [
        27095, 27096, 27097, 20564, 20565, 20566, 27098, 20145, 20176, 20553, 20157,
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let rukal = NPC_OID;
    let parina = NPC_OID + 1;
    let casian = NPC_OID + 2;
    let earth = NPC_OID + 3;
    let flame = NPC_OID + 4;
    let sylph = NPC_OID + 5;
    let undine = NPC_OID + 6;
    for (oid, npc) in [
        (rukal, 30629),
        (parina, 30391),
        (casian, 30612),
        (earth, 30409),
        (flame, 30411),
        (sylph, 30412),
        (undine, 30413),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 11; // Wizard
    }
    let q = "Q00228_TestOfMagus";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, rukal);
    ev(&mut world, rukal, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    talk(&mut world, parina);
    ev(&mut world, parina, "30391-02.html"); // → Parina's Letter, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    talk(&mut world, casian);
    ev(&mut world, casian, "30612-02.html"); // → Lilac Charm, cond 3
    assert_eq!(item_count(&world, 3001, 2843), 1, "Lilac Charm");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    kill(&mut world, 27095); // Phantasm → Golden Seed 1
    kill(&mut world, 27096); // Nightmare → Golden Seed 2
    kill(&mut world, 27097); // Darkling → Golden Seed 3, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    talk(&mut world, rukal);
    ev(&mut world, rukal, "30629-10.html"); // seeds → Score of Elements, cond 5
    assert_eq!(item_count(&world, 3001, 2847), 1, "Score of Elements");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    // --- Earth (Serpent) ---
    talk(&mut world, earth);
    ev(&mut world, earth, "30409-03.html"); // Serpent Charm
    inject(&mut world, 3001, 0x0228_0000, 2853, 9);
    kill(&mut world, 20564); // Monstereye → 10 shells
    inject(&mut world, 3001, 0x0228_0001, 2854, 9);
    kill(&mut world, 20565); // Stolen Golem → 10 powder
    inject(&mut world, 3001, 0x0228_0002, 2855, 9);
    kill(&mut world, 20566); // Iron Golem → 10 scrap
    talk(&mut world, earth); // → Tone of Earth
    assert_eq!(item_count(&world, 3001, 2859), 1, "Tone of Earth");
    // --- Fire (Salamander) ---
    talk(&mut world, flame); // gives Salamander Charm
    inject(&mut world, 3001, 0x0228_0003, 2849, 4);
    world.force_roll(0);
    kill(&mut world, 27098); // Ghost Fire → 5 crystals
    talk(&mut world, flame); // → Tone of Fire
    assert_eq!(item_count(&world, 3001, 2857), 1, "Tone of Fire");
    // --- Wind (Sylph) ---
    talk(&mut world, sylph);
    ev(&mut world, sylph, "30412-02.html"); // Sylph Charm
    inject(&mut world, 3001, 0x0228_0004, 2850, 19);
    kill(&mut world, 20145); // Harpy → 20 feathers
    inject(&mut world, 3001, 0x0228_0005, 2851, 9);
    world.force_roll(0);
    kill(&mut world, 20176); // Wyrm → 10 wingbone
    inject(&mut world, 3001, 0x0228_0006, 2852, 9);
    world.force_roll(0);
    kill(&mut world, 20553); // Windsus → 10 mane
    talk(&mut world, sylph); // → Tone of Wind
    assert_eq!(item_count(&world, 3001, 2858), 1, "Tone of Wind");
    // --- Water (Undine): the fourth tone → cond 6 ---
    talk(&mut world, undine); // gives Undine Charm
    inject(&mut world, 3001, 0x0228_0007, 2848, 19);
    kill(&mut world, 20157); // Marsh Stakato → 20 drops
    talk(&mut world, undine); // → Tone of Water, cond 6
    assert_eq!(item_count(&world, 3001, 2856), 1, "Tone of Water");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    // --- Completion ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, rukal);
    assert_eq!(item_count(&world, 3001, 2840), 1, "Mark of Magus awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 372154,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(6),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00229_test_of_witchcraft() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (3308..=3335).map(|id| (id, "Q229", true)).collect();
    items.push((3029, "Sword of Binding", true));
    items.push((3307, "Mark of Witchcraft", false));
    add_quest_items(&mut world, &items);
    for id in [20557, 20565, 20577, 20601, 27099, 27100, 27101] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let orim = NPC_OID;
    let alexandria = NPC_OID + 1;
    let iker = NPC_OID + 2;
    let kaira = NPC_OID + 3;
    let lara = NPC_OID + 4;
    let nestle = NPC_OID + 5;
    let leopold = NPC_OID + 6;
    let vasper = NPC_OID + 7;
    let vadin = NPC_OID + 8;
    let evert = NPC_OID + 9;
    for (oid, npc) in [
        (orim, 30630),
        (alexandria, 30098),
        (iker, 30110),
        (kaira, 30476),
        (lara, 30063),
        (nestle, 30314),
        (leopold, 30435),
        (vasper, 30417),
        (vadin, 30188),
        (evert, 30633),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 11; // Wizard
    }
    let q = "Q00229_TestOfWitchcraft";
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
    talk(&mut world, orim);
    ev(&mut world, orim, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    talk(&mut world, alexandria);
    ev(&mut world, alexandria, "30098-03.htm"); // Diagram → Alexandria's Book, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // Gem 1 (Iker): 20 each of three reagents.
    ev(&mut world, iker, "30110-03.htm"); // Iker's List
    inject(&mut world, 3001, 0x0229_0000, 3311, 19); // dire wyrm fang
    kill(&mut world, 20557); // +1 → 20
    inject(&mut world, 3001, 0x0229_0001, 3313, 20); // golem heartstone
    inject(&mut world, 3001, 0x0229_0002, 3312, 20); // leto charm
    talk(&mut world, iker); // → Aklantoth 1st Gem
    assert_eq!(item_count(&world, 3001, 3317), 1, "Aklantoth 1st Gem");
    // Gem 2 (Kaira)
    ev(&mut world, kaira, "30476-02.htm");
    assert_eq!(item_count(&world, 3001, 3318), 1, "Aklantoth 2nd Gem");
    // Gem 3 (Lara → Nameless Revenant)
    ev(&mut world, lara, "30063-02.htm"); // Lara's Memo
    kill(&mut world, 27099); // Nameless Revenant → 3rd Gem
    assert_eq!(item_count(&world, 3001, 3319), 1, "Aklantoth 3rd Gem");
    // Gems 4-6 (Nestle/Leopold → Skeletal Mercenary)
    ev(&mut world, nestle, "30314-02.htm"); // Nestle's Memo
    ev(&mut world, leopold, "30435-02.htm"); // Leopold's Journal
    kill(&mut world, 27100); // 4th Gem
    kill(&mut world, 27100); // 5th Gem
    kill(&mut world, 27100); // 6th Gem → cond 3
    assert_eq!(item_count(&world, 3001, 3322), 1, "Aklantoth 6th Gem");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    // Orim forges the First Brimstone and summons Zeruel.
    talk(&mut world, orim);
    ev(&mut world, orim, "30630-14.htm"); // gems → First Brimstone, cond 4
    assert_eq!(item_count(&world, 3001, 3323), 1, "First Brimstone");
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // Driving Zeruel off (attack while holding the brimstone) → cond 5.
    add_test_npc(&mut world, NPC_OID + 20, 27101, "Monster", 40, 40, 0, 0);
    combat::npc_receive_damage(&mut world, NPC_OID + 20, 3001, 10.0, false);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(5),
        "Zeruel driven off → cond 5"
    );
    talk(&mut world, orim);
    ev(&mut world, orim, "30630-16.htm"); // → Orim's Instructions + two letters, cond 6
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    // Sword of Binding path.
    ev(&mut world, vasper, "30417-03.htm"); // 1st Letter → Sir Vasper's Letter
    talk(&mut world, vadin); // Vasper's Letter → Vadin's Crucifix
    assert_eq!(item_count(&world, 3001, 3328), 1, "Vadin's Crucifix");
    inject(&mut world, 3001, 0x0229_0003, 3329, 19); // 19 Tamlin amulets
    world.force_roll(0); // roll(100)=0 < 50 → drop
    kill(&mut world, 20601); // +1 → 20
    talk(&mut world, vadin); // amulets → Vadin's Sanctions
    assert_eq!(item_count(&world, 3001, 3330), 1, "Vadin's Sanctions");
    talk(&mut world, vasper); // Sanctions → Sword of Binding
    assert_eq!(item_count(&world, 3001, 3029), 1, "Sword of Binding");
    // Soultrap Crystal path → cond 7.
    ev(&mut world, iker, "30110-08.htm"); // 2nd Letter → Soultrap Crystal, cond 7
    assert_eq!(item_count(&world, 3001, 3332), 1, "Soultrap Crystal");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, orim); // cond 8
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    // Evert summons Zeruel for the binding.
    ev(&mut world, evert, "30633-02.htm"); // → Second Brimstone, cond 9
    assert_eq!(item_count(&world, 3001, 3335), 1, "Second Brimstone");
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    // Bind Zeruel with a killing blow from the Sword of Binding.
    equip_weapon_row(&mut world, 3001, 3029);
    for (obj, item) in [
        (0x0229_0004, 3324),
        (0x0229_0005, 3335),
        (0x0229_0006, 3029),
        (0x0229_0007, 3332),
    ] {
        inject(&mut world, 3001, obj, item, 1); // instructions, brimstone2, sword, soultrap
    }
    kill(&mut world, 27101); // Zeruel (sword-struck) → Zeruel Bind Crystal, cond 10
    assert_eq!(item_count(&world, 3001, 3334), 1, "Zeruel Bind Crystal");
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    // --- Completion at Orim ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, orim);
    ev(&mut world, orim, "30630-22.htm");
    assert_eq!(
        item_count(&world, 3001, 3307),
        1,
        "Mark of Witchcraft awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 372154,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(10),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00227_test_of_the_reformer() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = [
        2822, 2823, 2824, 2825, 2826, 2827, 2828, 2829, 2830, 2831, 2832, 2833, 2834, 2835, 2836,
        2837, 2838, 3037, 5567, 5568,
    ]
    .iter()
    .map(|&id| (id, "Q227", true))
    .collect();
    items.push((2821, "Mark of Reformer", false));
    add_quest_items(&mut world, &items);
    for id in [27099, 27128, 27129, 27130, 27131, 27132, 20022, 20100] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let pupina = NPC_OID;
    let sla = NPC_OID + 1;
    let katari = NPC_OID + 2;
    let pilgrim = NPC_OID + 3;
    let kakan = NPC_OID + 4;
    let nyakuri = NPC_OID + 5;
    let ramus = NPC_OID + 6;
    for (oid, npc) in [
        (pupina, 30118),
        (sla, 30666),
        (katari, 30668),
        (pilgrim, 30732),
        (kakan, 30669),
        (nyakuri, 30670),
        (ramus, 30667),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 15; // Cleric
    }
    let q = "Q00227_TestOfTheReformer";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    // A skill hit: stash the skill id the way the damage path does, then strike.
    let skill_hit = |w: &mut World, npc_oid: i32, skill_id: i32| {
        w.quest_attack_skill = Some(skill_id);
        combat::npc_receive_damage(w, npc_oid, 3001, 10.0, false);
        w.quest_attack_skill = None;
    };
    talk(&mut world, pupina);
    ev(&mut world, pupina, "ACCEPT");
    assert_eq!(quest_memo(&world, 3001, q), 1);
    // --- Nameless Revenant: only Disrupt Undead (1031) marks it (scriptValue 1). ---
    // Negative first: a plain melee kill drops nothing.
    inject(&mut world, 3001, 0x0227_0000, 2831, 6); // 6 ripped diaries
    kill(&mut world, 27099); // melee (no skill) → scriptValue stays 0 → no drop
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(2),
        "melee kill does not reform the revenant"
    );
    // Now with Disrupt Undead:
    add_test_npc(&mut world, NPC_OID + 20, 27099, "Monster", 40, 40, 0, 0);
    skill_hit(&mut world, NPC_OID + 20, 1031); // Disrupt Undead → scriptValue 1
    npc::npc_do_die(&mut world, NPC_OID + 20, 3001); // 7th diary → spawn Araurune, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert!(!npcs_of(&mut world, 27128).is_empty(), "Araurune conjured");
    kill(&mut world, 27128); // Araurune → Huge Nail, memo 3, cond 3
    assert_eq!(item_count(&world, 3001, 2832), 1, "Huge Nail");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    ev(&mut world, pupina, "30118-06.html"); // → Letter of Introduction, memo 4, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    ev(&mut world, sla, "30666-04.html"); // → Sla's Letter, memo 5, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, katari); // memo 6, cond 6, spawn inspector
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    kill(&mut world, 27129); // Ol Mahum Inspector → memo 7, cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, pilgrim); // Ol Mahum Money, memo 8
    assert_eq!(item_count(&world, 3001, 2826), 1, "Ol Mahum Money");
    talk(&mut world, katari); // memo 8, cond 8, spawn betrayer
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    kill(&mut world, 27130); // Ol Mahum Betrayer → memo 9, cond 9, Letter of Betrayer
    assert_eq!(item_count(&world, 3001, 2833), 1, "Letter of Betrayer");
    talk(&mut world, katari); // → Katari's Letter, memo 10, cond 10
    assert_eq!(item_count(&world, 3001, 2827), 1, "Katari's Letter");
    talk(&mut world, sla); // money → Greetings, memo 11, cond 11
    assert_eq!(item_count(&world, 3001, 2825), 1, "Greetings");
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    talk(&mut world, kakan);
    // Register the duel monster's template so the staged spawn can conjure it.
    if world.data.npc_data.get(27131).is_none() {
        let mut t = crate::data::npc_data::default_template(27131);
        t.type_name = "Monster".into();
        world.data.npc_data.insert_for_test(t);
    }
    ev(&mut world, kakan, "30669-03.html"); // cond 12, spawn the staged duel
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    // The staged duel: a decoy Ol Mahum Pilgrim and the werewolf appear at
    // Java's fixed spots, and the wolf's hate points at the *decoy*, not the
    // player.
    {
        let wolf = npcs_of(&mut world, 27131)[0];
        let hate_on_decoy = world
            .objects
            .get_component::<AggroList>(&wolf)
            .is_some_and(|a| {
                a.0.keys().any(|t| {
                    world
                        .objects
                        .get_component::<model::npc::Npc>(t)
                        .is_some_and(|n| n.npc_id == 30732)
                })
            });
        assert!(hate_on_decoy, "the werewolf opens on the decoy pilgrim");
    }
    // --- Crimson Werewolf: flees from melee, credited only to a mage attack. ---
    add_test_npc(&mut world, NPC_OID + 21, 27131, "Monster", 40, 40, 0, 0);
    combat::npc_receive_damage(&mut world, NPC_OID + 21, 3001, 10.0, false); // melee → flees
    assert!(
        npcs_of(&mut world, 27131)
            .iter()
            .all(|&o| o != NPC_OID + 21),
        "melee makes the werewolf flee"
    );
    add_test_npc(&mut world, NPC_OID + 22, 27131, "Monster", 40, 40, 0, 0);
    skill_hit(&mut world, NPC_OID + 22, 1177); // Wind Strike → scriptValue = player
    npc::npc_do_die(&mut world, NPC_OID + 22, 3001); // → memo 12, cond 13
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    talk(&mut world, kakan); // → Kakan's Letter, memo 13, cond 14
    assert_eq!(item_count(&world, 3001, 3037), 1, "Kakan's Letter");
    talk(&mut world, nyakuri);
    ev(&mut world, nyakuri, "30670-03.html"); // cond 15, spawn krudel
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    kill(&mut world, 27132); // Krudel Lizardman → memo 14, cond 16
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    talk(&mut world, nyakuri); // → Nyakuri's Letter, memo 15, cond 17
    assert_eq!(item_count(&world, 3001, 2828), 1, "Nyakuri's Letter");
    talk(&mut world, ramus); // → Undead List, memo 16, cond 18
    assert_eq!(item_count(&world, 3001, 2829), 1, "Undead List");
    assert_eq!(quest_cond(&world, 3001, q), Some(18));
    // Five bone fragments → memo 17, cond 19.
    for id in [2834, 2835, 2836, 2837] {
        inject(&mut world, 3001, 0x0227_0100 + id, id, 1);
    }
    kill(&mut world, 20100); // Skeleton Archer → Bone Fragment 8 (fifth), cond 19
    assert_eq!(quest_cond(&world, 3001, q), Some(19));
    talk(&mut world, ramus); // bones → Ramus's Letter, memo 18, cond 20
    assert_eq!(item_count(&world, 3001, 2830), 1, "Ramus's Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(20));
    // --- Completion at Sla ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, sla);
    assert_eq!(
        item_count(&world, 3001, 2821),
        1,
        "Mark of the Reformer awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 226528,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(20),
        "one-time quest finished"
    );
}
