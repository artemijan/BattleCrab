//! The quest engine itself — timers, abort, the journal, kill credit — plus
//! the client requests that sit alongside it.

use super::*;

/// RequestShowMiniMap (0x6C): empty body, answered with `ShowMiniMap` —
/// map id 0 (base world map) plus the Seven Signs state byte.
#[test]
fn request_show_mini_map_opens_world_map() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    drain(&mut a_rx);

    on_packet(&mut world, 1, vec![cop::REQUEST_SHOW_MINI_MAP]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::SHOW_MINI_MAP);
    assert_eq!(
        i32::from_le_bytes(pkt[1..5].try_into().unwrap()),
        0,
        "base world map"
    );
    assert_eq!(pkt[5], 0, "Seven Signs state");
    assert_eq!(pkt.len(), 6);
}

/// The world map's data requests: `RequestAllCastleInfo` (0xD0:0x39) and
/// `RequestAllFortressInfo` (0xD0:0x3A) are answered.
///
/// The castle count is the **server's castle list**, as Java's
/// `CastleManager.getCastles()` is — not a fixed 9 — so the fixture seeds the
/// nine this dist ships. The fort list really is static: fort sieges are out
/// of scope, so all 21 are permanently unowned.
/// `castle_info_overlay_carries_owner_tax_and_siege` covers the per-castle
/// fields; this test covers the request plumbing.
#[test]
fn map_castle_and_fortress_info_requests_answered() {
    let (mut world, ..) = cast_test_world();
    world.castles = (1..=9)
        .map(|id| model::castle::Castle {
            show_npc_crest: false,
            id,
            name: format!("Castle{id}"),
            side: model::castle::CastleSide::Neutral,
            ticket_buy_count: 0,
            first_mid_victory: false,
            time_registration_over: true,
            siege_time_registration_end: 0,
            siege_date: 0,
            treasury: 0,
        })
        .collect();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    drain(&mut a_rx);

    on_packet(&mut world, 1, vec![cop::EX_PACKET, 0x39, 0x00]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::EX);
    assert_eq!(
        i16::from_le_bytes(pkt[1..3].try_into().unwrap()),
        server_packets::opcodes::EX_SHOW_CASTLE_INFO
    );
    assert_eq!(
        i32::from_le_bytes(pkt[3..7].try_into().unwrap()),
        9,
        "nine castles"
    );
    assert!(a_rx.try_recv().is_err(), "no PartyMemberPosition when solo");

    on_packet(&mut world, 1, vec![cop::EX_PACKET, 0x3A, 0x00]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::EX);
    assert_eq!(
        i16::from_le_bytes(pkt[1..3].try_into().unwrap()),
        server_packets::opcodes::EX_SHOW_FORTRESS_INFO
    );
    assert_eq!(
        i32::from_le_bytes(pkt[3..7].try_into().unwrap()),
        21,
        "twenty-one forts"
    );
}

/// RequestSkillList (0x50): empty body, re-sends the `SkillList` packet
/// (`player.sendSkillList()`) — the client asks for this when it opens the
/// skills panel.
#[test]
fn request_skill_list_resends_skill_list() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0); // 4 known skills
    drain(&mut a_rx);

    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_LIST]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], 0x5F, "SkillList opcode");
    assert_eq!(
        i32::from_le_bytes(pkt[1..5].try_into().unwrap()),
        4,
        "all known skills listed"
    );
}

/// `RequestStopMove` (`player.stopMove(getLocation())`): the in-flight move
/// and any pending path request are dropped, and `StopMove` is broadcast to
/// the mover (Player `broadcastPacket` includes self) at the current spot.
#[test]
fn request_stop_move_clears_movement_and_pending_path() {
    use crate::model::components::space::PathWait;
    use crate::model::movement::MoveData;

    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 5001, 700, 800, 0);

    // Simulate an in-flight move plus a still-outstanding path request.
    world.objects.add_components(
        &5001,
        Movement(MoveData {
            start_x: 700,
            start_y: 800,
            start_z: 0,
            dest_x: 2000,
            dest_y: 800,
            dest_z: 0,
            start_tick: world.tick,
            total_ticks: 100,
            geo_path: None,
        }),
    );
    world.objects.add_components(&5001, PathWait { seq: 42 });

    handle_request_stop_move(&mut world, 1);

    assert!(
        !world.objects.has_component::<Movement>(&5001),
        "move data deleted"
    );
    assert!(
        !world.objects.has_component::<PathWait>(&5001),
        "pending path dropped"
    );
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::STOP_MOVE
    );
}

