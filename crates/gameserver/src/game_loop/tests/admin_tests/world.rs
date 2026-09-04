//! `admin/world_cmds.rs`, `admin/instance.rs`, `admin/geo_editor.rs`,
//! `admin/debug_draw.rs`, `admin/pforge.rs` — server-wide commands, the
//! instance panel, the geo and debug overlays.

use super::*;

/// A GM's `//serverinfo` runs and answers with server-info text lines.
#[test]
fn admin_serverinfo_runs_for_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("serverinfo"),
        ]
        .concat(),
    );
    let pkts = drain(&mut gm_rx);
    assert_eq!(count_system_messages(&pkts), 3, "three server-info lines");
}

/// `//instancelist id=<t>` (G27) serves the real detail html with the live
/// instances of that template, each carrying teleport/destroy bypasses.
#[test]
fn admin_instance_detail_lists_live_instances() {
    use crate::data::instance_data::{ExitType, InstanceTemplate};
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();

    // A template with an empty default group (no NPC data needed) + a live copy.
    world
        .data
        .instance_templates
        .insert_for_test(InstanceTemplate {
            id: 900,
            name: Some("Test Arena".into()),
            max_worlds: -1,
            duration_min: 30,
            empty_destroy_min: 5,
            enter: Some((100, 200, 300)),
            exit: ExitType::Origin,
            doors: vec![],
            groups: vec![],
        });
    let iid = crate::game_loop::space::instances::create_from_template(&mut world, 900)
        .expect("template");

    let mut rx = ingame_player_access(&mut world, 1, 6440, 100);
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("instancelist id=900"),
        ]
        .concat(),
    );

    let pkts = drain(&mut rx);
    let html = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .expect("NpcHtmlMessage");
    let mut r = commons::network::PacketReader::new(&html[1..]);
    r.read_i32().unwrap();
    let content = r.read_string().unwrap();
    assert!(
        !content.contains("My text is missing"),
        "the real detail htm was served"
    );
    assert!(
        content.contains("Test Arena (900)"),
        "template name + id shown"
    );
    assert!(
        content.contains(&format!("admin_instanceteleport {iid}")),
        "a Teleport button targets the live instance"
    );
    assert!(
        content.contains(&format!("admin_instancedestroy {iid}")),
        "a Destroy button targets the live instance"
    );
}

/// `//instancecreate <t>` builds the instance and moves the GM into it (Alone).
#[test]
fn admin_instancecreate_enters_the_gm() {
    use crate::data::instance_data::{ExitType, InstanceTemplate};
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    world
        .data
        .instance_templates
        .insert_for_test(InstanceTemplate {
            id: 901,
            name: Some("Solo".into()),
            max_worlds: -1,
            duration_min: 0,
            empty_destroy_min: 0,
            enter: Some((1000, 2000, 300)),
            exit: ExitType::Origin,
            doors: vec![],
            groups: vec![],
        });
    let mut rx = ingame_player_access(&mut world, 1, 6441, 100);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("instancecreate 901"),
        ]
        .concat(),
    );

    let iid = crate::game_loop::helpers::instance_of(&world, 6441);
    assert!(iid >= 1, "the GM entered a freshly-created instance");
    assert_eq!(
        world.instances.member_count(iid),
        1,
        "GM is the sole member"
    );
}

/// `//gmchat` reaches every online GM (including the sender) but no normal
/// player.
#[test]
fn admin_gmchat_broadcasts_to_gms_only() {
    let (mut world, ..) = admin_world();
    let mut gm1 = ingame_player_access(&mut world, 1, 7302, 100);
    let mut gm2 = ingame_player_access(&mut world, 2, 7303, 100);
    let mut user = ingame_player_access(&mut world, 3, 7304, 0);
    drain(&mut gm1);
    drain(&mut gm2);
    drain(&mut user);

    on_packet(&mut world, 1, build_admin("gmchat hello gms"));
    let say = server_packets::opcodes::SAY2;
    assert!(
        drain(&mut gm1).iter().any(|p| p[0] == say),
        "sender GM sees it"
    );
    assert!(
        drain(&mut gm2).iter().any(|p| p[0] == say),
        "other GM sees it"
    );
    assert!(
        drain(&mut user).iter().all(|p| p[0] != say),
        "normal player does not"
    );
}

