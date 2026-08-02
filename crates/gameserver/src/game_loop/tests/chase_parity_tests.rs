//! `CreatureAI.maybeMoveToPawn` parity — the one helper Java runs for
//! `thinkAttack`, `thinkCast`, `thinkInteract` and `thinkPickUp` alike.
//!
//! The three behaviours the port used to be missing entirely:
//!
//! 1. the 100-unit engage hysteresis granted *while a follow is running*,
//! 2. the 100-unit deeper aim at a pawn that is moving, and
//! 3. the `isMovementDisabled()` branch — which is Java's one deliberate
//!    attack-versus-cast asymmetry (ATTACK gives up, everything else waits).
//!
//! Test-world geometry: player `base_atk_range` 20, collision radius 9;
//! monster 40001 collision radius 10 — so melee reach is 20 + 9 + 10 = 39 and
//! the slack band is (39, 139]. Skill 1177 has `cast_range` 600, so the cast
//! reach is 619 and its band is (619, 719].

use super::*;
use crate::model::components::{Following, Immobilized, Intent, Movement};

/// The gremlin from `combat_test_world`, spawned at (x, 0) with combat stats.
fn spawn_gremlin(world: &mut World, npc_oid: i32, x: i32) {
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, x, 0, 0, 5000, 30);
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
}

/// `maybeMoveToPawn`: `if (isFollowing()) { if (!isInsideRadius2D(target,
/// offsetWithCollision + 100)) return true; stopFollow(); return false; }`.
///
/// The first think finds the target out of reach and starts the follow; the
/// second — same tick, same positions — engages anyway because 100 units of
/// slack now apply. Without it a chase after anything that keeps walking
/// re-paths forever and never swings, since the strict gate is re-evaluated
/// (10× a second here) faster than the target can be caught.
#[test]
fn a_running_follow_engages_inside_the_100_unit_slack() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 21;
    // 100 units out: past reach 39, inside reach + 100 = 139.
    spawn_gremlin(&mut world, npc_oid, 100);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut rx);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));

    let first = drain(&mut rx);
    assert!(
        first
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "think 1: not following yet, so the strict gate starts the chase"
    );
    assert!(
        !first
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ATTACK),
        "think 1: no swing from outside the strict reach"
    );
    assert_eq!(
        world.objects.get_component::<Following>(&3001).copied(),
        Some(Following {
            target_object_id: npc_oid,
            offset: 20,
        }),
        "startFollow registered at the plain attack range (target is standing still)"
    );

    // Think 2 on the same tick: nothing has moved, but the follow latch is now
    // set, so the +100 band applies.
    for _ in 0..4 {
        world.forced_rolls.extend([0, 0, 0, 99, 10]);
    }
    combat::player_combat_tick(&mut world);
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ATTACK),
        "think 2: following + inside reach + 100 ⇒ engage"
    );
    assert!(
        world.objects.get_component::<Following>(&3001).is_none(),
        "engaging calls stopFollow, so the next think starts from the strict gate again"
    );
}

/// The other half of the same branch: the slack is 100 units, not unlimited.
/// Beyond `reach + 100` a running follow keeps chasing.
#[test]
fn the_slack_stops_at_100_units() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 22;
    // 400 units out: well past reach + 100 = 139.
    spawn_gremlin(&mut world, npc_oid, 400);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut rx);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    drain(&mut rx);
    assert!(
        world.objects.get_component::<Following>(&3001).is_some(),
        "the chase latched a follow"
    );

    combat::player_combat_tick(&mut world);
    assert!(
        !drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ATTACK),
        "still 400 units out: following does not hand out an unlimited engage range"
    );
    assert!(
        world.objects.get_component::<Following>(&3001).is_some(),
        "and the follow keeps running"
    );
}

