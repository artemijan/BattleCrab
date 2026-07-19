use super::*;
use crate::model::skill::{AffectObject, AffectScope};

/// `showTeleports` builds the button list: the fee suffix shows only above
/// the free-teleport level (`shouldPayFee`/`calculateFee`), the button
/// carries the `teleport <list> <index>` bypass, and the `npc_` route still
/// ends with the `ActionFailed` terminator.
#[test]
fn teleporter_list_shows_fee_only_above_free_level() {
    let (mut world, mut rx) = teleporter_world(10_000);

    // Level 45 > 40: fee shown.
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 45;
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_showTeleports")));
    let pkts = drain(&mut rx);
    let html = pkts.iter().find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE).expect("list html");
    assert!(contains_utf16(html, &format!("npc_{NPC_OID}_teleport NORMAL 0")), "teleport button bypass");
    assert!(contains_utf16(html, "<fstring>1010004</fstring> - 9400"), "fee suffix at level 45");
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::ACTION_FAIL), "npc_ terminator");

    // Level 30 ≤ 40: free — no fee suffix.
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 30;
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_showTeleports")));
    let pkts = drain(&mut rx);
    let html = pkts.iter().find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE).expect("list html");
    assert!(contains_utf16(html, "<fstring>1010004</fstring>"), "destination still listed");
    assert!(!contains_utf16(html, " - 9400"), "no fee suffix below the free level");
}

/// `teleport NORMAL 0`: above the free level the adena fee is charged and
/// the player lands on the destination; below it the teleport is free; on a
/// shortfall SM 279 fires and nothing moves.
#[test]
fn teleporter_charges_fee_and_teleports() {
    // Paid: level 45, 10 000 adena → 600 left, position moved.
    let (mut world, mut rx) = teleporter_world(10_000);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 45;
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_teleport NORMAL 0")));
    let pkts = drain(&mut rx);
    assert_eq!(adena_of(&world, 3001), 600, "9400 adena fee charged");
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION));
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (1000, 2000, -25));

    // Free: level 1 ≤ 40 → no charge.
    let (mut world, mut rx) = teleporter_world(10_000);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_teleport NORMAL 0")));
    drain(&mut rx);
    assert_eq!(adena_of(&world, 3001), 10_000, "free below MaxFreeTeleportLevel");
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (1000, 2000, -25));

    // Shortfall: SM 279, no movement, adena untouched.
    let (mut world, mut rx) = teleporter_world(100);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 45;
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_teleport NORMAL 0")));
    let pkts = drain(&mut rx);
    assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA));
    assert!(!pkts.iter().any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION));
    assert_eq!(adena_of(&world, 3001), 100);
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y), (0, 0), "shortfall must not teleport");
}

/// Malformed teleport bypasses (bad index, unknown list, wrong token count)
/// log-drop without acting; the same verbs on a non-Teleporter NPC fall to
/// the unhandled-verb log. All still send the `npc_` terminator only.
#[test]
fn teleporter_rejects_malformed_and_wrong_npc() {
    let (mut world, mut rx) = teleporter_world(10_000);
    for cmd in ["teleport NORMAL 5", "teleport NOPE 0", "teleport NORMAL", "teleport NORMAL 0 extra"] {
        handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_{cmd}")));
        let pkts = drain(&mut rx);
        assert!(!pkts.iter().any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION), "for {cmd}");
        let pos = world.objects.get_component::<Position>(&3001).unwrap();
        assert_eq!((pos.x, pos.y), (0, 0), "for {cmd}");
    }
    assert_eq!(adena_of(&world, 3001), 10_000, "nothing ever charged");

    // A Folk NPC with the same template lists registered: the verb is gated
    // on the Teleporter instance class, so nothing happens.
    add_test_npc(&mut world, NPC_OID + 1, 30002, "Folk", 5, 100, 0, 0);
    world.data.teleporters.insert_for_test(
        30002,
        crate::data::teleporter_data::TeleportHolder {
            name: "NORMAL".into(),
            teleport_type: crate::data::teleporter_data::TeleportType::Normal,
            locations: vec![crate::data::teleporter_data::TeleportLocation {
                x: 1, y: 2, z: 3, name: None, npc_string_id: -1, fee_id: 57, fee_count: 0,
            }],
        },
    );
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{}_teleport NORMAL 0", NPC_OID + 1)));
    drain(&mut rx);
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y), (0, 0), "non-teleporter NPC must not teleport");
}

/// Shared `/unstuck` fixture: the dist's 30 s `UnstuckInterval`, a map region
/// whose tile contains (0,0), and the static escape skill 2099 with the
/// `Escape TOWN` effect.
fn unstuck_world() -> crate::world::World {
    let (mut world, ..) = test_world();
    world.cfg.character.unstuck_interval = 30;
    world.data.map_region = crate::data::MapRegionData::from_regions(vec![
        crate::data::map_region::MapRegion {
            name: "test_town".into(),
            loc_id: 924,
            respawn_points: vec![(5000, 6000, -30)],
            tiles: vec![(20, 18)], // the tile containing (0,0)
        },
    ]);
    world.data.skill_data.insert_for_test(Skill {
        without_action: false,
        item_consume_id: 0,
        item_consume_count: 0,
        id: 2099,
        level: 1,
        name: "Escape".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type: 2, // static: the forced hit time is used verbatim
        magic_level: 0,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 300_000,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects: vec![crate::model::skill::SkillEffect::EscapeToTown],
    });
    world
}