/// `//announce` reaches every online player.
#[test]
fn admin_announce_reaches_all_players() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7501, 100);
    let mut u1 = ingame_player_access(&mut world, 2, 7502, 0);
    let mut u2 = ingame_player_access(&mut world, 3, 7503, 0);
    drain(&mut gm_rx);
    drain(&mut u1);
    drain(&mut u2);

    on_packet(&mut world, 1, build_admin("announce server restart soon"));
    assert_eq!(
        count_system_messages(&drain(&mut u1)),
        1,
        "player 1 got the announce"
    );
    assert_eq!(
        count_system_messages(&drain(&mut u2)),
        1,
        "player 2 got the announce"
    );
}

/// `//geo_pos` with no geodata loaded answers the "no geodata" line (does not
/// crash on the empty geo engine).
#[test]
fn admin_geo_pos_no_geodata() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8910, 100);
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("geo_pos"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "one geo status line"
    );
}

/// `//event_trigger` fans the 0xCF packet out (self included); `//playmovie`
/// sends the cinematic starter to the GM.
#[test]
fn admin_event_trigger_and_playmovie_send_their_packets() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6495, 100);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("event_trigger 21170110 true"),
        ]
        .concat(),
    );
    let got = drain(&mut rx);
    assert!(
        got.iter()
            .any(|p| p.first() == Some(&0xCF) && p[1..5] == 21170110i32.to_le_bytes() && p[5] == 1),
        "OnEventTrigger 0xCF with the id and enabled byte"
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("playmovie 101"),
        ]
        .concat(),
    );
    let got = drain(&mut rx);
    assert!(
        got.iter().any(|p| p.first() == Some(&0xFE)
            && p[1..3] == 0x9Au16.to_le_bytes()
            && p[3..7] == 101i32.to_le_bytes()),
        "ExStartScenePlayer with the movie id"
    );
}

/// `//announce_screen <msg>` puts the text on every player's screen as an
/// `ExShowScreenMessage` (top-centre, free text); `//announce_crit` stays a
/// plain system-message line, not a banner.
#[test]
fn admin_announce_screen_broadcasts_a_banner() {
    let (mut world, ..) = admin_world();
    let mut gm = ingame_player_access(&mut world, 1, 7601, 100);
    let mut user = ingame_player_access(&mut world, 2, 7602, 0);
    drain(&mut gm);
    drain(&mut user);

    /// Decode `ExShowScreenMessage`: the 11-int field block, then the text.
    fn decode_screen(pkt: &[u8]) -> Option<(i32, i32, i32, String)> {
        if pkt[0] != server_packets::opcodes::EX
            || i16::from_le_bytes([pkt[1], pkt[2]])
                != server_packets::opcodes::EX_SHOW_SCREEN_MESSAGE
        {
            return None;
        }
        let mut r = commons::network::PacketReader::new(&pkt[3..]);
        let msg_type = r.read_i32()?;
        r.read_i32()?; // sysMessageId
        let position = r.read_i32()?;
        for _ in 0..7 {
            r.read_i32()?; // unk1, size, unk2, unk3, effect, time, fade
        }
        let npc_string = r.read_i32()?;
        Some((msg_type, position, npc_string, r.read_string()?))
    }

    on_packet(&mut world, 1, build_admin("announce_screen hello world"));
    let (msg_type, position, npc_string, text) = drain(&mut user)
        .iter()
        .find_map(|p| decode_screen(p))
        .expect("screen message");
    assert_eq!(text, "hello world", "banner text broadcast");
    assert_eq!(msg_type, 2, "the (text, time) constructor's type");
    assert_eq!(position, 2, "TOP_CENTER");
    assert_eq!(npc_string, -1, "free text, no NpcString");

    // //announce_crit is the ordinary text line, not a screen banner.
    drain(&mut user);
    on_packet(&mut world, 1, build_admin("announce_crit red alert"));
    assert!(
        drain(&mut user).iter().all(|p| decode_screen(p).is_none()),
        "crit does not put a banner on screen"
    );
}

