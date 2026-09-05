//! Q00420 Little Wing and Q00421 Little Wing's Big Adventure — the
//! hatchling chain. Not occupation changes; they sit in the Q00400 block
//! only by numbering.

use super::super::*;

/// Little Wing (420): the hatchling-pet quest, normal (plain Fairy Stone) path
/// end to end — forge the stone, brew Monkshood Juice, take Exarion's scale,
/// farm 20 eggs, hatch, and redeem a Dragonflute. Plus the Deluxe stone's
/// `onAttack` shatter risk as a separate check.
#[test]
fn quest_q00420_little_wing() {
    const COOPER: i32 = 30829;
    const CRONOS: i32 = 30610;
    const MARIA: i32 = 30608;
    const BYRON: i32 = 30711;
    const MIMYU: i32 = 30747;
    const EXARION: i32 = 30748;
    // Materials
    const COAL: i32 = 1870;
    const CHARCOAL: i32 = 1871;
    const SILVER_NUGGET: i32 = 1873;
    const GEMSTONE_D: i32 = 2130;
    const TOAD_SKIN: i32 = 3820;
    // Quest items
    const FAIRY_STONE_LIST: i32 = 3818;
    const FAIRY_STONE: i32 = 3816;
    const DELUXE_FAIRY_STONE: i32 = 3817;
    const MONKSHOOD_JUICE: i32 = 3821;
    const EXARION_SCALE: i32 = 3822;
    const EXARION_EGG: i32 = 3823;
    const DRAGONFLUTE_OF_WIND: i32 = 3500;
    // Monsters
    const LETO_WARRIOR: i32 = 20580;
    const FLINE: i32 = 20589; // a Deluxe-stone breaker

    let (mut world, _db, _l) = quest_test_world();
    let ids = [
        COAL,
        CHARCOAL,
        SILVER_NUGGET,
        GEMSTONE_D,
        TOAD_SKIN,
        FAIRY_STONE_LIST,
        FAIRY_STONE,
        DELUXE_FAIRY_STONE,
        MONKSHOOD_JUICE,
        EXARION_SCALE,
        EXARION_EGG,
        3499, // FAIRY_DUST
    ];
    let mut items: Vec<(i32, &str, bool)> = ids.iter().map(|&i| (i, "q", true)).collect();
    items.push((DRAGONFLUTE_OF_WIND, "flute", false));
    add_quest_items(&mut world, &items);
    for id in [LETO_WARRIOR, FLINE] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100.0;
        world.data.npc_data.insert_for_test(t);
    }
    let cooper = NPC_OID;
    let cronos = NPC_OID + 1;
    let maria = NPC_OID + 2;
    let byron = NPC_OID + 3;
    let mimyu = NPC_OID + 4;
    let exarion = NPC_OID + 5;
    for (oid, npc) in [
        (cooper, COOPER),
        (cronos, CRONOS),
        (maria, MARIA),
        (byron, BYRON),
        (mimyu, MIMYU),
        (exarion, EXARION),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 200, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 40;
    let q = "Q00420_LittleWing";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let cond = |w: &World| quest_cond(w, 3001, q);

    // Accept → Cronos → pick the plain Fairy Stone (cond 2).
    talk(&mut world, cooper);
    ev(&mut world, cooper, "30829-02.htm");
    assert_eq!(cond(&world), Some(1));
    ev(&mut world, cronos, "30610-05.html");
    assert_eq!(cond(&world), Some(2), "plain stone chosen");
    assert_eq!(
        item_count(&world, 3001, FAIRY_STONE_LIST),
        1,
        "stone list given"
    );

    // Gather the materials, then Maria forges the Fairy Stone (cond 3).
    inject(&mut world, 3001, 0x0420_1000, COAL, 10);
    inject(&mut world, 3001, 0x0420_2000, CHARCOAL, 10);
    inject(&mut world, 3001, 0x0420_3000, GEMSTONE_D, 1);
    inject(&mut world, 3001, 0x0420_4000, SILVER_NUGGET, 3);
    inject(&mut world, 3001, 0x0420_5000, TOAD_SKIN, 10);
    ev(&mut world, maria, "30608-03.html");
    assert_eq!(cond(&world), Some(3));
    assert_eq!(
        item_count(&world, 3001, FAIRY_STONE),
        1,
        "Fairy Stone forged"
    );
    assert_eq!(item_count(&world, 3001, COAL), 0, "materials consumed");

    // Byron → Mimyu accepts the stone (cond 5) and brews Monkshood Juice.
    ev(&mut world, byron, "30711-03.html");
    assert_eq!(cond(&world), Some(4));
    ev(&mut world, mimyu, "30747-02.html");
    assert_eq!(cond(&world), Some(5));
    assert_eq!(
        item_count(&world, 3001, FAIRY_STONE),
        0,
        "stone handed to Mimyu"
    );
    ev(&mut world, mimyu, "30747-07.html");
    assert_eq!(
        item_count(&world, 3001, MONKSHOOD_JUICE),
        1,
        "Monkshood Juice"
    );

    // Exarion trades the juice for its Scale and a hunt (cond 6).
    ev(&mut world, exarion, "30748-02.html");
    assert_eq!(cond(&world), Some(6));
    assert_eq!(item_count(&world, 3001, EXARION_SCALE), 1, "Exarion Scale");
    assert_eq!(
        item_count(&world, 3001, MONKSHOOD_JUICE),
        0,
        "juice consumed"
    );

    // Farm 20 Exarion Eggs from Leto Warriors (drake_hunt).
    let mut mob = NPC_OID + 20;
    for _ in 0..20 {
        mob += 1;
        add_test_npc(&mut world, mob, LETO_WARRIOR, "Monster", 40, 110, 200, 0);
        world.force_roll(0); // give_item_randomly roll → drop
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, EXARION_EGG), 20, "20 eggs farmed");

    // Exarion hatches the egg (cond 7).
    talk(&mut world, exarion);
    assert_eq!(cond(&world), Some(7), "egg hatched → cond 7");
    assert_eq!(item_count(&world, 3001, EXARION_EGG), 1, "one hatched egg");
    assert_eq!(item_count(&world, 3001, EXARION_SCALE), 0, "scale consumed");

    // Mimyu redeems the egg for a Dragonflute (forced roll 0 → Wind), completing.
    world.force_roll(0); // give_reward roll(100) → Wind
    ev(&mut world, mimyu, "30747-12.html");
    assert_eq!(
        item_count(&world, 3001, DRAGONFLUTE_OF_WIND),
        1,
        "Dragonflute of Wind"
    );
    assert!(
        cond(&world).is_none(),
        "Little Wing is repeatable: the reward exit forgets the quest"
    );

    // --- Separately: a Deluxe Fairy Stone shatters when striking the fae. ---
    let (mut w2, _db2, _l2) = quest_test_world();
    add_quest_items(&mut w2, &[(DELUXE_FAIRY_STONE, "q", true)]);
    add_test_npc(&mut w2, NPC_OID, FLINE, "Monster", 40, 110, 200, 0);
    let _rx2 = ingame_player(&mut w2, 1, 3001, 100, 200, 0);
    w2.objects.get_component_mut::<Player>(&3001).unwrap().level = 40;
    {
        let quests = w2
            .objects
            .get_component_mut::<model::components::social::Quests>(&3001)
            .unwrap();
        let qs = quests.0.entry(q.to_string()).or_default();
        qs.state = model::quest::state::STARTED;
        qs.vars.insert("cond".to_string(), "6".to_string());
    }
    inject(&mut w2, 3001, 0x0420_9000, DELUXE_FAIRY_STONE, 1);
    w2.force_roll(0); // onAttack roll(100)==0 < 30 → shatter
    combat::npc_receive_damage(&mut w2, NPC_OID, 3001, 1.0, false);
    assert_eq!(
        item_count(&w2, 3001, DELUXE_FAIRY_STONE),
        0,
        "the Deluxe Fairy Stone shatters on the fae"
    );
}