/// `/unstuck` (user command 52) with the dist's 30 s `UnstuckInterval`:
/// sends the "You use Escape" line, starts a static 30 s cast of skill 2099,
/// refuses a second use mid-cast, and on landing the `Escape TOWN` effect
/// teleports to the map-region town respawn.
#[test]
fn unstuck_casts_escape_and_teleports_to_town() {
    let mut world = unstuck_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    // The bare test template spawns at 0 HP; the finish phase's HP re-check
    // (`cur_hp <= hp_consume`) needs a live caster.
    world.objects.get_component_mut::<Vitals>(&3001).unwrap().cur_hp = 100.0;

    super::user_commands::handle_bypass_user_cmd(&mut world, 1, &user_cmd_body(52));
    let pkts = drain(&mut rx);
    assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::S1_TEXT), "'You use Escape' message");
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE), "cast started");
    assert!(world.objects.has_component::<Casting>(&3001));

    // Mid-cast re-use refuses silently (Java `isCastingNow` → false).
    super::user_commands::handle_bypass_user_cmd(&mut world, 1, &user_cmd_body(52));
    assert!(!drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE));

    // 30 s (300 ticks) to launch + the 500 ms finish floor.
    advance_ticks(&mut world, 310);
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (5000, 6000, -25), "escaped to the town respawn (z lifted by 5)");
    let landed = drain(&mut rx);
    assert!(landed.iter().any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION));
    // `teleToLocation`'s `abortCast()`: without the cancel the client keeps
    // drawing the escape FX at the destination for skill 2099's own 5-minute
    // duration, even though the teleport already landed.
    assert!(
        landed.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED),
        "teleport must cancel the cast animation client-side"
    );
    assert!(!world.objects.has_component::<Casting>(&3001), "cast slot freed by the abort");

    // The pre-teleport cancel is lost in the client's world-reload teardown,
    // so `Appearing` must repeat it once the destination has loaded.
    super::death::handle_appearing(&mut world, 1);
    let appeared = drain(&mut rx);
    assert!(
        appeared.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED),
        "Appearing must re-send the cancel — the pre-teleport one never survives the reload"
    );

    // A later teleport with no cast in flight must not re-send it.
    crate::game_loop::death::teleport_player(&mut world, 3001, 0, 0, 0);
    super::death::handle_appearing(&mut world, 1);
    assert!(
        !drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED),
        "no stale cancel on a cast-free teleport"
    );
}

/// Regression: `/unstuck` with a target selected (here: the player's own
/// character, the everyday self-click) must still cancel the cast animation
/// on landing. Java's `canAbortCast` (`getTarget() == null`) skips the
/// `MagicSkillCanceled` in this case and leaves the escape FX playing at the
/// destination — a Mobius quirk `abort_cast_on_teleport` deliberately does
/// not port.
#[test]
fn unstuck_with_target_selected_still_cancels_cast_animation() {
    let mut world = unstuck_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Vitals>(&3001).unwrap().cur_hp = 100.0;
    world.objects.add_components(&3001, crate::model::components::TargetRef(Some(3001)));

    super::user_commands::handle_bypass_user_cmd(&mut world, 1, &user_cmd_body(52));
    drain(&mut rx);
    advance_ticks(&mut world, 310);

    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (5000, 6000, -25), "teleported despite the selection");
    let landed = drain(&mut rx);
    assert!(
        landed.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED),
        "cancel must go out even with a target selected"
    );
    assert!(!world.objects.has_component::<Casting>(&3001), "cast slot freed by the abort");

    super::death::handle_appearing(&mut world, 1);
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED),
        "Appearing must re-send the cancel with a target selected too"
    );
}

/// `/loc` (user command 0): inside a mapped region the region's `locId` is
/// sent as the system-message id with the three coordinates; outside any
/// region the CURRENT_LOCATION_S1 fallback carries them as text. Unknown
/// command ids are silent for ordinary players.
#[test]
fn loc_user_command_reports_region() {
    let (mut world, ..) = test_world();
    world.data.map_region = crate::data::MapRegionData::from_regions(vec![
        crate::data::map_region::MapRegion {
            name: "test_town".into(),
            loc_id: 924,
            respawn_points: vec![(5000, 6000, -30)],
            tiles: vec![(20, 18)],
        },
    ]);
    let mut rx = ingame_player(&mut world, 1, 3001, 123, 456, -78);
    drain(&mut rx);
    super::user_commands::handle_bypass_user_cmd(&mut world, 1, &user_cmd_body(0));
    let pkts = drain(&mut rx);
    assert_eq!(sm_ids_of(&pkts), vec![924], "region locId is the SM id");

    // Far outside every region tile: the plain-text fallback.
    let mut rx2 = ingame_player(&mut world, 2, 3002, 500_000, 500_000, 0);
    drain(&mut rx2);
    super::user_commands::handle_bypass_user_cmd(&mut world, 2, &user_cmd_body(0));
    let pkts = drain(&mut rx2);
    assert_eq!(sm_ids_of(&pkts), vec![server_packets::sm_ids::CURRENT_LOCATION_S1]);

    // Unknown command id: silence for a non-GM.
    super::user_commands::handle_bypass_user_cmd(&mut world, 1, &user_cmd_body(77));
    assert!(drain(&mut rx).is_empty(), "unknown user command must be silent");
}