/// `maybeMoveToPawn`: `if (((Creature) target).isMoving()) offset -= 100;`
/// (floored at 5). Aiming *past* the reach point is what makes the chase
/// converge on a runner instead of trailing it at exactly reach.
///
/// Driven at bow range so the subtraction lands clear of the floor: reach 519
/// static vs 419 moving ⇒ the destination sits exactly 100 units deeper.
#[test]
fn a_moving_pawn_is_chased_100_units_deeper() {
    fn chase_destination(target_moving: bool) -> i32 {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = NPC_OID + 23;
        spawn_gremlin(&mut world, npc_oid, 1000);
        // Long-range weapon: `getPhysicalAttackRange()` 500, as a bow declares.
        world
            .objects
            .get_component_mut::<CombatStats>(&3001)
            .unwrap()
            .atk_range = 500;
        if target_moving {
            world.objects.add_components(
                &npc_oid,
                Movement(crate::model::movement::MoveData {
                    start_x: 1000,
                    start_y: 0,
                    start_z: 0,
                    dest_x: 3000,
                    dest_y: 0,
                    dest_z: 0,
                    start_tick: world.tick,
                    total_ticks: 200,
                    geo_path: None,
                }),
            );
        }

        handle_action(&mut world, 1, &action_body(npc_oid, 0));
        drain(&mut rx);
        handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
        drain(&mut rx);

        let Movement(m) = world
            .objects
            .get_component::<Movement>(&3001)
            .expect("chase started");
        m.dest_x
    }

    let standing = chase_destination(false);
    let running = chase_destination(true);
    assert_eq!(
        running - standing,
        100,
        "a moving pawn is chased to `offset - 100`, so the walk actually catches it \
         (standing {standing}, running {running})"
    );
}

/// `maybeMoveToPawn`'s movement-disabled branch, attack half: "If player is
/// trying attack target but he cannot move to attack target change his
/// intention to idle." The port used to walk the rooted player anyway — the
/// chase leg never consulted `isMovementDisabled()` at all, and neither does
/// the movement tick that advances the path it lays down.
#[test]
fn a_rooted_attacker_gives_up_instead_of_walking() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 24;
    spawn_gremlin(&mut world, npc_oid, 400);
    // `Creature.setImmobilized(true)` — movement-only, so the click itself is
    // still allowed through (`isAttackDisabled()` stays false).
    world.objects.add_components(&3001, Immobilized);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut rx);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));

    assert!(
        !drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "a rooted player does not walk"
    );
    assert!(
        world.objects.get_component::<Movement>(&3001).is_none(),
        "and no server-side path is laid down either"
    );
    assert!(
        world.objects.get_component::<Intent>(&3001).is_none(),
        "AI_INTENTION_ATTACK becomes AI_INTENTION_IDLE"
    );
    assert!(
        world.objects.get_component::<Following>(&3001).is_none(),
        "no follow survives the abandoned intention"
    );
}

/// The same branch, cast half — and Java's one deliberate asymmetry between
/// the two: `maybeMoveToPawn` returns true without touching a CAST intention,
/// so the caster stands still and keeps waiting for the root to lift, where an
/// attacker would have given up.
#[test]
fn a_rooted_caster_keeps_waiting_where_an_attacker_gives_up() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 25;
    // 1500 units out: past skill 1177's cast reach of 600 + 9 + 10 = 619.
    spawn_gremlin(&mut world, npc_oid, 1500);
    world.objects.add_components(&3001, Immobilized);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    drain(&mut rx);
    combat::player_combat_tick(&mut world);
    drain(&mut rx);

    assert!(
        world.objects.get_component::<Movement>(&3001).is_none(),
        "a rooted caster does not walk into range"
    );
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(crate::model::PlayerIntent::Cast { .. }))
        ),
        "but the CAST intention survives — Java only flips ATTACK to IDLE here"
    );
}

/// A ground item is not a `Creature`, so `maybeMoveToPawn` never calls
/// `startFollow` for it — which means the pick-up walk gets none of the
/// moving-target slack, and stops at the strict 36 + picker-radius gate.
#[test]
fn a_pickup_walk_never_follows_and_so_never_gets_the_slack() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let item_oid = crate::game_loop::ground_items::spawn_ground_item(
        &mut world,
        57,
        5,
        0,
        100,
        0,
        0,
        0,
        crate::game_loop::ground_items::DropSource::Player,
    );

    combat::start_pickup_intent(&mut world, 3001, item_oid);
    drain(&mut rx);
    assert!(
        world.objects.get_component::<Following>(&3001).is_none(),
        "`moveToPawn` branch, not `startFollow` — an item is no Creature"
    );

    // 100 units out is inside reach + 100 (45 + 100) but well past reach
    // itself, so a second think must still be walking rather than lifting.
    combat::player_combat_tick(&mut world);
    drain(&mut rx);
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(crate::model::PlayerIntent::PickUp { .. }))
        ),
        "no slack for a non-creature pawn: still walking, item not yet lifted"
    );
}