/// `ExSendSelectedQuestZoneID` stores the selected zone id on the player
/// (default -1 → the client's choice), read later by quest teleports.
#[test]
fn ex_send_selected_quest_zone_id_sets_field() {
    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, 1, 5001, 10, 20, 30);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5001)
            .unwrap()
            .quest_zone_id,
        -1
    );

    handle_ex_send_selected_quest_zone_id(&mut world, 1, &int_body(7));

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5001)
            .unwrap()
            .quest_zone_id,
        7
    );
}

/// The quest UI's Abandon button (`RequestQuestAbort` 0x63): repeatable
/// exit without the finish sound — state forgotten, quest items destroyed.
#[test]
fn quest_abort_wipes_state_and_items() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 3;

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00258_BringWolfPelts 30001-03.html"
        )),
    );
    inventory::add_inventory_item(&mut world, 3001, 702, 5).unwrap();
    drain(&mut rx);
    drain_db(&mut db_rx);

    let mut w = PacketWriter::new();
    w.write_i32(258);
    on_packet(&mut world, 1, {
        let mut v = vec![cop::REQUEST_QUEST_ABORT];
        v.extend(w.into_bytes());
        v
    });

    let pkts = drain(&mut rx);
    assert!(
        !world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap()
            .0
            .contains_key("Q00258_BringWolfPelts"),
        "abort forgets the quest"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(702),
        0,
        "quest items destroyed"
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::QUEST_LIST),
        "QuestList refresh"
    );
    assert!(
        !sound_names(&pkts).contains(&"ItemSound.quest_finish".to_string()),
        "no finish sound on abort"
    );
    // Memory-first: the quest is forgotten in the Quests component (asserted
    // above); the flush reconcile drops its rows — no per-action DB write.
}

/// Quest-timer groundwork: a synthetic script starts a 500 ms timer via an
/// event bypass; it fires once through the scheduler (seq match) and a
/// cancelled one stays silent (seq bumped).
#[test]
fn quest_timer_fires_once_and_cancels() {
    struct TimerTestScript;
    impl quests::QuestScript for TimerTestScript {
        fn id(&self) -> i32 {
            -2
        }
        fn name(&self) -> &'static str {
            "TimerTest"
        }
        fn html_dir(&self) -> &'static str {
            ""
        }
        fn start_npcs(&self) -> &[i32] {
            &[]
        }
        fn talk_npcs(&self) -> &[i32] {
            &[30001]
        }
        fn on_talk(&self, _ctx: &mut quests::QuestCtx) -> Option<String> {
            None
        }
        fn on_event(&self, ctx: &mut quests::QuestCtx, event: &str) -> Option<String> {
            match event {
                "start" => ctx.start_quest_timer("tick", 500),
                "cancel" => ctx.cancel_quest_timer("tick"),
                _ => {}
            }
            None
        }
        fn on_timer(&self, ctx: &mut quests::QuestCtx, name: &str) {
            if name == "tick" {
                ctx.play_sound("timer_fired");
            }
        }
    }

    let (mut world, _db_rx, _link_rx) = quest_test_world();
    world.quests = Arc::new(quests::QuestRegistry::new(vec![Arc::new(TimerTestScript)]));
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest TimerTest start")),
    );
    drain(&mut rx);
    advance_ticks(&mut world, 5);
    let pkts = drain(&mut rx);
    assert!(
        sound_names(&pkts).contains(&"timer_fired".to_string()),
        "timer fired at 500 ms"
    );
    advance_ticks(&mut world, 10);
    assert!(drain(&mut rx).is_empty(), "non-repeating: fires once");

    // Start then cancel: the stale seq no-ops.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest TimerTest start")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest TimerTest cancel")),
    );
    drain(&mut rx);
    advance_ticks(&mut world, 10);
    assert!(
        sound_names(&drain(&mut rx)).is_empty(),
        "cancelled timer never fires"
    );
}

