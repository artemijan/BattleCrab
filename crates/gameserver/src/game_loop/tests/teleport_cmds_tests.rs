use super::*;
use crate::game_loop::client::user_commands;
use crate::model::skill::{AffectObject, AffectScope};

/// `showTeleports` builds the button list: the fee suffix shows only above
/// the free-teleport level (`shouldPayFee`/`calculateFee`), the button
/// carries the `teleport <list> <index>` bypass, and the `npc_` route still
/// ends with the `ActionFailed` terminator.
#[test]
fn teleporter_list_shows_fee_only_above_free_level() {
    let (mut world, mut rx) = teleporter_world(10_000);

    // Level 45 > 40: fee shown.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 45;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_showTeleports")),
    );
    let pkts = drain(&mut rx);
    let html = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .expect("list html");
    assert!(
        contains_utf16(html, &format!("npc_{NPC_OID}_teleport NORMAL 0")),
        "teleport button bypass"
    );
    assert!(
        contains_utf16(html, "<fstring>1010004</fstring> - 9400"),
        "fee suffix at level 45"
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::ACTION_FAIL),
        "npc_ terminator"
    );

    // Level 30 ≤ 40: free — no fee suffix.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 30;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_showTeleports")),
    );
    let pkts = drain(&mut rx);
    let html = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .expect("list html");
    assert!(
        contains_utf16(html, "<fstring>1010004</fstring>"),
        "destination still listed"
    );
    assert!(
        !contains_utf16(html, " - 9400"),
        "no fee suffix below the free level"
    );
}

/// `teleport NORMAL 0`: above the free level the adena fee is charged and
/// the player lands on the destination; below it the teleport is free; on a
/// shortfall SM 279 fires and nothing moves.
#[test]
fn teleporter_charges_fee_and_teleports() {
    // Paid: level 45, 10 000 adena → 600 left, position moved.
    let (mut world, mut rx) = teleporter_world(10_000);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 45;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_teleport NORMAL 0")),
    );
    let pkts = drain(&mut rx);
    assert_eq!(adena_of(&world, 3001), 600, "9400 adena fee charged");
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION)
    );
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (1000, 2000, -25));

    // Free: level 1 ≤ 40 → no charge.
    let (mut world, mut rx) = teleporter_world(10_000);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_teleport NORMAL 0")),
    );
    drain(&mut rx);
    assert_eq!(
        adena_of(&world, 3001),
        10_000,
        "free below MaxFreeTeleportLevel"
    );
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (1000, 2000, -25));

    // Shortfall: SM 279, no movement, adena untouched.
    let (mut world, mut rx) = teleporter_world(100);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 45;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_teleport NORMAL 0")),
    );
    let pkts = drain(&mut rx);
    assert!(
        ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA)
    );
    assert!(
        !pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION)
    );
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
    for cmd in [
        "teleport NORMAL 5",
        "teleport NOPE 0",
        "teleport NORMAL",
        "teleport NORMAL 0 extra",
    ] {
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_{cmd}")),
        );
        let pkts = drain(&mut rx);
        assert!(
            !pkts
                .iter()
                .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
            "for {cmd}"
        );
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
                x: 1,
                y: 2,
                z: 3,
                name: None,
                npc_string_id: -1,
                fee_id: 57,
                fee_count: 0,
                castle_ids: Vec::new(),
            }],
        },
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_teleport NORMAL 0", NPC_OID + 1)),
    );
    drain(&mut rx);
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (0, 0),
        "non-teleporter NPC must not teleport"
    );
}

