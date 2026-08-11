use super::*;

/// The queue slot is last-click-wins, both ways: a skill click supersedes a
/// queued move (Java: the `stopCasting` skill launch makes the new cast
/// forget `_nextIntention`), and a later move click wipes a queued skill
/// (Java `MoveBackwardToLocation.runImpl`'s "remove queued skill upon move
/// request").
#[test]
fn queued_action_slot_is_last_click_wins() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    world
        .objects
        .get_component_mut::<Speeds>(&3001)
        .unwrap()
        .run_spd = 100.0;
    world
        .objects
        .get_component_mut::<Speeds>(&3001)
        .unwrap()
        .running = true;

    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    drain(&mut a_rx);

    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (0, 0, 0), 1));
    assert!(matches!(
        world.objects.get_component::<QueuedAction>(&3001),
        Some(QueuedAction::Move { .. })
    ));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    assert!(matches!(
        world.objects.get_component::<QueuedAction>(&3001),
        Some(QueuedAction::Skill { skill_id: 1015, .. })
    ));
    handle_move_backward_to_location(&mut world, 1, &move_body((600, 0, 0), (0, 0, 0), 1));
    match world.objects.get_component::<QueuedAction>(&3001) {
        Some(&QueuedAction::Move { x, .. }) => {
            assert_eq!(x, 600, "move click wipes the queued skill")
        }
        other => panic!("expected the last move click in the slot: {other:?}"),
    }

    // Cast end: the last click (move) replays; no second cast starts.
    advance_ticks(&mut world, 45);
    assert!(!world.objects.has_component::<Casting>(&3001));
    let mv = world
        .objects
        .get_component::<Movement>(&3001)
        .expect("move started at cast end");
    assert_eq!((mv.0.dest_x, mv.0.dest_y), (600, 0));
}

/// `Action` selects a player target: the selector gets `MyTargetSelected`
/// + a `StatusUpdate` (target's HP) + the `ActionFailed` terminator; the
/// target itself gets `TargetSelected` (never `MyTargetSelected`). A
/// repeat click on the same target is a no-op (only `ActionFailed`).
/// `RequestTargetCanceld{target_lost:true}` clears it and broadcasts
/// `TargetUnselected` to everyone including the canceller (Java uses
/// includeSelf=true there; without it the client keeps its target).
#[test]
fn action_selects_switches_and_cancels_target() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);

    handle_action(&mut world, 1, &action_body(3002, 0));

    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(3002)
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MY_TARGET_SELECTED
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(a_rx.try_recv().is_err(), "no extra packets to the selector");

    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::TARGET_SELECTED
    );
    assert!(
        b_rx.try_recv().is_err(),
        "target never gets MyTargetSelected"
    );

    // Re-click the same target: no-op besides the ActionFailed terminator.
    handle_action(&mut world, 1, &action_body(3002, 0));
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(a_rx.try_recv().is_err());
    assert!(
        b_rx.try_recv().is_err(),
        "no TargetSelected rebroadcast on re-click"
    );

    // Cancel.
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(true));
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        None
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::TARGET_UNSELECTED,
        "canceller must receive TargetUnselected too"
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::TARGET_UNSELECTED
    );

    // Self-click: same select path as any other player target (Java
    // routes self-clicks through `PlayerAction` too).
    handle_action(&mut world, 1, &action_body(3001, 0));
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(3001)
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MY_TARGET_SELECTED
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::TARGET_SELECTED
    );
}

/// Entering the world sends `NpcInfo` for NPCs in the 3×3 region block and
/// nothing for NPCs beyond it (Java `addVisibleObject` over the region grid).
#[test]
fn enter_world_sends_npc_info_for_nearby_npcs_only() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 500, 0, 0);
    add_test_npc(&mut world, NPC_OID + 1, 30002, "Folk", 5, 5 * 2048, 0, 0); // 5 regions east
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    visibility::on_enter_world(&world, 1, 3001);

    let packets = drain(&mut rx);
    let npc_infos: Vec<_> = packets
        .iter()
        .filter(|p| p[0] == server_packets::opcodes::NPC_INFO)
        .collect();
    assert_eq!(npc_infos.len(), 1, "only the nearby NPC is described");
    let described = i32::from_le_bytes(npc_infos[0][1..5].try_into().unwrap());
    assert_eq!(described, NPC_OID);
}