/// TeleportWithCharm: the bare `Quest` click consumes the token and
/// teleports; without a token it shows the "come back with one" page.
#[test]
fn teleport_with_charm_consumes_token() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1659, "Gatekeeper Token", false)]);
    add_test_npc(&mut world, NPC_OID, 30540, "Teleporter", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);

    // No token: the explain page.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("no-token page");
    assert!(
        html.contains("Token") || html.contains("token"),
        "got: {html}"
    );

    // With a token: teleport + consumption.
    inventory::add_inventory_item(&mut world, 3001, 1659, 1);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    assert_eq!(item_count(&world, 3001, 1659), 0, "token consumed");
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (-80826, 149775, -3038),
        "destination z lifted by 5 (teleToLocation)"
    );
    assert!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .teleporting
    );
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == 0x22),
        "TeleportToLocation sent"
    );
}

/// TeleportToRaceTrack: a gatekeeper's free "Monster Race Track" button
/// sends the player to the arena and records the origin in `MONSTER_RETURN`;
/// the Race Manager reads it back and returns them, clearing the variable.
/// (Destination z is lifted by 5 by `teleToLocation`, as in the charm test.)
#[test]
fn teleport_to_race_track_round_trips_via_monster_return() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    // Trisha (Dion gatekeeper) and the Race Manager at the arena.
    add_test_npc(&mut world, NPC_OID, 30059, "Teleporter", 70, 100, 0, 0);
    add_test_npc(
        &mut world,
        NPC_OID + 1,
        30995,
        "RaceManager",
        70,
        12661,
        181687,
        -3540,
    );
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    drain(&mut rx);

    // Outbound: the gatekeeper's button.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest TeleportToRaceTrack")),
    );
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (12661, 181687, -3535),
        "at the race track"
    );
    assert_eq!(
        world
            .objects
            .get_component::<model::components::player::PlayerVariables>(&3001)
            .unwrap()
            .get_int("MONSTER_RETURN", -1),
        30059,
        "origin gatekeeper remembered"
    );
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == 0x22),
        "TeleportToLocation sent"
    );

    // Inbound: the manager sends them back to Trisha's town, not the default.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .teleporting = false;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest TeleportToRaceTrack", NPC_OID + 1)),
    );
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (15670, 142983, -2700),
        "returned to Dion"
    );
    assert_eq!(
        world
            .objects
            .get_component::<model::components::player::PlayerVariables>(&3001)
            .unwrap()
            .get_int("MONSTER_RETURN", -1),
        -1,
        "return point consumed"
    );
}

/// The Race Manager with no stored origin falls back to Trisha (Dion) —
/// Java's `TELEPORTER_LOCATIONS.get(30059)` branch.
#[test]
fn race_manager_without_monster_return_falls_back_to_dion() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    // Within interaction range of the player — the bypass is distance-gated.
    add_test_npc(&mut world, NPC_OID, 30995, "RaceManager", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest TeleportToRaceTrack")),
    );
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (15670, 142983, -2700),
        "default return is Dion"
    );
}

/// Boats (G24.5) — the milestone gate: board a docked ferry, ride it to the far
/// harbor, and step off.
#[test]
fn ferry_ride_board_travel_disembark() {
    use crate::model::boat::{InVehicle, RouteDef, VehiclePathPoint};
    use crate::model::components::space::Position;

    // A there-and-back route with docks at both ends; the ferry spawns docked
    // at the last waypoint (dock A, 1000,1000). Docks carry no schedule, so
    // the dwell is the silent default.
    let wp = |x: i32, dock: bool| VehiclePathPoint {
        x,
        y: 1000,
        z: -3600,
        move_speed: 200,
        rotation_speed: 800,
        dock,
        schedule: None,
    };
    let ride_route = RouteDef {
        waypoints: vec![
            wp(1400, false), // mid
            wp(1800, true),  // dock B
            wp(1400, false), // mid
            wp(1000, true),  // dock A (start)
        ],
        schedules: vec![],
    };

    let (mut world, _db, _l) = quest_test_world();
    let route = world.boat_routes.register(ride_route);
    let boat = crate::game_loop::space::boats::spawn_boat(&mut world, route);
    let _rx = ingame_player(&mut world, 1, 3001, 1000, 1000, -3600); // standing on dock A

    let ppos = |w: &World| -> (i32, i32) {
        let p = w.objects.get_component::<Position>(&3001).unwrap();
        (p.x, p.y)
    };

    // Board the anchored ferry.
    crate::game_loop::space::boats::board(&mut world, 3001, boat, (0, 0, 0));
    assert!(
        world
            .objects
            .get_component::<InVehicle>(&3001)
            .is_some_and(|v| v.boat_object_id == boat),
        "boarded the ferry"
    );
    assert_eq!(ppos(&world), (1000, 1000), "snapped to the boat");

    // Weigh anchor (skip the dwell) and sail to dock B (two 400-unit legs at
    // speed 200 ≈ 40 ticks).
    crate::game_loop::space::boats::depart(&mut world, boat);
    advance_ticks(&mut world, 45);
    assert_eq!(ppos(&world), (1800, 1000), "the passenger rode to dock B");

    // Step off onto the far dock.
    crate::game_loop::space::boats::disembark(&mut world, 3001, boat, (1810, 1000, -3600));
    assert!(
        world.objects.get_component::<InVehicle>(&3001).is_none(),
        "disembarked"
    );
    assert_eq!(ppos(&world), (1810, 1000), "stepped onto the far dock");
}

