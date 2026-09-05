//! The dwarven occupation quests: Q00417 Scavenger and Q00418 Artisan.

use super::super::*;

const Q418: &str = "Q00418_PathOfTheArtisan";

/// A Dwarven Fighter with Q00418 accepted (Silvera at NPC_OID).
fn q418_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> = (1632..=1641).map(|id| (id, "Q418", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in [20017, 20389, 20390] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30527, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 53; // Dwarven Fighter
        p.base_class_id = 53;
        p.race = 4;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q418}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q418} ACCEPT")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q418} 30527-06.htm")),
    );
    assert_eq!(item_count(&world, 3001, 1632), 1, "Silvery's ring");
    drain(&mut rx);
    (world, rx)
}

/// The leader-tooth roll is lopsided: below 5 it pays **only** when one tooth
/// is already held, so the first tooth comes at 50% and the second at 100%.
#[test]
fn quest_q00418_leader_tooth_roll_has_a_hole_at_zero() {
    let (mut world, _rx) = q418_world();
    let mut oid = NPC_OID + 100;

    // Roll 0 with zero teeth: the `< 5` branch does nothing at all.
    oid += 1;
    add_test_npc(&mut world, oid, 20390, "Monster", 20, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, oid, 3001);
    assert_eq!(
        item_count(&world, 3001, 1637),
        0,
        "roll<5 at zero teeth pays nothing"
    );

    // Roll 5 with zero teeth: the `else` branch always pays.
    oid += 1;
    add_test_npc(&mut world, oid, 20390, "Monster", 20, 30, 0, 0);
    world.force_roll(5);
    npc::npc_do_die(&mut world, oid, 3001);
    assert_eq!(item_count(&world, 3001, 1637), 1, "roll>=5 always pays");

    // Roll 0 with one tooth: now the `< 5` branch does pay.
    oid += 1;
    add_test_npc(&mut world, oid, 20390, "Monster", 20, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, oid, 3001);
    assert_eq!(
        item_count(&world, 3001, 1637),
        2,
        "roll<5 pays the second tooth"
    );
}

/// Ratman teeth cap at 10 on a 70% roll; a roll of 7 misses.
#[test]
fn quest_q00418_ratman_teeth_roll_is_seventy_percent() {
    let (mut world, _rx) = q418_world();
    let miss = NPC_OID + 200;
    add_test_npc(&mut world, miss, 20389, "Monster", 20, 30, 0, 0);
    world.force_roll(7);
    npc::npc_do_die(&mut world, miss, 3001);
    assert_eq!(item_count(&world, 3001, 1636), 0, "roll 7 is outside `< 7`");

    let hit = NPC_OID + 201;
    add_test_npc(&mut world, hit, 20389, "Monster", 20, 30, 0, 0);
    world.force_roll(6);
    npc::npc_do_die(&mut world, hit, 3001);
    assert_eq!(item_count(&world, 3001, 1636), 1, "roll 6 pays");
}

/// The whole chain: teeth → 1st pass → Kluto's letter → Pinter's footprint →
/// the stolen box → 2nd pass → the Final Pass Certificate.
#[test]
fn quest_q00418_full_chain_awards_the_final_pass() {
    let (mut world, mut rx) = q418_world();
    let (pinter, kluto) = (NPC_OID + 20, NPC_OID + 21);
    add_test_npc(&mut world, pinter, 30298, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, kluto, 30317, "Folk", 5, 100, 0, 0);
    for _ in 0..10 {
        inventory::add_inventory_item(&mut world, 3001, 1636, 1);
    }
    for _ in 0..2 {
        inventory::add_inventory_item(&mut world, 3001, 1637, 1);
    }

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q418} 30527-08b.html")),
    );
    assert_eq!(item_count(&world, 3001, 1633), 1, "first pass certificate");
    assert_eq!(item_count(&world, 3001, 1636), 0, "teeth handed over");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(3));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kluto}_Quest {Q418} 30317-04.html")),
    );
    assert_eq!(item_count(&world, 3001, 1638), 1, "Kluto's letter");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(4));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pinter}_Quest {Q418} 30298-03.html")),
    );
    assert_eq!(item_count(&world, 3001, 1639), 1, "footprint of thief");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(5));

    let orc = NPC_OID + 300;
    add_test_npc(&mut world, orc, 20017, "Monster", 20, 30, 0, 0);
    world.force_roll(0); // `getRandom(10) < 2`
    npc::npc_do_die(&mut world, orc, 3001);
    assert_eq!(item_count(&world, 3001, 1640), 1, "the stolen secret box");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(6));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pinter}_Quest {Q418} 30298-06.html")),
    );
    assert_eq!(item_count(&world, 3001, 1634), 1, "second pass certificate");
    assert_eq!(item_count(&world, 3001, 1641), 1, "the secret box");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(7));
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kluto}_Quest {Q418} 30317-10.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 1635),
        1,
        "the Final Pass Certificate"
    );
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q418].is_completed());
    }
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION)
    );
}