/// Crossing a region boundary introduces NPCs entering the 3×3 block
/// (`NpcInfo`) and removes NPCs leaving it (`DeleteObject`), dropping a
/// dangling NPC target like Java's forget event does.
#[test]
fn region_cross_sends_npc_deltas_and_drops_npc_target() {
    let (mut world, ..) = test_world();
    // NPC in region (3, 0): visible from region (2, 0) but not (0, 0).
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 3 * 2048 + 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    // Step into region (2, 0): the NPC appears.
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 2 * 2048 + 10;
    visibility::update_region(&mut world, 3001);
    let packets = drain(&mut rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_INFO),
        "NpcInfo on entering visibility range"
    );

    // Target it, then step back to region (0, 0): the dangling target is
    // released with an explicit TargetUnselected *before* the DeleteObject
    // (Java `switchRegion` runs `setTarget(null)` first, and the self-directed
    // TargetUnselected is what clears this client's ground ring).
    world
        .objects
        .get_component_mut::<TargetRef>(&3001)
        .unwrap()
        .0 = Some(NPC_OID);
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 10;
    visibility::update_region(&mut world, 3001);
    let packets = drain(&mut rx);
    let unselect_at = packets
        .iter()
        .position(|p| p[0] == server_packets::opcodes::TARGET_UNSELECTED)
        .expect("TargetUnselected for the dropped target");
    let delete_at = packets
        .iter()
        .position(|p| p[0] == server_packets::opcodes::DELETE_OBJECT)
        .expect("DeleteObject for the NPC leaving range");
    assert!(
        unselect_at < delete_at,
        "TargetUnselected must precede DeleteObject"
    );
    assert_eq!(
        i32::from_le_bytes(packets[unselect_at][1..5].try_into().unwrap()),
        3001,
        "payload carries the deselecting player"
    );
    assert_eq!(
        i32::from_le_bytes(packets[delete_at][1..5].try_into().unwrap()),
        NPC_OID
    );
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        None,
        "dangling NPC target dropped"
    );

    // Walk back into range: the NPC re-enters via NpcInfo only — no selection
    // packets, and the target stays dropped (the walk-away-and-back ring bug).
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 2 * 2048 + 10;
    visibility::update_region(&mut world, 3001);
    let packets = drain(&mut rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_INFO),
        "NpcInfo on re-entering visibility range"
    );
    assert!(
        !packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::TARGET_UNSELECTED
                || p[0] == server_packets::opcodes::MY_TARGET_SELECTED),
        "coming back must not touch target state"
    );
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        None,
        "target stays dropped after returning"
    );
}

/// `Action` on an NPC: first click selects (`ValidateLocation` +
/// `MyTargetSelected` + HP `StatusUpdate` + `ActionFailed`); a second click
/// on a talkable non-monster in interaction range opens the chat window
/// (`NpcHtmlMessage`).
#[test]
fn action_on_npc_selects_then_second_click_opens_chat_window() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(NPC_OID)
    );
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::VALIDATE_LOCATION
    );
    let mts = rx.try_recv().unwrap();
    assert_eq!(mts[0], server_packets::opcodes::MY_TARGET_SELECTED);
    assert_eq!(
        i16::from_le_bytes(mts[9..11].try_into().unwrap()),
        0,
        "no level color on a Folk"
    );
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    );
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(rx.try_recv().is_err());

    // Second click within INTERACTION_DISTANCE: the dialog opens (the html
    // file itself is absent in the synthetic world, so the "text is missing"
    // stub is served — the packet flow is what's under test).
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::NPC_HTML_MESSAGE
    );
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(rx.try_recv().is_err());
}

/// With `AltGameViewNpc` on, a shift-click (`Action`, action_id 1) on an NPC
/// opens the `NpcViewMod` info window instead of interacting — Java `Action`
/// case 1 → `Npc.onActionShift` → `NpcActionShift`'s `ALT_GAME_VIEWNPC`
/// branch, which sets the target first, then sends the html.
#[test]
fn shift_click_npc_opens_view_window_when_alt_game_view_npc() {
    let (mut world, ..) = test_world();
    world.cfg.npc.alt_game_view_npc = true;
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    handle_action(&mut world, 1, &action_body(NPC_OID, 1));

    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(NPC_OID),
        "target set like NpcActionShift"
    );
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::MY_TARGET_SELECTED),
        "target selected"
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        "info window opened"
    );
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "the info window must not start an attack/interact"
    );
}

