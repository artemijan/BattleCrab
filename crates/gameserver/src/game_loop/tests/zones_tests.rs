use super::*;
use crate::game_loop::{doors, party, pvp};

/// Melee-attacking a player inside a peace zone is refused with the peaceful-
/// zone message (`Creature.onForcedAttack`), and no attack intent is set.
#[test]
fn melee_player_in_peace_zone_is_refused() {
    use crate::model::components::{Intent, ZoneFlags};
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 30, 0, 0);
    // Both inside a peace zone.
    world
        .objects
        .get_component_mut::<ZoneFlags>(&5001)
        .unwrap()
        .mask = crate::data::zone_data::ZoneKind::Peace.bit();
    world
        .objects
        .get_component_mut::<ZoneFlags>(&5002)
        .unwrap()
        .mask = crate::data::zone_data::ZoneKind::Peace.bit();
    // Select first, then the attack-click.
    super::combat::start_attack_intent(&mut world, 1, 5001, 5002);

    assert!(
        !world.objects.has_component::<Intent>(&5001),
        "no attack intent in a peace zone"
    );
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::YOU_MAY_NOT_ATTACK_THIS_TARGET_IN_A_PEACEFUL_ZONE
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
}

/// Melee-attacking a player outside a peace zone sets the attack intent (the
/// swing then flags the attacker on landing, covered by the combat path).
#[test]
fn melee_player_outside_peace_zone_starts_attack() {
    use crate::model::PlayerIntent;
    use crate::model::components::Intent;
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 30, 0, 0);

    super::combat::start_attack_intent(&mut world, 1, 5001, 5002);

    assert!(
        matches!(
            world.objects.get_component::<Intent>(&5001).map(|i| i.0),
            Some(PlayerIntent::Attack {
                target_object_id: 5002
            })
        ),
        "attack intent against the player target",
    );
}

/// Hostile casts between players are refused while either side stands in a
/// peace zone (`Enemy`/`EnemyOnly.java` → SM 2167), while friendly skills
/// still land; revalidation pushes the peace compass code.
#[test]
fn peace_zone_blocks_hostile_casts_between_players() {
    let (mut world, ..) = cast_test_world();
    insert_zone(
        &mut world,
        crate::data::zone_data::ZoneKind::Peace,
        -500,
        500,
        -500,
        500,
    );
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    super::zones::revalidate_zone(&mut world, 3001, true);
    super::zones::revalidate_zone(&mut world, 3002, true);

    // The initial revalidate reports the peace compass code.
    let a_pkts = drain(&mut a_rx);
    assert_eq!(
        a_pkts
            .iter()
            .filter_map(|p| compass_code(p))
            .collect::<Vec<_>>(),
        vec![server_packets::compass_zone::PEACE]
    );

    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Force-use nuke on the player target: refused with SM 2167 and no cast.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::YOU_CANNOT_USE_SKILLS_THAT_MAY_HARM_OTHER_PLAYERS_IN_HERE
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(
        b_rx.try_recv().is_err(),
        "the target hears nothing about the refused cast"
    );

    // A friendly skill (Battle Heal, TARGET type) is not gated.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "heal must start casting in a peace zone"
    );
}

/// The peace gate only guards playable-vs-playable: with only the *attacker*
/// outside, hitting a player inside the zone is still refused; and once
/// both stand outside, the same cast goes through.
#[test]
fn peace_zone_gate_checks_both_sides() {
    let (mut world, ..) = cast_test_world();
    insert_zone(
        &mut world,
        crate::data::zone_data::ZoneKind::Peace,
        60,
        200,
        -500,
        500,
    );
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0); // outside
    let _b_rx = ingame_caster(&mut world, 2, 3002, 100, 0); // inside
    super::zones::revalidate_zone(&mut world, 3001, true);
    super::zones::revalidate_zone(&mut world, 3002, true);

    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::YOU_CANNOT_USE_SKILLS_THAT_MAY_HARM_OTHER_PLAYERS_IN_HERE
    );
    assert!(!world.objects.has_component::<Casting>(&3001));

    // Move the target out of the zone and revalidate: the cast now starts.
    world
        .objects
        .get_component_mut::<Position>(&3002)
        .unwrap()
        .x = 30;
    super::zones::revalidate_zone(&mut world, 3002, true);
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
}