/// `AbstractHtmlPacket.setHtml`'s guard, ported to the packet builder: an
/// oversized html is clipped to 17 200 chars instead of crashing the client.
#[test]
fn oversized_html_is_clipped_to_java_limit() {
    let big = "a".repeat(20_000);
    let pkt = server_packets::npc_html_message_item(0, 1, &big);
    let decoded = decode_npc_html(&pkt).expect("html packet");
    assert_eq!(decoded.chars().count(), 17_200);

    let small = "b".repeat(100);
    let pkt = server_packets::npc_html_message_item(0, 1, &small);
    assert_eq!(decode_npc_html(&pkt).unwrap(), small);
}

// ---------------------------------------------------------------------------
// GM invisibility (`admin_invis` family) + the Debug panel
// ---------------------------------------------------------------------------

/// **The Debug button opens the real Debug panel.** `admin_debug` used to
/// dump chat text; Java serves `debug.htm` with every `%…_status%` token
/// substituted. The packets toggle round-trips through `World::debug_packets`
/// and re-renders the panel with the flipped label.
#[test]
fn debug_menu_renders_and_packet_toggle_works() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7141, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("debug"));
    let html = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("the Debug panel is served");
    assert!(html.contains("Debug Menu"), "debug.htm served, got: {html}");
    assert!(
        !html.contains('%'),
        "every %token% substituted, got: {html}"
    );
    assert!(
        html.contains("admin_debug packets on menu"),
        "packets button offers enabling"
    );

    on_packet(&mut world, 1, build_admin("debug packets on menu"));
    assert!(world.debug_packets, "packet debugging enabled");
    let html = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("panel re-rendered");
    assert!(
        html.contains("admin_debug packets off menu"),
        "packets button now offers disabling"
    );

    on_packet(&mut world, 1, build_admin("debug packets off"));
    assert!(!world.debug_packets, "packet debugging disabled again");
}

// ---------------------------------------------------------------------------
// Category-4 sweep: punishment console, clan leader override, spawn controls
// ---------------------------------------------------------------------------

/// **`//server_shutdown` runs a real countdown** — announce on start, marks
/// while ticking, and the final beat requests the game-thread stop
/// (`shutdown_signal`; a test world's `None` just skips the request).
/// `//server_abort` cancels a pending countdown.
#[test]
fn server_shutdown_countdown_and_abort() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7801, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("server_shutdown 30"));
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    assert!(world.pending_shutdown.is_some(), "countdown armed");
    assert!(
        count_system_messages(&drain(&mut gm_rx)) >= 1,
        "start announcement"
    );

    // Run past the 10s / 5..1s marks and the deadline.
    advance_ticks(&mut world, 320);
    assert!(
        count_system_messages(&drain(&mut gm_rx)) >= 5,
        "mark announcements fired while ticking"
    );

    on_packet(&mut world, 1, build_admin("server_shutdown 60"));
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    on_packet(&mut world, 1, build_admin("server_abort"));
    assert!(
        world.pending_shutdown.is_none(),
        "abort clears the countdown"
    );
}

/// **`//server_gm_only` pushes a `ServerStatus` over the login link.**
#[test]
fn server_gm_only_sends_server_status() {
    use crate::loginlink::LoginLinkCommand;
    let (mut world, _db, _db_rx, mut link_rx) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7811, 100);
    drain(&mut gm_rx);
    while link_rx.try_recv().is_ok() {}

    on_packet(&mut world, 1, build_admin("server_gm_only"));
    let mut got = false;
    while let Ok(cmd) = link_rx.try_recv() {
        if matches!(cmd, LoginLinkCommand::ServerStatus { .. }) {
            got = true;
        }
    }
    assert!(got, "ServerStatus command reached the login link");
}

/// **`//tradeoff on` refuses incoming trade requests** (Java
/// `getTradeRefusal` in `TradeRequest`).
#[test]
fn tradeoff_refuses_trade_requests() {
    use model::components::combat::TargetRef;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7901, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 7902, 0);
    world.objects.add_components(&7902, TargetRef(Some(7901)));
    drain(&mut gm_rx);
    drain(&mut other_rx);

    on_packet(&mut world, 1, build_admin("tradeoff on"));
    assert!(
        world
            .objects
            .get_component::<Player>(&7901)
            .unwrap()
            .trade_refusal
    );

    // 7902 asks 7901 to trade — refused, no pending request lands.
    let mut body = Vec::new();
    body.extend_from_slice(&7901i32.to_le_bytes());
    crate::game_loop::commerce::trade::handle_request(&mut world, 2, &body);
    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::PendingTrade>(&7901),
        "no trade request while refusing"
    );
    assert!(
        count_system_messages(&drain(&mut other_rx)) >= 1,
        "requester told about refusal mode"
    );
}