/// Without `AltGameViewNpc` (the default), a shift-click on an NPC is just a
/// plain select (Java `onAction(player, false)`) — no info window.
#[test]
fn shift_click_npc_without_alt_game_view_npc_only_selects() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    handle_action(&mut world, 1, &action_body(NPC_OID, 1));

    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(NPC_OID)
    );
    assert!(
        !drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        "no info window without the config flag"
    );
}

/// A **GM**'s shift-click opens the admin `npcinfo.htm` window, not the player
/// view and not a plain select: Java `Action` case 1 sends every GM to
/// `Npc.onActionShift` → `NpcActionShift`'s `isGM()` branch, which is checked
/// *before* `ALT_GAME_VIEWNPC` — so the config is irrelevant here (it is left
/// at its default off to prove that).
#[test]
fn gm_shift_click_npc_opens_admin_npc_info_window() {
    let (mut world, ..) = admin_world();
    // The real npcinfo.htm, so the placeholder substitution is under test too.
    world.data.root = crate::data::DIST_GAME.to_string();
    assert!(!world.cfg.npc.alt_game_view_npc, "GM path needs no config");
    add_test_npc(&mut world, NPC_OID, 30001, "Monster", 5, 100, 0, 0);
    let mut rx = ingame_player_access(&mut world, 1, 3001, 70);

    handle_action(&mut world, 1, &action_body(NPC_OID, 1));

    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(NPC_OID),
        "NpcActionShift sets the target before sending the html"
    );
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "the admin window must not start an attack/interact"
    );
    let pkts = drain(&mut rx);
    let html = pkts
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("admin npc info window");
    assert!(
        html.contains("<title>NPC Info</title>"),
        "npcinfo.htm served"
    );
    assert!(
        html.contains(&format!("bypass admin_show_quests {}", 30001)),
        "template id substituted into the Quests button"
    );
    assert!(html.contains(&NPC_OID.to_string()), "object id substituted");
    // Every `%token%` is substituted — a leftover would show raw in the client.
    // (`width="100%"` is a literal in the file, hence the "% followed by a
    // letter" shape rather than a bare '%' scan.)
    let leftover: Vec<&str> = html
        .match_indices('%')
        .filter(|(i, _)| html[i + 1..].starts_with(|c: char| c.is_ascii_alphabetic()))
        .map(|(i, _)| &html[i..(i + 20).min(html.len())])
        .collect();
    assert!(
        leftover.is_empty(),
        "unsubstituted placeholders: {leftover:?}"
    );
}