/// Quest 421 — the full hatchling→strider arc, driven through the pet
/// infrastructure: the flute-enchant start gate, Mimyu binding the rite to the
/// flute's object id, the four-tree drink grind (only *the bound pet's* blows
/// count, and only past each tree's hit threshold), and redeeming the flute for
/// the Dragon Bugle once all four essences (`memoState == 15`) are drunk.
#[test]
fn quest_q00421_little_wings_big_adventure() {
    use crate::model::components::social::Quests;
    use crate::model::components::summons::{PetOf, SummonRef};
    use crate::model::inventory::Inventory;

    const CRONOS: i32 = 30610;
    const MIMYU: i32 = 30747;
    const FLUTE: i32 = 3500; // Dragonflute of Wind
    const BUGLE: i32 = 4422; // Dragon Bugle of Wind
    const LEAF: i32 = 4325; // Fairy Leaf
    const HATCHLING: i32 = 12311; // stand-in pet species
    // (tree npc id, min_hits, memo bit value)
    const TREES: [(i32, i32, i32); 4] = [
        (27185, 270, 1),
        (27186, 400, 2),
        (27187, 150, 4),
        (27188, 270, 8),
    ];
    const FLUTE_OID: i32 = 0x0042_1000;
    let q = "Q00421_LittleWingsBigAdventure";

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (FLUTE, "Dragonflute of Wind", false),
            (BUGLE, "Dragon Bugle of Wind", false),
            (LEAF, "Fairy Leaf", true),
        ],
    );

    let cronos = NPC_OID;
    let mimyu = NPC_OID + 1;
    add_test_npc(&mut world, cronos, CRONOS, "Folk", 55, 100, 200, 0);
    add_test_npc(&mut world, mimyu, MIMYU, "Folk", 55, 120, 200, 0);
    let tree_oids: Vec<i32> = TREES
        .iter()
        .enumerate()
        .map(|(i, (id, _, _))| {
            let oid = NPC_OID + 2 + i as i32;
            add_test_npc(&mut world, oid, *id, "Monster", 60, 300, 300 + i as i32, 0);
            oid
        })
        .collect();

    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 45;
    // One Dragonflute of Wind, enchant 60 (its enchant level is the hatchling's).
    inject(&mut world, 3001, FLUTE_OID, FLUTE, 1);

    let set_enchant = |w: &mut World, level: i32| {
        w.objects
            .get_component_mut::<Inventory>(&3001)
            .unwrap()
            .set_item_enchant_level(FLUTE, level);
    };
    let memo = |w: &World| -> i32 {
        w.objects
            .get_component::<Quests>(&3001)
            .and_then(|qc| qc.0.get(q))
            .and_then(|qs| qs.vars.get("memoState"))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    };
    let started = |w: &World| -> bool {
        w.objects
            .get_component::<Quests>(&3001)
            .and_then(|qc| qc.0.get(q))
            .is_some_and(|qs| qs.state == model::quest::state::STARTED)
    };
    let set_hits = |w: &mut World, n: i32| {
        w.objects
            .get_component_mut::<Quests>(&3001)
            .unwrap()
            .0
            .get_mut(q)
            .unwrap()
            .vars
            .insert("hits".into(), n.to_string());
    };
    let event = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let bind_pet = |w: &mut World, oid: i32, collar: i32| {
        add_test_npc(w, oid, HATCHLING, "Pet", 55, 130, 200, 0);
        w.objects.add_components(
            &oid,
            PetOf {
                collar_object_id: collar,
                fed: 100,
                max_fed: 100,
                level: 55,
                exp: 0,
                sp: 0,
                exp_before_death: 0,
            },
        );
        match w.objects.get_component_mut::<SummonRef>(&3001) {
            Some(s) => s.pet = Some(oid),
            None => w.objects.add_components(
                &3001,
                SummonRef {
                    servitor: None,
                    pet: Some(oid),
                },
            ),
        }
    };

    // --- Enchant gate: an under-enchanted flute (hatchling < 55) can't start. ---
    set_enchant(&mut world, 40);
    talk(&mut world, cronos); // creates the CREATED quest state (Java getQuestState(true))
    event(&mut world, cronos, "30610-05.htm");
    assert!(
        !started(&world),
        "under-enchanted flute cannot start the rite"
    );
    set_enchant(&mut world, 60);

    // --- Start: the rite binds to this flute's object id. ---
    event(&mut world, cronos, "30610-05.htm");
    assert!(
        started(&world),
        "the rite started with a level-60 hatchling"
    );
    assert_eq!(memo(&world), 100, "memoState 100 on start");
    assert_eq!(
        world.objects.get_component::<Quests>(&3001).unwrap().0[q]
            .vars
            .get("fluteObjectId")
            .map(|s| s.as_str()),
        Some(FLUTE_OID.to_string().as_str()),
        "rite bound to the flute's object id"
    );

    // --- Mimyu intro (100 → 200). ---
    talk(&mut world, mimyu);
    assert_eq!(memo(&world), 200, "Mimyu's intro advances memoState to 200");

    // Without the hatchling out, Mimyu withholds the Fairy Leaves.
    event(&mut world, mimyu, "30747-05.html");
    assert_eq!(
        item_count(&world, 3001, LEAF),
        0,
        "no leaves without the pet"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(2),
        "no cond 2 without the pet"
    );

    // Summon the bound hatchling; now Mimyu hands over four leaves.
    bind_pet(&mut world, NPC_OID + 20, FLUTE_OID);
    event(&mut world, mimyu, "30747-05.html");
    assert_eq!(
        item_count(&world, 3001, LEAF),
        4,
        "four Fairy Leaves granted"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "cond 2 to drink");
    assert_eq!(memo(&world), 0, "memoState reset to the 4-bit drink field");

    // --- The player's own blow does not count. ---
    quests::notify_attack(&mut world, 3001, tree_oids[0], TREES[0].0, None, false);
    assert_eq!(memo(&world), 0, "a player blow drinks nothing");
    assert_eq!(
        item_count(&world, 3001, LEAF),
        4,
        "no leaf spent by the player"
    );

    // A pet blow below the hit threshold drinks nothing either.
    quests::notify_attack(&mut world, 3001, tree_oids[0], TREES[0].0, None, true);
    assert_eq!(memo(&world), 0, "below the threshold, no essence taken");
    assert_eq!(
        item_count(&world, 3001, LEAF),
        4,
        "no leaf spent below threshold"
    );

    // --- The four-tree grind: past each threshold, the bound pet drinks. ---
    for (i, (id, min_hits, value)) in TREES.iter().enumerate() {
        set_hits(&mut world, min_hits - 1); // next blow reaches the threshold
        world.force_roll(0); // the 2% essence roll → success
        let before = memo(&world);
        quests::notify_attack(&mut world, 3001, tree_oids[i], *id, None, true);
        assert_eq!(memo(&world), before + value, "tree {id} sets its memo bit");
    }
    assert_eq!(memo(&world), 15, "all four essences drunk");
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(3),
        "cond 3 once all drunk"
    );
    assert_eq!(
        item_count(&world, 3001, LEAF),
        0,
        "all four leaves consumed"
    );

    // --- Redemption: Mimyu grows the hatchling into a strider. ---
    talk(&mut world, mimyu); // memoState 15, pet present, no leaves → 16
    assert_eq!(memo(&world), 16, "Mimyu readies the transformation");
    world
        .objects
        .get_component_mut::<SummonRef>(&3001)
        .unwrap()
        .pet = None; // dismiss the hatchling
    talk(&mut world, mimyu); // memoState 16, no summon, bound flute → the Bugle
    assert_eq!(item_count(&world, 3001, BUGLE), 1, "Dragon Bugle awarded");
    assert_eq!(item_count(&world, 3001, FLUTE), 0, "the flute is consumed");
    assert!(
        world
            .objects
            .get_component::<Quests>(&3001)
            .is_none_or(|qc| !qc.0.contains_key(q)),
        "the repeatable quest is forgotten on completion"
    );
}