/// **`//reload config` re-reads the ini values from disk.**
#[test]
fn reload_config_rereads_ini() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7921, 100);
    drain(&mut gm_rx);

    world.cfg.feature.allow_ride_wyvern_always = true; // drift from the ini
    on_packet(&mut world, 1, build_admin("reload config"));
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    assert!(
        !world.cfg.feature.allow_ride_wyvern_always,
        "ini value (False) restored by the reload"
    );
}

// ---------------------------------------------------------------------------
// Debug panel drawing toggles
// ---------------------------------------------------------------------------

/// **All four Debug-panel toggles are live.** The geodata toggle draws the
/// NSWE arrow grid as `ExServerPrimitive` (FE:11) packets and redraws after
/// the GM moves; toggling off erases; the panel reflects each state.
#[test]
fn debug_panel_geodata_toggle_draws_grid() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7951, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("debug geodata on menu"));
    let pkts = drain(&mut gm_rx);
    let prim_count = pkts
        .iter()
        .filter(|p| {
            p[0] == 0xFE && p.len() > 2 && i16::from_le_bytes(p[1..3].try_into().unwrap()) == 0x11
        })
        .count();
    assert!(
        prim_count >= 42,
        "41×41 cells / 40 per packet → 43 ExServerPrimitive frames, got {prim_count}"
    );
    assert!(
        pkts.iter()
            .filter_map(|p| decode_npc_html(p))
            .any(|h| h.contains("geodata off")),
        "panel shows the toggle as Disable"
    );
    assert!(admin::debug_draw::flags(&world, 7951).1, "geo flag set");

    // Moving > 15 units redraws on the next beat (15 ticks).
    world
        .objects
        .get_component_mut::<Position>(&7951)
        .unwrap()
        .x += 100;
    drain(&mut gm_rx);
    advance_ticks(&mut world, 16);
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == 0xFE),
        "grid redrawn after moving"
    );

    on_packet(&mut world, 1, build_admin("debug geodata off"));
    assert!(
        !admin::debug_draw::flags(&world, 7951).1,
        "geo flag cleared"
    );
    assert!(
        drain(&mut gm_rx).iter().filter(|p| p[0] == 0xFE).count() >= 42,
        "erase frames sent for every grid packet"
    );
}

/// **`//geogrid` draws the NSWE grid, `//geogrid off` erases it.** Java's
/// `AdminGeodata.admin_geogrid` is one-shot (`GeoUtils.debugGrid` /
/// `hideDebugGrid`): it arms no redraw beat and leaves the Debug panel's
/// `geodata` flag untouched. Draw frames carry 40 cells × 16 arrow lines;
/// the erase frames carry one zero-length line, so the two are told apart by
/// packet size, not just by count.
#[test]
fn admin_geogrid_draws_and_erases_grid() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7955, 100);
    drain(&mut gm_rx);

    let prims = |pkts: &[Vec<u8>]| -> Vec<usize> {
        pkts.iter()
            .filter(|p| {
                p[0] == 0xFE
                    && p.len() > 2
                    && i16::from_le_bytes(p[1..3].try_into().unwrap()) == 0x11
            })
            .map(|p| p.len())
            .collect()
    };

    on_packet(&mut world, 1, build_admin("geogrid"));
    let drawn = prims(&drain(&mut gm_rx));
    assert!(
        drawn.len() >= 42,
        "41×41 cells / 40 per packet → 43 ExServerPrimitive frames, got {}",
        drawn.len()
    );
    assert!(
        drawn.iter().take(drawn.len() - 1).all(|&n| n > 1000),
        "full grid frames carry 640 arrow lines: {drawn:?}"
    );
    assert!(
        !admin::debug_draw::flags(&world, 7955).1,
        "one-shot draw must not set the Debug panel's geodata flag"
    );

    // No redraw loop: moving and letting the geo beat (15 ticks) pass is quiet.
    world
        .objects
        .get_component_mut::<Position>(&7955)
        .unwrap()
        .x += 500;
    advance_ticks(&mut world, 20);
    assert!(
        prims(&drain(&mut gm_rx)).is_empty(),
        "//geogrid arms no redraw task (Java's is one-shot)"
    );

    on_packet(&mut world, 1, build_admin("geogrid off"));
    let erased = prims(&drain(&mut gm_rx));
    assert!(
        erased.len() >= 42,
        "erase frame per grid packet, got {}",
        erased.len()
    );
    assert!(
        erased.iter().all(|&n| n < 200),
        "erase frames are a single zero-length black line: {erased:?}"
    );
}