/// `RequestQuestList` (0x62, G33): opening the quest journal resends `QuestList`.
#[test]
fn request_quest_list_resends_the_journal() {
    const QUEST_LIST_OPCODE: u8 = 0x86;
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    world
        .objects
        .add_components(&5001, model::components::social::Quests(Default::default()));
    drain(&mut rx);

    on_packet(&mut world, 1, vec![cop::REQUEST_QUEST_LIST]);

    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p.first() == Some(&QUEST_LIST_OPCODE)),
        "QuestList resent on journal open"
    );
}

// ---------------------------------------------------------------------------
// Q255 Tutorial (the newbie starting quest)
// ---------------------------------------------------------------------------

/// `getRandomPartyMember`: the kill credit can land on a party mate — a
/// killer who never took the quest still feeds a started cond-1 member in
/// range, and a member parked across the map gets nothing.
#[test]
fn quest_kill_credit_reaches_a_party_member() {
    use crate::game_loop::npc;
    use crate::model::components::social::PartyRef;

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(963, "Orcish Arrowhead", true)]);
    let mut t = crate::data::npc_data::default_template(20361);
    t.type_name = "Monster".into();
    t.level = 11;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30029, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0); // the killer, questless
    let _rx2 = ingame_player(&mut world, 2, 3005, 40, 0, 0); // the collector
    world
        .objects
        .get_component_mut::<Player>(&3005)
        .unwrap()
        .level = 10;
    drain_db(&mut db_rx);
    drain(&mut rx);

    // The collector accepts the quest.
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00303_CollectArrowheads")),
    );
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00303_CollectArrowheads 30029-04.htm"
        )),
    );
    assert_eq!(
        quest_cond(&world, 3005, "Q00303_CollectArrowheads"),
        Some(1)
    );

    // Party them up.
    let party_id = world.next_party_id;
    world.next_party_id += 1;
    let seq = world.next_request_seq();
    world.parties.insert(
        party_id,
        model::party::Party::new(3001, LootRule::FindersKeepers, seq),
    );
    world.objects.add_components(&3001, PartyRef(party_id));
    crate::game_loop::party::add_party_member(&mut world, party_id, 3005);

    // The questless killer fells the marksman: the pick (roll 0 over one
    // candidate) and the 40% drop roll both forced.
    let mob = NPC_OID + 40;
    add_test_npc(&mut world, mob, 20361, "Monster", 11, 30, 0, 0);
    world.force_rolls([0, 0]);
    npc::npc_do_die(&mut world, mob, 3001);
    assert_eq!(
        item_count(&world, 3005, 963),
        1,
        "the arrowhead landed on the started party mate"
    );
    assert_eq!(item_count(&world, 3001, 963), 0, "not on the killer");

    // Across the map (past AltPartyRange 1500), the mate collects nothing.
    world
        .objects
        .get_component_mut::<Position>(&3005)
        .unwrap()
        .x = 99_999;
    add_test_npc(&mut world, mob + 1, 20361, "Monster", 11, 30, 0, 0);
    world.force_rolls([0, 0]);
    npc::npc_do_die(&mut world, mob + 1, 3001);
    assert_eq!(
        item_count(&world, 3005, 963),
        1,
        "an out-of-range mate collects nothing"
    );
}