/// `%spawnfile%` names the XML the NPC was placed from — Java prints
/// `SpawnTemplate.getFile()` relative to `data/spawns/`, which is how a GM
/// finds the line to edit. An NPC with no spawn line behind it (minions,
/// `//spawn`, quest spawns) keeps Java's red `--`.
#[test]
fn gm_npc_info_names_the_spawn_file() {
    use crate::data::spawn_data::{NpcSpawnDef, SpawnGroup, SpawnTemplate};

    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    // Index 0 is the `spawn_ref` sentinel a runtime spawn carries, so the line
    // under test has to sit past it for the "unreferenced" half to mean
    // anything — an unrelated template holds that slot.
    world.data.spawn_data.spawns.push(SpawnTemplate {
        file: "Other/Filler.xml".to_string(),
        name: None,
        ai: None,
        parameters: Default::default(),
        territories: Vec::new(),
        groups: Vec::new(),
    });
    let template_idx = world.data.spawn_data.spawns.len();
    world.data.spawn_data.spawns.push(SpawnTemplate {
        file: "Giran/Giran.xml".to_string(),
        name: Some("Giran".to_string()),
        ai: None,
        parameters: Default::default(),
        territories: Vec::new(),
        groups: vec![SpawnGroup {
            name: Some("dayTime".to_string()),
            spawn_by_default: true,
            territories: Vec::new(),
            npcs: vec![NpcSpawnDef {
                npc_id: 30001,
                count: 1,
                loc: None,
                respawn_secs: 60,
                respawn_random_secs: 0,
                chase_range: 0,
                db_save: false,
            }],
        }],
    });
    add_test_npc(&mut world, NPC_OID, 30001, "Monster", 5, 100, 0, 0);
    let mut rx = ingame_player_access(&mut world, 1, 3001, 70);

    // A runtime spawn (the default `(0, 0, 0)` reference) shows the placeholder.
    handle_action(&mut world, 1, &action_body(NPC_OID, 1));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("admin npc info window");
    assert!(
        !html.contains("Giran/Giran.xml"),
        "an unreferenced npc must not borrow another line's file"
    );

    // Point the NPC at the spawn line above: now the file, name and group all
    // resolve off that template.
    world
        .objects
        .get_component_mut::<crate::model::npc::Npc>(&NPC_OID)
        .unwrap()
        .spawn_ref = (template_idx, 0, 0);
    handle_action(&mut world, 1, &action_body(NPC_OID, 1));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("admin npc info window");
    assert!(
        html.contains("Giran/Giran.xml"),
        "spawn file path shown in the GM window"
    );
    assert!(html.contains("dayTime"), "spawn group still resolves");
}

/// A non-GM never reaches the admin window even with `AltGameViewNpc` on —
/// Java's `NpcActionShift` gates it on `isGM()` and falls through to the
/// player-facing `NpcViewMod` view.
#[test]
fn non_gm_shift_click_gets_player_view_not_admin_window() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    world.cfg.npc.alt_game_view_npc = true;
    add_test_npc(&mut world, NPC_OID, 30001, "Monster", 5, 100, 0, 0);
    let mut rx = ingame_player_access(&mut world, 1, 3001, 0);

    handle_action(&mut world, 1, &action_body(NPC_OID, 1));

    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("player info window");
    assert!(
        !html.contains("<title>NPC Info</title>"),
        "admin window is GM-only"
    );
}

/// The admin window's `Skills` button (`bypass NpcViewMod skills <objId>`) —
/// Java `sendNpcSkillView`: one icon/name/id/level row per template skill,
/// through `Skills.htm`. The verb was unrouted, so the button did nothing.
#[test]
fn npc_view_skills_page_lists_the_npcs_skills() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: 4045,
            level: 1,
            name: "Resist Shock".into(),
            icon: "icon.skill4045".into(),
            ..Default::default()
        });
    let mut t = crate::data::npc_data::default_template(30001);
    t.name = "Test Mob".into();
    t.type_name = "Monster".into();
    t.skill_list = vec![(4045, 1), (9999, 7)]; // the second id has no template
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30001, "Monster", 5, 100, 0, 0);
    let mut rx = ingame_player_access(&mut world, 1, 3001, 70);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("NpcViewMod skills {NPC_OID}")),
    );

    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("skill view sent");
    assert!(html.contains("NPC Skill List"), "Skills.htm served");
    assert!(html.contains("Test Mob"), "%npc_name% substituted");
    assert!(
        html.contains("icon.skill4045") && html.contains("Resist Shock"),
        "the skill's real icon and name are rendered, got: {html}"
    );
    assert!(
        !html.contains("9999"),
        "an id with no skill template is skipped, like Java's null addSkill"
    );
}