/// **`//world_missing_htmls`** lists talkable NPCs with no dialog page of
/// their own — the datapack audit a builder runs before shipping content.
///
/// The three exclusions are the point, and each is a different reason to skip:
/// a **monster** is not folk, a **non-talkable** NPC has no chat window to
/// miss, and an NPC whose chat window is supplied by a **script**
/// (`ON_NPC_FIRST_TALK`) needs no file at all. A sweep that only checked "is
/// there a .htm" would report all three as broken.
#[test]
fn missing_htmls_reports_folk_without_a_page_and_skips_the_three_exclusions() {
    let (mut world, ..) = admin_world();

    // A talkable Folk with no `data/html/default/<id>.htm` — the real finding.
    // 90501 is synthetic, so no dist file can exist for it.
    add_test_npc(&mut world, 7001, 90501, "Folk", 20, 100, 0, 0);
    // A monster: excluded regardless of html.
    add_test_npc(&mut world, 7002, 90502, "Monster", 20, 120, 0, 0);
    // A non-talkable Folk: nothing to open.
    add_test_npc(&mut world, 7003, 90503, "Folk", 20, 140, 0, 0);
    if let Some(t) = world.data.npc_data.get(90503).cloned() {
        let mut t = t;
        t.talkable = false;
        world.data.npc_data.insert_for_test(t);
    }

    let found: Vec<i32> = admin::missing_htmls::scan_for_test(&mut world, None)
        .into_iter()
        .map(|(id, ..)| id)
        .collect();

    assert!(
        found.contains(&90501),
        "the talkable folk with no page is reported: {found:?}"
    );
    assert!(
        !found.contains(&90502),
        "a monster is not folk and is skipped: {found:?}"
    );
    assert!(
        !found.contains(&90503),
        "a non-talkable NPC has no window to miss: {found:?}"
    );
}

/// The geomap-scoped sweep only reports NPCs inside the GM's own geodata tile
/// — that is the whole difference between it and the world sweep.
#[test]
fn geomap_missing_htmls_is_scoped_to_the_tile() {
    let (mut world, ..) = admin_world();
    add_test_npc(&mut world, 7010, 90511, "Folk", 20, 100, 0, 0);

    // A box around the near NPC includes it; a far-away box does not.
    let near = admin::missing_htmls::scan_for_test(&mut world, Some((-1000, -1000, 1000, 1000)));
    let far =
        admin::missing_htmls::scan_for_test(&mut world, Some((500_000, 500_000, 600_000, 600_000)));

    assert!(
        near.iter().any(|&(id, ..)| id == 90511),
        "inside the tile it is reported"
    );
    assert!(
        !far.iter().any(|&(id, ..)| id == 90511),
        "outside it is not"
    );
}

/// **`//forge_send sc`** puts the forged bytes on the GM's own socket — the
/// whole point of the tool, and the half that unit tests of the encoder cannot
/// see.
///
/// `$oid` is resolved before the operand is written, so the packet carries the
/// GM's object id rather than the literal token.
#[test]
fn forge_send_sc_writes_the_forged_packet_to_the_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    drain(&mut gm_rx);

    // opcode 0x2F, one dword operand: the GM's own object id.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("forge_send sc 0x2F d $oid"),
        ]
        .concat(),
    );

    let pkts = drain(&mut gm_rx);
    let forged = pkts
        .iter()
        .find(|p| p.len() == 5 && p[0] == 0x2F)
        .expect("the forged packet reached the GM");
    assert_eq!(
        i32::from_le_bytes(forged[1..5].try_into().unwrap()),
        5001,
        "$oid was substituted, not written literally"
    );
}