/// Fourth quest running with a route dead at both ends.
#[test]
fn artisan_dead_branch_is_dead_at_both_ends() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00418_PathOfTheArtisan/"
    );
    for npc in ["31956", "31963", "32052"] {
        let any = (1..=9).any(|n| std::path::Path::new(&format!("{DIST}{npc}-0{n}.html")).exists());
        assert!(any, "{npc} ships pages but is registered nowhere");
    }
    // Only 08b is offered; 08c (the memoState 10 entry) is not.
    // Pages only — the .java source naturally names `08c` as a case label,
    // which is exactly the handler we are proving is unreachable.
    let mut offers_08b = false;
    for entry in std::fs::read_dir(DIST).expect("quest dir") {
        let path = entry.expect("entry").path();
        let is_page = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e == "htm" || e == "html");
        if !is_page {
            continue;
        }
        let body = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            !body.contains("30527-08c"),
            "no page may offer the dead 08c entry"
        );
        offers_08b |= body.contains("30527-08b");
    }
    assert!(offers_08b, "08b is the live route and is offered");
}

const Q417: &str = "Q00417_PathOfTheScavenger";

/// A Dwarven Fighter with Q00417 accepted (Pipi at NPC_OID).
fn q417_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> = (1642..=1657).map(|id| (id, "Q417", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in [20403, 20508, 20777, 27058] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30524, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 53; // Dwarven Fighter
        p.base_class_id = 53;
        p.race = 4;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q417}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q417} ACCEPT")),
    );
    assert_eq!(item_count(&world, 3001, 1643), 1, "Pipi's letter");
    drain(&mut rx);
    (world, rx)
}

/// Mark `npc_oid` as spoiled by `player`, the way the Spoil effect would.
fn mark_spoiled(world: &mut World, npc_oid: i32, player: i32) {
    if let Some(n) = world.objects.get_component_mut::<model::npc::Npc>(&npc_oid) {
        n.spoiler_object_id = player;
    }
}

/// The payout is gated on the corpse being **spoiled** — the Scavenger's own
/// mechanic. An unspoiled Honey Bear pays nothing.
#[test]
fn quest_q00417_payout_requires_a_spoiled_corpse() {
    for (spoil, expected) in [(false, 0), (true, 1)] {
        let (mut world, _rx) = q417_world();
        inventory::add_inventory_item(&mut world, 3001, 1653, 1); // bear picture
        let bear = NPC_OID + 100;
        add_test_npc(&mut world, bear, 27058, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, bear, 3001, 10.0, false);
        if spoil {
            // Spoiled by someone else, so the attack-time disqualifier
            // (spoiler == attacker) does not fire.
            mark_spoiled(&mut world, bear, 9999);
        }
        npc::npc_do_die(&mut world, bear, 3001);
        assert_eq!(
            item_count(&world, 3001, 1655),
            expected,
            "spoiled={spoil}: honey only drops off a spoiled corpse"
        );
    }
}