/// The admin window's `AggroList` button (`bypass NpcViewMod aggrolist
/// <objId>`) — Java `sendAggroListView`: attacker name / hate / damage per
/// aggro entry. Also unrouted before.
#[test]
fn npc_view_aggro_page_lists_hate_and_damage() {
    use crate::model::npc::{AggroInfo, AggroList};

    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    add_test_npc(&mut world, NPC_OID, 30001, "Monster", 5, 100, 0, 0);
    let mut rx = ingame_player_access(&mut world, 1, 3001, 70);
    let mut list = AggroList(std::collections::HashMap::new());
    list.0.insert(
        3001,
        AggroInfo {
            hate: 123.0,
            damage: 456.0,
        },
    );
    // A stale attacker id (logged out / despawned) is Java's "NULL" row.
    list.0.insert(
        999_999,
        AggroInfo {
            hate: 7.0,
            damage: 0.0,
        },
    );
    world.objects.add_components(&NPC_OID, list);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("NpcViewMod aggrolist {NPC_OID}")),
    );

    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("aggro view sent");
    assert!(html.contains("NPC Aggro List"), "AggroList.htm served");
    assert!(html.contains("P3001"), "the attacking player is named");
    assert!(
        html.contains("123") && html.contains("456"),
        "hate + damage"
    );
    assert!(html.contains("NULL"), "a vanished attacker renders as NULL");
    // The Refresh button re-issues the bypass against the object id.
    assert!(
        html.contains(&format!("NpcViewMod aggrolist {NPC_OID}")),
        "%objid% is the object id, not the template id"
    );
}

/// `view`/`skills`/`aggrolist` take an **optional** object id and fall back to
/// the player's current target (Java's shared `st.hasMoreElements()` prologue);
/// only `dropList` demands its arguments.
#[test]
fn npc_view_verbs_fall_back_to_the_current_target() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    add_test_npc(&mut world, NPC_OID, 30001, "Monster", 5, 100, 0, 0);
    let mut rx = ingame_player_access(&mut world, 1, 3001, 70);
    handle_action(&mut world, 1, &action_body(NPC_OID, 0)); // select it
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body("NpcViewMod skills"));

    assert!(
        drain(&mut rx)
            .iter()
            .filter_map(|p| decode_npc_html(p))
            .any(|h| h.contains("NPC Skill List")),
        "the argument-less form uses the current target"
    );
}

/// The drop page is a **community-board** html (Java `Util.sendCBHtml`), each
/// row carrying the item's own icon and a `DecimalFormat("0.00##")` chance.
/// All three were wrong: it went out as an `NpcHtmlMessage`, every row used
/// the question-mark placeholder icon, and chances were truncated to two
/// decimals (so a 0.0123% drop read "0.01").
#[test]
fn npc_view_drop_page_uses_board_channel_real_icons_and_java_chance_format() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    world.cfg.rates.death_drop_chance_multiplier = 1.0;
    world.cfg.rates.death_drop_amount_multiplier = 1.0;
    // The real item catalogue, so the rows carry real `<set name="icon">`
    // values (the synthetic test data has none).
    world.data.item_data = dist::items_owned();
    let mut t = crate::data::npc_data::default_template(30001);
    t.name = "Test Mob".into();
    t.type_name = "Monster".into();
    // Adena (57) has a real `<set name="icon">` in the dist item data.
    t.drop_list_death = vec![
        crate::data::npc_data::DropHolder {
            item_id: 57,
            min: 1,
            max: 1,
            chance: 0.0123,
        },
        crate::data::npc_data::DropHolder {
            item_id: 57,
            min: 10,
            max: 20,
            chance: 1.5,
        },
    ];
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30001, "Monster", 5, 100, 0, 0);
    let mut rx = ingame_player_access(&mut world, 1, 3001, 70);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("NpcViewMod dropList DROP {NPC_OID}")),
    );

    let pkts = drain(&mut rx);
    assert!(
        pkts.iter().all(|p| decode_npc_html(p).is_none()),
        "the drop list must not go out as an NPC dialog"
    );
    let board: Vec<_> = pkts
        .iter()
        .filter(|p| p[0] == server_packets::opcodes::SHOW_BOARD)
        .collect();
    assert_eq!(board.len(), 3, "sendCBHtml sends three ShowBoard chunks");
    let html = {
        let mut r = commons::network::PacketReader::new(&board[0][2..]);
        for _ in 0..8 {
            r.read_string().unwrap();
        }
        r.read_string().unwrap()
    };
    assert!(html.contains("Test Mob"), "%name% substituted");
    let adena_icon = world.data.item_data.icon(57).to_string();
    assert_ne!(adena_icon, "icon.etc_question_mark_i00", "dist has an icon");
    assert!(
        html.contains(&adena_icon),
        "the item's own icon is used, got: {html}"
    );
    assert!(
        html.contains("0.0123%"),
        "4-decimal chance kept (0.00## ), got: {html}"
    );
    assert!(
        html.contains("1.50%"),
        "trailing zeros trimmed only to 2 decimals, got: {html}"
    );
}