/// `/unstuck` (user command 52) with the dist's 30 s `UnstuckInterval`:
/// sends the "You use Escape" line, starts a static 30 s cast of skill 2099,
/// refuses a second use mid-cast, and on landing the `Escape TOWN` effect
/// teleports to the map-region town respawn.
#[test]
fn unstuck_casts_escape_and_teleports_to_town() {
    let (mut world, ..) = test_world();
    world.cfg.character.unstuck_interval = 30;
    with_town(&mut world);
    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        without_action: false,
        trait_type: model::skill::TraitType::None,
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
        effects: vec![model::skill::SkillEffect::Escape {
            dest: model::skill::EscapeDest::Town,
        }],
        ..Default::default()
    });
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    // The bare test template spawns at 0 HP; the finish phase's HP re-check
    // (`cur_hp <= hp_consume`) needs a live caster.
    world
        .objects
        .get_component_mut::<Vitals>(&3001)
        .unwrap()
        .cur_hp = 100.0;

    user_commands::handle_bypass_user_cmd(&mut world, 1, &user_cmd_body(52));
    let pkts = drain(&mut rx);
    // Both lines are sent, in Java's order: `SkillCaster.castSkill` runs phase 0
    // synchronously (`skillCaster.run()` → `startCasting`), so the skill's
    // YOU_USE_S1 ("You use Escape (5-minute).", named by the *client* after
    // skill 2099) reaches the player before the handler's own
    // "You use Escape: 30 seconds." chat line on `Unstuck.java:147`.
    let sms = ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE);
    let use_s1 = sms
        .iter()
        .position(|id| *id == server_packets::sm_ids::YOU_USE_S1)
        .expect("YOU_USE_S1 for the escape skill");
    let chat = sms
        .iter()
        .position(|id| *id == server_packets::sm_ids::S1_TEXT)
        .expect("'You use Escape: 30 seconds.' chat line");
    assert!(
        use_s1 < chat,
        "the skill's YOU_USE_S1 precedes the handler's chat line (sms: {sms:?})"
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "cast started"
    );
    assert!(world.objects.has_component::<Casting>(&3001));

    // Mid-cast re-use refuses silently (Java `isCastingNow` → false).
    user_commands::handle_bypass_user_cmd(&mut world, 1, &user_cmd_body(52));
    assert!(
        !drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
    );

    // 30 s (300 ticks) to launch + the 500 ms finish floor.
    advance_ticks(&mut world, 310);
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (5000, 6000, -25),
        "escaped to the town respawn (z lifted by 5)"
    );
    let landed = drain(&mut rx);
    assert!(
        landed
            .iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION)
    );
    // `teleToLocation`'s `abortCast()`: without the cancel the client keeps
    // drawing the escape FX at the destination for skill 2099's own 5-minute
    // duration, even though the teleport already landed.
    assert!(
        landed
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED),
        "teleport must cancel the cast animation client-side"
    );
    assert!(
        !world.objects.has_component::<Casting>(&3001),
        "cast slot freed by the abort"
    );
}

/// **A refused escape must not claim it worked.** Java gates the
/// "You use Escape: 30 seconds." line on `SkillCaster.castSkill` returning a
/// caster — a null answer gets `ActionFailed` +
/// `setIntention(AI_INTENTION_ACTIVE)` and no message at all
/// (`Unstuck.java:135-141`). Here the escape carries an initial MP cost the
/// player cannot pay, so the cast bails before the `Casting` slot is taken.
#[test]
fn unstuck_says_nothing_when_the_cast_is_refused() {
    let (mut world, ..) = test_world();
    world.cfg.character.unstuck_interval = 30;
    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        id: 2099,
        level: 1,
        name: "Escape".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        magic_type: 2, // static: the forced hit time is used verbatim
        hit_time: 300_000,
        mp_initial_consume: 50,
        effects: vec![model::skill::SkillEffect::Escape {
            dest: model::skill::EscapeDest::Town,
        }],
        ..Default::default()
    });
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let vitals = world.objects.get_component_mut::<Vitals>(&3001).unwrap();
        vitals.cur_hp = 100.0;
        vitals.cur_mp = 0.0; // cannot pay the initial consume
    }
    drain(&mut rx);

    user_commands::handle_bypass_user_cmd(&mut world, 1, &user_cmd_body(52));
    let pkts = drain(&mut rx);
    assert!(
        !world.objects.has_component::<Casting>(&3001),
        "the cast must not have started"
    );
    assert!(
        !ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_TEXT),
        "no 'You use Escape' line when the escape was refused"
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::ACTION_FAIL),
        "Java answers the refusal with ActionFailed"
    );
}

