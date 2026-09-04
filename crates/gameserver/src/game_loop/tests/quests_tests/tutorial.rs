//! The newbie tutorial, the newbie guide's chat window, and the NPC radar
//! the guide points at.

use super::*;

/// Register a Newbie Guide (30598, Talking Island / Human) as a live NPC,
/// with the `<race>HUMAN</race>` its dist template declares.
fn add_newbie_guide(world: &mut World) {
    let mut t = crate::data::npc_data::default_template(30598);
    t.type_name = "Folk".into();
    t.name = "Newbie Guide".into();
    t.race = Some(0); // HUMAN
    world.data.npc_data.insert_for_test(t);
    add_test_npc(world, NPC_OID, 30598, "Folk", 70, 0, 0, 0);
}

/// `NewbieGuide.onFirstTalk`: an `addFirstTalkId` script owns the whole chat
/// window. Without the first-talk route the guide has no
/// `data/html/default/30598.htm`, so it degrades to `npcdefault.htm`'s lone
/// "Quest" button — the four-entry menu below is the regression guard.
#[test]
fn newbie_guide_first_talk_replaces_the_default_chat_window() {
    let (mut world, ..) = quest_test_world();
    add_newbie_guide(&mut world);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    // First click targets, second interacts (Java `Player.doInteract`).
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("guide window");
    assert!(html.contains("Ask for an advice"), "advice entry: {html}");
    assert!(
        html.contains("Quest NpcLocationInfo"),
        "npc-location entry: {html}"
    );
    assert!(
        html.contains("Link default/SupportMagic.htm"),
        "support-magic entry: {html}"
    );
    assert!(
        html.contains("action=\"bypass -h Quest\">Quest"),
        "quest entry: {html}"
    );
    assert!(
        !html.contains("I have nothing to say"),
        "not the npcdefault fallback: {html}"
    );
}

/// The race gate: a guide only advises its own race (`npc.getRace() !=
/// player.getRace()` → `-no.htm`).
#[test]
fn newbie_guide_turns_away_other_races() {
    let (mut world, ..) = quest_test_world();
    add_newbie_guide(&mut world);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .race = 1; // ELF
    drain(&mut rx);

    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("refusal window");
    assert!(!html.contains("Ask for an advice"), "menu withheld: {html}");
}

/// The advice pages: `Quest NewbieGuide <n>` picks `<npcId>-<n><m|f>.htm`,
/// `f` for the fighter class this test's player carries. Event `0` returns
/// to the menu.
#[test]
fn newbie_guide_advice_pages_follow_the_class_suffix() {
    let (mut world, ..) = quest_test_world();
    add_newbie_guide(&mut world);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.add_components(&3001, LastFolkNpc(NPC_OID));
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest NewbieGuide 1"));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("advice page");
    assert!(
        html.contains("What should I do now?"),
        "30598-1f.htm: {html}"
    );

    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest NewbieGuide 0"));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("menu");
    assert!(
        html.contains("Ask for an advice"),
        "back to the menu: {html}"
    );
}

/// `NpcLocationInfo`: the bare bypass opens the profession list, a page name
/// navigates, and a whitelisted npc id drops a radar marker on its spawn.
#[test]
fn npc_location_info_marks_the_requested_npc_on_the_radar() {
    let (mut world, ..) = quest_test_world();
    add_newbie_guide(&mut world);
    // Gatekeeper Roxxy — a whitelisted target, spawned so the lookup lands.
    add_test_npc(
        &mut world,
        NPC_OID + 1,
        30006,
        "Teleporter",
        70,
        500,
        600,
        700,
    );
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.add_components(&3001, LastFolkNpc(NPC_OID));
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest NpcLocationInfo"));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("profession list");
    assert!(html.contains("Teleporter"), "30598.htm: {html}");

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("Quest NpcLocationInfo 30598-1.htm"),
    );
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("teleporter page");
    assert!(html.contains("Gatekeeper Roxxy"), "30598-1.htm: {html}");

    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest NpcLocationInfo 30006"));
    let pkts = drain(&mut rx);
    let html = pkts
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("MoveToLoc page");
    assert!(
        html.contains("direction of the arrow"),
        "MoveToLoc.htm: {html}"
    );
    // `Radar.addMarker` sends a *pair*: `RadarControl(2, 2, …)` clears whatever
    // marker already stands at the spot, then `(0, 1, …)` shows the new one.
    // This assertion used to be a bare "some RadarControl arrived", which is
    // why it kept passing while `add_radar` sent only the second leg — the
    // community-board path sent both and the quest path did not. Assert the
    // shape, not its presence.
    let radars: Vec<_> = pkts.iter().filter(|p| p[0] == 0xF1).collect();
    assert_eq!(radars.len(), 2, "addMarker sends two RadarControl packets");
    let legs: Vec<(i32, i32, i32, i32, i32)> = radars
        .iter()
        .map(|p| {
            let mut r = commons::network::PacketReader::new(&p[1..]);
            (
                r.read_i32().unwrap(),
                r.read_i32().unwrap(),
                r.read_i32().unwrap(),
                r.read_i32().unwrap(),
                r.read_i32().unwrap(),
            )
        })
        .collect();
    assert_eq!(legs[0], (2, 2, 500, 600, 700), "clear leg, at the spawn");
    assert_eq!(legs[1], (0, 1, 500, 600, 700), "show leg, at the spawn");

    // Off-whitelist id: Java returns null, so nothing is sent.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest NpcLocationInfo 99999"));
    assert!(drain(&mut rx).is_empty(), "no window for an unlisted npc");
}

const TUTORIAL: &str = "Q00255_Tutorial";

const BLUE_GEM: i32 = 6353;