/// `Action` on a talkable NPC outside `INTERACTION_DISTANCE`: the second
/// click can't open the dialog immediately (`Npc.canInteract` fails), so the
/// player walks in first (`AI_INTENTION_INTERACT` / `Interact` intent) —
/// `MoveToPawn` goes out, then once movement ticks close the distance the
/// chat window opens on its own, with no further client click.
#[test]
fn action_on_far_npc_walks_in_then_opens_chat_window() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 2000, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Speeds>(&3001)
        .unwrap()
        .run_spd = 100.0;

    // First click: select (far away, selection itself isn't range-gated).
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    drain(&mut rx);

    // Second click: too far to talk (2000 > INTERACTION_DISTANCE=250) — walks
    // in instead of doing nothing.
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    let packets = drain(&mut rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "out-of-range talk click must start walking toward the NPC"
    );
    assert!(
        !packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        "no dialog yet — still far away"
    );
    assert!(matches!(
        world.objects.get_component::<Intent>(&3001).copied(),
        Some(Intent(crate::model::PlayerIntent::Interact { target_object_id })) if target_object_id == NPC_OID
    ));

    // Run the movement + combat-tick systems until the player arrives and
    // re-triggers the interact click on its own (Java: `EVT_ARRIVED` →
    // `thinkInteract` → `doInteract` re-dispatching `onAction`).
    advance_world(&mut world, 400);
    let packets = drain(&mut rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        "chat window must open once the walk-in arrives"
    );
    assert!(
        world.objects.get_component::<Intent>(&3001).is_none(),
        "interact intent consumed on arrival"
    );
}

/// Bypass plumbing (G11): `npc_<oid>_<verb>` parses, range-checks, and always
/// terminates with `ActionFailed`; malformed/empty/unknown commands drop
/// without a reply (and without a panic); clicking an NPC records it as
/// `LastFolkNpc`, which bare `Quest …` bypasses resolve through.
#[test]
fn bypass_routes_npc_commands_and_tracks_last_folk_npc() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    // Clicking the NPC records it as the last folk NPC (`NpcAction.action`).
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    assert_eq!(
        world.objects.get_component::<LastFolkNpc>(&3001),
        Some(&LastFolkNpc(NPC_OID)),
        "NPC click must set LastFolkNpc"
    );
    drain(&mut rx);

    // `npc_`-prefixed command on an in-range NPC whose verb has no handler
    // (log-drop): the `ActionFailed` terminator still arrives — Java sends it
    // from the `npc_` branch regardless of the outcome.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_NoSuchVerb")),
    );
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(rx.try_recv().is_err());

    // A handled verb answers before that terminator: `Chat 0` sends the NPC's
    // dialog page (here the "text is missing" stub — this world has no html
    // root) ahead of the `ActionFailed`.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Chat 0")),
    );
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::NPC_HTML_MESSAGE
    );
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(rx.try_recv().is_err());

    // Malformed `npc_` forms never act but still terminate: missing command
    // tail, non-numeric id, unknown object id.
    for cmd in ["npc_12345", "npc_x_y", "npc_999_Chat 0"] {
        handle_request_bypass_to_server(&mut world, 1, &bypass_body(cmd));
        assert_eq!(
            rx.try_recv().unwrap()[0],
            server_packets::opcodes::ACTION_FAIL,
            "for {cmd}"
        );
        assert!(rx.try_recv().is_err(), "for {cmd}");
    }

    // Empty and unknown bare commands drop silently (deviation: Java
    // disconnects on empty; unhandled prefixes only log there too).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(""));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("no_such_command"));
    assert!(rx.try_recv().is_err());

    // Bare `Quest` with no LastFolkNpc (fresh player who never clicked an
    // NPC): dropped, no packets, no panic.
    let mut rx2 = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    handle_request_bypass_to_server(&mut world, 2, &bypass_body("Quest"));
    assert!(rx2.try_recv().is_err());
}

