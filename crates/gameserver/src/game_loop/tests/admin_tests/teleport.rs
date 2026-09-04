//! `admin/teleport.rs` — the teleport commands and the click-to-move latch.

use super::*;

/// `//teleport x y z` moves the GM to those coordinates and broadcasts a
/// TeleportToLocation.
#[test]
fn admin_teleport_moves_gm_to_coords() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7104, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("teleport 100 200 300"));
    let pos = *world.objects.get_component::<Position>(&7104).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (100, 200, 305),
        "moved to coords (z lifted by 5)"
    );
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
        "TeleportToLocation broadcast"
    );
}

/// `//recall <name>` brings the named online player to the GM's location.
#[test]
fn admin_recall_brings_player_to_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7105, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 7106, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);

    set_position(&mut world, 7105, (500, 600, 700));
    on_packet(&mut world, 1, build_admin("recall P7106"));
    let pos = *world.objects.get_component::<Position>(&7106).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (500, 600, 705),
        "recalled to GM position + 5 collision adjustment"
    );
}

/// The Character panel's "Go To" button (`admin_goto_char_menu <name>`) sends
/// the GM to the character already picked on the previous page — it must resolve
/// the name argument (Java `World.getPlayer(command.substring(21))`) and never
/// demand a live target, which is what the `//teleto` alias used to do.
#[test]
fn admin_goto_char_menu_uses_the_named_character_not_the_target() {
    use model::components::space::Position;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7305, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 7306, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);

    set_position(&mut world, 7306, (1500, 1600, 1700));
    // Nothing selected on the GM: the button follows the name, not a target.
    assert!(
        world
            .objects
            .get_component::<TargetRef>(&7305)
            .is_none_or(|t| t.0.is_none()),
        "GM has no target selected"
    );
    on_packet(&mut world, 1, build_admin("goto_char_menu P7306"));
    let pos = *world.objects.get_component::<Position>(&7305).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (1500, 1600, 1705),
        "GM teleported to the named character (+5 collision adjustment)"
    );

    // A stale target must not win over the name argument either.
    set_position(&mut world, 7306, (2500, 2600, 2700));
    world.objects.add_components(&7305, TargetRef(Some(7305)));
    on_packet(&mut world, 1, build_admin("goto_char_menu P7306"));
    let pos = *world.objects.get_component::<Position>(&7305).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (2500, 2600, 2705),
        "the name argument beats the GM's own selection"
    );
}

/// `//gonorth <offset>` moves the GM north (-y) by the offset.
#[test]
fn admin_gonorth_moves_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8909, 100);
    drain(&mut gm_rx);
    let y0 = world.objects.get_component::<Position>(&8909).unwrap().y;
    on_packet(&mut world, 1, build_admin("gonorth 200"));
    assert_eq!(
        world.objects.get_component::<Position>(&8909).unwrap().y,
        y0 - 200
    );
}

/// The "Additional Movement Options" button on `teleports.htm` fires
/// `bypass -h admin_tele`, which Java answers with `showTeleportWindow` →
/// `html/admin/move.htm`: the nudge pad, the click-to-move mode row, the GM
/// speed row and the tele/walk box. The port used to alias `admin_tele` onto
/// the *coordinate* teleport, so the button answered "Usage: //teleport <x> <y>
/// <z>" and the window never opened.
#[test]
fn admin_tele_opens_the_additional_movement_options_window() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8920, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("tele"));
    let out = drain(&mut gm_rx);
    let html = last_admin_html(&out).expect("a page came back");
    assert!(
        html.contains("Teleport Menu") && html.contains("admin_instant_move"),
        "move.htm, not teleports.htm: {html}"
    );
    // The old aliasing answered with the usage line instead of a page.
    assert_eq!(
        count_system_messages(&out),
        0,
        "the button opens a window, it does not complain about coordinates"
    );
}