fn tutorial_memo(world: &World, player: i32) -> i32 {
    world
        .objects
        .get_component::<model::components::social::Quests>(&player)
        .and_then(|q| q.0.get(TUTORIAL))
        .map(|qs| qs.get_int("memoState"))
        .unwrap_or(-1)
}

/// Login queues the 5 s intro timer; firing it starts the quest (cond 1,
/// memoState 1) and opens the tutorial window with the class voice line. A
/// player past level 6 gets nothing.
#[test]
fn tutorial_starts_on_newbie_login() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    quests::notify_login(&mut world, 1, 3001);
    assert!(
        drain(&mut rx).is_empty(),
        "nothing before the 5 s timer fires"
    );
    advance_ticks(&mut world, 51);
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::TUTORIAL_SHOW_HTML),
        "tutorial window opens"
    );
    let voice = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::PLAY_SOUND && p[1] == 2)
        .expect("tutorial voice line (PlaySound type 2)");
    // UTF-16LE "tutorial_voice_001a" (class 0) starts at offset 5.
    let name: Vec<u8> = voice[5..43].to_vec();
    let text: String = name.chunks(2).map(|c| char::from(c[0])).collect();
    assert_eq!(text, "tutorial_voice_001a");
    assert_eq!(quest_cond(&world, 3001, TUTORIAL), Some(1));
    assert_eq!(tutorial_memo(&world, 3001), 1);

    // A second login while memoState < 4 re-plays the intro (Java parity).
    quests::notify_login(&mut world, 1, 3001);
    advance_ticks(&mut world, 51);
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::TUTORIAL_SHOW_HTML)
    );

    // Outgrowing the tutorial: level 7+ logins queue nothing.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 7;
    quests::notify_login(&mut world, 1, 3001);
    advance_ticks(&mut world, 51);
    assert!(drain(&mut rx).is_empty(), "level gate");
}

/// The tutorial-window buttons arrive as `RequestTutorialPassCmdToServer`
/// bypasses; question mark 1 + the mark click show the radar hint.
#[test]
fn tutorial_window_buttons_and_question_mark() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    quests::notify_login(&mut world, 1, 3001);
    advance_ticks(&mut world, 51);
    drain(&mut rx);

    // "Next page" button.
    let mut body = PacketWriter::new();
    body.write_string("Quest Q00255_Tutorial tutorial_02.html");
    on_packet(
        &mut world,
        1,
        [
            vec![cp::opcodes::REQUEST_TUTORIAL_PASS_CMD_TO_SERVER],
            body.into_bytes(),
        ]
        .concat(),
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::TUTORIAL_SHOW_HTML)
    );

    // The "close and show the question mark" button.
    let mut body = PacketWriter::new();
    body.write_string("Quest Q00255_Tutorial question_mark_1");
    on_packet(
        &mut world,
        1,
        [
            vec![cp::opcodes::REQUEST_TUTORIAL_PASS_CMD_TO_SERVER],
            body.into_bytes(),
        ]
        .concat(),
    );
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::TUTORIAL_SHOW_QUESTION_MARK)
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::TUTORIAL_CLOSE_HTML)
    );

    // Clicking the shown mark: screen message + radar + tutorial_04.
    let mut body = PacketWriter::new();
    body.write_u8(0);
    body.write_i32(1);
    on_packet(
        &mut world,
        1,
        [
            vec![cp::opcodes::REQUEST_TUTORIAL_QUESTION_MARK],
            body.into_bytes(),
        ]
        .concat(),
    );
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::RADAR_CONTROL)
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::TUTORIAL_SHOW_HTML)
    );
}

/// memoState 2: a gremlin kill rolls the Blue Gemstone onto the ground;
/// picking it up advances to memoState 3 with question mark 5.
#[test]
fn tutorial_gremlin_gem_drop_and_pickup() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    world.id_pool = 0x2200_0000..0x2200_1000;
    add_quest_items(
        &mut world,
        &[
            (BLUE_GEM, "Blue Gemstone", true),
            (5789, "Soulshot: No Grade", false),
        ],
    );
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    quests::notify_login(&mut world, 1, 3001);
    advance_ticks(&mut world, 51);
    drain(&mut rx);

    // Helper first-talk moves 1 → 2 (first click targets, second talks).
    add_test_npc(&mut world, NPC_OID, 30009, "Folk", 5, 100, 0, 0);
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::TUTORIAL_CLOSE_HTML),
        "helper closes the tutorial window"
    );
    assert_eq!(tutorial_memo(&world, 3001), 2);

    // Gremlin kill with a forced sub-30 roll: the gem hits the ground.
    add_test_npc(&mut world, 9200, 20001, "Monster", 5, 10, 0, 0);
    world.force_roll(0);
    quests::notify_kill(&mut world, 3001, 9200, 20001, false);
    let gem_oid = world
        .ground_item_regions
        .values()
        .flat_map(|v| v.iter().copied())
        .find(|oid| {
            world
                .objects
                .get_component::<model::components::commerce::GroundItem>(oid)
                .is_some_and(|g| g.item_id == BLUE_GEM)
        })
        .expect("gem dropped");
    drain(&mut rx);

    // Pickup: memoState 3 + question mark 5 + the voice line.
    ground_items::pickup_ground_item(&mut world, 1, 3001, gem_oid);
    let pkts = drain(&mut rx);
    assert_eq!(tutorial_memo(&world, 3001), 3);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::TUTORIAL_SHOW_QUESTION_MARK)
    );

    // Turn-in: helper takes the gem, hands out 200 soulshots, memoState 4.
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    drain(&mut rx);
    assert_eq!(tutorial_memo(&world, 3001), 4);
    assert_eq!(count_of_item(&world, 3001, BLUE_GEM), 0, "gem taken");
    assert_eq!(count_of_item(&world, 3001, 5789), 200, "soulshots given");
}