/// Entering/leaving a `WaterZone` flips the swim-speed branch
/// (`Creature.getMoveSpeed`'s water case) and re-broadcasts `UserInfo`,
/// with the compass staying GENERAL.
#[test]
fn water_zone_flips_swim_state_and_speeds() {
    let (mut world, ..) = cast_test_world();
    insert_zone(
        &mut world,
        crate::data::zone_data::ZoneKind::Water,
        5000,
        6000,
        -500,
        500,
    );
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    {
        let speeds = world.objects.get_component_mut::<Speeds>(&3001).unwrap();
        speeds.run_spd = 120.0;
        speeds.swim_run_spd = 50.0;
    }
    super::zones::revalidate_zone(&mut world, 3001, true);
    let pkts = drain(&mut rx);
    assert_eq!(
        pkts.iter()
            .filter_map(|p| compass_code(p))
            .collect::<Vec<_>>(),
        vec![server_packets::compass_zone::GENERAL],
        "the first revalidate pushes GENERAL (Java's _lastCompassZone starts at 0; \
         without a valid code the client won't open the world map)"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&3001)
            .unwrap()
            .move_speed(),
        120.0
    );

    // Wade in: swim speeds take over and a fresh UserInfo goes out.
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 5500;
    super::zones::revalidate_zone(&mut world, 3001, false);
    let speeds = *world.objects.get_component::<Speeds>(&3001).unwrap();
    assert!(speeds.swimming);
    assert_eq!(speeds.move_speed(), 50.0);
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter().any(|p| p[0] == 0x32),
        "UserInfo re-broadcast on water enter"
    );
    assert!(
        pkts.iter().all(|p| compass_code(p).is_none()),
        "water does not change the compass"
    );

    // Wade out: ground speeds return.
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 0;
    super::zones::revalidate_zone(&mut world, 3001, false);
    let speeds = *world.objects.get_component::<Speeds>(&3001).unwrap();
    assert!(!speeds.swimming);
    assert_eq!(speeds.move_speed(), 120.0);
    assert!(drain(&mut rx).iter().any(|p| p[0] == 0x32));
}

/// `SiegeZone.onEnter`/`onExit`: entering an active siege zone sends the
/// combat-zone message (no flag while inside); leaving sends the exit message
/// and raises the PvP flag, which then blinks out.
#[test]
fn siege_zone_combat_messages_and_leave_flag() {
    use crate::model::components::{Position, PvpState, ZoneFlags};
    use crate::model::siege::Siege;
    let (mut world, ..) = cast_test_world();
    insert_siege_zone(&mut world, 3, 5000, 6000, -500, 500);
    world.sieges.insert(3, {
        let mut s = Siege::new(3);
        s.in_progress = true;
        s
    });
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0); // outside
    super::zones::revalidate_zone(&mut world, 3001, true); // baseline, no transition
    drain(&mut rx);

    // Enter the active siege zone → combat-zone message, still unflagged.
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 5500;
    super::zones::revalidate_zone(&mut world, 3001, false);
    assert!(
        world
            .objects
            .get_component::<ZoneFlags>(&3001)
            .unwrap()
            .in_active_siege
    );
    assert!(
        sm_ids_of(&drain(&mut rx))
            .contains(&server_packets::sm_ids::YOU_HAVE_ENTERED_A_COMBAT_ZONE),
        "entered-combat-zone message"
    );
    assert_eq!(
        world.objects.get_component::<PvpState>(&3001).unwrap().flag,
        0,
        "no flag while inside"
    );

    // Leave → exit message + the flag is raised (the leave-blink).
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 0;
    super::zones::revalidate_zone(&mut world, 3001, false);
    assert!(
        !world
            .objects
            .get_component::<ZoneFlags>(&3001)
            .unwrap()
            .in_active_siege
    );
    assert!(
        sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::YOU_HAVE_LEFT_A_COMBAT_ZONE),
        "left-combat-zone message"
    );
    assert_eq!(
        world.objects.get_component::<PvpState>(&3001).unwrap().flag,
        1,
        "flagged on leaving the siege zone"
    );
}