/// The "Move:" row of `move.htm` arms `Player.setTeleMode(...)`; the click that
/// follows is consumed by `MoveBackwardToLocation`. Each of the three armed
/// modes announces itself with Java's exact line; "Normal mode" (`//teleto
/// end`) disarms silently.
#[test]
fn teleto_mode_words_arm_the_click_to_move_latch() {
    use crate::enums::AdminTeleportType;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8921, 100);
    drain(&mut gm_rx);
    let mode = |w: &World| w.objects.get_component::<Player>(&8921).unwrap().tele_mode;

    assert_eq!(mode(&world), AdminTeleportType::Normal, "off by default");

    on_packet(&mut world, 1, build_admin("instant_move"));
    assert_eq!(mode(&world), AdminTeleportType::Demonic);
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "ready line");

    on_packet(&mut world, 1, build_admin("teleto sayune"));
    assert_eq!(mode(&world), AdminTeleportType::Sayune);
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1);

    on_packet(&mut world, 1, build_admin("teleto charge"));
    assert_eq!(mode(&world), AdminTeleportType::Charge);
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1);

    on_packet(&mut world, 1, build_admin("teleto end"));
    assert_eq!(mode(&world), AdminTeleportType::Normal, "disarmed");
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        0,
        "Java's `admin_teleto end` arm sends no line"
    );
}

/// A bare `//teleto` keeps its teleport-to-target meaning — only the three mode
/// words are latches, so the alias the char-management pages use is not
/// swallowed by the new arm.
#[test]
fn bare_teleto_still_teleports_to_the_target() {
    use crate::enums::AdminTeleportType;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8922, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 8923, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);
    set_position(&mut world, 8923, (2500, 2600, 2700));
    world.objects.add_components(&8922, TargetRef(Some(8923)));

    on_packet(&mut world, 1, build_admin("teleto"));
    let pos = *world.objects.get_component::<Position>(&8922).unwrap();
    assert_eq!((pos.x, pos.y), (2500, 2600), "teleported to the target");
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8922)
            .unwrap()
            .tele_mode,
        AdminTeleportType::Normal,
        "no latch armed"
    );
}

/// `//walk <x> <y> <z>` — the "Walk" button beside "Tele" on `move.htm`. Java
/// sets `AI_INTENTION_MOVE_TO`, so the GM *walks* there; the port used to alias
/// it onto the coordinate teleport, which made the two buttons identical.
#[test]
fn admin_walk_walks_instead_of_teleporting() {
    use model::components::space::Movement;
    use model::components::stats::Speeds;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8924, 100);
    {
        let speeds = world.objects.get_component_mut::<Speeds>(&8924).unwrap();
        speeds.run_spd = 120.0;
        speeds.running = true;
    }
    set_position(&mut world, 8924, (1000, 1000, 0));
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("walk 1300 1000 0"));

    assert!(
        world.objects.has_component::<Movement>(&8924),
        "a walk is in flight"
    );
    assert_eq!(
        world.objects.get_component::<Position>(&8924).unwrap().x,
        1000,
        "still at the start — it walks there, it does not jump"
    );
    let out = drain(&mut gm_rx);
    assert!(
        !out.iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
        "no teleport"
    );
    assert!(
        out.iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION),
        "a MoveToLocation instead"
    );
}

/// **The movement toggle draws the walk line.** Enabling while standing is
/// clean; once the GM walks, the beat sends the green destination line.
#[test]
fn debug_panel_movement_toggle_draws_walk_line() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7961, 100);
    {
        let speeds = world.objects.get_component_mut::<Speeds>(&7961).unwrap();
        speeds.run_spd = 100.0;
        speeds.running = true;
    }
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("debug movement on"));
    drain(&mut gm_rx);

    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (0, 0, 0), 1));
    // The test world doesn't interpolate movement — walk the position
    // forward by hand so the beat sees >15 units from its anchor.
    world
        .objects
        .get_component_mut::<Position>(&7961)
        .unwrap()
        .x += 100;
    advance_ticks(&mut world, 3);
    assert!(
        drain(&mut gm_rx).iter().any(|p| {
            p[0] == 0xFE && p.len() > 2 && i16::from_le_bytes(p[1..3].try_into().unwrap()) == 0x11
        }),
        "movement line drawn while walking"
    );
}