/// Quest 421 — killing a Tree of Vision (rather than drinking from it) summons a
/// 20-strong Guardian Ghost ambush that despawns after five minutes.
#[test]
fn quest_q00421_guardian_ambush_despawns() {
    use crate::model::components::social::Quests;

    const TREE: i32 = 27185; // Tree of Wind
    const GUARDIAN: i32 = 27189;

    let (mut world, _db, _l) = quest_test_world();
    {
        let mut t = crate::data::npc_data::default_template(GUARDIAN);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let tree = NPC_OID;
    add_test_npc(&mut world, tree, TREE, "Monster", 60, 300, 300, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 300, 300, 0);
    let q = "Q00421_LittleWingsBigAdventure";
    {
        let quests = world.objects.get_component_mut::<Quests>(&3001).unwrap();
        let qs = quests.0.entry(q.to_string()).or_default();
        qs.state = model::quest::state::STARTED;
        qs.vars.insert("cond".to_string(), "2".to_string());
    }

    // Fell the tree — the ambush spawns. Since the G22 `ai/others` sweep the
    // standalone `FairyTrees` script swarms the same trees with 20 more (Java
    // registers both scripts on this kill), so 40 appear; theirs last 30 s,
    // this quest's 5 minutes.
    drain(&mut rx);
    combat::npc_receive_damage(&mut world, tree, 3001, 10_000.0, false);
    assert_eq!(
        npcs_of(&mut world, GUARDIAN).len(),
        40,
        "20 Guardian Ghosts from the quest + 20 from ai/others/FairyTrees"
    );

    // The dying tree's parting shot: `npc.doCast(VICIOUS_POISON)` as the first
    // guardian appears. This is a *real* cast, so assert the wire rather than
    // trusting the call — `npc_cast` returns false and does nothing when the
    // skill id is absent or the use-conditions refuse, and a silent no-op here
    // would look exactly like a working port.
    let cast_skills: Vec<i32> = drain(&mut rx)
        .iter()
        .filter(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
        .map(|p| {
            let mut r = commons::network::PacketReader::new(&p[1..]);
            for _ in 0..3 {
                let _ = r.read_i32(); // cast bar, caster, target
            }
            r.read_i32().unwrap() // skill id
        })
        .collect();
    assert!(
        cast_skills.contains(&4243),
        "the tree casts Venomous Poison on its killer: {cast_skills:?}"
    );

    // After 30 s the FairyTrees half is gone and the quest's ambush remains.
    advance_ticks(&mut world, 301);
    assert_eq!(
        npcs_of(&mut world, GUARDIAN).len(),
        20,
        "the FairyTrees guardians (30 s) expire first"
    );

    // Five minutes later, they are gone.
    advance_ticks(&mut world, 2700); // the rest of the 300_000 ms
    assert!(
        npcs_of(&mut world, GUARDIAN).is_empty(),
        "the ambush despawns after 5 minutes"
    );
}