/// Entering an active siege zone broadcasts the attackable relation both ways
/// with everyone already in the zone — without it the client never shows
/// combatants as attackable.
///
/// **The siege *icon* is a different question from attackability.** Java's
/// `getRelation` sets INSIEGE off the viewer's own `_siegeState`, which only a
/// registered participant has, while `isAutoAttackable` makes the whole active
/// siege zone hostile (its "siege PvP zone" arm). So two clanless bystanders
/// caught in the battlefield *are* attackable but carry **no** siege bits —
/// which is what this asserts, and what the port used to get wrong by deriving
/// the icon from the zone.
#[test]
fn siege_zone_broadcasts_attackable_relation_on_enter() {
    use crate::model::components::Position;
    use crate::model::siege::Siege;
    let (mut world, ..) = cast_test_world();
    insert_siege_zone(&mut world, 3, 5000, 6000, -500, 500);
    world.sieges.insert(3, {
        let mut s = Siege::new(3);
        s.in_progress = true;
        s
    });
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 5500, 0); // already inside
    super::zones::revalidate_zone(&mut world, 3001, true);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 0, 0); // outside
    super::zones::revalidate_zone(&mut world, 3002, true);
    drain(&mut a_rx);
    drain(&mut b_rx);

    // B walks into the active siege zone (keep its region cell in sync).
    world
        .objects
        .get_component_mut::<Position>(&3002)
        .unwrap()
        .x = 5500;
    world.set_player_region(3002, crate::world::region_of(5500, 0));
    super::zones::revalidate_zone(&mut world, 3002, false);

    // (auto-attackable, relation bits) for the RelationChanged about `about`.
    let rc = |pkts: &[Vec<u8>], about: i32| -> Option<(u8, i32)> {
        pkts.iter().find_map(|p| {
            (p[0] == server_packets::opcodes::RELATION_CHANGED
                && i32::from_le_bytes(p[2..6].try_into().unwrap()) == about)
                .then(|| (p[10], i32::from_le_bytes(p[6..10].try_into().unwrap())))
        })
    };
    let (a_attackable, a_rel) = rc(&drain(&mut a_rx), 3002).expect("A is told about B");
    let (b_attackable, b_rel) = rc(&drain(&mut b_rx), 3001).expect("B is told about A");
    assert_eq!(a_attackable, 1, "A sees B as attackable");
    assert_eq!(b_attackable, 1, "B sees A as attackable");
    assert_eq!(
        a_rel & (0x200 | 0x1000),
        0,
        "…but with no siege bits: neither is registered for this siege"
    );
    assert_eq!(b_rel & (0x200 | 0x1000), 0);

    // Give them opposing sides and the icon appears: INSIEGE both ways, ENEMY
    // because the states differ, and ATTACKER on the besieger's own crown.
    world
        .objects
        .get_component_mut::<crate::model::Player>(&3001)
        .unwrap()
        .siege_state = 2;
    world
        .objects
        .get_component_mut::<crate::model::Player>(&3002)
        .unwrap()
        .siege_state = 1;
    pvp::broadcast_siege_relation(&world, 3002);
    let (_, rel) = rc(&drain(&mut a_rx), 3002).expect("A is told about B again");
    assert_eq!(
        rel & (0x200 | 0x1000 | 0x400),
        0x200 | 0x1000 | 0x400,
        "INSIEGE | ENEMY | ATTACKER for an attacker seen by a defender"
    );
}

/// A clan leader's siege `RelationChanged` carries the leader bit (`0x80`,
/// `RELATION_LEADER`) that draws the on-head crown — the RelationChanged layout,
/// distinct from UserInfo's (where the leader bit is `0x40`).
#[test]
fn siege_relation_carries_clan_leader_crown_bit() {
    use crate::model::Player;
    use crate::model::components::Position;
    use crate::model::siege::Siege;
    let (mut world, ..) = cast_test_world();
    insert_siege_zone(&mut world, 3, 5000, 6000, -500, 500);
    world.sieges.insert(3, {
        let mut s = Siege::new(3);
        s.in_progress = true;
        s
    });
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 5500, 0); // clan leader, inside
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.clan_id = 42;
        p.clan_leader = true;
    }
    super::zones::revalidate_zone(&mut world, 3001, true);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 0, 0); // outside
    super::zones::revalidate_zone(&mut world, 3002, true);
    drain(&mut a_rx);
    drain(&mut b_rx);

    // B walks into the active siege zone; the relation broadcast about the leader
    // (3001) must set the crown bit alongside the siege enemy bits.
    world
        .objects
        .get_component_mut::<Position>(&3002)
        .unwrap()
        .x = 5500;
    world.set_player_region(3002, crate::world::region_of(5500, 0));
    super::zones::revalidate_zone(&mut world, 3002, false);

    let leader_crown_rc = drain(&mut b_rx).iter().any(|p| {
        p[0] == server_packets::opcodes::RELATION_CHANGED
            && i32::from_le_bytes(p[2..6].try_into().unwrap()) == 3001
            && {
                let rel = i32::from_le_bytes(p[6..10].try_into().unwrap());
                rel & 0x80 != 0 // RELATION_LEADER (crown)
            }
    });
    assert!(
        leader_crown_rc,
        "the leader's siege RelationChanged sets the 0x80 crown bit"
    );
}