/// `/loc` (user command 0): inside a mapped region the region's `locId` is
/// sent as the system-message id with the three coordinates; outside any
/// region the CURRENT_LOCATION_S1 fallback carries them as text. Unknown
/// command ids are silent for ordinary players.
#[test]
fn loc_user_command_reports_region() {
    let (mut world, ..) = test_world();
    with_town(&mut world);
    let mut rx = ingame_player(&mut world, 1, 3001, 123, 456, -78);
    drain(&mut rx);
    user_commands::handle_bypass_user_cmd(&mut world, 1, &user_cmd_body(0));
    let pkts = drain(&mut rx);
    assert_eq!(
        ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE),
        vec![924],
        "region locId is the SM id"
    );

    // Far outside every region tile: the plain-text fallback.
    let mut rx2 = ingame_player(&mut world, 2, 3002, 500_000, 500_000, 0);
    drain(&mut rx2);
    user_commands::handle_bypass_user_cmd(&mut world, 2, &user_cmd_body(0));
    let pkts = drain(&mut rx2);
    assert_eq!(
        ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::CURRENT_LOCATION_S1]
    );

    // Unknown command id: silence for a non-GM. (255 is unregistered — 77 is
    // `/time` since the user-command sweep.)
    user_commands::handle_bypass_user_cmd(&mut world, 1, &user_cmd_body(255));
    assert!(
        drain(&mut rx).is_empty(),
        "unknown user command must be silent"
    );
}

// --- Row 9: the gatekeeper tails ------------------------------------------

/// **A subclass pays even below the free-teleport level.** Java's
/// `shouldPayFee`/`calculateFee` both add `isSubClassActive()`, so a level-20
/// character on a subclass is charged the full fare a base-class one rides free.
#[test]
fn a_subclass_pays_the_teleport_fee() {
    let (mut world, mut rx) = teleporter_world(20_000);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20; // below MaxFreeTeleportLevel (40)
        p.base_class_id = 0;
        p.class_id = 0;
    }
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_teleport NORMAL 0")),
    );
    drain(&mut rx);
    assert_eq!(adena_of(&world, 3001), 20_000, "a base class rides free");

    // Same level, but playing a subclass: the fare is charged. (The free ride
    // moved the player, so put them back in front of the gatekeeper first.)
    {
        let pos = world.objects.get_component_mut::<Position>(&3001).unwrap();
        pos.x = 0;
        pos.y = 0;
        pos.z = 0;
    }
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .class_id = 88;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_teleport NORMAL 0")),
    );
    drain(&mut rx);
    assert_eq!(
        adena_of(&world, 3001),
        20_000 - 9_400,
        "a subclass pays the full fee"
    );
}

/// **Monday/Tuesday from 20:00 is half price** (Java's `Calendar` branch,
/// evaluated in UTC here). Epoch day 0 was a Thursday, so day 4 is a Monday.
#[test]
fn the_monday_tuesday_evening_window_halves_the_fee() {
    use crate::game_loop::npc::teleporter::is_half_price_window;

    const DAY: i64 = 86_400_000;
    const HOUR: i64 = 3_600_000;
    // Monday (epoch day 4) at 20:00 and 19:59.
    assert!(is_half_price_window(4 * DAY + 20 * HOUR));
    assert!(!is_half_price_window(4 * DAY + 19 * HOUR + 59 * 60_000));
    // Tuesday 23:00 yes, Wednesday 20:00 no, Sunday 20:00 no.
    assert!(is_half_price_window(5 * DAY + 23 * HOUR));
    assert!(!is_half_price_window(6 * DAY + 20 * HOUR));
    assert!(!is_half_price_window(3 * DAY + 20 * HOUR));

    // The clock every other teleport test is pinned to must sit *outside* the
    // window, or those tests would assert half prices while claiming full
    // ones. This guard keeps `FULL_PRICE_CLOCK` honest if anyone retunes it.
    assert!(
        !is_half_price_window(FULL_PRICE_CLOCK),
        "FULL_PRICE_CLOCK drifted into the discount window"
    );

    // …and the fee actually halves inside the window.
    let (mut world, _rx) = teleporter_world(0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 80;
    let holder = world
        .data
        .teleporters
        .holder(30001, "NORMAL")
        .expect("the fixture list")
        .clone();
    let loc = holder.locations[0].clone();
    let fee_at = |world: &World, now: i64| {
        crate::game_loop::npc::teleporter::calculate_fee_at(world, 80, 3001, &holder, &loc, now)
    };
    assert_eq!(
        fee_at(&world, 6 * DAY + 20 * HOUR),
        9_400,
        "Wednesday: full"
    );
    assert_eq!(
        fee_at(&world, 4 * DAY + 20 * HOUR),
        4_700,
        "Monday 20:00: half"
    );
}

/// **A destination whose castle is under siege is refused** — the dist ships
/// `TeleportWhileSiegeInProgress = False`.
#[test]
fn a_besieged_destination_is_refused() {
    let (mut world, mut rx) = teleporter_world(20_000);
    world.cfg.character.teleport_while_siege_in_progress = false;
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 80;
    // Tie the destination to castle 1 and start its siege.
    world.data.teleporters.insert_for_test(
        30001,
        crate::data::teleporter_data::TeleportHolder {
            name: "NORMAL".into(),
            teleport_type: crate::data::teleporter_data::TeleportType::Normal,
            locations: vec![crate::data::teleporter_data::TeleportLocation {
                x: 1000,
                y: 2000,
                z: -30,
                name: None,
                npc_string_id: 1010004,
                fee_id: 57,
                fee_count: 9400,
                castle_ids: vec![1],
            }],
        },
    );
    let mut siege = model::siege::Siege::new(1);
    siege.in_progress = true;
    world.sieges.insert(1, siege);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_teleport NORMAL 0")),
    );

    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CANNOT_TELEPORT_TO_A_VILLAGE_THAT_IS_IN_A_SIEGE)
    );
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y), (0, 0), "and nobody moved");
    assert_eq!(adena_of(&world, 3001), 20_000, "nothing was charged");
}