/// `cs` refuses rather than forging an inbound packet — matching Java, whose
/// branch throws `UnsupportedOperationException`. The refusal is the ported
/// behaviour, so it must be visible rather than silent.
#[test]
fn forge_send_cs_refuses_like_java() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("forge_send cs 0x2F"),
        ]
        .concat(),
    );

    let pkts = drain(&mut gm_rx);
    assert!(
        count_system_messages(&pkts) > 0,
        "the refusal is reported to the GM"
    );
    assert!(
        !pkts.iter().any(|p| p.len() == 1 && p[0] == 0x2F),
        "and nothing is forged"
    );
}

/// `//playmovie` carries Java's `MovieHolder` bookkeeping: the state is
/// remembered, a second movie is refused while one plays, `EndScenePlayer`
/// only clears on the matching id, Esc (`RequestExEscapeScene`) ends an
/// escapable movie with `ExStopScenePlayer` but is ignored for a
/// non-escapable one, and an id outside the `Movie` table is refused
/// outright (Java's `findByClientId` → catch → usage).
#[test]
fn playmovie_movie_holder_bookkeeping() {
    use model::components::space::InMovie;

    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 6496, 100);
    drain(&mut rx);

    let play = |world: &mut World, id: &str| {
        on_packet(
            world,
            1,
            [
                vec![cop::SEND_BYPASS_BUILD_CMD],
                build_cmd_body(&format!("playmovie {id}")),
            ]
            .concat(),
        );
    };
    let in_movie = |world: &World| {
        world
            .objects
            .get_component::<InMovie>(&6496)
            .map(|m| (m.movie_id, m.escapable))
    };
    let stop_sent = |pkts: &[Vec<u8>], id: i32| {
        pkts.iter().any(|p| {
            p.first() == Some(&0xFE)
                && p[1..3] == 0xE7u16.to_le_bytes()
                && p[3..7] == id.to_le_bytes()
        })
    };

    // 39 is a hole in the Movie enum — refused, no state.
    play(&mut world, "39");
    assert_eq!(in_movie(&world), None, "an unknown id never starts a movie");

    play(&mut world, "101"); // SI_ILLUSION_01_QUE, escapable
    assert_eq!(in_movie(&world), Some((101, true)));
    drain(&mut rx);

    // A second movie while one is playing is Java's `_movieHolder != null`
    // refusal.
    play(&mut world, "102");
    assert_eq!(in_movie(&world), Some((101, true)), "still the first movie");

    // The end notice must echo the running movie's id to count.
    on_packet(&mut world, 1, ex_packet(0x58, &102i32.to_le_bytes()));
    assert_eq!(in_movie(&world), Some((101, true)), "wrong id ignored");
    on_packet(&mut world, 1, ex_packet(0x58, &101i32.to_le_bytes()));
    assert_eq!(in_movie(&world), None, "matching id ends the movie");

    // Esc ends an escapable movie (single viewer: the vote passes at once)…
    play(&mut world, "101");
    drain(&mut rx);
    on_packet(&mut world, 1, ex_packet(0x90, &[]));
    assert_eq!(in_movie(&world), None, "Esc ended the escapable movie");
    assert!(
        stop_sent(&drain(&mut rx), 101),
        "ExStopScenePlayer answers the escape"
    );

    // …but a non-escapable one ignores it (15 = SC_BOSS_FREYA_OPENING).
    play(&mut world, "15");
    assert_eq!(in_movie(&world), Some((15, false)));
    on_packet(&mut world, 1, ex_packet(0x90, &[]));
    assert_eq!(
        in_movie(&world),
        Some((15, false)),
        "Esc is ignored for a non-escapable movie"
    );
}