/// The UserInfo relation's in-siege bit (0x80 — the siege crown) is set for a
/// registered siege participant standing in the active siege zone, and only
/// then: not for a non-participant in the same zone, and not once the siege ends.
#[test]
fn user_info_relation_sets_in_siege_crown_bit_for_participant() {
    use crate::model::Player;
    use crate::model::siege::{Siege, SiegeClanType};
    let (mut world, ..) = cast_test_world();
    insert_siege_zone(&mut world, 3, 5000, 6000, -500, 500);
    world.sieges.insert(3, {
        let mut s = Siege::new(3);
        s.in_progress = true;
        s.add_clan(77, SiegeClanType::Attacker);
        s
    });
    // Registered attacker (clan 77) and a non-participant (clan 88), both inside.
    let _rx = ingame_caster(&mut world, 1, 3001, 5500, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = 77;
    let _rx2 = ingame_caster(&mut world, 2, 3002, 5500, 0);
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .clan_id = 88;

    let rel = |world: &World, oid: i32| {
        let p = world.objects.get_component::<Player>(&oid).unwrap().clone();
        party::calculate_relation(world, &p)
    };
    assert!(
        rel(&world, 3001) & 0x80 != 0,
        "registered participant in the active siege zone gets the crown bit"
    );
    assert!(
        rel(&world, 3002) & 0x80 == 0,
        "a non-participant in the zone does not"
    );

    // Siege no longer in progress → the bit clears.
    world.sieges.get_mut(&3).unwrap().in_progress = false;
    assert!(
        rel(&world, 3001) & 0x80 == 0,
        "the crown bit clears once the siege ends"
    );
}

/// The 100-unit revalidation filter: a small drift does not re-run the zone
/// query (the water flag stays stale until a real move), a forced call does.
#[test]
fn zone_revalidation_distance_filter() {
    let (mut world, ..) = cast_test_world();
    insert_zone(
        &mut world,
        crate::data::zone_data::ZoneKind::Water,
        5000,
        6000,
        -500,
        500,
    );
    let _rx = ingame_caster(&mut world, 1, 3001, 5990, 0);
    super::zones::revalidate_zone(&mut world, 3001, true);
    assert!(
        world
            .objects
            .get_component::<Speeds>(&3001)
            .unwrap()
            .swimming
    );

    // A 50-unit drift out of the zone edge: unforced revalidate is skipped.
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 6040;
    super::zones::revalidate_zone(&mut world, 3001, false);
    assert!(
        world
            .objects
            .get_component::<Speeds>(&3001)
            .unwrap()
            .swimming,
        "filtered — still stale"
    );
    super::zones::revalidate_zone(&mut world, 3001, true);
    assert!(
        !world
            .objects
            .get_component::<Speeds>(&3001)
            .unwrap()
            .swimming,
        "forced — recomputed"
    );
}

/// Enter-world burst includes StaticObjectInfo + DoorStatusUpdate for a
/// nearby door (and nothing for a far one).
#[test]
fn enter_world_sends_door_info_for_nearby_doors() {
    let (mut world, ..) = test_world();
    crate::model::door::spawn_door_for_test(
        &mut world,
        test_door(9001, crate::data::door_data::DoorOpenMethod::None),
    );
    let mut far = test_door(9002, crate::data::door_data::DoorOpenMethod::None);
    far.x = 50_000;
    far.node_x = [49_998, 50_002, 50_002, 49_998];
    crate::model::door::spawn_door_for_test(&mut world, far);

    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    visibility::on_enter_world(&world, 1, 3001);
    let pkts = drain(&mut rx);
    let so: Vec<_> = pkts.iter().filter(|p| is_static_object_info(p)).collect();
    let dsu: Vec<_> = pkts.iter().filter(|p| is_door_status(p)).collect();
    assert_eq!(so.len(), 1, "only the nearby door renders");
    assert_eq!(dsu.len(), 1);
    assert_eq!(door_packet_closed(so[0]), 1, "closed by default");
    // StaticObjectInfo leads with the door template id.
    assert_eq!(i32::from_le_bytes(so[0][1..5].try_into().unwrap()), 9001);
}

/// A closed door refuses casts through it (SM 181 via `can_see_target`);
/// opening it broadcasts the state change and un-blocks the cast.
#[test]
fn closed_door_blocks_cast_los_until_opened() {
    let (mut world, ..) = cast_test_world();
    let door_oid = crate::model::door::spawn_door_for_test(
        &mut world,
        test_door(9001, crate::data::door_data::DoorOpenMethod::None),
    );
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 200, 0);

    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::CANNOT_SEE_TARGET
    );
    assert!(!world.objects.has_component::<Casting>(&3001));

    // Open the door: both nearby players get the status packets…
    doors::open_door(&mut world, door_oid);
    let pkts = drain(&mut a_rx);
    assert!(
        pkts.iter()
            .any(|p| is_static_object_info(p) && door_packet_closed(p) == 0)
    );
    assert!(
        pkts.iter()
            .any(|p| is_door_status(p) && door_packet_closed(p) == 0)
    );

    // …and the cast now starts.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
}