/// **Carrying a siege ward blocks the gatekeeper** (Java
/// `isCombatFlagEquipped`).
#[test]
fn a_ward_carrier_cannot_teleport() {
    let (mut world, mut rx) = teleporter_world(20_000);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 80;
    // The combat flag (9819) in the bag.
    let mut flag = world.data.item_data.get(57).cloned().unwrap();
    flag.item_id = 9819;
    flag.name = "Combat Flag".into();
    world.data.item_data.insert_for_test(flag);
    items::add_inventory_item(&mut world, 3001, 9819, 1);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_teleport NORMAL 0")),
    );

    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CANNOT_TELEPORT_WHILE_IN_POSSESSION_OF_A_WARD)
    );
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y), (0, 0));
}

/// **`showNoblesSelect` serves the noble page only to nobles.**
#[test]
fn the_noble_list_page_gates_on_nobless() {
    let (mut world, mut rx) = teleporter_world(0);
    world.data.root = crate::data::DIST_GAME.to_string();

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_showNoblesSelect")),
    );
    let page = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .unwrap_or_default();
    assert!(
        !page.contains("showTeleports NOBLE"),
        "a non-noble gets the refusal page: {page}"
    );

    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .is_noble = true;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_showNoblesSelect")),
    );
    let page = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("the noble page");
    assert!(
        page.contains("showTeleports"),
        "a noble gets the destination menu: {page}"
    );
}

// ---------------------------------------------------------------------------
// `Teleporter.showChatWindow` — the castle-ground gate
// ---------------------------------------------------------------------------

const TELE_DIST: &str = crate::data::DIST_GAME;

/// Roxxy's real Talking Island spawn (`spawns/Gludio/Gludio.xml`).
const ROXXY: (i32, i32, i32) = (-84108, 244604, -3729);
/// Inside Gludio castle's `SiegeZone` (the blacksmith's dist spawn).
const IN_GLUDIO_CASTLE: (i32, i32, i32) = (-17680, 109519, -2656);