/// `//instancedestroy` warns everyone inside with the "destroyed by Game
/// Master" screen banner before the teleport-out, like Java's AdminInstance.
#[test]
fn admin_instancedestroy_warns_the_players_inside() {
    use crate::data::instance_data::{ExitType, InstanceTemplate};
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    world
        .data
        .instance_templates
        .insert_for_test(InstanceTemplate {
            id: 902,
            name: Some("Doomed Arena".into()),
            max_worlds: -1,
            duration_min: 60,
            empty_destroy_min: 5,
            enter: Some((100, 200, 300)),
            exit: ExitType::Origin,
            doors: vec![],
            groups: vec![],
        });
    let iid = crate::game_loop::space::instances::create_from_template(&mut world, 902)
        .expect("template");
    let mut gm = ingame_player_access(&mut world, 1, 6441, 100);
    let mut inhabitant = ingame_player_access(&mut world, 2, 6442, 0);
    crate::game_loop::space::instances::enter(&mut world, 6442, iid);
    drain(&mut gm);
    drain(&mut inhabitant);

    // `admin_instancedestroy` carries `confirmDlg="true"` — answer it.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body(&format!("instancedestroy {iid}")),
        ]
        .concat(),
    );
    const S1_3: i32 = server_packets::S1_3_MESSAGE_ID;
    on_packet(
        &mut world,
        1,
        [vec![cop::DLG_ANSWER], dlg_answer_body(S1_3, 1, 0)].concat(),
    );

    let pkts = drain(&mut inhabitant);
    let warned = pkts
        .iter()
        .any(|p| is_ex(p, server_packets::opcodes::EX_SHOW_SCREEN_MESSAGE));
    assert!(warned, "the inhabitant saw the Game Master banner");
    assert!(world.instances.get(iid).is_none(), "the instance is gone");
}

/// `AdminZone`'s visualiser: adena markers every 10 units along each boundary
/// of every zone covering the GM, and a clear that removes exactly those.
#[test]
fn zone_visual_outlines_the_zone_and_clears_it_again() {
    use crate::data::zone_data::{Zone, ZoneKind};
    let (mut world, ..) = admin_world();
    world.data.admin = crate::data::AdminData::load_from(crate::data::DIST_GAME);
    world.next_npc_object_id = 0x6000_0000;
    let mut gm_rx = ingame_player_access(&mut world, 1, 5401, 100);
    // A 200x200 cuboid around the origin: its border is 4 × (200/10) = 80
    // markers.
    world.data.zone_data.insert(Zone {
        id: 4242,
        name: "test_visual".into(),
        kind: ZoneKind::Peace,
        territory: Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid {
                x1: -100,
                x2: 100,
                y1: -100,
                y2: 100,
            },
            min_z: -1000,
            max_z: 1000,
        },
        castle_id: 0,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
        condition: None,
        mother_tree: None,
    });
    drain(&mut gm_rx);

    admin::use_admin_command(&mut world, 1, "admin_zone_visual all", false);

    assert_eq!(
        world.zone_debug_items.len(),
        80,
        "20 steps per side, both sides of each axis"
    );
    let sample = world.zone_debug_items[0];
    assert!(
        world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&sample),
        "the markers are real ground items"
    );

    admin::use_admin_command(&mut world, 1, "admin_zone_visual_clear", false);

    assert!(world.zone_debug_items.is_empty(), "the list is emptied");
    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&sample),
        "and the markers are gone from the world"
    );
}

/// A numeric argument visualises that one zone by id; an unknown id says so
/// and drops nothing.
#[test]
fn zone_visual_takes_a_zone_id() {
    let (mut world, ..) = admin_world();
    world.data.admin = crate::data::AdminData::load_from(crate::data::DIST_GAME);
    world.data.zone_data = crate::data::zone_data::ZoneData::load_from(crate::data::DIST_GAME);
    world.next_npc_object_id = 0x6100_0000;
    let mut gm_rx = ingame_player_access(&mut world, 1, 5402, 100);
    let zone_id = world
        .data
        .zone_data
        .zones
        .iter()
        .map(|z| z.id)
        .find(|&id| id > 0)
        .expect("the dist ships zones with ids");
    drain(&mut gm_rx);

    admin::use_admin_command(
        &mut world,
        1,
        &format!("admin_zone_visual {zone_id}"),
        false,
    );
    assert!(
        !world.zone_debug_items.is_empty(),
        "the named zone was outlined"
    );

    admin::use_admin_command(&mut world, 1, "admin_zone_visual_clear", false);
    admin::use_admin_command(&mut world, 1, "admin_zone_visual 99999999", false);
    assert!(
        world.zone_debug_items.is_empty(),
        "an unknown id outlines nothing"
    );
}