/// `AdminMenu.teleportToCharacter` reopens `charmanage.htm` on every path but
/// the unresolved target, which returns straight out. The self-target case is
/// the counterexample that stops the tail from being read as "on success only".
#[test]
fn goto_char_reopens_the_page_except_on_an_unresolved_target() {
    use model::components::combat::TargetRef;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7811, 100);
    drain(&mut gm_rx);

    // Nothing targeted → INVALID_TARGET and no page.
    on_packet(&mut world, 1, build_admin("goto_char_menu"));
    let pkts = drain(&mut gm_rx);
    assert!(
        has_system_message(&pkts, server_packets::sm_ids::INVALID_TARGET),
        "INVALID_TARGET"
    );
    assert!(!has_admin_html(&pkts), "no char-manage page");

    // Targeting yourself is refused by message, but the page still re-opens.
    world.objects.add_components(&7811, TargetRef(Some(7811)));
    on_packet(&mut world, 1, build_admin("goto_char_menu"));
    let pkts = drain(&mut gm_rx);
    assert!(
        has_system_message(
            &pkts,
            server_packets::sm_ids::YOU_CANNOT_USE_THIS_ON_YOURSELF
        ),
        "YOU_CANNOT_USE_THIS_ON_YOURSELF"
    );
    assert!(
        has_admin_html(&pkts),
        "the self-target refusal still re-opens charmanage.htm"
    );
}

/// `//teleportto <name>` sends the GM to a *named* player. Java's two guards:
/// an unknown name answers `INVALID_TARGET`, and your own name answers
/// `YOU_CANNOT_USE_THIS_ON_YOURSELF` — neither moves anybody.
#[test]
fn admin_teleportto_moves_the_gm_to_a_named_player() {
    use model::components::space::Position;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7120, 100);
    let _target_rx = ingame_player(&mut world, 2, 7121, 50_000, 60_000, -3000);
    {
        let p = world.objects.get_component_mut::<Player>(&7121).unwrap();
        p.name = "Wanda".into();
    }
    let gm_pos = |w: &World| {
        let p = w.objects.get_component::<Position>(&7120).unwrap();
        (p.x, p.y)
    };
    let start = gm_pos(&world);
    drain(&mut gm_rx);

    // The assertions below check system-message *ids*, not just a count: a
    // self-teleport is positionally invisible (you land where you already are),
    // so the refusal message is the only witness that the guard fired at all.

    // Unknown name: INVALID_TARGET, nobody moves.
    on_packet(&mut world, 1, build_admin("teleportto Nobody"));
    assert_eq!(
        ids_after_opcode(&drain(&mut gm_rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::INVALID_TARGET],
        "an unknown name answers INVALID_TARGET"
    );
    assert_eq!(gm_pos(&world), start, "and moves nothing");

    // Own name: refused with Java's own message, not the success line.
    let gm_name = world
        .objects
        .get_component::<Player>(&7120)
        .unwrap()
        .name
        .clone();
    on_packet(&mut world, 1, build_admin(&format!("teleportto {gm_name}")));
    assert_eq!(
        ids_after_opcode(&drain(&mut gm_rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::YOU_CANNOT_USE_THIS_ON_YOURSELF],
        "targeting yourself is refused — and refusing is only observable here, \
         since teleporting to yourself would not move you"
    );
    assert_eq!(gm_pos(&world), start, "still put");

    // A real name: the GM lands on them.
    on_packet(&mut world, 1, build_admin("teleportto Wanda"));
    assert_eq!(
        gm_pos(&world),
        (50_000, 60_000),
        "the GM is moved onto the named player"
    );
}

/// `//instancezone` draws the per-character reuse page. The table is always
/// empty here — no template on this dist ever writes a reuse time — so what
/// this pins is the page, the subject resolution and the clear's messaging.
#[test]
fn admin_instancezone_draws_the_reuse_page_for_a_named_player() {
    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 7403, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7404, 0);
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("instancezone P7404"));
    let page = last_admin_html(&drain(&mut rx)).expect("instance page");
    assert!(page.contains("Character Instances"));
    assert!(
        page.contains("Instances for P7404"),
        "the named player is the subject, not the GM"
    );

    // Offline / unknown name: two lines and no page.
    on_packet(&mut world, 1, build_admin("instancezone Nobody"));
    let pkts = drain(&mut rx);
    assert!(last_admin_html(&pkts).is_none());
    assert_eq!(count_system_messages(&pkts), 2, "not-online + usage");

    // The clear tells both sides and redraws the GM's page.
    drain(&mut victim_rx);
    on_packet(&mut world, 1, build_admin("instancezone_clear P7404 136"));
    assert!(
        count_system_messages(&drain(&mut victim_rx)) >= 1,
        "the player is told their reuse was cleared"
    );
    assert!(
        last_admin_html(&drain(&mut rx))
            .expect("page redrawn")
            .contains("Character Instances")
    );
}