/// `Teleporter.showChatWindow` resolves its castle through
/// `CastleManager.getCastle(this)` — strict `checkIfInZone` containment — not
/// `Npc.getCastle()`/`findNearestCastle`, which falls back to the closest
/// castle at *any* distance. Resolving through the nearest-castle form put
/// every town gatekeeper in the world on "castle ground", so Roxxy answered
/// `castleteleporter-no.htm` ("How dare you talk to me!") instead of her own
/// page and no one could teleport.
#[test]
fn town_gatekeeper_is_not_on_castle_ground() {
    let (mut world, mut rx) = teleporter_world(0);
    world.data.root = TELE_DIST.to_string();
    world.data.zone_data = crate::data::zone_data::ZoneData::load_from(TELE_DIST);

    let roxxy = NPC_OID + 50;
    add_test_npc(
        &mut world,
        roxxy,
        30006,
        "Teleporter",
        70,
        ROXXY.0,
        ROXXY.1,
        ROXXY.2,
    );
    // The nearest-castle form does claim her — that is exactly the trap.
    assert!(
        world
            .data
            .zone_data
            .nearest_castle_at(ROXXY.0, ROXXY.1, ROXXY.2)
            .is_some(),
        "findNearestCastle has no distance bound, so it answers for Talking Island too"
    );
    assert_eq!(
        crate::game_loop::npc::teleporter::castle_landing_page(&world, roxxy, 3001),
        None,
        "Roxxy stands in no castle's siege zone: Java falls through to super.showChatWindow"
    );

    show_chat_window(&mut world, 1, roxxy, 0);
    let page = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("Roxxy's landing page");
    assert!(
        !page.contains("How dare you talk to me"),
        "no castle refusal for a town gatekeeper: {page}"
    );
    assert!(
        page.contains("showTeleports"),
        "she offers the teleport menu: {page}"
    );

    // A gatekeeper actually standing inside a castle still gets the refusal:
    // the clan owns nothing and no siege is running.
    let castle_gk = NPC_OID + 51;
    add_test_npc(
        &mut world,
        castle_gk,
        30006,
        "Teleporter",
        70,
        IN_GLUDIO_CASTLE.0,
        IN_GLUDIO_CASTLE.1,
        IN_GLUDIO_CASTLE.2,
    );
    assert_eq!(
        crate::game_loop::npc::teleporter::castle_landing_page(&world, castle_gk, 3001),
        Some("castleteleporter-no.htm".to_string()),
        "inside the siege zone the castle branch still runs"
    );
}

/// `Npc.showChatWindow`'s reputation gate: a criminal gets `<npcId>-pk.htm`
/// instead of the shop, and an `ActionFailed` so the client stops waiting.
///
/// The gate only bites where the datapack wrote a refusal — Java's
/// `showPkDenyChatWindow` returns false for a missing file and falls through
/// to the ordinary dialog. 92 merchants, 24 teleporters and one fisherman have
/// one on this dist; every other NPC serves a criminal normally.
#[test]
fn a_criminal_is_refused_by_merchants_that_have_a_pk_page() {
    /// Trader Lector — has `merchant/30001-pk.htm`.
    const LECTOR: i32 = 30001;
    /// Blacksmith Ferris — a Merchant with **no** `-pk.htm` on this dist.
    const NO_PK_PAGE: i32 = 30847;

    let (mut world, ..) = test_world();
    world.data.root = TELE_DIST.to_string();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    let lector = NPC_OID + 60;
    let ferris = NPC_OID + 61;
    add_test_npc(&mut world, lector, LECTOR, "Merchant", 40, 0, 0, 0);
    add_test_npc(&mut world, ferris, NO_PK_PAGE, "Merchant", 40, 0, 0, 0);

    let page = |w: &mut World, rx: &mut _, npc: i32| {
        drain(rx);
        show_chat_window(w, 1, npc, 0);
        drain(rx).iter().find_map(|p| decode_npc_html(p))
    };

    // Clean reputation: the ordinary dialog, not the refusal.
    let clean = page(&mut world, &mut rx, lector).expect("a page");
    assert!(
        !clean.contains("scoundrel"),
        "an honest player is served: {clean}"
    );

    // Karma: the refusal page.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .reputation = -500;
    let refused = page(&mut world, &mut rx, lector).expect("a page");
    assert!(
        refused.contains("scoundrel"),
        "the criminal gets 30001-pk.htm: {refused}"
    );

    // …but a merchant with no refusal page still serves them, which is the
    // half that a blanket "criminals cannot shop" gate would get wrong.
    let served = page(&mut world, &mut rx, ferris).expect("a page");
    assert!(
        !served.contains("scoundrel"),
        "no -pk.htm means no refusal: {served}"
    );
}