/// `giveItemRandomly`'s chance is a 0..1 fraction and this quest passes **50**,
/// so every qualifying kill drops. No forced roll is used: if the port had
/// "corrected" it to 0.5 this would be flaky, and at 0.5 with the RNG it would
/// fail about half the time.
#[test]
fn quest_q00417_drop_chance_fifty_means_always() {
    let (mut world, _rx) = q417_world();
    inventory::add_inventory_item(&mut world, 3001, 1654, 1); // tarantula picture
    for i in 0..6 {
        let mob = NPC_OID + 200 + i;
        add_test_npc(&mut world, mob, 20403, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, mob, 3001, 10.0, false);
        mark_spoiled(&mut world, mob, 9999);
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(
        item_count(&world, 3001, 1656),
        6,
        "six kills, six beads — chance 50 is not 50%"
    );
}

/// The Honey Bear summon meter escalates at `20 * flag` percent and resets on
/// success.
#[test]
fn quest_q00417_honey_bear_summon_meter_escalates() {
    let (mut world, _rx) = q417_world();
    inventory::add_inventory_item(&mut world, 3001, 1653, 1); // bear picture

    // First kill: flag is 0, so no roll happens at all — it just rises.
    let b1 = NPC_OID + 300;
    add_test_npc(&mut world, b1, 20777, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, b1, 3001, 10.0, false);
    npc::npc_do_die(&mut world, b1, 3001);
    assert!(
        npcs_of(&mut world, 27058).is_empty(),
        "flag 0 never summons"
    );

    // Second kill with the roll inside `20 * 1`: the bear appears.
    let b2 = NPC_OID + 301;
    add_test_npc(&mut world, b2, 20777, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, b2, 3001, 10.0, false);
    world.force_roll(5); // 5 < 20
    npc::npc_do_die(&mut world, b2, 3001);
    assert_eq!(
        npcs_of(&mut world, 27058).len(),
        1,
        "the Honey Bear was summoned"
    );
}

/// The delivery round-trip bumps the **tens** digit of `memoStateEx(1)`, and
/// the second hand-in promotes to cond 3.
#[test]
fn quest_q00417_deliveries_bump_the_tens_digit() {
    let (mut world, _rx) = q417_world();
    let shari = NPC_OID + 20;
    add_test_npc(&mut world, shari, 30517, "Folk", 5, 100, 0, 0);

    inventory::add_inventory_item(&mut world, 3001, 1648, 1); // Shari's axe
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{shari}_Quest {Q417}")),
    );
    assert_eq!(item_count(&world, 3001, 1651), 1, "Shari's pay");
    assert_eq!(
        quest_memo_ex(&world, 3001, Q417, 1),
        10,
        "tens digit bumped"
    );
    assert!(
        quest_cond(&world, 3001, Q417) != Some(3),
        "not promoted on the first"
    );

    inventory::add_inventory_item(&mut world, 3001, 1648, 1);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{shari}_Quest {Q417}")),
    );
    assert_eq!(quest_memo_ex(&world, 3001, Q417, 1), 20);
    inventory::add_inventory_item(&mut world, 3001, 1648, 1);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{shari}_Quest {Q417}")),
    );
    assert_eq!(
        quest_cond(&world, 3001, Q417),
        Some(3),
        "the third hand-in promotes"
    );
}

/// Torai hands over the undies and **deletes himself**; Raut then pays the
/// Ring of Raven.
#[test]
fn quest_q00417_torai_vanishes_and_raut_pays_the_ring() {
    let (mut world, mut rx) = q417_world();
    let (torai, raut) = (NPC_OID + 30, NPC_OID + 31);
    add_test_npc(&mut world, torai, 30557, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, raut, 30316, "Folk", 5, 100, 0, 0);
    inventory::add_inventory_item(&mut world, 3001, 1644, 1); // teleport scroll

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{torai}_Quest {Q417} 30557-03.html")),
    );
    assert_eq!(item_count(&world, 3001, 1645), 1, "succubus undies");
    assert_eq!(quest_cond(&world, 3001, Q417), Some(11));
    assert!(
        world
            .objects
            .get_component::<model::npc::Npc>(&torai)
            .is_none(),
        "Torai deleted himself"
    );
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{raut}_Quest {Q417}")),
    );
    assert_eq!(item_count(&world, 3001, 1642), 1, "the Ring of Raven");
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q417].is_completed());
    }
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION)
    );
}

fn quest_memo_ex(world: &World, player: i32, quest: &str, slot: i32) -> i32 {
    world
        .objects
        .get_component::<model::components::social::Quests>(&player)
        .and_then(|q| q.0.get(quest))
        .and_then(|qs| qs.vars.get(&format!("memoStateEx{slot}")))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
}