/// `bypasshandlers/ChatLink` end to end against the real datapack: talking to
/// Trader Lector (30001, a `Merchant`) shows `merchant/30001.htm`, and its
/// "I want to trade some equipment" button (`Chat 1`) walks to
/// `merchant/30001-1.htm` — the page carrying the `Buy` buttons. Before the
/// `Chat` verb was handled, that button silently dropped and the shop was
/// unreachable.
#[test]
fn chat_bypass_walks_merchant_dialog_pages() {
    let (mut world, ..) = test_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    add_test_npc(&mut world, NPC_OID, 30001, "Merchant", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    // Landing page (`Npc.showChatWindow(player, 0)`), via the NPC click.
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    let landing = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("landing page");
    assert!(
        landing.contains("Trader Lector"),
        "merchant/30001.htm: {landing}"
    );
    assert!(
        landing.contains(&format!("npc_{NPC_OID}_Chat 1")),
        "trade button carries the objectId"
    );

    // The trade button: page 1 is the buy-list menu.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Chat 1")),
    );
    let page1 = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("page 1");
    assert!(
        page1.contains("What would you like to trade?"),
        "merchant/30001-1.htm: {page1}"
    );
    assert!(
        page1.contains(&format!("npc_{NPC_OID}_Buy 3000101")),
        "warrior buy list reachable"
    );
}

/// Walking across a region boundary out of / back into an observer's 3×3
/// block sends `DeleteObject` / `CharInfo` (Java `World.switchRegion`), and a
/// newly visible mover is introduced mid-move (`describeStateToPlayer` →
/// `MoveToLocation`).
#[test]
fn region_crossing_exchanges_delete_object_and_char_info() {
    let (mut world, ..) = test_world();
    let mut mover_rx = ingame_player(&mut world, 1, 6201, 0, 0, 0);
    let mut watcher_rx = ingame_player(&mut world, 2, 6202, 3000, 0, 0); // region (1,0)
    world
        .objects
        .get_component_mut::<Speeds>(&6201)
        .unwrap()
        .run_spd = 500.0;
    world
        .objects
        .get_component_mut::<TargetRef>(&6202)
        .unwrap()
        .0 = Some(6201);

    // Walk west: region 0 → -1 → -2; (−1,0) is no longer adjacent to (1,0).
    handle_move_backward_to_location(&mut world, 1, &move_body((-2500, 0, 0), (0, 0, 0), 1));
    assert_eq!(
        mover_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MOVE_TO_LOCATION
    );
    assert_eq!(
        watcher_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MOVE_TO_LOCATION
    );
    for _ in 0..100 {
        world.tick += 1;
        visibility::movement_tick(&mut world);
    }
    assert!(
        !world.objects.has_component::<Movement>(&6201),
        "move must have finished"
    );

    // The first movement tick also fires the watcher's initial zone
    // revalidate (ExSetCompassZoneCode) — unrelated to visibility, drop it.
    let to_watcher: Vec<_> = drain(&mut watcher_rx)
        .into_iter()
        .filter(|p| p[0] != server_packets::opcodes::EX)
        .collect();
    assert_eq!(
        to_watcher.len(),
        2,
        "TargetUnselected then DeleteObject after the move; got opcodes {:02x?}",
        to_watcher.iter().map(|p| p[0]).collect::<Vec<_>>()
    );
    assert_eq!(
        to_watcher[0][0],
        server_packets::opcodes::TARGET_UNSELECTED,
        "ring released before the delete"
    );
    assert_eq!(delete_object_id(&to_watcher[1]), 6201);
    let to_mover: Vec<_> = drain(&mut mover_rx)
        .into_iter()
        .filter(|p| p[0] != server_packets::opcodes::EX)
        .collect();
    assert_eq!(delete_object_id(to_mover.last().unwrap()), 6202);
    assert_eq!(
        world.objects.get_component::<TargetRef>(&6202).unwrap().0,
        None,
        "dangling target dropped"
    );

    // Walk back east: crossing into region 0 re-enters the watcher's block —
    // CharInfo, then the in-flight move (describeStateToPlayer).
    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (-2500, 0, 0), 1));
    assert_eq!(
        mover_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MOVE_TO_LOCATION
    );
    for _ in 0..100 {
        world.tick += 1;
        visibility::movement_tick(&mut world);
    }
    // CharInfo, its paired RelationChanged (Java `sendInfo`), then the in-flight move.
    let to_watcher = drain(&mut watcher_rx);
    assert_eq!(to_watcher.len(), 3);
    assert_eq!(char_info_object_id(&to_watcher[0]), 6201);
    assert_eq!(to_watcher[1][0], server_packets::opcodes::RELATION_CHANGED);
    assert_eq!(to_watcher[2][0], server_packets::opcodes::MOVE_TO_LOCATION);
    let to_mover = drain(&mut mover_rx);
    assert_eq!(
        to_mover.len(),
        2,
        "watcher isn't moving → CharInfo + RelationChanged only"
    );
    assert_eq!(char_info_object_id(&to_mover[0]), 6202);
    assert_eq!(to_mover[1][0], server_packets::opcodes::RELATION_CHANGED);
}