// ---------------------------------------------------------------------------
// Row 16: the last commands that were absent against ported systems
// ---------------------------------------------------------------------------

/// `//config_server` draws the live rates, and `//setconfig` writes them back —
/// the only three parameters Java's `switch` has cases for.
#[test]
fn admin_setconfig_edits_the_three_live_rates() {
    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 7401, 100);
    world.cfg.rates.rate_xp = 1.0;
    world.cfg.rates.rate_sp = 1.0;
    world.cfg.rates.spoil_drop_chance_multiplier = 1.0;
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("config_server"));
    let page = last_admin_html(&drain(&mut rx)).expect("config page");
    assert!(page.contains("Config Server Panel"));
    assert!(page.contains("Rate EXP</font> = 1"), "the value in force");
    assert!(
        page.contains("bypass -h admin_setconfig RateXp $param1"),
        "each row posts back through //setconfig"
    );

    on_packet(&mut world, 1, build_admin("setconfig RateXp 3"));
    assert_eq!(world.cfg.rates.rate_xp, 3.0, "the live rate moved");
    let after = drain(&mut rx);
    assert!(
        last_admin_html(&after)
            .expect("panel redrawn")
            .contains("Rate EXP</font> = 3"),
        "Java's `finally` re-shows the panel with the new value"
    );

    on_packet(&mut world, 1, build_admin("setconfig RateSp 2.5"));
    on_packet(&mut world, 1, build_admin("setconfig RateDropSpoil 4"));
    assert_eq!(world.cfg.rates.rate_sp, 2.5);
    assert_eq!(world.cfg.rates.spoil_drop_chance_multiplier, 4.0);

    // A parameter Java has no case for is announced and does nothing; a
    // non-numeric value is the usage line.
    drain(&mut rx);
    on_packet(&mut world, 1, build_admin("setconfig RateDropItems 9"));
    assert_eq!(world.cfg.rates.rate_xp, 3.0, "nothing else moved");
    on_packet(&mut world, 1, build_admin("setconfig RateXp lots"));
    assert_eq!(world.cfg.rates.rate_xp, 3.0, "not a number, not applied");
}

/// `//server_login` serves the Server Management Menu — the page every other
/// `//server_*` command is a button on, and which nothing served before.
#[test]
fn admin_server_login_draws_the_management_page() {
    let (mut world, ..) = admin_world();
    // The page is a datapack file (`data/html/admin/login.htm`), so the world
    // needs the real root to read it.
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 7406, 100);
    world.login.server_name = Some("Bartz".into());
    world.login.max_players = 100;
    world.cfg.server.server_list_type = 0x01 | 0x40; // Normal+Free
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("server_login"));
    let page = last_admin_html(&drain(&mut rx)).expect("login page");
    assert!(page.contains("Server Management Menu"));
    assert!(page.contains("Bartz"), "registered-as name");
    assert!(page.contains("Auto"), "the status it starts on");
    assert!(page.contains("Normal+Free"), "the type mask spelled out");
    assert!(page.contains("100"), "max players");

    // The status commands remember what they pushed and redraw the page with
    // it — Java's `showMainPage` at the end of every branch.
    on_packet(&mut world, 1, build_admin("server_gm_only"));
    let page = last_admin_html(&drain(&mut rx)).expect("page redrawn");
    assert!(page.contains("Gm Only"), "the pushed status is shown back");

    on_packet(&mut world, 1, build_admin("server_max_player 42"));
    assert_eq!(world.login.max_players, 42);
    assert!(
        last_admin_html(&drain(&mut rx))
            .expect("page redrawn")
            .contains("42")
    );
}

/// **`//find_dualbox` renders the panel** (GitHub #6). Java fills
/// `dualbox.htm`'s `%multibox%` / `%results%` and sends it; the port printed the
/// hits to system chat, losing both the `admin_find_ip` links and the
/// re-run buttons.
#[test]
fn find_dualbox_sends_the_panel_not_chat_lines() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7702, 100);
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("find_dualbox"),
        ]
        .concat(),
    );

    let packets = drain(&mut gm_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        "the dualbox panel is sent as html"
    );
    assert!(
        packets
            .iter()
            .filter_map(|p| system_message_text(p))
            .all(|t| !t.contains("=== Dualbox")),
        "and not as chat lines"
    );
}