/// A script-opened door with a `closeTime` shuts itself (`AutoClose`), and
/// a re-close before the timer makes the stale task a no-op.
#[test]
fn opened_door_auto_closes_after_close_time() {
    let (mut world, ..) = test_world();
    let door_oid = crate::model::door::spawn_door_for_test(
        &mut world,
        test_door(9001, crate::data::door_data::DoorOpenMethod::None),
    );

    doors::open_door(&mut world, door_oid);
    assert!(world.geo.doors.is_open(9001));
    // closeTime = 2 s = 20 ticks.
    advance_ticks(&mut world, 19);
    assert!(world.geo.doors.is_open(9001));
    advance_ticks(&mut world, 1);
    assert!(!world.geo.doors.is_open(9001), "auto-closed");

    // Re-open, close by hand, then let the (stale) auto-close fire: no flip.
    doors::open_door(&mut world, door_oid);
    doors::close_door(&mut world, door_oid);
    doors::open_door(&mut world, door_oid);
    doors::close_door(&mut world, door_oid);
    assert!(!world.geo.doors.is_open(9001));
    advance_ticks(&mut world, 40);
    assert!(
        !world.geo.doors.is_open(9001),
        "stale auto-close is a no-op"
    );
}

/// BY_TIME doors cycle on their own: closed → open after `closeTime`,
/// open → closed after `openTime` (Java `TimerOpen`), forever.
#[test]
fn by_time_door_cycles() {
    let (mut world, ..) = test_world();
    model::door::spawn_door_for_test(
        &mut world,
        test_door(9001, crate::data::door_data::DoorOpenMethod::ByTime),
    );
    doors::start_time_cycles(&mut world);

    assert!(!world.geo.doors.is_open(9001));
    // Initial delay while closed = closeTime (2 s).
    advance_ticks(&mut world, 20);
    assert!(world.geo.doors.is_open(9001), "opened by the cycle");
    // Now open: next toggle after closeTime (2 s) per TimerOpen's delay pick.
    advance_ticks(&mut world, 20);
    assert!(!world.geo.doors.is_open(9001), "closed again");
    // Closed: next toggle after openTime (3 s).
    advance_ticks(&mut world, 30);
    assert!(world.geo.doors.is_open(9001), "cycle keeps running");
}

/// Static objects (town maps/thrones) render on enter world with the Java
/// `StaticObjectInfo(StaticObject)` field shape, scoped by region.
#[test]
fn enter_world_sends_static_object_info_nearby() {
    let (mut world, ..) = test_world();
    world.data.static_object_data.objects.push(
        crate::data::static_object_data::StaticObjectTemplate {
            id: 17250001,
            name: "town_map".into(),
            kind: 0,
            x: 100,
            y: 100,
            z: 0,
        },
    );
    world.data.static_object_data.objects.push(
        crate::data::static_object_data::StaticObjectTemplate {
            id: 17250002,
            name: "far_map".into(),
            kind: 0,
            x: 60_000,
            y: 60_000,
            z: 0,
        },
    );
    crate::model::static_object::spawn_static_objects(&mut world);

    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    visibility::on_enter_world(&world, 1, 3001);
    let pkts = drain(&mut rx);
    let so: Vec<_> = pkts.iter().filter(|p| is_static_object_info(p)).collect();
    assert_eq!(so.len(), 1, "only the nearby panel renders");
    assert_eq!(
        i32::from_le_bytes(so[0][1..5].try_into().unwrap()),
        17250001
    );
    // type field (offset 9..13) is 0, targetable (13..17) is 1.
    assert_eq!(i32::from_le_bytes(so[0][9..13].try_into().unwrap()), 0);
    assert_eq!(i32::from_le_bytes(so[0][13..17].try_into().unwrap()), 1);
}