/// A shift-click is not an attack. `Action` case 1 sends a plain player to
/// `obj.onAction(player, false)` — `NpcAction` with **interact false**, which
/// selects an unselected NPC and skips the whole `else if (interact)` arm where
/// attacking and talking live. On an *already selected* monster that leaves
/// nothing to do at all, so only the handler's trailing `ActionFailed` comes
/// back. (There is no melee `dontMove` in Java to refuse with, either.)
#[test]
fn shift_click_via_action_packet_is_not_an_attack_at_all() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 34;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 200, 0, 0, 5000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);

    // Select the far monster (plain click just targets it).
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    // Shift-click it (Action, action_id = 1): swallowed whole.
    handle_action(&mut world, 1, &action_body(npc_oid, 1));
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "onActionShift never sets an attack intention"
    );
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "and never walks"
    );
    let packets = drain(&mut a_rx);
    assert!(
        !packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE),
        "no system message either — there is no refusal to report, the click \
         simply does not reach the attack handler"
    );
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ACTION_FAIL),
        "just the handler's trailing ActionFailed"
    );

    // A plain click on the already-selected target does engage and chase.
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(crate::model::PlayerIntent::Attack { .. }))
        ),
        "a non-shift click engages (and will chase)"
    );
}

/// `EnableRandomEnchantEffect` (True on this dist): every NPC instance rolls
/// its own visual weapon-enchant level in Java's `Rnd.get(4, 21)` range at
/// spawn, so mobs' blades glow. With the config off the template's
/// `<equipment weaponEnchant>` shows instead. Java `Npc` ctor:
/// `_currentEnchant = Config.ENABLE_RANDOM_ENCHANT_EFFECT ? Rnd.get(4, 21)
/// : getTemplate().getWeaponEnchant()`.
#[test]
fn npc_weapon_glow_rolls_per_instance_when_enabled() {
    const GLOWY: i32 = 20001;
    let (mut world, _db, _l) = combat_test_world();
    {
        let mut t = crate::data::npc_data::default_template(GLOWY);
        t.type_name = "Monster".into();
        t.rhand = 2; // a weapon to glow on
        t.weapon_enchant = 3;
        world.data.npc_data.insert_for_test(t);
    }

    // Config off → the template value, verbatim.
    world.cfg.npc.enable_random_enchant_effect = false;
    let plain = crate::model::npc::spawn_npc_at(&mut world, GLOWY, 0, 0, 0, 0).expect("spawned");
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&plain)
            .unwrap()
            .enchant_effect,
        3,
        "config off falls back to <equipment weaponEnchant>"
    );

    // Config on → a per-instance roll inside Java's inclusive 4..=21.
    world.cfg.npc.enable_random_enchant_effect = true;
    let mut seen = std::collections::HashSet::new();
    for _ in 0..60 {
        let oid = crate::model::npc::spawn_npc_at(&mut world, GLOWY, 0, 0, 0, 0).expect("spawned");
        let e = world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .unwrap()
            .enchant_effect;
        assert!(
            (4..=21).contains(&e),
            "roll {e} outside Java's Rnd.get(4, 21)"
        );
        seen.insert(e);
    }
    // Rolled per instance, not once per template: 60 draws over 18 values
    // practically never collapse to one (p < 18 * (1/18)^59).
    assert!(seen.len() > 1, "each instance rolls its own glow");
}
